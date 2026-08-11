//! The thread every output driver runs on.
//!
//! `ARCHITECTURE_SPEC.md` §3 gives each output its own thread and §7 says what
//! that thread owes the rest of the system: `catch_unwind`, exponential
//! reconnect backoff from 100 ms to 5 s, and a health value the UI can show as
//! a status light per interface. This module is that thread, written once for
//! every kind of output — which is why [`DmxOutput`] carries `connect` and
//! `universes` beyond the three methods §7 sketches.
//!
//! # Why it is a state machine and not a loop with sleeps in it
//!
//! [`OutputRunner::step`] takes a `prism_engine::Clock`, so the whole of the
//! reconnect policy can be driven by `ManualClock` in a unit test: the
//! assertion that the third retry lands 700 ms after the cable was pulled is
//! exact and takes microseconds. [`run`](OutputRunner::run) is then a loop
//! around `step` with nothing in it, and [`spawn`] is `run` on a named thread.
//!
//! # Why it never sleeps longer than one cadence
//!
//! The backoff can reach five seconds, and a thread asleep for five seconds is
//! a thread that takes five seconds to notice the show is being shut down.
//! `step` therefore always wakes at the output's own cadence and compares the
//! clock against the retry time, rather than sleeping until it.
//!
//! # Why the cadence is the output's and not the engine's
//!
//! S2 measured it: four driver threads polling at 500 µs cost the engine 45
//! ticks in ten minutes, and the same threads at 5 ms cost it none. An Open DMX
//! adapter can send at about 30–40 Hz (`ARCHITECTURE_SPEC.md` §7.1), so that is
//! what its thread wakes at. Falling behind the 44 Hz engine is the designed
//! behaviour, not a fault: the triple buffer always hands over the *newest*
//! frame, so a slow output shows the current look, never a queued old one.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use prism_domain::{OutputHealth, UniverseId};
use prism_engine::{Clock, FrameSubscriber, SystemClock, TICK_PERIOD, UNIVERSE_CHANNELS};

use crate::device::DeviceProfile;
use crate::output::{DmxOutput, OutputError};

/// How the reconnect delay grows while an interface stays down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffConfig {
    /// The first delay after a connection is lost.
    pub initial: Duration,
    /// The ceiling. A cable that has been unplugged for an hour is still
    /// retried every five seconds, so plugging it back in is noticed promptly.
    pub max: Duration,
    /// What each delay is multiplied by.
    pub factor: u32,
}

impl Default for BackoffConfig {
    /// `ARCHITECTURE_SPEC.md` §7: 100 ms → 5 s.
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(5),
            factor: 2,
        }
    }
}

/// The reconnect delay, as it stands after some number of failures.
#[derive(Debug, Clone, Copy)]
pub struct Backoff {
    config: BackoffConfig,
    next: Option<Duration>,
}

impl Backoff {
    /// A backoff that has not failed yet.
    #[must_use]
    pub const fn new(config: BackoffConfig) -> Self {
        Self { config, next: None }
    }

    /// The delay to wait before the next attempt, and the growth that goes with
    /// it.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self
            .next
            .unwrap_or(self.config.initial)
            .min(self.config.max);
        self.next = Some(
            delay
                .checked_mul(self.config.factor)
                .unwrap_or(self.config.max)
                .min(self.config.max),
        );
        delay
    }

    /// Forgets the failures. Called when a connection succeeds, so an interface
    /// that drops twice in an evening is not five seconds slow the second time.
    pub const fn reset(&mut self) {
        self.next = None;
    }
}

/// How an output thread is paced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerConfig {
    /// The interval between frames this output attempts.
    pub cadence: Duration,
    /// The reconnect policy.
    pub backoff: BackoffConfig,
}

impl RunnerConfig {
    /// The pacing an adapter's own profile asks for.
    #[must_use]
    pub fn for_profile(profile: &DeviceProfile) -> Self {
        Self {
            cadence: profile.timing.min_frame_interval,
            backoff: BackoffConfig::default(),
        }
    }
}

impl Default for RunnerConfig {
    /// One engine tick period, which is as fast as there is ever any point
    /// sending: the engine produces nothing new in between. Outputs whose
    /// hardware is slower than that say so through
    /// [`for_profile`](Self::for_profile).
    fn default() -> Self {
        Self {
            cadence: TICK_PERIOD,
            backoff: BackoffConfig::default(),
        }
    }
}

/// What one interface is doing, readable from any thread.
///
/// `ARCHITECTURE_SPEC.md` §7: health is reported outwards so the UI can show a
/// status light per interface. Atomics rather than a lock, because the reader
/// is the telemetry path and the writer is a driver thread that must not be
/// held up by it.
#[derive(Debug, Default)]
pub struct OutputStatus {
    health: AtomicU8,
    frames: AtomicU64,
    errors: AtomicU64,
    panics: AtomicU64,
    connections: AtomicU64,
    stop: AtomicBool,
}

/// `OutputHealth` as one byte. The mapping is private, and the round trip is
/// asserted for all three states: a health that came back as the wrong value
/// would be a green light over a dead universe.
const fn encode_health(health: OutputHealth) -> u8 {
    match health {
        OutputHealth::Ok => 0,
        OutputHealth::Degraded => 1,
        OutputHealth::Disconnected => 2,
    }
}

const fn decode_health(byte: u8) -> OutputHealth {
    match byte {
        0 => OutputHealth::Ok,
        1 => OutputHealth::Degraded,
        _ => OutputHealth::Disconnected,
    }
}

impl OutputStatus {
    /// A status for an interface nobody has heard from: disconnected, as
    /// `prism_domain::OutputHealth` defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            health: AtomicU8::new(encode_health(OutputHealth::Disconnected)),
            ..Self::default()
        }
    }

    /// What the status light for this interface should show.
    #[must_use]
    pub fn health(&self) -> OutputHealth {
        decode_health(self.health.load(Ordering::Relaxed))
    }

    /// Universes put on the wire since the thread started. One frame here is
    /// one universe, so a two-universe node counts two per cadence.
    #[must_use]
    pub fn frames_sent(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    /// Frames and connection attempts that failed.
    #[must_use]
    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    /// Panics caught inside the driver. Monotonic on purpose: the health goes
    /// green again as soon as frames flow, and the fact that it happened should
    /// not go green with it.
    #[must_use]
    pub fn panics(&self) -> u64 {
        self.panics.load(Ordering::Relaxed)
    }

    /// Successful connections, the first one included. A count above one means
    /// the interface has been away and come back.
    #[must_use]
    pub fn connections(&self) -> u64 {
        self.connections.load(Ordering::Relaxed)
    }

    /// Asks the thread to shut its output down and finish.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Whether a stop has been asked for.
    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Publishes the health of the interface.
    pub fn set_health(&self, health: OutputHealth) {
        self.health.store(encode_health(health), Ordering::Relaxed);
    }

    fn count_frame(&self) {
        self.frames.fetch_add(1, Ordering::Relaxed);
    }

    fn count_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    fn count_panic(&self) {
        self.panics.fetch_add(1, Ordering::Relaxed);
    }

    fn count_connection(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }
}

/// What one iteration of the driver thread did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// Frames went out on every universe this output carries.
    Sent,
    /// The interface is down and the backoff has not expired yet.
    Waiting,
    /// The interface came up.
    Connected,
    /// It did not, and the backoff grew.
    ConnectFailed,
    /// A frame did not go out cleanly, but the interface is still there.
    Faulted,
    /// The interface went away during this step.
    Disconnected,
    /// The driver panicked, and did not take the process with it.
    Panicked,
    /// A stop was requested; the loop is over.
    Stopped,
}

/// One output driver, and the policy its thread runs.
///
/// Generic over the clock so the reconnect timing is a unit test rather than a
/// wait — see this module's documentation.
pub struct OutputRunner<O: DmxOutput, C: Clock> {
    output: O,
    subscriber: FrameSubscriber,
    clock: C,
    config: RunnerConfig,
    backoff: Backoff,
    status: Arc<OutputStatus>,
    /// The universes this output carries that the engine actually publishes,
    /// paired with their position in the frame. Resolved once, at set-up: a
    /// lookup per universe per frame would be work done 40 times a second to
    /// answer a question that cannot change.
    mapped: Vec<(UniverseId, usize)>,
    /// The universes it carries that the engine does not publish. Kept rather
    /// than discarded, for the same reason S5 counts unresolved cue parts:
    /// dropped silently, nobody can find out why a universe is dark.
    unmapped: Vec<UniverseId>,
    next_wake: Duration,
    retry_at: Duration,
}

impl<O: DmxOutput, C: Clock> OutputRunner<O, C> {
    /// Attaches an output to a frame stream.
    ///
    /// The subscriber comes from `FramePublisher::subscribe`, which allocates
    /// and must therefore be called during set-up rather than while the tick is
    /// running — S2's decision log, and it is why outputs are attached before
    /// the engine starts.
    #[must_use]
    pub fn new(output: O, subscriber: FrameSubscriber, clock: C, config: RunnerConfig) -> Self {
        let mut mapped = Vec::new();
        let mut unmapped = Vec::new();
        for &universe in output.universes() {
            match subscriber.layout().index_of(universe) {
                Some(position) => mapped.push((universe, position)),
                None => unmapped.push(universe),
            }
        }
        let backoff = Backoff::new(config.backoff);
        Self {
            output,
            subscriber,
            clock,
            config,
            backoff,
            status: Arc::new(OutputStatus::new()),
            mapped,
            unmapped,
            next_wake: Duration::ZERO,
            retry_at: Duration::ZERO,
        }
    }

    /// The health and counters this thread publishes, shareable with the UI.
    #[must_use]
    pub const fn status(&self) -> &Arc<OutputStatus> {
        &self.status
    }

    /// The universes this output carries that the engine does not publish.
    #[must_use]
    pub fn unmapped(&self) -> &[UniverseId] {
        &self.unmapped
    }

    /// The runner's own view of the time, for tests on a simulated clock.
    #[must_use]
    pub fn now(&self) -> Duration {
        self.clock.now()
    }

    /// Waits for the next cadence and then does one thing: connect, or send.
    ///
    /// Both possibilities run inside [`catch_unwind`]. `CLAUDE.md`'s zero-crash
    /// invariant is about exactly this boundary — a fault in a driver has to
    /// cost frames, never the process — so a panic is counted, the interface is
    /// marked degraded, and the loop goes round again.
    pub fn step(&mut self) -> StepOutcome {
        if self.status.stop_requested() {
            return StepOutcome::Stopped;
        }
        self.clock.sleep_until(self.next_wake);
        let now = self.clock.now();
        self.next_wake = now + self.config.cadence;

        if self.output.health().is_sending() {
            let outcome = self.send_step();
            if outcome == StepOutcome::Disconnected {
                // Losing the interface starts the backoff, rather than the
                // first failed reconnection starting it: otherwise every
                // unplug is followed by one immediate attempt that cannot
                // succeed, and the delays are all one attempt late.
                self.retry_at = now + self.backoff.next_delay();
            }
            outcome
        } else if now < self.retry_at {
            // Down, and it is not time to try again. Nothing happened, so
            // nothing is reported: the health stays as the last step left it.
            StepOutcome::Waiting
        } else {
            self.connect_step(now)
        }
    }

    fn connect_step(&mut self, now: Duration) -> StepOutcome {
        let output = &mut self.output;
        let attempt = catch_unwind(AssertUnwindSafe(|| output.connect()));
        let outcome = match attempt {
            Ok(Ok(())) => {
                self.backoff.reset();
                self.status.count_connection();
                self.status.set_health(self.output.health());
                return StepOutcome::Connected;
            }
            Ok(Err(error)) => {
                self.status.count_error();
                self.status.set_health(error.health());
                StepOutcome::ConnectFailed
            }
            Err(_) => {
                self.status.count_panic();
                self.status.set_health(OutputHealth::Degraded);
                StepOutcome::Panicked
            }
        };
        // A driver that panics on every call would spin a core if the panic
        // were not treated as a failed attempt, so it backs off like one.
        self.retry_at = now + self.backoff.next_delay();
        outcome
    }

    fn send_step(&mut self) -> StepOutcome {
        let Self {
            output,
            subscriber,
            mapped,
            status,
            ..
        } = self;
        let attempt = catch_unwind(AssertUnwindSafe(|| {
            Self::send_all(output, subscriber, mapped, status)
        }));
        match attempt {
            Ok(outcome) => {
                if outcome != StepOutcome::Disconnected {
                    self.status.set_health(self.output.health());
                }
                outcome
            }
            Err(_) => {
                self.status.count_panic();
                self.status.set_health(OutputHealth::Degraded);
                StepOutcome::Panicked
            }
        }
    }

    /// Sends the newest frame on every universe this output carries.
    ///
    /// An associated function rather than a method so the closure it runs in
    /// borrows the fields it needs and not the whole runner.
    ///
    /// The refresh takes the newest published frame if there is one and leaves
    /// the previous one in place if there is not — and the frame is sent
    /// either way. A DMX receiver that stops being refreshed times out, so
    /// "the engine published nothing new" means "hold this look", not "stop
    /// driving the line".
    fn send_all(
        output: &mut O,
        subscriber: &mut FrameSubscriber,
        mapped: &[(UniverseId, usize)],
        status: &OutputStatus,
    ) -> StepOutcome {
        subscriber.refresh();
        let frame = subscriber.frame();
        let mut outcome = StepOutcome::Sent;
        for &(universe, position) in mapped {
            // A position that came out of the layout always names a universe of
            // exactly 512 bytes in a frame of that layout, so neither of these
            // can fail from a subscriber this runner was built with. It is a
            // skip rather than a panic because the alternative on a driver
            // thread is a dark stage, and it is the only line in the crate that
            // no test can reach — deliberately, since inventing a frame that
            // disagreed with its own layout would test nothing real.
            let Some(channels) = frame
                .universe(position)
                .and_then(|slice| <&[u8; UNIVERSE_CHANNELS]>::try_from(slice).ok())
            else {
                continue;
            };
            match output.send_frame(universe, channels) {
                Ok(()) => status.count_frame(),
                Err(error) => {
                    status.count_error();
                    if error == OutputError::Disconnected {
                        // The interface has gone. The universes after this one
                        // are on the same cable, so there is nothing to try.
                        status.set_health(OutputHealth::Disconnected);
                        return StepOutcome::Disconnected;
                    }
                    outcome = StepOutcome::Faulted;
                }
            }
        }
        outcome
    }

    /// Steps until a stop is asked for, then shuts the output down.
    ///
    /// The shutdown is contained too: a driver that panics on the way out must
    /// not turn a clean shutdown into a crash.
    pub fn run(&mut self) {
        while self.step() != StepOutcome::Stopped {}
        let output = &mut self.output;
        if catch_unwind(AssertUnwindSafe(|| output.shutdown())).is_err() {
            self.status.count_panic();
        }
        self.status.set_health(OutputHealth::Disconnected);
    }
}

/// A driver running on its own thread.
///
/// Dropping it stops the thread and waits for it: an output thread that
/// outlived its handle would hold a cable open after the output was deleted
/// from the show.
pub struct OutputThread {
    status: Arc<OutputStatus>,
    join: Option<JoinHandle<()>>,
}

impl OutputThread {
    /// The health and counters the thread publishes.
    #[must_use]
    pub const fn status(&self) -> &Arc<OutputStatus> {
        &self.status
    }

    /// Asks the thread to finish and waits for it.
    pub fn stop(mut self) {
        self.stop_and_join();
    }

    fn stop_and_join(&mut self) {
        self.status.request_stop();
        if let Some(join) = self.join.take() {
            // A driver thread cannot panic out of `run` — everything it calls
            // is contained — but joining is not the place to find out.
            drop(join.join());
        }
    }
}

impl Drop for OutputThread {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

/// Puts an output on its own named thread.
///
/// The name is what `ARCHITECTURE_SPEC.md` §3 calls the thread — `out-opendmx-1`
/// and so on — and it is what a debugger and a crash dump will show.
///
/// # Errors
///
/// Whatever the operating system says if the thread cannot be created.
pub fn spawn<O: DmxOutput + 'static>(
    name: &str,
    output: O,
    subscriber: FrameSubscriber,
    config: RunnerConfig,
) -> std::io::Result<OutputThread> {
    let mut runner = OutputRunner::new(output, subscriber, SystemClock::new(), config);
    let status = Arc::clone(runner.status());
    let join = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || runner.run())?;
    Ok(OutputThread {
        status,
        join: Some(join),
    })
}

#[cfg(test)]
mod tests {
    use super::{Backoff, BackoffConfig, OutputRunner, OutputStatus, RunnerConfig, StepOutcome};
    use crate::device::SH_RS09B;
    use crate::ftdi::{FtdiError, MockFtdi, MockFtdiHandle};
    use crate::opendmx::OpenDmxUsb;
    use crate::output::{MockOutput, MockOutputHandle, OutputError};
    use prism_domain::{OutputHealth, OutputId, UniverseId};
    use prism_engine::{FrameLayout, FramePublisher, ManualClock, SystemClock};
    use proptest::prelude::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    /// A publisher over universes 1 and 2, which is what the runner maps
    /// against.
    fn publisher() -> FramePublisher {
        let layout = FrameLayout::new([universe(1), universe(2)]).unwrap();
        FramePublisher::new(Arc::new(layout))
    }

    /// Ten milliseconds: short enough that a hundred-millisecond backoff is ten
    /// steps rather than a hundred, and it costs nothing because the clock is
    /// simulated.
    const CADENCE: Duration = Duration::from_millis(10);

    fn config() -> RunnerConfig {
        RunnerConfig {
            cadence: CADENCE,
            backoff: BackoffConfig::default(),
        }
    }

    /// A runner over a mock output carrying `universes`, on a clock that only
    /// moves when it is told to.
    fn runner(
        publisher: &mut FramePublisher,
        universes: &[u32],
    ) -> (
        OutputRunner<MockOutput, ManualClock>,
        MockOutputHandle,
        Arc<OutputStatus>,
    ) {
        let output = MockOutput::new(
            OutputId::new(1),
            universes.iter().copied().map(UniverseId::new),
        );
        let handle = output.handle();
        let runner = OutputRunner::new(output, publisher.subscribe(), ManualClock::new(), config());
        let status = Arc::clone(runner.status());
        (runner, handle, status)
    }

    /// Steps until the outcome is one of `wanted`, or gives up. Returns the
    /// outcome and the simulated time it happened at.
    fn step_until(
        runner: &mut OutputRunner<MockOutput, ManualClock>,
        wanted: &[StepOutcome],
        limit: usize,
    ) -> (StepOutcome, Duration) {
        for _ in 0..limit {
            let outcome = runner.step();
            if wanted.contains(&outcome) {
                return (outcome, runner.now());
            }
        }
        panic!("no {wanted:?} within {limit} steps");
    }

    #[test]
    fn the_backoff_climbs_from_a_hundred_milliseconds_to_five_seconds() {
        // ARCHITECTURE_SPEC.md §7 and the S7 plan: 100 ms → 5 s.
        let mut backoff = Backoff::new(BackoffConfig::default());
        let delays: Vec<Duration> = (0..8).map(|_| backoff.next_delay()).collect();
        assert_eq!(
            delays,
            vec![
                Duration::from_millis(100),
                Duration::from_millis(200),
                Duration::from_millis(400),
                Duration::from_millis(800),
                Duration::from_millis(1600),
                Duration::from_millis(3200),
                Duration::from_millis(5000),
                Duration::from_millis(5000),
            ]
        );
    }

    #[test]
    fn a_connection_that_comes_back_resets_the_backoff() {
        // Otherwise a cable that is unplugged twice in an evening is five
        // seconds slow to come back the second time, for no reason.
        let mut backoff = Backoff::new(BackoffConfig::default());
        for _ in 0..4 {
            backoff.next_delay();
        }
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn a_backoff_can_be_configured_and_still_has_a_ceiling() {
        let mut backoff = Backoff::new(BackoffConfig {
            initial: Duration::from_millis(50),
            max: Duration::from_millis(120),
            factor: 3,
        });
        assert_eq!(backoff.next_delay(), Duration::from_millis(50));
        assert_eq!(backoff.next_delay(), Duration::from_millis(120));
        assert_eq!(backoff.next_delay(), Duration::from_millis(120));
    }

    proptest! {
        #[test]
        fn a_backoff_never_exceeds_its_ceiling_however_long_the_fault_lasts(
            failures in 0usize..200,
        ) {
            let config = BackoffConfig::default();
            let mut backoff = Backoff::new(config);
            for _ in 0..failures {
                let delay = backoff.next_delay();
                prop_assert!(delay >= config.initial);
                prop_assert!(delay <= config.max);
            }
        }
    }

    #[test]
    fn a_status_nobody_has_reported_on_is_disconnected() {
        let status = OutputStatus::new();
        assert_eq!(status.health(), OutputHealth::Disconnected);
        assert_eq!(status.frames_sent(), 0);
        assert_eq!(status.panics(), 0);
        assert_eq!(status.errors(), 0);
        assert_eq!(status.connections(), 0);
        assert!(!status.stop_requested());
    }

    #[test]
    fn every_health_survives_the_trip_through_the_shared_status() {
        // The runner writes this from its own thread and the UI reads it from
        // another, so it is one byte rather than a lock — and a byte that came
        // back as the wrong state would be a green light over a dead universe.
        let status = OutputStatus::new();
        for health in [
            OutputHealth::Ok,
            OutputHealth::Degraded,
            OutputHealth::Disconnected,
        ] {
            status.set_health(health);
            assert_eq!(status.health(), health);
        }
    }

    #[test]
    fn the_runner_connects_before_it_sends_anything() {
        let mut publisher = publisher();
        let (mut runner, handle, status) = runner(&mut publisher, &[1]);
        assert_eq!(status.health(), OutputHealth::Disconnected);

        assert_eq!(runner.step(), StepOutcome::Connected);
        assert_eq!(handle.connect_attempts(), 1);
        assert_eq!(handle.frame_count(), 0);
        assert_eq!(status.health(), OutputHealth::Ok);
        assert_eq!(status.connections(), 1);

        assert_eq!(runner.step(), StepOutcome::Sent);
        assert_eq!(handle.frame_count(), 1);
        assert_eq!(status.frames_sent(), 1);
    }

    #[test]
    fn the_runner_sends_every_universe_the_output_carries() {
        let mut publisher = publisher();
        let (mut runner, handle, _) = runner(&mut publisher, &[1, 2]);
        publisher.frame_mut().universe_mut(0).unwrap().fill(11);
        publisher.frame_mut().universe_mut(1).unwrap().fill(22);
        publisher.publish();

        assert_eq!(runner.step(), StepOutcome::Connected);
        assert_eq!(runner.step(), StepOutcome::Sent);

        let frames = handle.frames();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], (universe(1), vec![11u8; 512]));
        assert_eq!(frames[1], (universe(2), vec![22u8; 512]));
    }

    #[test]
    fn a_universe_the_engine_does_not_carry_is_reported_and_not_sent() {
        // An output configured for a universe that is not in the patch is a
        // configuration mistake. Dropping it silently is how a rig ends up dark
        // with every light in the UI green.
        let mut publisher = publisher();
        let (mut runner, handle, _) = runner(&mut publisher, &[2, 9]);
        assert_eq!(runner.unmapped(), [universe(9)]);
        assert_eq!(runner.step(), StepOutcome::Connected);
        assert_eq!(runner.step(), StepOutcome::Sent);
        let frames = handle.frames();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, universe(2));
    }

    #[test]
    fn the_runner_keeps_driving_the_line_when_the_engine_publishes_nothing_new() {
        // A DMX receiver that stops being refreshed times out. The engine
        // publishing nothing new means "hold this look", not "stop sending".
        let mut publisher = publisher();
        let (mut runner, handle, _) = runner(&mut publisher, &[1]);
        publisher.frame_mut().universe_mut(0).unwrap().fill(5);
        publisher.publish();

        assert_eq!(runner.step(), StepOutcome::Connected);
        for _ in 0..4 {
            assert_eq!(runner.step(), StepOutcome::Sent);
        }
        assert_eq!(handle.frame_count(), 4);
        for (_, channels) in handle.frames() {
            assert_eq!(channels, vec![5u8; 512]);
        }
    }

    #[test]
    fn the_runner_wakes_at_its_own_cadence_and_not_the_engines() {
        // Decision log, S2: a driver thread that polls next to the engine costs
        // the engine its deadline. This one wakes at the interval its hardware
        // can actually sustain.
        let mut publisher = publisher();
        let (mut runner, _, _) = runner(&mut publisher, &[1]);
        // The first step happens at once — an output should not be dark for a
        // cadence at start-up — and every one after it is a cadence later.
        for step in 0..5u32 {
            runner.step();
            assert_eq!(runner.now(), CADENCE * step);
        }
    }

    #[test]
    fn the_cadence_of_an_open_dmx_output_comes_from_its_profile() {
        // ~30–40 Hz, which is what the adapter can do (ARCHITECTURE_SPEC.md
        // §7.1) — not the 44 Hz the engine runs at.
        let config = RunnerConfig::for_profile(&SH_RS09B);
        assert_eq!(config.cadence, SH_RS09B.timing.min_frame_interval);
        assert!(config.cadence > Duration::from_micros(22_727));
        assert_eq!(config.backoff, BackoffConfig::default());
    }

    #[test]
    fn a_disconnect_is_retried_on_the_backoff_and_the_engine_never_notices() {
        let mut publisher = publisher();
        let (mut runner, handle, status) = runner(&mut publisher, &[1]);
        publisher.frame_mut().universe_mut(0).unwrap().fill(1);
        publisher.publish();
        assert_eq!(runner.step(), StepOutcome::Connected);
        assert_eq!(runner.step(), StepOutcome::Sent);

        // The cable is pulled, and every reconnection for the next while fails.
        handle.fail_send(1, OutputError::Disconnected);
        handle.fail_connect(3, OutputError::Disconnected);
        assert_eq!(runner.step(), StepOutcome::Disconnected);
        let lost_at = runner.now();
        assert_eq!(status.health(), OutputHealth::Disconnected);

        // The retries land on 100 ms, 200 ms, 400 ms after the loss — and the
        // runner waits rather than hammering the port every cadence.
        for expected in [100u64, 300, 700] {
            let (_, at) = step_until(&mut runner, &[StepOutcome::ConnectFailed], 200);
            assert_eq!(at - lost_at, Duration::from_millis(expected));
        }

        // Meanwhile the engine has gone on publishing, without ever waiting for
        // this driver: 500 frames into a triple buffer nobody was reading.
        for value in 0..500u32 {
            publisher
                .frame_mut()
                .universe_mut(0)
                .unwrap()
                .fill((value % 256) as u8);
            publisher.publish();
        }
        let published = publisher.frame().sequence();

        let (_, back_at) = step_until(&mut runner, &[StepOutcome::Connected], 2000);
        assert_eq!(back_at - lost_at, Duration::from_millis(1500));
        assert_eq!(status.health(), OutputHealth::Ok);
        assert_eq!(runner.step(), StepOutcome::Sent);

        // And what goes out is the *newest* frame, not the one it was holding
        // when the cable was pulled.
        let (_, channels) = handle.last_frame().unwrap();
        assert_eq!(channels, vec![(499 % 256) as u8; 512]);
        assert_eq!(published, 501);
        assert!(status.errors() >= 4);
    }

    #[test]
    fn a_reconnection_starts_the_backoff_from_the_bottom_again() {
        let mut publisher = publisher();
        let (mut runner, handle, _) = runner(&mut publisher, &[1]);
        handle.fail_connect(2, OutputError::Disconnected);
        step_until(&mut runner, &[StepOutcome::Connected], 200);

        // Second outage: if the backoff had not been reset the first retry
        // would be 400 ms rather than 100 ms.
        handle.fail_send(1, OutputError::Disconnected);
        handle.fail_connect(1, OutputError::Disconnected);
        step_until(&mut runner, &[StepOutcome::Disconnected], 10);
        let lost_at = runner.now();
        let (_, retried_at) = step_until(&mut runner, &[StepOutcome::ConnectFailed], 200);
        assert_eq!(retried_at - lost_at, Duration::from_millis(100));
    }

    #[test]
    fn a_frame_that_does_not_go_out_cleanly_degrades_the_output_without_dropping_it() {
        let mut publisher = publisher();
        let (mut runner, handle, status) = runner(&mut publisher, &[1]);
        assert_eq!(runner.step(), StepOutcome::Connected);
        handle.fail_send(1, OutputError::Faulted);
        assert_eq!(runner.step(), StepOutcome::Faulted);
        assert_eq!(status.health(), OutputHealth::Degraded);
        assert_eq!(status.errors(), 1);
        // No reconnection: the link is up, the next frame is simply sent.
        assert_eq!(runner.step(), StepOutcome::Sent);
        assert_eq!(status.health(), OutputHealth::Ok);
        assert_eq!(handle.connect_attempts(), 1);
    }

    #[test]
    fn a_panic_in_the_driver_is_contained_and_the_output_marked_degraded() {
        // CLAUDE.md's zero-crash invariant, at the output boundary: a fault in
        // a driver must cost frames, not the process.
        let mut publisher = publisher();
        let (mut runner, handle, status) = runner(&mut publisher, &[1]);
        assert_eq!(runner.step(), StepOutcome::Connected);
        handle.panic_on_send(1);

        assert_eq!(runner.step(), StepOutcome::Panicked);
        assert_eq!(status.panics(), 1);
        assert_eq!(status.health(), OutputHealth::Degraded);

        // The thread is still alive and the next frame goes out.
        assert_eq!(runner.step(), StepOutcome::Sent);
        assert_eq!(status.health(), OutputHealth::Ok);
        assert_eq!(status.panics(), 1);
    }

    #[test]
    fn a_panic_while_connecting_is_contained_too_and_backs_off() {
        let mut publisher = publisher();
        let (mut runner, handle, status) = runner(&mut publisher, &[1]);
        handle.panic_on_connect(1);
        assert_eq!(runner.step(), StepOutcome::Panicked);
        assert_eq!(status.panics(), 1);
        let panicked_at = runner.now();

        // Treated as a failed connection: retried on the backoff rather than
        // immediately, or a driver that panics every call would spin a core.
        let (_, back_at) = step_until(&mut runner, &[StepOutcome::Connected], 200);
        assert_eq!(back_at - panicked_at, Duration::from_millis(100));
    }

    #[test]
    fn the_default_cadence_is_one_engine_tick() {
        // As fast as there is ever any point sending: the engine publishes
        // nothing new in between. An adapter that is slower says so through
        // its own profile.
        let default = RunnerConfig::default();
        assert_eq!(default.cadence, prism_engine::TICK_PERIOD);
        assert_eq!(default.backoff, BackoffConfig::default());
        assert!(default.cadence < RunnerConfig::for_profile(&SH_RS09B).cadence);
    }

    #[test]
    fn a_driver_that_falls_over_while_shutting_down_does_not_take_the_thread_with_it() {
        let mut publisher = publisher();
        let (mut runner, handle, status) = runner(&mut publisher, &[1]);
        assert_eq!(runner.step(), StepOutcome::Connected);
        handle.panic_on_shutdown(1);
        status.request_stop();
        runner.run();
        assert_eq!(handle.shutdowns(), 1);
        assert_eq!(status.panics(), 1);
        assert_eq!(status.health(), OutputHealth::Disconnected);
    }

    #[test]
    fn a_stopped_runner_shuts_its_output_down() {
        let mut publisher = publisher();
        let (mut runner, handle, status) = runner(&mut publisher, &[1]);
        assert_eq!(runner.step(), StepOutcome::Connected);
        status.request_stop();
        assert_eq!(runner.step(), StepOutcome::Stopped);
        runner.run();
        assert_eq!(handle.shutdowns(), 1);
        assert_eq!(status.health(), OutputHealth::Disconnected);
    }

    #[test]
    fn an_open_dmx_driver_runs_on_a_thread_and_stops_cleanly() {
        let mut publisher = publisher();
        // Subscribe first and publish afterwards, which is the order the daemon
        // uses: `subscribe` allocates, so outputs are attached during set-up
        // and the tick starts once they are all there.
        let subscriber = publisher.subscribe();
        publisher.frame_mut().universe_mut(0).unwrap().fill(77);
        publisher.publish();
        let ftdi = MockFtdi::new();
        let cable: MockFtdiHandle = ftdi.handle();
        let driver = OpenDmxUsb::new(OutputId::new(1), universe(1), ftdi);
        let thread = super::spawn(
            "out-opendmx-1",
            driver,
            subscriber,
            RunnerConfig {
                cadence: Duration::from_millis(1),
                backoff: BackoffConfig::default(),
            },
        )
        .unwrap();

        wait_for(|| thread.status().frames_sent() >= 3);
        assert_eq!(thread.status().health(), OutputHealth::Ok);
        thread.stop();

        let writes = cable.writes();
        assert!(writes.len() >= 3);
        for packet in &writes {
            assert_eq!(packet.len(), 513);
            assert_eq!(packet[0], 0x00);
            assert_eq!(&packet[1..], &[77u8; 512][..]);
        }
        assert!(
            !cable.is_open(),
            "the thread closed the cable on the way out"
        );
    }

    #[test]
    fn a_driver_thread_outlives_a_cable_that_is_pulled_out() {
        let mut publisher = publisher();
        publisher.publish();
        let ftdi = MockFtdi::new();
        let cable = ftdi.handle();
        let driver = OpenDmxUsb::new(OutputId::new(1), universe(1), ftdi);
        let thread = super::spawn(
            "out-opendmx-1",
            driver,
            publisher.subscribe(),
            RunnerConfig {
                cadence: Duration::from_millis(1),
                backoff: BackoffConfig {
                    initial: Duration::from_millis(1),
                    max: Duration::from_millis(5),
                    factor: 2,
                },
            },
        )
        .unwrap();

        wait_for(|| thread.status().frames_sent() >= 1);
        cable.fail_write(1, FtdiError::Disconnected);
        wait_for(|| thread.status().errors() >= 1);
        // It comes back on its own, and the process is none the wiser.
        let sent = thread.status().frames_sent();
        wait_for(|| thread.status().frames_sent() > sent + 2);
        assert!(thread.status().connections() >= 2);
        thread.stop();
    }

    #[test]
    fn a_driver_thread_outlives_a_driver_that_panics() {
        let mut publisher = publisher();
        publisher.publish();
        let output = MockOutput::new(OutputId::new(1), [universe(1)]);
        let handle = output.handle();
        let thread = super::spawn(
            "out-mock-1",
            output,
            publisher.subscribe(),
            RunnerConfig {
                cadence: Duration::from_millis(1),
                backoff: BackoffConfig {
                    initial: Duration::from_millis(1),
                    max: Duration::from_millis(5),
                    factor: 2,
                },
            },
        )
        .unwrap();

        wait_for(|| thread.status().frames_sent() >= 1);
        handle.panic_on_send(1);
        wait_for(|| thread.status().panics() >= 1);
        let sent = thread.status().frames_sent();
        wait_for(|| thread.status().frames_sent() > sent + 2);
        thread.stop();
        assert_eq!(handle.shutdowns(), 1);
    }

    #[test]
    fn a_dropped_thread_handle_stops_the_thread() {
        // A driver thread that outlived its handle would keep a cable open
        // after the output was deleted from the show.
        let mut publisher = publisher();
        let output = MockOutput::new(OutputId::new(1), [universe(1)]);
        let handle = output.handle();
        let thread = super::spawn(
            "out-mock-1",
            output,
            publisher.subscribe(),
            RunnerConfig {
                cadence: Duration::from_millis(1),
                backoff: BackoffConfig::default(),
            },
        )
        .unwrap();
        wait_for(|| thread.status().frames_sent() >= 1);
        drop(thread);
        assert_eq!(handle.shutdowns(), 1);
    }

    #[test]
    fn a_runner_on_the_real_clock_paces_itself() {
        // No jitter measurement anywhere in this crate — only the floor, which
        // a busy machine cannot make fail: five frames at 10 ms cannot be done
        // in less than 40 ms.
        let mut publisher = publisher();
        let output = MockOutput::new(OutputId::new(1), [universe(1)]);
        let mut runner =
            OutputRunner::new(output, publisher.subscribe(), SystemClock::new(), config());
        let start = Instant::now();
        for _ in 0..5 {
            runner.step();
        }
        assert!(start.elapsed() >= CADENCE * 4);
    }

    /// Waits for a condition, with a timeout generous enough that a loaded
    /// two-core CI runner cannot fail it.
    fn wait_for(mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if done() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("the driver thread did not get there within ten seconds");
    }
}
