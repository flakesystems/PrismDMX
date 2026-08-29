//! The control surface, attached to a running daemon — decision **D11**.
//!
//! `prism-surface` is three layers and no port: bytes in, bytes out, no thread,
//! no clock and no session (`ARCHITECTURE_SPEC.md` §10.1 allows it no platform
//! code at all). This module is the other half — the part that has a port, a
//! clock and the daemon's state — and it is deliberately small:
//!
//! ```text
//!   port ──▶ [`SurfaceController`] ──▶ [`Bindings`] ──▶ `Desk::command`
//!            events, the §5 rules      a `Command`      the same door every
//!                     ▲                                 client uses
//!                     │
//!   the show and the session ──▶ paint ──▶ pump ──▶ port
//! ```
//!
//! # Why this is what makes D11 true
//!
//! The X-Touch does not remote-control the user interface. A press becomes a
//! `Command`, the command reaches the daemon's own state, and the daemon
//! broadcasts the delta to whoever is attached — **which may be nobody**. A view
//! switched from the console while the UI was closed is simply part of the
//! snapshot the UI receives when it comes back, and there is no second command
//! path for the console to keep in step. `crates/prismd/tests/surface_gate.rs`
//! is that sentence as a test.
//!
//! # The clock is here, and it is the only one
//!
//! Every entry point below the crate boundary takes `now`. This is where a real
//! instant is read, once per poll, from a monotonic origin taken when the
//! surface was attached — so the whole of `prism-surface` stays testable with
//! arithmetic and nothing in it waits for anything.
//!
//! # Pacing, and the fault that is the reason for it
//!
//! [`SURFACE_PERIOD`] is the poll cadence and it is `SurfaceTiming::min_gap`,
//! because the send pause is enforced against the caller's clock: a loop that
//! polled every 10 ms would send at most 100 messages a second and a resync
//! burst — 156 messages — would take a second and a half. S20 found that
//! saturating the X-Touch in both directions at once can stop its transmitter
//! altogether while it goes on receiving perfectly, and that only a power cycle
//! recovers it (`docs/MCU_MAPPING.md` §2.7). The layer below refuses to send
//! faster than the floor; this loop's job is to offer it the chance often
//! enough.
//!
//! That fault is also why a surface that has gone quiet is **reported to the
//! operator**: the port stays open and writes still land on the display, so a
//! daemon that only watched for disconnection would show a green light beside a
//! dead console. [`SurfaceHealth::Unresponsive`] carries the remedy — *power-cycle
//! it* — and this module puts it on the wire as a `Delta::Notice`.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use prism_domain::{AttributeType, Delta, EXECUTORS_PER_PAGE, ExecutorId, NoticeLevel, ViewId};
use prism_ipc::CommandOutcome;
use prism_surface::{
    Bindings, ButtonId, Control, DisplayLine, Fader, GlobalButton, LedState, MAX_MESSAGE_BYTES,
    MAX_STRIPS, StripButton, StripColor, SurfaceController, SurfaceCounters, SurfaceEvent,
    SurfaceHealth, SurfaceTiming, X_TOUCH,
};

use crate::core::Core;
use crate::log;
use crate::server::Desk;

/// How often the surface is polled: the minimum gap between two outbound
/// messages, so the pacing floor below is the thing that limits the rate.
pub const SURFACE_PERIOD: Duration = SurfaceTiming::DEFAULT.min_gap;

/// The largest MIDI packet a port hands over in one piece.
///
/// A SysEx from this surface is at most `MAX_MESSAGE_BYTES`; the extra room is
/// for a backend that delivers several short messages in one buffer, which is
/// what a USB MIDI packet stream does.
const INBOX_BYTES: usize = 512;

/// A MIDI port, from the daemon's side.
///
/// The seam `CLAUDE.md` requires: every hardware interface sits behind a trait,
/// so the whole suite runs with nothing plugged in. It is also where the
/// platform lives — opening a MIDI device is `midir`'s or the operating
/// system's business, and neither `prism-surface` nor this crate may contain
/// `#[cfg(target_os = …)]` (`ARCHITECTURE_SPEC.md` §10.1).
///
/// **S36 gave it a real implementation**, and put it in a crate of its own for
/// exactly that reason: `prism_midi::MidiSurfacePort`, wrapped by
/// [`RealSurfacePort`] below in the one place that knows about both. The other
/// two — [`MockSurfacePort`] and [`FileSurfacePort`] — are unchanged, and the
/// whole suite including the D11 gate still runs on them with nothing plugged
/// in.
pub trait SurfacePort: Send {
    /// Takes the next packet the surface sent, if one is waiting.
    ///
    /// Returns how many bytes were written into `buffer`. `None` means nothing
    /// is waiting, which is the ordinary case — an X-Touch speaks only when it
    /// is touched.
    fn read(&mut self, buffer: &mut [u8]) -> Option<usize>;

    /// Sends one MIDI message.
    ///
    /// Infallible by design: a port that has gone away is a fault the health
    /// states describe, not something to answer every message with. An
    /// implementation that cannot write drops the bytes and says so through
    /// [`connected`](Self::connected).
    fn write(&mut self, bytes: &[u8]);

    /// Whether the port is still there.
    ///
    /// Default `true`: a port that cannot tell is not a port that is gone.
    fn connected(&self) -> bool {
        true
    }

    /// Gives the port a chance to notice a cable — S36.
    ///
    /// Called once per poll, with the same instant everything else in this
    /// module is given, which is S21's rule kept one layer further down: the
    /// port owns no clock either. A port with nothing to notice does nothing,
    /// which is why this defaults to nothing at all.
    ///
    /// What it must **not** do is reopen because the desk has gone quiet. A
    /// surface that has stopped transmitting while still receiving is
    /// [`SurfaceHealth::Unresponsive`], it is S20's finding, and only a power
    /// cycle recovers it (`docs/MCU_MAPPING.md` §2.7) — so a port that
    /// reconnected on silence would churn the one state that cannot be
    /// recovered that way and take the diagnosis away from the operator who has
    /// to read it. `prism_midi` says the same thing in its own tests.
    fn refresh(&mut self, _now: Duration) {}

    /// What this port is, for a log line and a settings panel.
    ///
    /// The name the operating system gave it where there is one, and something
    /// a person can read where there is not.
    fn describe(&self) -> String {
        "a control surface".to_owned()
    }

    /// The MIDI port that is actually open, by name — S36.
    ///
    /// `None` for a port that is not open **and** for one that is not a MIDI
    /// port at all: a mock and a file are surfaces without being ports, and a
    /// settings window asking *which device is the desk on* is asking about a
    /// device. What it is answered with instead is nothing, which is true.
    fn open_name(&self) -> Option<String> {
        None
    }

    /// How many times this port has been reopened after a cable came out —
    /// S36's count, offered to a settings panel in S37.
    ///
    /// Zero for a port that cannot have a cable pulled: a mock and a file are
    /// surfaces without being devices, and *never reconnected* is the truth
    /// about both.
    fn reconnects(&self) -> u64 {
        0
    }
}

/// This crate's health as the domain spells it — S37.
///
/// Two types on purpose, for `crate::log::Level`'s reason: `prism-domain` is
/// not allowed to know what a feedback model is, and a panel is not allowed to
/// know what one does.
const fn domain_health(health: SurfaceHealth) -> prism_domain::SurfaceHealth {
    match health {
        SurfaceHealth::Disconnected => prism_domain::SurfaceHealth::Disconnected,
        SurfaceHealth::Connected => prism_domain::SurfaceHealth::Connected,
        SurfaceHealth::Live => prism_domain::SurfaceHealth::Live,
        SurfaceHealth::Probing => prism_domain::SurfaceHealth::Probing,
        SurfaceHealth::Unresponsive => prism_domain::SurfaceHealth::Unresponsive,
    }
}

/// A surface attached to a daemon.
///
/// Holds the controller (layers 1 and 2), the binding table (layer 3), the port
/// and the clock. One poll does four things in order: read what the operator
/// did, apply it, repaint what the show wants shown, and send what the desk is
/// owed.
pub struct SurfaceLink {
    controller: SurfaceController,
    bindings: Bindings,
    port: Box<dyn SurfacePort>,
    /// Where this surface's clock started. The only real instant in the whole
    /// three-layer stack.
    started: Instant,
    inbox: [u8; INBOX_BYTES],
    outbox: [u8; MAX_MESSAGE_BYTES],
    /// Reused so a busy fader bank does not allocate once per packet.
    events: Vec<SurfaceEvent>,
    /// Reused for the scribble strips, for the same reason.
    text: String,
    /// When the picture may next be rebuilt from the show.
    next_paint: Duration,
    /// The health last reported to the operator.
    reported: SurfaceHealth,
    /// A button whose **release** is still to be eaten - S38's learn.
    ///
    /// Learn is one shot, so the press that named a control disarmed it and the
    /// release arrives with learn already off. A release is not nothing: layer 3
    /// forwards both edges of an `ExecutorButton`, so letting it through would
    /// release a `Flash` that was never held. See [`SurfaceLink::learn`].
    swallow: Option<ButtonId>,
}

impl core::fmt::Debug for SurfaceLink {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SurfaceLink")
            .field("health", &self.controller.health())
            .field("counters", &self.controller.counters())
            .finish_non_exhaustive()
    }
}

impl SurfaceLink {
    /// Attaches a port, with a binding table.
    ///
    /// A port that is **there** is marked connected immediately, which
    /// invalidates the shadow model and therefore draws the whole picture once —
    /// §5.3's resync burst, paced by the layer below rather than poured into the
    /// port.
    ///
    /// A port that is **not** there is attached all the same and starts
    /// disconnected (S36). That is the exit criterion rather than a nicety: a
    /// configured port whose desk is switched off is a warning and a daemon that
    /// starts, the picture goes on being maintained while it is away, and the
    /// moment somebody plugs it in [`follow_the_cable`](Self::follow_the_cable)
    /// draws the whole of it. Starting *connected* and discovering otherwise on
    /// the first poll would have said *the surface has gone* about a surface
    /// that was never there.
    #[must_use]
    pub fn attach(port: Box<dyn SurfacePort>, bindings: Bindings) -> Self {
        let mut controller = SurfaceController::new(X_TOUCH);
        if port.connected() {
            controller.connected(Duration::ZERO);
        }
        let reported = controller.health();
        Self {
            controller,
            bindings,
            port,
            started: Instant::now(),
            inbox: [0; INBOX_BYTES],
            outbox: [0; MAX_MESSAGE_BYTES],
            events: Vec::new(),
            text: String::new(),
            next_paint: Duration::ZERO,
            reported,
            swallow: None,
        }
    }

    /// What is known about the surface at the other end.
    #[must_use]
    pub const fn health(&self) -> SurfaceHealth {
        self.controller.health()
    }

    /// What the surface layers have done and not done.
    #[must_use]
    pub const fn counters(&self) -> SurfaceCounters {
        self.controller.counters()
    }

    /// The binding table in force.
    #[must_use]
    pub const fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// Everything the Devices panel draws about this surface — S37.
    ///
    /// Built here rather than by the caller because it is the one place that
    /// holds all four: the feedback model's health and counters, the port's
    /// reconnection count, and the binding table. The domain's types rather than
    /// this crate's, for `crate::log::Level`'s reason — what a panel draws and
    /// what a feedback layer switches on are the same fact and two types.
    #[must_use]
    pub fn status(&self, profile: Option<&Path>) -> prism_domain::SurfaceStatus {
        let counters = self.counters();
        let health = self.health();
        prism_domain::SurfaceStatus {
            health: domain_health(health),
            remedy: health.remedy().map(str::to_owned),
            sent: counters.sent,
            superseded: counters.superseded,
            touch_suppressed: counters.touch_suppressed,
            resyncs: counters.resyncs,
            reserved: counters.reserved,
            probes: counters.probes,
            reconnects: self.port.reconnects(),
            profile: profile.map(|path| path.display().to_string()),
            bound_controls: u32::try_from(self.bindings.bound()).unwrap_or(u32::MAX),
        }
    }

    /// Gives the port back, so it can be attached with a different binding
    /// table — S37.
    ///
    /// The port is the expensive half: it holds an open MIDI connection, a
    /// reconnection schedule and a count of how many times the cable has come
    /// out. Re-reading a profile must not throw any of that away, so the link is
    /// rebuilt round the same port rather than the port being reopened — which
    /// would also be the one thing S20's finding says cannot recover a desk that
    /// has gone quiet.
    ///
    /// The shadow model **is** thrown away, and that is right: a new table can
    /// mean a different scribble strip on every strip, so the whole picture is
    /// redrawn (§5.3's burst as the ordinary diff).
    #[must_use]
    pub fn into_port(self) -> Box<dyn SurfacePort> {
        self.port
    }

    /// Reads, applies, repaints and sends. Returns what to broadcast.
    ///
    /// The deltas are the daemon's own — a command applied here produces exactly
    /// the deltas it would have produced from a client, because it goes through
    /// the same door.
    pub fn poll(&mut self, desk: &Desk) -> Vec<Delta> {
        let now = self.started.elapsed();
        let mut deltas = Vec::new();
        self.receive(now);
        if !self.events.is_empty() {
            deltas = self.apply(desk);
        }
        if now >= self.next_paint {
            self.next_paint = now.saturating_add(self.controller.timing().frame);
            self.paint(&desk.core());
        }
        self.send(now);
        self.follow_the_cable(now);
        if let Some(notice) = self.health_notice() {
            deltas.push(notice);
        }
        deltas
    }

    /// Keeps the controller's idea of the surface in step with the port's — the
    /// hot-plug edge, **S36**.
    ///
    /// Two transitions and nothing else, and the engine hears about neither:
    ///
    /// - **Gone.** `docs/MCU_MAPPING.md` §5.3 — a hardware fault like any other.
    ///   The picture is kept and goes on being maintained, so whatever the show
    ///   did meanwhile is on the desk when it comes back; nothing is sent; a
    ///   hand that was on a fader is taken off it, or that fader would be
    ///   suppressed for ever.
    /// - **Back.** `connected` invalidates the shadow model, so the resync burst
    ///   of §5.3 is the ordinary diff rather than a special path — 156 messages,
    ///   paced at the same floor as everything else, which is about 156 ms.
    ///
    /// The port is asked *after* the send: a write is one of the two things that
    /// discovers a cable has come out (`prism_midi::MidiSurfacePort::write`), so
    /// asking first would report a port gone one poll later than it went.
    fn follow_the_cable(&mut self, now: Duration) {
        self.port.refresh(now);
        let there = self.port.connected();
        let believed = self.controller.health().is_attached();
        if there && !believed {
            log::info(
                "surface",
                &format!("{} is back; redrawing it", self.port.describe()),
            );
            self.controller.connected(now);
        } else if !there && believed {
            log::warn("surface", &format!("{} has gone", self.port.describe()));
            self.controller.disconnected();
        }
    }

    /// What the port is, for a log line and a settings panel.
    #[must_use]
    pub fn describe(&self) -> String {
        self.port.describe()
    }

    /// The MIDI port this surface is actually open on, by name — S36.
    #[must_use]
    pub fn open_name(&self) -> Option<String> {
        self.port.open_name()
    }

    /// Feeds everything the port has into the controller.
    fn receive(&mut self, now: Duration) {
        let Self {
            controller,
            port,
            inbox,
            events,
            ..
        } = self;
        events.clear();
        while let Some(length) = port.read(inbox) {
            let Some(packet) = inbox.get(..length) else {
                // A port that reports more than it was given. Nothing to do with
                // it but ignore it — the alternative is a panic on the thread
                // that owns the desk.
                break;
            };
            controller.push(packet, now, |event| events.push(event));
        }
    }

    /// Turns this poll's events into commands and applies them.
    ///
    /// **Unless learn is armed**, in which case the first event that names a
    /// control names it and fires nothing - S38. See [`Self::learn`].
    fn apply(&mut self, desk: &Desk) -> Vec<Delta> {
        let context = context_of(&desk.core());
        let mut deltas = Vec::new();
        let mut learning = desk.core().is_learning();
        // The events are taken rather than borrowed, because learning writes to
        // `self`. `std::mem::take` on a reused buffer costs the allocation the
        // reuse was for, so the vector is swapped out and swapped back — the
        // capacity is the same one on the way out as on the way in.
        let mut events = core::mem::take(&mut self.events);
        for event in &events {
            if learning || self.swallow.is_some() {
                let (learned, still) = self.learn(*event, learning, desk);
                deltas.extend(learned);
                learning = still;
                continue;
            }
            let Some(command) = self.bindings.command(*event, &context) else {
                continue;
            };
            log::debug("surface", &format!("{event:?} -> {command:?}"));
            match desk.command(command) {
                CommandOutcome::Applied { deltas: applied } => deltas.extend(applied),
                CommandOutcome::Refused { message } => {
                    // Not a fault of the surface's: an operator can press Go on
                    // an executor that has no sequence. The desk says why at
                    // debug level and the show carries on.
                    log::debug("surface", &format!("refused: {message}"));
                }
            }
        }
        // Back where it came from, so the next poll writes into the same
        // capacity: `tests/surface_allocations.rs` asserts a busy desk costs no
        // allocation and this is one of the two buffers that promise is about.
        events.clear();
        self.events = events;
        deltas
    }

    /// Names the control an operator touched instead of obeying it - S38.
    ///
    /// # Why a press must not fire while learn is armed
    ///
    /// This is the half of learn that is worth the code. An operator finding out
    /// what the Record key is called would otherwise clear their programmer to
    /// find out, and one learning a transport key would start a cue on a stage.
    /// So while learn is armed nothing this poll produces reaches
    /// `Desk::command` at all.
    ///
    /// # And the release has to go with the press
    ///
    /// Learn is **one shot**: the press names the control and disarms. The
    /// button's *release* arrives on a later poll with learn already off, and a
    /// release on its own is not nothing - `SurfaceAction::is_momentary` forwards
    /// both edges of an `ExecutorButton`, so a `Flash` would be released without
    /// ever having been held. [`Self::swallow`] is that one button, remembered
    /// until its release has been eaten.
    ///
    /// Touch is deliberately not learnable. A hand landing on a fader is layer
    /// 2's business and produces no command of its own (`Bindings::command`
    /// answers `None` for it), and the `Moved` that follows names the same
    /// fader, so learning from touch would name a control twice and disarm on
    /// the half of the gesture that means least.
    fn learn(&mut self, event: SurfaceEvent, learning: bool, desk: &Desk) -> (Vec<Delta>, bool) {
        let control = match event {
            SurfaceEvent::Button { button, pressed } => {
                if self.swallow == Some(button) && !pressed {
                    self.swallow = None;
                    return (Vec::new(), learning);
                }
                if !pressed {
                    return (Vec::new(), learning);
                }
                match button {
                    ButtonId::Strip { button, .. } => {
                        prism_domain::BoundControl::StripButton { button }
                    }
                    ButtonId::Global(button) => prism_domain::BoundControl::Global { button },
                }
            }
            SurfaceEvent::Moved { fader, .. } => match fader {
                Fader::Strip(_) => prism_domain::BoundControl::StripFader,
                Fader::Main => prism_domain::BoundControl::MainFader,
            },
            SurfaceEvent::Encoder { .. } => prism_domain::BoundControl::StripEncoder,
            SurfaceEvent::Jog { .. } => prism_domain::BoundControl::Jog,
            SurfaceEvent::Touch { .. } => return (Vec::new(), learning),
        };
        if !learning {
            // The tail of a gesture whose head was learned, and nothing else
            // reaches here: the press disarmed learn and this poll is still
            // walking the events it arrived with.
            return (Vec::new(), learning);
        }
        if let SurfaceEvent::Button { button, .. } = event {
            self.swallow = Some(button);
        }
        (desk.core().learned(control), false)
    }

    /// Puts what the show wants shown into the picture.
    ///
    /// Only the parts §4.1 gives the surface: the executors of the current page
    /// on the faders and the scribble strips, the selected executor on the main
    /// fader, and the Save lamp on the unsaved-changes flag. Everything else is
    /// left as it is, because a picture drawn from nothing would be a desk
    /// asserting that nothing is assigned.
    fn paint(&mut self, core: &Core) {
        let session = core.file.session.session();
        let page = session.executor_page;
        for slot in 0..MAX_STRIPS {
            let Ok(index) = u8::try_from(slot) else {
                continue;
            };
            let id = ExecutorId::from_page_and_slot(page, slot as u32);
            let executor = core.file.show.executor(id);
            // The name, the colour, the level and the lamp all come off the cue
            // list standing on the slot — S45. An executor is a handle, so a
            // second one on the same list shows the same figures and its motor
            // fader follows the first, which is punch-list entry B18 as an
            // operator meets it.
            let sequence = executor
                .and_then(|executor| executor.sequence_id)
                .and_then(|sequence| core.file.show.sequence(sequence));
            let level = fader_reading(executor, sequence);
            self.controller.set_fader(Fader::Strip(index), level);
            self.controller.set_led(
                ButtonId::Strip {
                    strip: index,
                    button: StripButton::Select,
                },
                if sequence.is_some_and(|sequence| sequence.is_active) {
                    LedState::On
                } else {
                    LedState::Off
                },
            );
            // The name is the sequence's, because that is what an operator
            // named. An executor with no sequence shows its own number, which is
            // what makes an empty strip readable rather than blank.
            self.text.clear();
            match sequence.map(|sequence| sequence.name.as_str()) {
                Some(name) => self.text.push_str(name),
                None => {
                    let _ = write!(self.text, "Ex {}", id.get());
                }
            }
            self.controller
                .set_text(index, DisplayLine::Upper, &self.text);
            // The backlight is the other half of what an operator wrote on the
            // strip, and it comes from the same place the name does: the cue
            // list. `prism_surface::color` quantises the twenty-four bits to one
            // of the eight corners a strip can light, hue first.
            //
            // **A list with no colour is white, not off.** An unlit strip cannot
            // be read (`docs/MCU_MAPPING.md` §2.3), so *no colour chosen* has to
            // be the readable default rather than the dark one — and an empty
            // slot is white for the same reason: it is showing "Ex 5".
            match sequence.and_then(|sequence| sequence.color) {
                Some(color) => self.controller.set_color_rgb(index, color),
                None => self.controller.set_color(index, StripColor::White),
            }
            self.text.clear();
            // **§4.1's "value", read as *what this strip does*** — S45, and a
            // departure from the row that is written down in
            // `docs/MCU_MAPPING.md` §4.1. The percentage that used to be here is
            // the position of the motor fader directly under it, which an
            // operator can already see; what they cannot see on an X-Touch is
            // what the four keys do, and that is exactly what B15 is about. So
            // the lower line is the strip's **legend**: the four keys as one
            // letter each, then the fader's own.
            //
            // Seven characters is what a scribble strip has (`STRIP_CHARS`), and
            // this fits in six. The full words are on the screen, in the
            // `Executors` window's editor.
            let _ = write!(self.text, "{}", legend(executor));
            self.controller
                .set_text(index, DisplayLine::Lower, &self.text);
        }
        let selected = session
            .selected_executor
            .and_then(|id| core.file.show.executor(id))
            .map_or(0, |executor| {
                let sequence = executor
                    .sequence_id
                    .and_then(|sequence| core.file.show.sequence(sequence));
                fader_reading(Some(executor), sequence)
            });
        self.controller.set_fader(Fader::Main, selected);
        // §4.1: "Save | LED lit while unsaved changes exist".
        self.controller.set_led(
            ButtonId::Global(GlobalButton::Save),
            if core.file.is_dirty() {
                LedState::On
            } else {
                LedState::Off
            },
        );
    }

    /// Sends whatever the pacing allows.
    fn send(&mut self, now: Duration) {
        let Self {
            controller,
            port,
            outbox,
            ..
        } = self;
        let profile = *controller.profile();
        controller.pump(now, |feedback| {
            if let Ok(length) = feedback.encode_into(&profile, outbox)
                && let Some(bytes) = outbox.get(..length)
            {
                port.write(bytes);
            }
        });
    }

    /// A notice for the operator when the surface's health has changed.
    fn health_notice(&mut self) -> Option<Delta> {
        let health = self.controller.health();
        if health == self.reported {
            return None;
        }
        self.reported = health;
        let notice = notice_for(health)?;
        if let Delta::Notice { message, .. } = &notice {
            log::warn("surface", message);
        }
        Some(notice)
    }
}

/// What to tell the operator about a surface in this state, if anything.
///
/// The wording is [`SurfaceHealth::remedy`]'s rather than this module's, and the
/// unresponsive one is the reason the whole state exists: the port is still
/// open and writes still land on the display, so the obvious advice —
/// reconnect — is the one thing S20 established does **not** work
/// (`docs/MCU_MAPPING.md` §2.7). It is an `Error` rather than a `Warn` because
/// the desk is not coming back without somebody walking over to it.
fn notice_for(health: SurfaceHealth) -> Option<Delta> {
    let message = health.remedy()?;
    Some(Delta::Notice {
        level: match health {
            SurfaceHealth::Unresponsive => NoticeLevel::Error,
            _ => NoticeLevel::Warn,
        },
        message: message.to_owned(),
    })
}

/// What a strip's motor fader stands at: the number its own function names.
///
/// **S45.** A fader was always the master before, because that was the only
/// number an executor had. Now what it moves is `Executor::fader_function` and
/// what it shows has to be the same thing, or an operator moves a speed fader
/// and watches a master.
///
/// A crossfade reads **nought**: where a crossfade fader stands is a gesture in
/// progress rather than show state (`prism_core::Effect::ExecutorXFade`), so
/// there is nothing to put a motor at, and the resting end is the honest place
/// for it. An empty fader and a slot with no cue list read nought for the same
/// reason — there is no number.
fn fader_reading(
    executor: Option<&prism_domain::Executor>,
    sequence: Option<&prism_domain::Sequence>,
) -> u16 {
    let (Some(executor), Some(sequence)) = (executor, sequence) else {
        return 0;
    };
    match executor.fader_function {
        prism_domain::ExecutorFaderFunction::Master => sequence.master_level,
        prism_domain::ExecutorFaderFunction::Speed => sequence.speed,
        prism_domain::ExecutorFaderFunction::XFade | prism_domain::ExecutorFaderFunction::Empty => {
            0
        }
    }
}

/// The strip's legend: what its four keys do, then what its fader does.
///
/// One character each, because a scribble strip is seven wide and the words do
/// not fit — `docs/MCU_MAPPING.md` §4.1 has the departure and the reason. A slot
/// with no executor at all is blank rather than five dashes: an empty strip
/// already says `Ex 5` on the line above, and a row of punctuation under it
/// would read as something being switched off.
fn legend(executor: Option<&prism_domain::Executor>) -> String {
    use prism_domain::ExecutorButtonFunction as Button;
    use prism_domain::ExecutorFaderFunction as Fader_;

    let Some(executor) = executor else {
        return String::new();
    };
    let mut legend = String::with_capacity(usize::from(prism_domain::EXECUTOR_BUTTONS) + 2);
    for index in 0..usize::from(prism_domain::EXECUTOR_BUTTONS) {
        legend.push(match executor.button_functions.get(index) {
            Some(Button::GoForward) => '>',
            Some(Button::GoBack) => '<',
            Some(Button::On) => 'O',
            Some(Button::Off) => 'x',
            Some(Button::Flash) => 'F',
            Some(Button::Toggle) => 'T',
            Some(Button::LearnSpeed) => 'L',
            Some(Button::CommandLine { .. }) => '*',
            Some(Button::Empty) | None => '-',
        });
    }
    legend.push(' ');
    legend.push(match executor.fader_function {
        Fader_::Master => 'M',
        Fader_::Speed => 'S',
        Fader_::XFade => 'X',
        Fader_::Empty => '-',
    });
    legend
}

/// What the session knows, in the shape layer 3 asks for.
///
/// The one place the surface stack is told about the session, and it is answers
/// rather than models: `prism-surface` holds no view library and no show, so the
/// neighbours of the active view are resolved here.
#[must_use]
pub fn context_of(core: &Core) -> prism_surface::SurfaceContext {
    let state = &core.file.session;
    let session = state.session();
    let active = session.active_view_id;
    let mut previous: Option<ViewId> = None;
    let mut next: Option<ViewId> = None;
    // The views come out in number order, so the neighbours are the last one
    // before the active number and the first one after it. Written this way
    // rather than by index so that an `activeViewId` naming a view that is not
    // stored — which a hand-edited file can produce — still has neighbours.
    for view in state.views() {
        if view.id < active {
            previous = Some(view.id);
        } else if view.id > active && next.is_none() {
            next = Some(view.id);
        }
    }
    prism_surface::SurfaceContext {
        executor_page: session.executor_page,
        selected_executor: session.selected_executor,
        previous_view: previous,
        next_view: next,
        programmer_page: session.programmer_page,
        parameter: parameter_of(session.encoder_bank, session.programmer_param_index),
    }
}

/// The attribute the jog wheel turns.
///
/// The encoder bank names a feature group and the parameter index counts within
/// it, in `AttributeType::ALL`'s order. **This is `FeatureGroup::attributes` and
/// nothing else**, which is the whole point: S26's encoder bar numbers its
/// encoders out of the same table — exported to TypeScript with the rest of the
/// bindings as `FEATURE_GROUP_ATTRIBUTES` — so the wheel turns what is
/// highlighted. S22 left this as a warning because there was nothing on the
/// other side of it yet; now there is, and the way the two are kept in step is
/// that there is one of them.
///
/// An index past the end is nothing rather than the last one: the wheel then
/// does nothing, which is what an operator who has paged past the parameters
/// should feel.
fn parameter_of(group: prism_domain::FeatureGroup, index: u32) -> Option<AttributeType> {
    group.parameter(index)
}

/// The executors one page holds, for a caller that wants to name the slot.
///
/// Here rather than inline so that `EXECUTORS_PER_PAGE` and [`MAX_STRIPS`] are
/// asserted to be the same number in one place: **D7** says a page is eight
/// executors and the surface has eight strips, and a device with a different
/// count would need paging arithmetic rather than a coincidence.
const _: () = assert!(EXECUTORS_PER_PAGE as usize == MAX_STRIPS);

/// What a mock port and its handle share.
#[derive(Debug, Default)]
struct MockState {
    inbound: VecDeque<Vec<u8>>,
    outbound: Vec<Vec<u8>>,
    unplugged: bool,
}

/// A MIDI port with no device behind it.
///
/// `CLAUDE.md`: every hardware interface is mocked, and the tests run
/// deterministically with nothing connected — including the D11 gate, which is
/// about a surface driving a daemon and not about a surface. It is public
/// rather than a test fixture for the same reason `prism_protocols::MockOutput`
/// is: the daemon's integration targets link the library, and a mode nothing
/// outside this file can reach is not one.
#[derive(Debug)]
pub struct MockSurfacePort {
    state: Arc<Mutex<MockState>>,
}

/// The other end of a [`MockSurfacePort`]: what a test presses and reads.
#[derive(Debug, Clone)]
pub struct MockSurfaceHandle {
    state: Arc<Mutex<MockState>>,
}

impl MockSurfacePort {
    /// A port and the handle that drives it.
    #[must_use]
    pub fn new() -> (Self, MockSurfaceHandle) {
        let state = Arc::new(Mutex::new(MockState::default()));
        (
            Self {
                state: Arc::clone(&state),
            },
            MockSurfaceHandle { state },
        )
    }

    /// The shared state, ignoring poisoning for the reason [`Desk::core`] does:
    /// a test that panicked while holding this has finished, and a daemon that
    /// stopped answering because of it would be the fault under test.
    fn state(&self) -> std::sync::MutexGuard<'_, MockState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl SurfacePort for MockSurfacePort {
    fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
        let packet = self.state().inbound.pop_front()?;
        let length = packet.len().min(buffer.len());
        buffer
            .get_mut(..length)?
            .copy_from_slice(packet.get(..length)?);
        Some(length)
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut state = self.state();
        if state.unplugged {
            return;
        }
        state.outbound.push(bytes.to_vec());
    }

    fn connected(&self) -> bool {
        !self.state().unplugged
    }
}

impl MockSurfaceHandle {
    /// The shared state.
    fn state(&self) -> std::sync::MutexGuard<'_, MockState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The desk sends a packet.
    pub fn send(&self, bytes: &[u8]) {
        self.state().inbound.push_back(bytes.to_vec());
    }

    /// A button pressed and released, as the surface sends it: Note On with
    /// velocity 127, then the same note with velocity 0.
    ///
    /// The two bytes are `docs/MCU_MAPPING.md` §2.1's, written out by the
    /// caller: a helper that asked the profile for the note number would be
    /// asking the code under test what to press.
    pub fn press(&self, channel_status: u8, note: u8) {
        self.send(&[channel_status, note, 127]);
        self.send(&[channel_status, note, 0]);
    }

    /// Every message the desk has been sent, in order.
    #[must_use]
    pub fn received(&self) -> Vec<Vec<u8>> {
        self.state().outbound.clone()
    }

    /// Forgets what the desk has been sent.
    pub fn clear_received(&self) {
        self.state().outbound.clear();
    }

    /// Pulls the cable out. The port stays open and reports itself gone.
    pub fn unplug(&self) {
        self.state().unplugged = true;
    }

    /// Puts it back in — S36.
    ///
    /// The other half of [`unplug`](Self::unplug), and the reason the hot-plug
    /// edge can be asserted with nothing plugged in: `SurfaceLink` learns a port
    /// has come back through `SurfacePort::connected` and nothing else, so a
    /// mock that can answer both ways exercises exactly the path a real cable
    /// does.
    pub fn replug(&self) {
        self.state().unplugged = false;
    }
}

/// The X-Touch that is actually plugged in — **S36**.
///
/// One line of substance and a reason for existing: `prism_midi` knows nothing
/// about a daemon and this crate may hold no platform code, so the two are
/// joined here, where a `MidiSurfacePort` becomes a [`SurfacePort`] and nothing
/// else happens at all.
///
/// Everything interesting is one layer down. The port keeps a **name** rather
/// than a device, so it survives the cable coming out and going back in; it
/// retries on a doubling backoff to a five-second ceiling; and it reopens only
/// when the port has genuinely gone — never because the desk has stopped
/// talking, which is S20's finding and the one fault reconnecting cannot fix
/// (`docs/MCU_MAPPING.md` §2.7).
#[derive(Debug)]
pub struct RealSurfacePort(prism_midi::MidiSurfacePort);

impl RealSurfacePort {
    /// Opens the port a configured name selects, or arranges to keep trying.
    ///
    /// **Never fails.** A configured port that is not there is a warning and a
    /// daemon that starts; [`why`](Self::why) is the warning's text.
    #[must_use]
    pub fn attach(port: &str) -> Self {
        Self(prism_midi::MidiSurfacePort::attach(port))
    }

    /// Why the port is not open, or `None` when it is.
    #[must_use]
    pub fn why(&self) -> Option<String> {
        self.0.error().map(ToString::to_string)
    }

    /// The port name the configuration asked for.
    #[must_use]
    pub fn configured(&self) -> &str {
        self.0.configured()
    }
}

impl SurfacePort for RealSurfacePort {
    fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
        self.0.read(buffer)
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }

    fn connected(&self) -> bool {
        self.0.connected()
    }

    fn refresh(&mut self, now: Duration) {
        self.0.refresh(now);
    }

    fn describe(&self) -> String {
        match self.0.port_name() {
            Some(name) => format!("the surface on MIDI port {name:?}"),
            None => format!(
                "the surface configured on MIDI port {:?}",
                self.0.configured()
            ),
        }
    }

    fn open_name(&self) -> Option<String> {
        self.0.port_name().map(str::to_owned)
    }

    fn reconnects(&self) -> u64 {
        self.0.reconnects()
    }
}

/// A control surface with no device behind it, fed from a file.
///
/// [`MockSurfacePort`] is the same idea inside the process; this is the one a
/// *separate* process can drive, and it exists for one reason: **D11 is a claim
/// about what an operator sees**, and the only way to observe it rather than
/// assert it is to have a real daemon, a real browser and a real console press
/// at the same time. `CLAUDE.md` forbids the third from being a device, so it is
/// bytes appended to a file — the same MIDI a `docs/MCU_MAPPING.md` §2.1 button
/// sends, read by the same three layers.
///
/// It is `--mock-surface`, and it is the console's `--mock-output`: a mode a
/// person can also use to try a binding table with nothing plugged in.
///
/// # What it does not do
///
/// **Nothing goes back out.** A file has no motor faders and no scribble
/// strips, so the feedback is dropped and counted. That is not a limitation to
/// work around: the outbound path is `prism-surface`'s, it is tested to the byte
/// there (S21), and a port that wrote 156 messages into a file every time the
/// shadow model was invalidated would be a growing file and nothing else.
///
/// # Framing
///
/// Whatever bytes have arrived since the last poll, in one piece. The layer
/// above is a **stream** decoder with running-status and SysEx reassembly
/// (S19), so a message split across two reads is reassembled exactly as it is
/// when a USB packet boundary lands in the middle of one.
#[derive(Debug)]
pub struct FileSurfacePort {
    file: std::fs::File,
    /// How many messages were dropped for want of anywhere to put them.
    dropped: u64,
}

impl FileSurfacePort {
    /// Opens the file, creating it if it is not there.
    ///
    /// Created rather than refused so that the daemon can be started *before*
    /// whatever is going to press the buttons exists — which is the ordinary
    /// order in a test, and in a shell session where somebody is about to run
    /// `printf`.
    ///
    /// # Errors
    ///
    /// Whatever the operating system says. The caller warns and carries on: a
    /// surface that will not open must never stop a daemon starting, for the
    /// same reason a malformed profile must not (S22).
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        Ok(Self { file, dropped: 0 })
    }

    /// How many outbound messages have gone nowhere.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

impl SurfacePort for FileSurfacePort {
    fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
        use std::io::Read as _;
        // A short read is the ordinary case and end of file is *nothing is
        // waiting*, not an error: the writer appends whenever a button is
        // pressed, and between presses there is simply nothing there.
        match self.file.read(buffer) {
            Ok(0) | Err(_) => None,
            Ok(length) => Some(length),
        }
    }

    fn write(&mut self, _bytes: &[u8]) {
        self.dropped = self.dropped.saturating_add(1);
    }
}

/// Reads a binding profile from disk, and **cannot fail**.
///
/// The filesystem half of `prism_surface::Bindings::load`: which file it was is
/// this crate's business, because a crate that took a path could not have its
/// fallback tested without one. Every way this can go wrong — no file, no
/// permission, bad JSON, a binding on the reserved button — produces the
/// built-in defaults and a warning in the log, never a daemon that will not
/// start.
/// Whether a bound control keeps reaching PrismDMX while the surface is also
/// driving a sound console — `docs/MCU_MAPPING.md` §4.3, and S38's *ownership
/// shown*.
///
/// The answer is `McuProfile::permanent`'s, asked through the layer-2 vocabulary
/// that holds it: a `BoundControl` is what a *table* names and a `Control` is
/// what the shadow model owns, and the two are the same desk seen from either
/// side of layer 3. It is answered here rather than by a client because a client
/// holds no device profile — telling an operator that a key is always in reach
/// when it is not is precisely the mistake §4.3 exists to prevent.
///
/// A strip row is asked about **strip 0**, and that is exact rather than a
/// sample: `Strip[*]` is one binding for all eight strips (**D7**), and the
/// permanent set contains no strip control at all, so the eight cannot disagree.
#[must_use]
pub fn is_permanent(control: prism_domain::BoundControl) -> bool {
    use prism_domain::BoundControl;
    let control = match control {
        BoundControl::StripFader => Control::Fader(Fader::Strip(0)),
        BoundControl::StripEncoder => Control::Encoder(0),
        BoundControl::StripButton { button } => {
            Control::Button(ButtonId::Strip { strip: 0, button })
        }
        BoundControl::MainFader => Control::Fader(Fader::Main),
        BoundControl::Global { button } => Control::Button(ButtonId::Global(button)),
        BoundControl::Jog => Control::Jog,
    };
    X_TOUCH.holds(control, prism_surface::SurfaceMode::Shared)
}

/// Reads a binding profile from disk, or answers `None` — S38.
///
/// [`load_profile`]'s other half, and the difference is what a caller has to
/// fall back **on**. `load_profile` answers with the built-in defaults, which is
/// right for a desk starting up with nothing else; this answers with nothing,
/// which is right for a desk that already has a table of its own. S22's rule is
/// *a malformed profile never blocks anything* — it is not *a malformed profile
/// replaces what an operator has been working on*, and once the table is
/// editable those two come apart.
///
/// It warns for the same reasons and in the same words, so a person reading the
/// log cannot tell which of the two the caller used, which is correct: the file
/// was not usable either way.
#[must_use]
pub fn read_profile(path: &Path) -> Option<Bindings> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            log::warn(
                "surface",
                &format!(
                    "{} could not be read ({error}); the binding table in force is unchanged",
                    path.display()
                ),
            );
            return None;
        }
    };
    match Bindings::load(&text, &X_TOUCH) {
        (_, Some(error)) => {
            log::warn(
                "surface",
                &format!(
                    "{} was not used: {error}. The binding table in force is unchanged",
                    path.display()
                ),
            );
            None
        }
        (bindings, None) => {
            log::info(
                "surface",
                &format!("{} loaded: {} controls bound", path.display(), {
                    bindings.bound()
                }),
            );
            Some(bindings)
        }
    }
}

#[must_use]
pub fn load_profile(path: &Path) -> Bindings {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            log::warn(
                "surface",
                &format!(
                    "{} could not be read ({error}); using the built-in bindings",
                    path.display()
                ),
            );
            return Bindings::defaults();
        }
    };
    let (bindings, problem) = Bindings::load(&text, &X_TOUCH);
    match problem {
        Some(error) => log::warn(
            "surface",
            &format!(
                "{} was not used: {error}. The built-in bindings are in force",
                path.display()
            ),
        ),
        None => log::info(
            "surface",
            &format!("{} loaded: {} controls bound", path.display(), {
                bindings.bound()
            }),
        ),
    }
    bindings
}

#[cfg(test)]
mod tests {
    use super::{SurfacePort, legend, load_profile, notice_for, parameter_of};
    use prism_domain::{AttributeType, Delta, FeatureGroup, NoticeLevel};
    use prism_surface::{Bindings, SurfaceHealth};

    #[test]
    fn a_desk_that_has_stopped_sending_is_reported_and_the_words_say_power_cycle() {
        // The fault S20 found: the port stays open, writes still land on the
        // display, and only a power cycle recovers it. A daemon that reported
        // this as *disconnected* would send the operator to check a cable that
        // is fine.
        let Some(Delta::Notice { level, message }) = notice_for(SurfaceHealth::Unresponsive) else {
            panic!("an unresponsive surface has to be reported");
        };
        assert_eq!(level, NoticeLevel::Error);
        assert!(message.contains("power-cycle"), "{message}");
        assert!(
            message.contains("will not bring it back"),
            "and it has to say that reconnecting is what does not work: {message}"
        );

        let Some(Delta::Notice { level, message }) = notice_for(SurfaceHealth::Disconnected) else {
            panic!("a surface that has gone has to be reported");
        };
        assert_eq!(level, NoticeLevel::Warn);
        assert!(message.contains("no surface"), "{message}");

        // The three healthy states say nothing at all: a desk nobody has touched
        // is not news, and an X-Touch speaks only when it is touched.
        for health in [
            SurfaceHealth::Connected,
            SurfaceHealth::Live,
            SurfaceHealth::Probing,
        ] {
            assert_eq!(notice_for(health), None, "{health:?}");
        }
    }

    /// **S45**: the strip's lower line says what its five controls do.
    ///
    /// It was the master as a percentage until then, and that is the reading the
    /// motor fader directly under it already gives. What an operator cannot see
    /// on an X-Touch is what the four keys do — which is punch-list entry B15 —
    /// so the seven characters go to the legend instead.
    /// `docs/MCU_MAPPING.md` §4.1 carries the departure.
    #[test]
    fn the_lower_line_says_what_the_five_controls_do() {
        use prism_domain::{
            ExecutorButtonFunction as Button, ExecutorEncoderFunction, ExecutorFaderFunction,
            ExecutorId,
        };
        let mut executor = prism_domain::Executor {
            id: ExecutorId::new(3),
            sequence_id: None,
            fader_function: ExecutorFaderFunction::Master,
            button_functions: vec![
                Button::GoForward,
                Button::GoBack,
                Button::Off,
                Button::Empty,
            ],
            encoder_function: ExecutorEncoderFunction::Empty,
        };
        assert_eq!(legend(Some(&executor)), "><x- M");
        // Seven is what a scribble strip has, and this is what has to fit in it.
        assert!(legend(Some(&executor)).len() <= prism_surface::STRIP_CHARS);

        // A reassignment relabels it, which is the whole point.
        executor.button_functions[3] = Button::CommandLine {
            line: "Go+ Sequence 3".to_owned(),
        };
        executor.fader_function = ExecutorFaderFunction::XFade;
        assert_eq!(legend(Some(&executor)), "><x* X");

        executor.button_functions = Vec::new();
        executor.fader_function = ExecutorFaderFunction::Empty;
        assert_eq!(legend(Some(&executor)), "---- -");

        // A slot with no executor at all is blank rather than punctuation: the
        // line above it already says `Ex 5`, and a row of dashes under that
        // would read as five controls somebody had switched off.
        assert_eq!(legend(None), "");
    }

    #[test]
    fn the_jog_wheel_turns_the_parameter_the_encoder_bank_and_index_name() {
        assert_eq!(
            parameter_of(FeatureGroup::Position, 0),
            Some(AttributeType::Pan)
        );
        assert_eq!(
            parameter_of(FeatureGroup::Position, 1),
            Some(AttributeType::Tilt)
        );
        assert_eq!(
            parameter_of(FeatureGroup::Dimmer, 0),
            Some(AttributeType::Dimmer)
        );
        // Past the end is nothing rather than the last one.
        assert_eq!(parameter_of(FeatureGroup::Position, 2), None);
        assert_eq!(parameter_of(FeatureGroup::Dimmer, 9), None);
    }

    /// **The wheel and the encoder bar walk the same list** (S26).
    ///
    /// `FeatureGroup::attributes` is what `prism-domain` exports to the
    /// interface as `FEATURE_GROUP_ATTRIBUTES`, and it is what the wheel
    /// resolves through here. Asserted for every bank and every index rather
    /// than for the two the test above happens to name, because the failure
    /// this prevents — turning one parameter while another is highlighted —
    /// would show up on whichever bank was got wrong.
    #[test]
    fn the_wheel_walks_the_table_the_encoder_bar_is_given() {
        for group in FeatureGroup::ALL {
            let on_it = group.attributes();
            for (index, &attribute) in on_it.iter().enumerate() {
                let index = u32::try_from(index).expect("a bank has few parameters");
                assert_eq!(parameter_of(group, index), Some(attribute), "{group:?}");
            }
            let past = u32::try_from(on_it.len()).expect("a bank has few parameters");
            assert_eq!(parameter_of(group, past), None, "{group:?}");
        }
    }

    #[test]
    fn a_profile_that_is_not_there_leaves_the_defaults_in_force() {
        let missing = std::path::Path::new("no-such-directory/no-such-profile.json");
        assert_eq!(load_profile(missing), Bindings::defaults());
    }

    #[test]
    fn a_profile_that_will_not_parse_leaves_the_defaults_in_force() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("broken.json");
        std::fs::write(&path, "{ this is not a profile").expect("write");
        assert_eq!(load_profile(&path), Bindings::defaults());
    }

    #[test]
    fn the_profile_this_repository_ships_loads_from_disk() {
        // The path the daemon is pointed at in an installation, read the way it
        // will be read there.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../profiles/surface/xtouch.json");
        assert_eq!(load_profile(&path), Bindings::defaults());
    }

    #[test]
    fn attaching_a_surface_draws_it_and_reports_what_it_holds() {
        // A link before its first poll: connected, nothing counted, and the
        // bindings it was handed. `connected` is what invalidates the shadow
        // model, so the resync burst of §5.3 is owed from this moment.
        let (port, _handle) = super::MockSurfacePort::new();
        let link = super::SurfaceLink::attach(Box::new(port), Bindings::defaults());
        assert_eq!(link.health(), SurfaceHealth::Connected);
        assert_eq!(link.counters(), prism_surface::SurfaceCounters::default());
        assert_eq!(link.bindings(), &Bindings::defaults());
        // What a person sees at a breakpoint is the two things worth seeing.
        let shown = format!("{link:?}");
        assert!(shown.contains("health"), "{shown}");
        assert!(shown.contains("counters"), "{shown}");
    }

    #[test]
    fn a_mock_port_carries_bytes_both_ways_until_it_is_unplugged() {
        let (mut port, handle) = super::MockSurfacePort::new();
        handle.send(&[0x90, 54, 127]);
        let mut buffer = [0u8; 8];
        assert_eq!(port.read(&mut buffer), Some(3));
        assert_eq!(&buffer[..3], &[0x90, 54, 127]);
        assert_eq!(port.read(&mut buffer), None, "nothing else is waiting");

        port.write(&[0xE0, 0, 64]);
        assert_eq!(handle.received(), vec![vec![0xE0, 0, 64]]);
        handle.clear_received();
        assert!(handle.received().is_empty());

        // A packet longer than the buffer is truncated rather than refused: the
        // alternative is a panic on the thread that owns the desk.
        handle.send(&[0u8; 32]);
        assert_eq!(port.read(&mut buffer), Some(8));

        assert!(port.connected());
        handle.unplug();
        assert!(!port.connected());
        port.write(&[0xE0, 0, 64]);
        assert!(
            handle.received().is_empty(),
            "nothing goes to a port that is not there"
        );
    }

    #[test]
    fn a_file_surface_reads_what_is_appended_and_drops_what_comes_back() {
        use std::io::Write as _;

        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("console.midi");
        // Opened before anything exists, which is the order a daemon starts in.
        let mut port = super::FileSurfacePort::open(&path).expect("a file surface opens");
        let mut buffer = [0u8; 16];
        assert_eq!(port.read(&mut buffer), None, "nothing has been pressed yet");

        // F1, note 54 on channel 1 — `docs/MCU_MAPPING.md` §2.1, written out
        // rather than asked for.
        let mut writer = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("the file is there to append to");
        writer.write_all(&[0x90, 54, 127]).expect("append");
        writer.flush().expect("flush");
        assert_eq!(port.read(&mut buffer), Some(3));
        assert_eq!(&buffer[..3], &[0x90, 54, 127]);
        assert_eq!(port.read(&mut buffer), None, "and then nothing again");

        // More arrives later, and the port picks up where it left off rather
        // than reading the file from the beginning.
        writer.write_all(&[0x90, 54, 0]).expect("append");
        writer.flush().expect("flush");
        assert_eq!(port.read(&mut buffer), Some(3));
        assert_eq!(&buffer[..3], &[0x90, 54, 0]);

        // A file has no faders, so the picture goes nowhere and says so.
        assert_eq!(port.dropped(), 0);
        port.write(&[0xE0, 0, 64]);
        assert_eq!(port.dropped(), 1);
        assert!(port.connected(), "a file does not fall out of its socket");
    }

    #[test]
    fn a_file_surface_that_cannot_be_opened_says_so_rather_than_panicking() {
        // The daemon turns this into a warning and starts anyway (S22's rule
        // for the profile, and the same reason: nothing about a console may
        // stop a show being run).
        let missing = std::path::Path::new("no-such-directory/no-such-file.midi");
        assert!(super::FileSurfacePort::open(missing).is_err());
    }

    /// **The hot-plug edge, S36**, asserted with nothing plugged in.
    ///
    /// A `SurfaceLink` learns that a port has gone or come back through
    /// `SurfacePort::connected` and nothing else, so a mock that answers both
    /// ways exercises exactly the path a real cable does. What is asserted is
    /// the pair of transitions and the resync: coming back **invalidates the
    /// shadow model**, which is `docs/MCU_MAPPING.md` §5.3's burst arriving as
    /// the ordinary diff rather than as a special path.
    #[test]
    fn a_cable_pulled_and_put_back_costs_one_notice_each_way_and_a_redraw() {
        use prism_surface::Fader;

        let (port, handle) = super::MockSurfacePort::new();
        let mut link = super::SurfaceLink::attach(Box::new(port), Bindings::defaults());
        assert_eq!(link.health(), SurfaceHealth::Connected);

        // Draw something and let it out, so the shadow model believes something.
        link.controller.set_fader(Fader::Strip(0), u16::MAX);
        for step in 0..400 {
            link.send(std::time::Duration::from_millis(step));
        }
        assert!(!handle.received().is_empty());
        let drawn = handle.received().len();
        link.send(std::time::Duration::from_millis(500));
        assert_eq!(
            handle.received().len(),
            drawn,
            "a picture that has not changed costs nothing"
        );

        // Out.
        handle.unplug();
        link.follow_the_cable(std::time::Duration::from_millis(501));
        assert_eq!(link.health(), SurfaceHealth::Disconnected);
        let Some(Delta::Notice { level, message }) = link.health_notice() else {
            panic!("the operator has to be told the desk has gone");
        };
        assert_eq!(level, NoticeLevel::Warn);
        assert!(message.contains("no surface"), "{message}");

        // And nothing is sent to a port that is not there.
        handle.clear_received();
        link.controller.set_fader(Fader::Strip(1), u16::MAX);
        for step in 502..600 {
            link.send(std::time::Duration::from_millis(step));
        }
        assert!(handle.received().is_empty(), "nothing goes to a dead port");

        // Back in. The whole picture is owed again, because `connected`
        // invalidates what the desk was believed to be showing.
        handle.replug();
        link.follow_the_cable(std::time::Duration::from_millis(601));
        assert_eq!(link.health(), SurfaceHealth::Connected);
        assert_eq!(
            link.health_notice(),
            None,
            "a desk that is there again is not news to report as a fault"
        );
        for step in 602..1200 {
            link.send(std::time::Duration::from_millis(step));
        }
        let redrawn = handle.received().len();
        assert!(
            redrawn >= drawn,
            "the whole surface is redrawn ({redrawn}), not just the one fader that              changed while it was away — the first draw was {drawn} messages"
        );
        assert!(
            redrawn > 100,
            "and a whole surface is a hundred and fifty-odd messages, not one: {redrawn}"
        );
    }

    /// A configured port whose desk is switched off: the daemon has a surface,
    /// the surface is disconnected, and **nothing claims it has gone** — because
    /// it was never there.
    #[test]
    fn a_port_that_was_never_there_starts_disconnected_and_says_nothing_about_going() {
        let (port, handle) = super::MockSurfacePort::new();
        handle.unplug();
        let mut link = super::SurfaceLink::attach(Box::new(port), Bindings::defaults());
        assert_eq!(link.health(), SurfaceHealth::Disconnected);
        assert_eq!(
            link.health_notice(),
            None,
            "the daemon warned about the port at start-up; this is not a second event"
        );
        link.follow_the_cable(std::time::Duration::from_millis(1));
        assert_eq!(link.health(), SurfaceHealth::Disconnected);

        // And the moment it appears, it is drawn.
        handle.replug();
        link.follow_the_cable(std::time::Duration::from_millis(2));
        assert_eq!(link.health(), SurfaceHealth::Connected);
    }

    /// The real port, described without one being plugged in.
    ///
    /// `CLAUDE.md`: no test touches a device, and this machine has an X-Touch
    /// attached that the suite must not open. The name asked for is one nothing
    /// can be called.
    ///
    /// **The reason is asserted as one of two**, and that is the point rather
    /// than a hedge: a build with a MIDI backend says *no MIDI port called …*
    /// and one without says *this build has no backend* — and from the caller's
    /// side those are the same fact, which is why both produce a port that is
    /// attached, not open, and not a failure to start. The Linux CI job runs
    /// the second case for real, so a test that assumed the first was a test
    /// that only ran on Windows — which is what it turned out to be.
    #[test]
    fn a_real_port_that_is_not_there_is_attached_all_the_same_and_says_why() {
        let port = super::RealSurfacePort::attach("no such port \u{1F50C}");
        assert_eq!(port.configured(), "no such port \u{1F50C}");
        assert!(!port.connected());
        assert_eq!(port.open_name(), None);
        let why = port.why().expect("a port that is not open says why");
        assert!(
            why.contains("no such port") || why.contains("no MIDI backend"),
            "the reason has to name the port or say the build cannot look: {why}"
        );
        // What a log line and a settings panel show.
        let shown = port.describe();
        assert!(shown.contains("configured on MIDI port"), "{shown}");

        // Reading, writing and being asked to look again are all ordinary
        // no-ops on a port that is not there rather than anything worse —
        // which is the whole of *a configured port that is absent is a warning
        // and a daemon that starts*, seen from the seam.
        let mut port = port;
        let mut buffer = [0u8; 8];
        assert_eq!(port.read(&mut buffer), None);
        port.write(&[0xE0, 0, 0]);
        port.refresh(std::time::Duration::from_millis(1));
        assert!(!port.connected());

        // Attached to a link, it is a surface that is simply not there yet —
        // which is the exit criterion in one line.
        let link = super::SurfaceLink::attach(Box::new(port), Bindings::defaults());
        assert_eq!(link.health(), SurfaceHealth::Disconnected);
        assert_eq!(link.open_name(), None);
        assert!(link.describe().contains("no such port"));
    }

    /// The two ports that are not MIDI ports answer the same way about being
    /// one, which is what a settings window needs in order to draw nothing
    /// rather than draw a guess.
    #[test]
    fn a_surface_that_is_not_a_midi_port_names_none() {
        let (port, _handle) = super::MockSurfacePort::new();
        assert_eq!(port.open_name(), None);
        assert_eq!(port.describe(), "a control surface");
    }

    #[test]
    fn a_port_that_cannot_tell_whether_it_is_there_says_it_is() {
        struct Silent;
        impl SurfacePort for Silent {
            fn read(&mut self, _buffer: &mut [u8]) -> Option<usize> {
                None
            }
            fn write(&mut self, _bytes: &[u8]) {}
        }
        assert!(Silent.connected());
        // And a port with nothing to notice notices nothing, rather than
        // needing to say so.
        let mut silent = Silent;
        silent.refresh(std::time::Duration::from_secs(1));
        assert_eq!(silent.open_name(), None);
        assert_eq!(silent.read(&mut [0u8; 4]), None);
        silent.write(&[0x90, 54, 127]);
        assert_eq!(silent.describe(), "a control surface");
    }
}
