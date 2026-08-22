//! A configured MIDI port, and the cable coming and going — S36.
//!
//! [`MidiSurfacePort`] is what `prismd` puts behind its `SurfacePort` seam. It
//! holds a **name** rather than a device: the port it wants may not be there
//! yet, may go away in the middle of a show, and may come back — and none of
//! those is a failure that reaches the daemon, let alone the engine.
//!
//! # The state machine, and the two ways a port can go
//!
//! ```text
//!            ┌───────────── refresh(now), after the backoff ─────────────┐
//!            ▼                                                           │
//!        [ closed ] ── open succeeds ──▶ [ open ] ── gone from the list ──┤
//!         error says                                └── a write failed ───┘
//!         why, and it
//!         is a warning
//! ```
//!
//! **Neither edge is silence.** A desk that has stopped sending is not a desk
//! that has gone: S20 measured an X-Touch that went on receiving perfectly —
//! text written to it still appeared — while sending nothing at all, and
//! established that reopening the port does not help and neither does a fresh
//! process (`docs/MCU_MAPPING.md` §2.7). Noticing that and saying *power-cycle
//! it* is `prism_surface::SurfaceHealth::Unresponsive`'s job, one layer up, and
//! this layer must not undo it by reconnecting underneath. So the only two
//! things that close a port here are the port disappearing from the enumeration
//! and a write the operating system refused.

use core::fmt;
use core::time::Duration;

use crate::PortList;

/// The longest a port that is not there waits between attempts.
///
/// The backoff doubles from [`FIRST_ATTEMPT`] to here and stays. Five seconds
/// because that is about how long it takes to walk to a desk and plug it in:
/// long enough that a missing port costs a rehearsal nothing, short enough that
/// nobody stands there wondering.
pub const RECONNECT_CEILING: Duration = Duration::from_secs(5);

/// How long after a failed attempt the next one is made.
const FIRST_ATTEMPT: Duration = Duration::from_millis(250);

/// How often an open port is checked against the enumeration.
///
/// Enumerating is a system call and this is on the surface thread, which polls
/// every millisecond (`prismd::surface::SURFACE_PERIOD`): asking the operating
/// system what is plugged in a thousand times a second to learn something that
/// changes when a person moves a plug would be a waste with a measurable cost.
const PRESENCE_INTERVAL: Duration = Duration::from_millis(1000);

/// Why a port is not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortError {
    /// This build has no MIDI backend — see the crate documentation and the
    /// `alsa` feature. Enumeration answers an empty list and nothing opens.
    NoBackend,
    /// Nothing plugged in answers to that name.
    ///
    /// The ordinary state of a correct configuration on a machine whose desk is
    /// switched off, and the reason a configured port that is not there is a
    /// **warning and a daemon that starts**.
    NotFound {
        /// What was asked for.
        configured: String,
    },
    /// The operating system refused, in its own words.
    Refused(String),
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBackend => f.write_str(
                "this build has no MIDI backend (Linux needs --features prism-midi/alsa)",
            ),
            Self::NotFound { configured } => {
                write!(f, "no MIDI port called {configured:?} is plugged in")
            }
            Self::Refused(why) => write!(f, "the MIDI port could not be opened: {why}"),
        }
    }
}

impl core::error::Error for PortError {}

/// What a platform's MIDI service can do, and the seam a test stands in for.
///
/// `CLAUDE.md`: every hardware interface sits behind a trait so the suite runs
/// with nothing plugged in. This is that trait for MIDI, and it is the same
/// move `prism_protocols::FtdiBackend` makes one cable along — which is why the
/// reconnection arithmetic below can be asserted exactly, with a fake that
/// unplugs itself on demand, on a machine that has a real X-Touch attached.
pub trait MidiBackend: Send {
    /// What is plugged in. **Must not fail**: a machine with nothing attached
    /// answers with an empty list.
    fn ports(&self) -> PortList;

    /// Opens the port a configured name selects, in both directions.
    ///
    /// # Errors
    ///
    /// [`PortError`], and the caller warns and tries again later. A surface
    /// that will not open must never stop a daemon starting.
    fn open(&self, configured: &str) -> Result<Box<dyn OpenMidiPort>, PortError>;
}

/// One open port, both directions.
pub trait OpenMidiPort: Send {
    /// What the operating system calls it — the *port's* name, which may carry
    /// decoration the configured name did not.
    fn name(&self) -> &str;

    /// The next message, if one is waiting.
    ///
    /// A message longer than `buffer` is delivered in pieces and the rest is
    /// kept for the next call, because the layer above is a **stream** decoder
    /// with SysEx reassembly (S19): a split message is put back together
    /// exactly as it is when a USB packet boundary lands in the middle of one.
    /// Truncating instead would corrupt a scribble-strip reply.
    fn read(&mut self, buffer: &mut [u8]) -> Option<usize>;

    /// Sends one message. `false` means the port has gone.
    fn write(&mut self, bytes: &[u8]) -> bool;
}

/// The queue between the MIDI service's callback thread and the surface poll.
///
/// **Here rather than in the backend** because it is the one piece of a real
/// port that is arithmetic rather than platform: whole messages go in, and what
/// comes out is however many bytes the caller had room for, with the rest kept
/// for the next call.
///
/// # Why the rest is kept rather than dropped
///
/// The layer above is a **stream** decoder with running-status handling and
/// SysEx reassembly (S19), so a message split across two reads is put back
/// together exactly as it is when a USB packet boundary lands in the middle of
/// one. Truncating instead would corrupt a scribble-strip reply into a
/// different message, and the codec would count it rather than notice it — the
/// worst of the two failures, because the desk would look like it was answering.
#[derive(Debug, Default)]
pub struct Inbox {
    waiting: std::collections::VecDeque<u8>,
}

impl Inbox {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes one whole message from the service.
    pub fn push(&mut self, message: &[u8]) {
        self.waiting.extend(message);
    }

    /// Fills as much of `buffer` as there is, or `None` if there is nothing.
    pub fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
        let length = self.waiting.len().min(buffer.len());
        if length == 0 {
            return None;
        }
        for slot in buffer.get_mut(..length)? {
            *slot = self.waiting.pop_front()?;
        }
        Some(length)
    }

    /// Whether anything is waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

/// A MIDI port named in a configuration, present or not.
///
/// See the module documentation for the state machine. The four methods below
/// are exactly what `prismd::surface::SurfacePort` needs, which is deliberate:
/// this type does not implement that trait — `prism-midi` knows nothing about a
/// daemon — and the one-line implementation lives in `prismd`.
pub struct MidiSurfacePort {
    backend: Box<dyn MidiBackend>,
    /// The name as the configuration wrote it. Never changed by anything that
    /// happens to a cable.
    configured: String,
    open: Option<Box<dyn OpenMidiPort>>,
    /// Why it is not open, when it is not.
    error: Option<PortError>,
    /// When the next attempt to open may be made.
    next_attempt: Duration,
    /// How long to wait after the next failure.
    backoff: Duration,
    /// When the enumeration is next consulted about an open port.
    next_presence: Duration,
    /// How many times the port has been opened, ever.
    opens: u64,
    /// Messages that went nowhere because there was no port.
    dropped: u64,
}

impl fmt::Debug for MidiSurfacePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MidiSurfacePort")
            .field("configured", &self.configured)
            .field("open", &self.port_name())
            .field("error", &self.error)
            .field("reconnects", &self.reconnects())
            .field("dropped", &self.dropped)
            .finish()
    }
}

impl MidiSurfacePort {
    /// Opens the port a configured name selects, or arranges to keep trying.
    ///
    /// **Never fails**, which is the exit criterion rather than a convenience:
    /// a configured port that is not there is a warning and a daemon that
    /// starts, never a daemon that will not. [`error`](Self::error) is why, for
    /// the warning.
    #[must_use]
    pub fn attach(configured: impl Into<String>) -> Self {
        Self::attach_with(crate::system_backend(), configured)
    }

    /// The same, over a backend the caller supplies.
    ///
    /// Public because the reconnection arithmetic is worth asserting and a
    /// device cannot be asked to fall out of its socket on cue.
    #[must_use]
    pub fn attach_with(backend: Box<dyn MidiBackend>, configured: impl Into<String>) -> Self {
        let mut port = Self {
            backend,
            configured: configured.into(),
            open: None,
            error: None,
            next_attempt: Duration::ZERO,
            backoff: FIRST_ATTEMPT,
            next_presence: PRESENCE_INTERVAL,
            opens: 0,
            dropped: 0,
        };
        port.try_open(Duration::ZERO);
        port
    }

    /// The name the configuration asked for.
    #[must_use]
    pub fn configured(&self) -> &str {
        &self.configured
    }

    /// What the operating system calls the port that is open, or `None`.
    #[must_use]
    pub fn port_name(&self) -> Option<&str> {
        self.open.as_deref().map(OpenMidiPort::name)
    }

    /// Why the port is not open, or `None` when it is.
    #[must_use]
    pub const fn error(&self) -> Option<&PortError> {
        self.error.as_ref()
    }

    /// How many times a port that was there has come back.
    ///
    /// Zero for a port that opened at the first attempt and never went — the
    /// first open is not a reconnection.
    #[must_use]
    pub const fn reconnects(&self) -> u64 {
        self.opens.saturating_sub(1)
    }

    /// Messages that went nowhere because there was no port to send them to.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Whether the port is open.
    #[must_use]
    pub const fn connected(&self) -> bool {
        self.open.is_some()
    }

    /// The next message from the surface, if one is waiting.
    pub fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
        self.open.as_mut()?.read(buffer)
    }

    /// Sends one message, or counts it as dropped.
    ///
    /// A write the operating system refuses **closes the port**: that is one of
    /// the two things this crate treats as a cable coming out, and the layer
    /// above turns it into `SurfaceHealth::Disconnected`.
    pub fn write(&mut self, bytes: &[u8]) {
        let Some(port) = self.open.as_mut() else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        if !port.write(bytes) {
            self.dropped = self.dropped.saturating_add(1);
            self.close(PortError::Refused(
                "the port stopped accepting writes".to_owned(),
            ));
        }
    }

    /// Gives the port a chance to notice a cable — one that has gone, or one
    /// that has come back.
    ///
    /// Called once per poll by whoever owns the clock. It does at most one
    /// thing per call and usually nothing at all: an open port is compared
    /// against the enumeration once a second, and a closed one is retried on a
    /// backoff that doubles to [`RECONNECT_CEILING`].
    ///
    /// **Silence is not one of the things it looks at.** See the module
    /// documentation, and `docs/MCU_MAPPING.md` §2.7 for why that matters.
    pub fn refresh(&mut self, now: Duration) {
        if self.open.is_some() {
            if now < self.next_presence {
                return;
            }
            self.next_presence = now.saturating_add(PRESENCE_INTERVAL);
            // The name is taken here rather than inside, because a `still_listed`
            // that had to ask whether a port was open would carry an arm the
            // caller has already ruled out and no test could reach (S21's rule:
            // an unreachable branch is removed rather than covered).
            let name = self.open.as_deref().map(OpenMidiPort::name).unwrap_or("");
            if !self.still_listed(name) {
                self.close(PortError::NotFound {
                    configured: self.configured.clone(),
                });
                // Retried at once rather than after the backoff: a port that was
                // there a second ago and is gone now is most likely a plug being
                // moved, and the person moving it is standing there.
                self.next_attempt = now;
                self.backoff = FIRST_ATTEMPT;
            }
            return;
        }
        if now >= self.next_attempt {
            self.try_open(now);
        }
    }

    /// Whether the enumeration still offers the port that is open, by name.
    ///
    /// Both directions, because a surface is both: a desk whose output half has
    /// gone is a desk with no motor faders and no scribble strips, and going on
    /// as though it were there would be a picture nobody can see.
    fn still_listed(&self, name: &str) -> bool {
        let listed = self.backend.ports();
        listed.inputs.iter().any(|port| port == name)
            && listed.outputs.iter().any(|port| crate::selects(name, port))
    }

    /// One attempt, and the bookkeeping either way.
    fn try_open(&mut self, now: Duration) {
        match self.backend.open(&self.configured) {
            Ok(port) => {
                self.open = Some(port);
                self.error = None;
                self.opens = self.opens.saturating_add(1);
                self.backoff = FIRST_ATTEMPT;
                self.next_presence = now.saturating_add(PRESENCE_INTERVAL);
            }
            Err(error) => {
                self.error = Some(error);
                self.next_attempt = now.saturating_add(self.backoff);
                self.backoff = self.backoff.saturating_mul(2).min(RECONNECT_CEILING);
            }
        }
    }

    /// Puts the port down and says why.
    fn close(&mut self, error: PortError) {
        self.open = None;
        self.error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FIRST_ATTEMPT, MidiBackend, MidiSurfacePort, OpenMidiPort, PortError, RECONNECT_CEILING,
    };
    use crate::PortList;
    use core::time::Duration;
    use std::sync::{Arc, Mutex};

    /// The name the fake device below answers to.
    ///
    /// Decorated the way Windows decorates one, so the fake exercises
    /// `crate::selects`' second rule rather than only its first: a
    /// configuration saying `X-Touch` has to find a port called `2- X-Touch`.
    const PORT_TEST_NAME: &str = "2- X-Touch";

    /// What the fake device is doing, from the test's side.
    #[derive(Debug, Default)]
    struct Rig {
        /// Whether the cable is in.
        plugged: bool,
        /// What the operating system refuses with, if it refuses.
        refuse: Option<String>,
        /// Packets waiting to be read.
        inbound: Vec<Vec<u8>>,
        /// Everything written to the port.
        outbound: Vec<Vec<u8>>,
        /// Whether a write is accepted.
        writable: bool,
        /// How many times the port has been opened.
        opens: u32,
        /// How many times opening it has been *attempted*, refused or not.
        attempts: u32,
        /// How many times it has been enumerated.
        enumerations: u32,
    }

    #[derive(Debug, Clone, Default)]
    struct Fake(Arc<Mutex<Rig>>);

    impl Fake {
        fn plugged_in() -> Self {
            let fake = Self::default();
            fake.with(|rig| {
                rig.plugged = true;
                rig.writable = true;
            });
            fake
        }

        fn with<T>(&self, act: impl FnOnce(&mut Rig) -> T) -> T {
            act(&mut self.0.lock().expect("a test holds this alone"))
        }

        fn backend(&self) -> Box<dyn MidiBackend> {
            Box::new(self.clone())
        }
    }

    impl MidiBackend for Fake {
        fn ports(&self) -> PortList {
            self.with(|rig| {
                rig.enumerations = rig.enumerations.saturating_add(1);
                if rig.plugged {
                    PortList {
                        inputs: vec![PORT_TEST_NAME.to_owned()],
                        outputs: vec![PORT_TEST_NAME.to_owned()],
                    }
                } else {
                    PortList::default()
                }
            })
        }

        fn open(&self, configured: &str) -> Result<Box<dyn OpenMidiPort>, PortError> {
            self.with(|rig| {
                rig.attempts = rig.attempts.saturating_add(1);
                if let Some(why) = &rig.refuse {
                    return Err(PortError::Refused(why.clone()));
                }
                if !rig.plugged || !crate::selects(configured, PORT_TEST_NAME) {
                    return Err(PortError::NotFound {
                        configured: configured.to_owned(),
                    });
                }
                rig.opens = rig.opens.saturating_add(1);
                Ok(())
            })?;
            Ok(Box::new(FakePort {
                rig: Arc::clone(&self.0),
            }))
        }
    }

    struct FakePort {
        rig: Arc<Mutex<Rig>>,
    }

    impl OpenMidiPort for FakePort {
        fn name(&self) -> &str {
            PORT_TEST_NAME
        }

        fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
            let mut rig = self.rig.lock().expect("a test holds this alone");
            if rig.inbound.is_empty() {
                return None;
            }
            let packet = rig.inbound.remove(0);
            let length = packet.len().min(buffer.len());
            buffer
                .get_mut(..length)?
                .copy_from_slice(packet.get(..length)?);
            Some(length)
        }

        fn write(&mut self, bytes: &[u8]) -> bool {
            let mut rig = self.rig.lock().expect("a test holds this alone");
            if !rig.writable {
                return false;
            }
            rig.outbound.push(bytes.to_vec());
            true
        }
    }

    fn ms(milliseconds: u64) -> Duration {
        Duration::from_millis(milliseconds)
    }

    /// The framing rule the real backend depends on, tested where a device is
    /// not needed for it.
    #[test]
    fn a_message_too_long_for_the_caller_is_delivered_in_pieces_rather_than_cut() {
        use super::Inbox;

        let mut inbox = Inbox::new();
        assert!(inbox.is_empty());
        let mut buffer = [0u8; 4];
        assert_eq!(inbox.read(&mut buffer), None, "nothing is waiting");

        // A scribble-strip write: 63 bytes of SysEx into a four-byte buffer.
        let sysex: Vec<u8> = core::iter::once(0xF0u8)
            .chain((0..61).map(|byte| byte as u8))
            .chain(core::iter::once(0xF7))
            .collect();
        inbox.push(&sysex);
        assert!(!inbox.is_empty());
        let mut back = Vec::new();
        while let Some(length) = inbox.read(&mut buffer) {
            back.extend_from_slice(buffer.get(..length).expect("a length it just wrote"));
        }
        assert_eq!(back, sysex, "every byte arrives, in order, and none twice");
        assert!(inbox.is_empty());

        // Two messages queued behind each other come out as one stream, which
        // is what the decoder above wants: running status crosses a message
        // boundary and reassembly crosses a packet one.
        inbox.push(&[0x90, 54, 127]);
        inbox.push(&[0x90, 54, 0]);
        let mut wide = [0u8; 16];
        assert_eq!(inbox.read(&mut wide), Some(6));
        assert_eq!(&wide[..6], &[0x90, 54, 127, 0x90, 54, 0]);
        assert_eq!(inbox.read(&mut wide), None);

        // A zero-length buffer is not a reason to lose anything.
        inbox.push(&[0xF7]);
        assert_eq!(inbox.read(&mut []), None);
        assert_eq!(inbox.read(&mut wide), Some(1));
    }

    #[test]
    fn a_port_that_is_there_opens_at_once_and_carries_bytes_both_ways() {
        let fake = Fake::plugged_in();
        fake.with(|rig| rig.inbound.push(vec![0x90, 54, 127]));
        let mut port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        assert!(port.connected());
        assert_eq!(port.error(), None);
        assert_eq!(port.configured(), "X-Touch");
        assert_eq!(port.port_name(), Some(PORT_TEST_NAME));
        assert_eq!(port.reconnects(), 0, "the first open is not a reconnection");

        let mut buffer = [0u8; 8];
        assert_eq!(port.read(&mut buffer), Some(3));
        assert_eq!(&buffer[..3], &[0x90, 54, 127]);
        assert_eq!(port.read(&mut buffer), None);

        port.write(&[0xE0, 0x7C, 0x7F]);
        assert_eq!(
            fake.with(|rig| rig.outbound.clone()),
            vec![vec![0xE0, 0x7C, 0x7F]]
        );
        assert_eq!(port.dropped(), 0);
        // What a person sees at a breakpoint.
        let shown = format!("{port:?}");
        assert!(shown.contains("X-Touch"), "{shown}");
    }

    /// The exit criterion: a configured port that is not there is a warning,
    /// not a refusal to exist.
    #[test]
    fn a_port_that_is_not_there_is_a_reason_rather_than_a_failure() {
        let fake = Fake::default();
        let mut port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        assert!(!port.connected());
        assert_eq!(
            port.error(),
            Some(&PortError::NotFound {
                configured: "X-Touch".to_owned()
            })
        );
        assert!(
            port.error()
                .expect("a reason")
                .to_string()
                .contains("X-Touch")
        );
        // Reading and writing are ordinary no-ops rather than anything worse.
        let mut buffer = [0u8; 8];
        assert_eq!(port.read(&mut buffer), None);
        port.write(&[0xE0, 0, 0]);
        assert_eq!(port.dropped(), 1);
    }

    #[test]
    fn a_desk_plugged_in_after_the_daemon_started_comes_up_by_itself() {
        let fake = Fake::default();
        let mut port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        assert!(!port.connected());

        // Nothing happens before the backoff has run out, however often it is
        // asked — a busy poll must not become a busy retry.
        port.refresh(ms(1));
        port.refresh(ms(100));
        assert_eq!(fake.with(|rig| rig.opens), 0);

        fake.with(|rig| rig.plugged = true);
        port.refresh(ms(200));
        assert!(!port.connected(), "not until the backoff is up");
        port.refresh(FIRST_ATTEMPT);
        assert!(port.connected());
        assert_eq!(port.error(), None);
        assert_eq!(port.reconnects(), 0, "it had never been open");
    }

    /// The schedule, read off the *attempts* rather than described.
    ///
    /// A port that is not there is retried on a doubling backoff with a
    /// ceiling, and both halves matter: without the doubling, a desk nobody is
    /// ever going to plug in costs a system call every 250 ms for the length of
    /// a show; without the ceiling, a desk plugged in after an hour would wait
    /// most of another one.
    #[test]
    fn the_backoff_doubles_from_the_first_attempt_and_stops_at_the_ceiling() {
        let fake = Fake::default();
        let mut port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        assert_eq!(fake.with(|rig| rig.attempts), 1, "one at construction");

        // Walk the surface thread's own millisecond cadence and write down when
        // each attempt happened.
        let mut at = Vec::new();
        let mut seen = 1;
        let mut now = Duration::ZERO;
        while now < ms(30_000) {
            now = now.saturating_add(ms(1));
            port.refresh(now);
            let attempts = fake.with(|rig| rig.attempts);
            if attempts > seen {
                seen = attempts;
                at.push(now);
            }
        }
        let gaps: Vec<Duration> = at
            .iter()
            .zip(core::iter::once(&Duration::ZERO).chain(at.iter()))
            .map(|(this, previous)| this.saturating_sub(*previous))
            .collect();
        assert_eq!(
            gaps.get(..6),
            Some(
                [
                    ms(250),
                    ms(500),
                    ms(1000),
                    ms(2000),
                    ms(4000),
                    RECONNECT_CEILING
                ]
                .as_slice()
            ),
            "{gaps:?}"
        );
        assert!(
            gaps.iter().skip(5).all(|gap| *gap == RECONNECT_CEILING),
            "and it stays there: {gaps:?}"
        );
        assert_eq!(FIRST_ATTEMPT, ms(250));
    }

    #[test]
    fn a_cable_pulled_mid_show_closes_the_port_and_a_replug_opens_it_again() {
        let fake = Fake::plugged_in();
        let mut port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        assert!(port.connected());

        // The enumeration is consulted once a second and not once a
        // millisecond: a thousand refreshes inside the interval ask nothing.
        for tick in 1..=999 {
            port.refresh(ms(tick));
        }
        assert_eq!(fake.with(|rig| rig.enumerations), 0);

        fake.with(|rig| rig.plugged = false);
        port.refresh(ms(1000));
        assert!(!port.connected());
        assert_eq!(fake.with(|rig| rig.enumerations), 1);
        assert_eq!(
            port.error(),
            Some(&PortError::NotFound {
                configured: "X-Touch".to_owned()
            })
        );

        // Plugged back in: the next refresh takes it, because a plug being moved
        // is somebody standing at the desk rather than a fault to back off from.
        fake.with(|rig| rig.plugged = true);
        port.refresh(ms(1001));
        assert!(port.connected());
        assert_eq!(port.reconnects(), 1);
        assert_eq!(fake.with(|rig| rig.opens), 2);
    }

    #[test]
    fn a_write_the_system_refuses_is_a_cable_that_has_gone() {
        let fake = Fake::plugged_in();
        let mut port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        fake.with(|rig| rig.writable = false);
        port.write(&[0xE0, 0, 0]);
        assert!(
            !port.connected(),
            "a refused write is the other way a port goes"
        );
        assert_eq!(port.dropped(), 1);
        assert!(matches!(port.error(), Some(PortError::Refused(_))));
    }

    /// **S20's finding, as a rule this layer keeps.**
    ///
    /// The X-Touch can stop transmitting while it goes on receiving perfectly,
    /// and only a power cycle recovers it — reopening the port does not, and
    /// neither does a fresh process. A port layer that reconnected on silence
    /// would churn the one device state that cannot be recovered that way, and
    /// would take the `Unresponsive` health away from the operator who needs to
    /// read it.
    #[test]
    fn a_desk_that_has_gone_quiet_is_not_reopened() {
        let fake = Fake::plugged_in();
        let mut port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        let mut buffer = [0u8; 8];
        // Ten seconds of nothing at all coming back, with writes still landing —
        // which is exactly the state §2.7 describes.
        let mut now = Duration::ZERO;
        while now < ms(10_000) {
            now = now.saturating_add(ms(1));
            port.refresh(now);
            assert_eq!(port.read(&mut buffer), None);
            port.write(&[0x90, 0x5E, 0x7F]);
        }
        assert!(port.connected(), "the port never went; only the desk did");
        assert_eq!(
            fake.with(|rig| rig.opens),
            1,
            "it was opened once and left alone"
        );
        assert_eq!(port.reconnects(), 0);
        assert_eq!(port.dropped(), 0);
    }

    /// The convenience that uses this machine's own MIDI service, asked for a
    /// port **nothing can be called**.
    ///
    /// `CLAUDE.md`: no test touches a device, and a real X-Touch is attached to
    /// the machine this was written on. Nothing is opened here — the name
    /// selects no port, so the attempt stops at the enumeration — and what is
    /// asserted is the one thing that must be true on every machine: it answers
    /// with a port that is not there rather than refusing to exist.
    #[test]
    fn the_system_backend_is_reachable_without_a_device() {
        let port = MidiSurfacePort::attach("no such port \u{1F50C}");
        assert!(!port.connected());
        assert_eq!(port.configured(), "no such port \u{1F50C}");
        assert_eq!(port.port_name(), None);
        assert!(port.error().is_some(), "and it says why");
    }

    #[test]
    fn a_system_that_refuses_says_so_in_its_own_words() {
        let fake = Fake::plugged_in();
        fake.with(|rig| rig.refuse = Some("the device is in use".to_owned()));
        let port = MidiSurfacePort::attach_with(fake.backend(), "X-Touch");
        assert!(!port.connected());
        let shown = port.error().expect("a reason").to_string();
        assert!(shown.contains("the device is in use"), "{shown}");
        assert!(
            PortError::NoBackend.to_string().contains("alsa"),
            "a build with no backend has to say what turns one on"
        );
    }
}
