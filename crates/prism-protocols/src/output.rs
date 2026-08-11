//! What every DMX output looks like from the outside, and one that goes
//! nowhere.
//!
//! `ARCHITECTURE_SPEC.md` §7 gives the trait as three methods — `id`,
//! `send_frame` and `health`. Two more are needed to make it the thing an
//! output *thread* is written against rather than an interface a caller drives
//! by hand: the same §7 requires "exponential reconnect backoff" from every
//! driver, and a reconnect that only the concrete type knows how to perform
//! cannot be expressed in a generic thread body. [`DmxOutput::connect`] and
//! [`DmxOutput::shutdown`] are therefore part of the trait, and
//! [`OutputRunner`](crate::OutputRunner) is written once for every kind of
//! output rather than once per kind.
//!
//! [`DmxOutput::universes`] is the third addition, and it is what lets the
//! runner map a frame position to a wire: an Open DMX cable carries exactly one
//! universe (§7.1), an ArtNet node carries several, and neither of them should
//! have to be told which by the caller.

use core::fmt;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use prism_domain::{OutputHealth, OutputId, UniverseId};
use prism_engine::UNIVERSE_CHANNELS;

/// Why a frame did not go out.
///
/// Three variants, split by what the driver thread has to do next rather than
/// by what went wrong underneath: reconnect, carry on, or fix the
/// configuration. [`health`](Self::health) is the single place that turns one
/// into the status light an operator sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputError {
    /// The interface is not there. The runner marks it disconnected and retries
    /// with backoff.
    Disconnected,
    /// The interface is there and this frame did not go out intact — a short
    /// write, a refused transfer. The line is still up, so the next frame is
    /// attempted; the output is degraded, not gone.
    Faulted,
    /// The frame was for a universe this interface does not carry. A
    /// configuration mistake rather than a hardware fault, and never silent:
    /// dropping it quietly is how a universe ends up dark with every light in
    /// the UI green.
    UniverseNotCarried(UniverseId),
}

impl OutputError {
    /// The health this error leaves the output in.
    #[must_use]
    pub const fn health(self) -> OutputHealth {
        match self {
            Self::Disconnected => OutputHealth::Disconnected,
            Self::Faulted | Self::UniverseNotCarried(_) => OutputHealth::Degraded,
        }
    }
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "the interface is not connected"),
            Self::Faulted => write!(f, "the frame did not reach the interface intact"),
            Self::UniverseNotCarried(universe) => {
                write!(f, "this interface does not carry universe {universe}")
            }
        }
    }
}

impl std::error::Error for OutputError {}

/// One configured DMX output: a USB adapter, an ArtNet node, an sACN sender.
///
/// `Send` because every driver owns a thread (`ARCHITECTURE_SPEC.md` §3), and
/// object-safe because the daemon holds a list of outputs of different kinds.
pub trait DmxOutput: Send {
    /// Which configured output this is, as the UI and the telemetry channel
    /// name it.
    fn id(&self) -> OutputId;

    /// The universes this interface puts on the wire, in the order it sends
    /// them.
    fn universes(&self) -> &[UniverseId];

    /// Opens the interface and makes it ready to send.
    ///
    /// Called by the runner at start-up and again after every disconnect, so it
    /// must be safe to call on an output that is already open and on one that
    /// has just failed.
    ///
    /// # Errors
    ///
    /// [`OutputError`] if the interface cannot be brought up; the runner then
    /// waits out its backoff and asks again.
    fn connect(&mut self) -> Result<(), OutputError>;

    /// Puts one universe of channel data on the wire.
    ///
    /// # Errors
    ///
    /// [`OutputError`] if the frame did not go out. The variant decides whether
    /// the runner reconnects or simply tries the next frame.
    fn send_frame(
        &mut self,
        universe: UniverseId,
        data: &[u8; UNIVERSE_CHANNELS],
    ) -> Result<(), OutputError>;

    /// What the status light for this interface should show.
    fn health(&self) -> OutputHealth;

    /// Closes the interface. Infallible: it runs on the shutdown path, where
    /// there is nothing left to do about a failure.
    fn shutdown(&mut self);
}

/// An output that accepts every frame and puts it nowhere.
///
/// Not test scaffolding: `ARCHITECTURE_SPEC.md` §12 has the end-to-end tests
/// run "against a daemon in mock-output mode", and this is that mode. It also
/// makes the runner's own guarantees — reconnect backoff, panic containment —
/// testable without involving a cable at all, since a fault in a `DmxOutput` is
/// exactly what the runner is written to survive.
pub struct MockOutput {
    id: OutputId,
    universes: Vec<UniverseId>,
    health: OutputHealth,
    state: Arc<Mutex<MockState>>,
}

/// A test's view of a [`MockOutput`], usable after the output has been moved
/// onto its thread.
#[derive(Clone)]
pub struct MockOutputHandle {
    state: Arc<Mutex<MockState>>,
}

#[derive(Default)]
struct MockState {
    frames: Vec<(UniverseId, Vec<u8>)>,
    connect_attempts: usize,
    shutdowns: usize,
    connect_faults: VecDeque<OutputError>,
    send_faults: VecDeque<OutputError>,
    connect_panics: usize,
    send_panics: usize,
    shutdown_panics: usize,
}

/// Takes a lock without caring whether a previous holder panicked — the panic
/// tests unwind through code holding it, and the runner has to survive that.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl MockOutput {
    /// An output carrying the given universes, disconnected until
    /// [`connect`](DmxOutput::connect) is called.
    #[must_use]
    pub fn new(id: OutputId, universes: impl IntoIterator<Item = UniverseId>) -> Self {
        Self {
            id,
            universes: universes.into_iter().collect(),
            health: OutputHealth::Disconnected,
            state: Arc::new(Mutex::new(MockState::default())),
        }
    }

    /// A handle onto this output's recording, cloneable and usable from another
    /// thread once the output itself has been moved into a runner.
    #[must_use]
    pub fn handle(&self) -> MockOutputHandle {
        MockOutputHandle {
            state: Arc::clone(&self.state),
        }
    }
}

impl MockOutputHandle {
    /// Every frame accepted so far, in order.
    #[must_use]
    pub fn frames(&self) -> Vec<(UniverseId, Vec<u8>)> {
        lock(&self.state).frames.clone()
    }

    /// How many frames have been accepted.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        lock(&self.state).frames.len()
    }

    /// The most recent frame accepted.
    #[must_use]
    pub fn last_frame(&self) -> Option<(UniverseId, Vec<u8>)> {
        lock(&self.state).frames.last().cloned()
    }

    /// How many times the output has been asked to connect, successfully or
    /// not — which is what a backoff test counts.
    #[must_use]
    pub fn connect_attempts(&self) -> usize {
        lock(&self.state).connect_attempts
    }

    /// How many times the output has been shut down.
    #[must_use]
    pub fn shutdowns(&self) -> usize {
        lock(&self.state).shutdowns
    }

    /// Makes the next `times` connections fail.
    pub fn fail_connect(&self, times: usize, error: OutputError) {
        lock(&self.state)
            .connect_faults
            .extend(std::iter::repeat_n(error, times));
    }

    /// Makes the next `times` sends fail.
    pub fn fail_send(&self, times: usize, error: OutputError) {
        lock(&self.state)
            .send_faults
            .extend(std::iter::repeat_n(error, times));
    }

    /// Makes the next `times` sends panic — the fault `CLAUDE.md`'s zero-crash
    /// invariant is really about.
    pub fn panic_on_send(&self, times: usize) {
        lock(&self.state).send_panics += times;
    }

    /// Makes the next `times` connections panic.
    pub fn panic_on_connect(&self, times: usize) {
        lock(&self.state).connect_panics += times;
    }

    /// Makes the next `times` shutdowns panic — a clean stop must not become a
    /// crash because a driver fell over on the way out.
    pub fn panic_on_shutdown(&self, times: usize) {
        lock(&self.state).shutdown_panics += times;
    }
}

impl DmxOutput for MockOutput {
    fn id(&self) -> OutputId {
        self.id
    }

    fn universes(&self) -> &[UniverseId] {
        &self.universes
    }

    // A test double whose whole purpose is to produce the fault the runner has
    // to survive. The crate otherwise denies `panic!` outside tests, and this
    // is the one place where causing one is the feature.
    #[allow(clippy::panic)]
    fn connect(&mut self) -> Result<(), OutputError> {
        let fault = {
            let mut state = lock(&self.state);
            state.connect_attempts += 1;
            if state.connect_panics > 0 {
                state.connect_panics -= 1;
                drop(state);
                panic!("MockOutput was told to panic while connecting");
            }
            state.connect_faults.pop_front()
        };
        match fault {
            Some(error) => {
                self.health = error.health();
                Err(error)
            }
            None => {
                self.health = OutputHealth::Ok;
                Ok(())
            }
        }
    }

    #[allow(clippy::panic)] // See `connect` above.
    fn send_frame(
        &mut self,
        universe: UniverseId,
        data: &[u8; UNIVERSE_CHANNELS],
    ) -> Result<(), OutputError> {
        if !self.universes.contains(&universe) {
            return Err(OutputError::UniverseNotCarried(universe));
        }
        if !self.health.is_sending() {
            return Err(OutputError::Disconnected);
        }
        let fault = {
            let mut state = lock(&self.state);
            if state.send_panics > 0 {
                state.send_panics -= 1;
                drop(state);
                panic!("MockOutput was told to panic while sending");
            }
            match state.send_faults.pop_front() {
                Some(error) => Some(error),
                None => {
                    state.frames.push((universe, data.to_vec()));
                    None
                }
            }
        };
        match fault {
            Some(error) => {
                self.health = error.health();
                Err(error)
            }
            None => {
                self.health = OutputHealth::Ok;
                Ok(())
            }
        }
    }

    fn health(&self) -> OutputHealth {
        self.health
    }

    #[allow(clippy::panic)] // See `connect` above.
    fn shutdown(&mut self) {
        let mut state = lock(&self.state);
        state.shutdowns += 1;
        if state.shutdown_panics > 0 {
            state.shutdown_panics -= 1;
            drop(state);
            panic!("MockOutput was told to panic while shutting down");
        }
        drop(state);
        self.health = OutputHealth::Disconnected;
    }
}

#[cfg(test)]
mod tests {
    use super::{DmxOutput, MockOutput, OutputError};
    use prism_domain::{OutputHealth, OutputId, UniverseId};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    fn output(universes: &[u32]) -> MockOutput {
        MockOutput::new(
            OutputId::new(1),
            universes.iter().copied().map(UniverseId::new),
        )
    }

    #[test]
    fn an_output_that_has_not_connected_yet_is_disconnected() {
        // Same default as `prism_domain::OutputHealth`: an output nobody has
        // heard from is not "fine", it is silent.
        let output = output(&[1]);
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert_eq!(output.id(), OutputId::new(1));
        assert_eq!(output.universes(), [universe(1)]);
    }

    #[test]
    fn an_output_sends_only_once_it_is_connected() {
        let mut output = output(&[1]);
        assert_eq!(
            output.send_frame(universe(1), &[7; 512]),
            Err(OutputError::Disconnected)
        );
        output.connect().unwrap();
        assert_eq!(output.health(), OutputHealth::Ok);
        assert_eq!(output.send_frame(universe(1), &[7; 512]), Ok(()));
    }

    #[test]
    fn a_mock_output_records_the_frames_it_was_given() {
        let mut output = output(&[1, 2]);
        let handle = output.handle();
        output.connect().unwrap();
        output.send_frame(universe(1), &[1; 512]).unwrap();
        output.send_frame(universe(2), &[2; 512]).unwrap();

        assert_eq!(handle.frame_count(), 2);
        let frames = handle.frames();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].0, universe(1));
        assert_eq!(frames[0].1, vec![1u8; 512]);
        assert_eq!(handle.last_frame(), Some((universe(2), vec![2u8; 512])));
    }

    #[test]
    fn an_output_refuses_a_universe_it_does_not_carry() {
        // ARCHITECTURE_SPEC.md §7.1: exactly one universe per Open DMX adapter.
        // Sending universe 2 at a cable patched to universe 1 would put the
        // wrong show on the wire, so it is an error rather than a no-op.
        let mut output = output(&[1]);
        output.connect().unwrap();
        assert_eq!(
            output.send_frame(universe(2), &[0; 512]),
            Err(OutputError::UniverseNotCarried(universe(2)))
        );
        assert_eq!(output.handle().frame_count(), 0);
    }

    #[test]
    fn an_output_can_be_told_to_fail_a_send() {
        let mut output = output(&[1]);
        let handle = output.handle();
        output.connect().unwrap();
        handle.fail_send(1, OutputError::Disconnected);
        assert_eq!(
            output.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Disconnected)
        );
        // A lost link takes the output down with it, as a real one does.
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert_eq!(handle.frame_count(), 0);
    }

    #[test]
    fn a_faulted_send_leaves_the_output_up_but_degraded() {
        let mut output = output(&[1]);
        let handle = output.handle();
        output.connect().unwrap();
        handle.fail_send(1, OutputError::Faulted);
        assert_eq!(
            output.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Faulted)
        );
        assert_eq!(output.health(), OutputHealth::Degraded);
        // Still connected, so the next frame is attempted.
        assert_eq!(output.send_frame(universe(1), &[0; 512]), Ok(()));
        assert_eq!(output.health(), OutputHealth::Ok);
    }

    #[test]
    fn an_output_can_be_told_to_refuse_to_connect() {
        let mut output = output(&[1]);
        let handle = output.handle();
        handle.fail_connect(2, OutputError::Disconnected);
        assert_eq!(output.connect(), Err(OutputError::Disconnected));
        assert_eq!(output.connect(), Err(OutputError::Disconnected));
        assert_eq!(output.connect(), Ok(()));
        assert_eq!(handle.connect_attempts(), 3);
    }

    #[test]
    fn an_output_can_be_told_to_panic() {
        // The zero-crash invariant is asserted against this in `runner`; here
        // it is only established that the mock can produce the fault at all.
        let mut output = output(&[1]);
        let handle = output.handle();
        output.connect().unwrap();
        handle.panic_on_send(1);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            output.send_frame(universe(1), &[0; 512])
        }));
        assert!(outcome.is_err());
        // And the next send is normal again.
        assert_eq!(output.send_frame(universe(1), &[0; 512]), Ok(()));
    }

    #[test]
    fn an_output_can_be_told_to_panic_while_connecting() {
        let mut output = output(&[1]);
        output.handle().panic_on_connect(1);
        let outcome = catch_unwind(AssertUnwindSafe(|| output.connect()));
        assert!(outcome.is_err());
        assert_eq!(output.connect(), Ok(()));
    }

    #[test]
    fn an_output_can_be_told_to_panic_on_the_way_out() {
        let mut output = output(&[1]);
        let handle = output.handle();
        output.connect().unwrap();
        handle.panic_on_shutdown(1);
        let outcome = catch_unwind(AssertUnwindSafe(|| output.shutdown()));
        assert!(outcome.is_err());
        assert_eq!(handle.shutdowns(), 1);
    }

    #[test]
    fn shutting_an_output_down_disconnects_it() {
        let mut output = output(&[1]);
        let handle = output.handle();
        output.connect().unwrap();
        output.shutdown();
        assert_eq!(output.health(), OutputHealth::Disconnected);
        assert_eq!(handle.shutdowns(), 1);
        assert_eq!(
            output.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Disconnected)
        );
    }

    #[test]
    fn an_output_error_maps_to_the_health_the_status_light_shows() {
        // One place decides this, because the runner reports health outwards
        // and must not invent its own opinion about what an error means.
        assert_eq!(
            OutputError::Disconnected.health(),
            OutputHealth::Disconnected
        );
        assert_eq!(OutputError::Faulted.health(), OutputHealth::Degraded);
        assert_eq!(
            OutputError::UniverseNotCarried(universe(3)).health(),
            OutputHealth::Degraded
        );
    }

    #[test]
    fn an_output_error_says_what_went_wrong_in_words() {
        let errors = [
            (OutputError::Disconnected, "the interface is not connected"),
            (
                OutputError::Faulted,
                "the frame did not reach the interface intact",
            ),
            (
                OutputError::UniverseNotCarried(universe(3)),
                "this interface does not carry universe 3",
            ),
        ];
        for (error, text) in errors {
            assert_eq!(error.to_string(), text);
            let as_error: &dyn std::error::Error = &error;
            assert_eq!(as_error.to_string(), text);
        }
    }

    #[test]
    fn an_output_can_be_used_through_a_trait_object() {
        // The daemon holds a list of configured outputs of different kinds, so
        // the trait has to stay object-safe.
        let mut outputs: Vec<Box<dyn DmxOutput>> = vec![Box::new(output(&[1]))];
        for out in &mut outputs {
            out.connect().unwrap();
            out.send_frame(universe(1), &[9; 512]).unwrap();
            assert_eq!(out.health(), OutputHealth::Ok);
        }
    }

    #[test]
    fn a_mock_output_carries_every_universe_it_was_given() {
        let mut output = output(&[4, 7]);
        output.connect().unwrap();
        assert_eq!(output.universes(), [universe(4), universe(7)]);
        assert_eq!(output.send_frame(universe(4), &[0; 512]), Ok(()));
        assert_eq!(output.send_frame(universe(7), &[0; 512]), Ok(()));
        assert_eq!(
            output.send_frame(universe(5), &[0; 512]),
            Err(OutputError::UniverseNotCarried(universe(5)))
        );
    }
}
