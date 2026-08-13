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
    Bindings, ButtonId, DisplayLine, Fader, GlobalButton, LedState, MAX_MESSAGE_BYTES, MAX_STRIPS,
    StripButton, SurfaceController, SurfaceCounters, SurfaceEvent, SurfaceHealth, SurfaceTiming,
    X_TOUCH,
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
/// `#[cfg(target_os = …)]` (`ARCHITECTURE_SPEC.md` §10.1). **No implementation
/// that opens a real device exists yet**: S20's probe is the only code in the
/// repository that does, and it lives outside the workspace on purpose.
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
    /// The surface is marked connected immediately, which invalidates the shadow
    /// model and therefore draws the whole picture once — §5.3's resync burst,
    /// paced by the layer below rather than poured into the port.
    #[must_use]
    pub fn attach(port: Box<dyn SurfacePort>, bindings: Bindings) -> Self {
        let mut controller = SurfaceController::new(X_TOUCH);
        controller.connected(Duration::ZERO);
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
            reported: SurfaceHealth::Connected,
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
        if !self.port.connected() && self.controller.health().is_attached() {
            // The port itself says it has gone. §5.3: a hardware fault like any
            // other — the picture is kept, nothing is sent, and the engine is
            // never told.
            self.controller.disconnected();
        }
        if let Some(notice) = self.health_notice() {
            deltas.push(notice);
        }
        deltas
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
    fn apply(&mut self, desk: &Desk) -> Vec<Delta> {
        let context = context_of(&desk.core());
        let mut deltas = Vec::new();
        for event in &self.events {
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
        deltas
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
            let level = executor.map_or(0, |executor| executor.master_level);
            self.controller.set_fader(Fader::Strip(index), level);
            self.controller.set_led(
                ButtonId::Strip {
                    strip: index,
                    button: StripButton::Select,
                },
                if executor.is_some_and(|executor| executor.is_active) {
                    LedState::On
                } else {
                    LedState::Off
                },
            );
            // The name is the sequence's, because that is what an operator
            // named. An executor with no sequence shows its own number, which is
            // what makes an empty strip readable rather than blank.
            self.text.clear();
            let name = executor
                .and_then(|executor| executor.sequence_id)
                .and_then(|sequence| core.file.show.sequence(sequence))
                .map(|sequence| sequence.name.as_str());
            match name {
                Some(name) => self.text.push_str(name),
                None => {
                    let _ = write!(self.text, "Ex {}", id.get());
                }
            }
            self.controller
                .set_text(index, DisplayLine::Upper, &self.text);
            self.text.clear();
            // §4.1's "value": the master as a percentage, which is what the
            // fader beside it is showing.
            let _ = write!(self.text, "{}%", percent(level));
            self.controller
                .set_text(index, DisplayLine::Lower, &self.text);
        }
        let selected = session
            .selected_executor
            .and_then(|id| core.file.show.executor(id))
            .map_or(0, |executor| executor.master_level);
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

/// A level as whole per cent.
fn percent(level: u16) -> u32 {
    (u32::from(level) * 100).div_ceil(u32::from(u16::MAX))
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
/// it, in `AttributeType::ALL`'s order — the same order the encoder bar will
/// show (S26). An index past the end is nothing rather than the last one: the
/// wheel then does nothing, which is what an operator who has paged past the
/// parameters should feel.
fn parameter_of(group: prism_domain::FeatureGroup, index: u32) -> Option<AttributeType> {
    AttributeType::ALL
        .into_iter()
        .filter(|attribute| attribute.feature_group() == group)
        .nth(index as usize)
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
}

/// Reads a binding profile from disk, and **cannot fail**.
///
/// The filesystem half of `prism_surface::Bindings::load`: which file it was is
/// this crate's business, because a crate that took a path could not have its
/// fallback tested without one. Every way this can go wrong — no file, no
/// permission, bad JSON, a binding on the reserved button — produces the
/// built-in defaults and a warning in the log, never a daemon that will not
/// start.
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
    use super::{SurfacePort, load_profile, notice_for, parameter_of, percent};
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

    #[test]
    fn a_level_reads_as_the_percentage_an_operator_expects() {
        // Rounding up, so that anything above zero shows as at least 1 % and a
        // fader off the stop is visibly not off.
        assert_eq!(percent(0), 0);
        assert_eq!(percent(u16::MAX), 100);
        assert_eq!(percent(u16::MAX / 2), 50);
        assert_eq!(percent(1), 1);
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
    fn a_port_that_cannot_tell_whether_it_is_there_says_it_is() {
        struct Silent;
        impl SurfacePort for Silent {
            fn read(&mut self, _buffer: &mut [u8]) -> Option<usize> {
                None
            }
            fn write(&mut self, _bytes: &[u8]) {}
        }
        assert!(Silent.connected());
    }
}
