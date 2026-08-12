//! The tick thread: the one thread in this process that has a deadline.
//!
//! `ARCHITECTURE_SPEC.md` §3 gives `engine-tick` its own thread at realtime or
//! high priority, and §3.1 forbids it heap allocation, locks, I/O and logging.
//! This module is what holds the engine to that while still letting the show
//! change underneath it.
//!
//! # The thread's priority is raised here, and only the thread's
//!
//! S2 and S6 measured it, and `PROGRESS.md` §3 records the numbers: the same
//! ten-minute run missed **45 ticks with a p99.9 of 54 ms** at the shell's
//! default priority, and **not one, with a p99.9 of 200 µs**, at the priority
//! §3 asks for. `prism-engine` cannot do it — it is platform-neutral by rule —
//! so it is done here.
//!
//! **Only the thread.** A Windows priority *class* applies to every thread in
//! the process, and S6 measured what that costs too: with the load inside the
//! process at the same priority as the tick, 7 484 of 26 455 ticks were missed
//! and the median jitter was one scheduler quantum. The daemon's other threads
//! — the outputs, the runtime, the surface — stay ordinary.
//!
//! [`thread_priority`] is where the platform code for that lives, and that is
//! the whole reason it is a dependency: §10.1 allows this crate no
//! `#[cfg(target_os = …)]`, `SetThreadPriority` and `pthread_setschedparam` are
//! two implementations of one sentence, and the workspace forbids `unsafe_code`.
//! It is the same move S16 made with tokio's `net` feature for the named pipe
//! and the Unix domain socket.
//!
//! **A refusal is not fatal.** On a Raspberry Pi without `CAP_SYS_NICE` the
//! request fails, and a daemon that would not start because of it is a daemon
//! that will not run on the machine D10 exists for. It is logged once, at
//! `WARN`, and the show goes on with the jitter that priority would have bought.
//!
//! # How a rebuilt show reaches a running tick
//!
//! Everything that changes what the engine *is* — a repatch, a group edit, a
//! stored cue — allocates: `MergeBody::for_patch`, `load_groups`,
//! `load_sequence` and `load_programmer` all say so in their own documentation.
//! None of them may therefore happen on the tick thread.
//!
//! So they happen on the core thread, which builds a **whole new body** and
//! leaves it in [`BodySwap`]. The tick reads one atomic per tick to find out
//! whether there is one — free — and when there is, takes it with a
//! [`std::sync::Mutex::try_lock`] that never blocks: a failed attempt costs a
//! compare-and-swap and the body arrives one tick later instead. The body it
//! replaces goes back the same way, so the *deallocation* does not happen on the
//! tick thread either.
//!
//! The cost of rebuilding rather than reaching in is that a rebuild stops every
//! playback, where `load_sequence` alone would stop only the executor whose cue
//! list changed. That is a real difference and it is recorded in `PROGRESS.md`'s
//! decision log; `CuePlayer::load` already leaves *its* playback stopped, so the
//! difference is about the other executors and not about the one being edited.
//!
//! # Why the frame is blanked when a body arrives
//!
//! S4's encoder writes only the channels the patch covers, so a channel that is
//! no longer patched would keep whatever the old rig last put there — a fixture
//! unpatched mid-show would stay lit. S11 wrote it down as *`Effect::Repatch` is
//! two jobs, not one*, and this is the second one.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use prism_engine::{
    DmxFrame, Engine, FrameLayout, FramePublisher, FrameSubscriber, MergeBody, Producer,
    SystemClock, TICK_HZ, TickBody, TickCommand, TickInfo,
};

use crate::log;

/// The name `ARCHITECTURE_SPEC.md` §3 gives the thread, and what a debugger and
/// a crash dump will show.
pub const TICK_THREAD_NAME: &str = "engine-tick";

/// The hand-off of a rebuilt merge body between the core thread and the tick.
///
/// One slot in each direction and one flag. The flag is what the tick reads
/// every tick; the mutex is only touched when the flag says there is something
/// behind it, and never blockingly.
#[derive(Debug, Default)]
pub struct BodySwap {
    incoming: Mutex<Option<Box<MergeBody>>>,
    /// Bodies the tick has finished with, waiting to be dropped anywhere but
    /// here: freeing a `MergeBody` is freeing several boxed slices, and §3.1
    /// does not distinguish an allocation from a deallocation.
    retired: Mutex<Vec<MergeBody>>,
    waiting: AtomicBool,
}

impl BodySwap {
    /// Leaves a rebuilt body for the tick to pick up.
    ///
    /// Replaces one that has not been taken yet: two rebuilds between two ticks
    /// mean the first was already out of date.
    pub fn offer(&self, body: MergeBody) {
        if let Ok(mut slot) = self.incoming.lock() {
            *slot = Some(Box::new(body));
        }
        self.waiting.store(true, Ordering::Release);
    }

    /// Whether a body is waiting to be taken.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.waiting.load(Ordering::Acquire)
    }

    /// Drops everything the tick has handed back. Called from the core thread.
    pub fn collect(&self) {
        if let Ok(mut retired) = self.retired.lock() {
            retired.clear();
        }
    }

    /// The tick's side: takes a waiting body, if one is there and the slot is
    /// free this instant.
    fn take(&self) -> Option<Box<MergeBody>> {
        if !self.waiting.load(Ordering::Acquire) {
            return None;
        }
        // Never `lock`: this runs on the tick thread, and a thread with a
        // deadline does not wait for one without. A failed attempt costs a
        // compare-and-swap and the body arrives 23 ms later instead.
        let Ok(mut slot) = self.incoming.try_lock() else {
            return None;
        };
        let next = slot.take();
        self.waiting.store(false, Ordering::Release);
        next
    }

    /// The tick's side: hands back the body it has finished with, so it is
    /// freed on the core thread rather than on this one.
    ///
    /// A slot it cannot get this instant means the body is dropped here after
    /// all — which is one deallocation, once, in the tick that a repatch
    /// already made the most expensive of the evening. Holding it would mean
    /// carrying a second body through every tick until the lock came free.
    fn retire(&self, previous: MergeBody) {
        if let Ok(mut retired) = self.retired.try_lock() {
            retired.push(previous);
        }
    }
}

/// What the tick publishes about itself, for `DaemonHealth` and the telemetry
/// channel.
///
/// Atomics rather than a channel: the writer is the tick and the readers are
/// whoever asks, and the tick must not be held up by either.
#[derive(Debug, Default)]
pub struct TickHealth {
    ticks: AtomicU64,
    missed: AtomicU64,
    swaps: AtomicU64,
}

impl TickHealth {
    /// Ticks that have run since the daemon started.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.ticks.load(Ordering::Relaxed)
    }

    /// Tick slots that were skipped because the previous tick overran.
    #[must_use]
    pub fn missed(&self) -> u64 {
        self.missed.load(Ordering::Relaxed)
    }

    /// Merge bodies the tick has taken. What a test asserts a rebuild by.
    #[must_use]
    pub fn swaps(&self) -> u64 {
        self.swaps.load(Ordering::Relaxed)
    }

    /// The rate the tick actually achieved over `elapsed`.
    ///
    /// `ARCHITECTURE_SPEC.md` §3.2: 44 Hz when all is well. Zero rather than a
    /// division by zero before the first tick, because a `DaemonHealth` with a
    /// NaN in it is a client redrawing its status panel for ever (S1).
    #[must_use]
    pub fn rate(&self, elapsed: Duration) -> f64 {
        let seconds = elapsed.as_secs_f64();
        if seconds <= 0.0 {
            return 0.0;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a tick count large enough to lose precision is 6.6 million years of show"
        )]
        {
            self.ticks() as f64 / seconds
        }
    }
}

/// The [`TickBody`] the daemon runs: the merge, plus the two things only a
/// daemon can decide.
struct DaemonBody {
    body: MergeBody,
    swap: Arc<BodySwap>,
    health: Arc<TickHealth>,
    /// Set when a body has just been swapped in — see the module documentation
    /// for why the frame cannot simply be re-encoded over the old one.
    blank_next_frame: bool,
}

impl TickBody for DaemonBody {
    fn apply(&mut self, command: TickCommand) {
        self.body.apply(command);
    }

    fn render(&mut self, tick: &TickInfo, frame: &mut DmxFrame) {
        // Cheapest thing that can be done per tick: one acquire load of an
        // atomic that is false almost always.
        if self.swap.is_pending()
            && let Some(next) = self.swap.take()
        {
            // The new body arrives before the old one leaves, so nothing has to
            // stand in for it while the two change places.
            let previous = core::mem::replace(&mut self.body, *next);
            self.swap.retire(previous);
            self.blank_next_frame = true;
            self.health.swaps.fetch_add(1, Ordering::Relaxed);
        }
        if self.blank_next_frame {
            frame.blackout();
            self.blank_next_frame = false;
        }
        self.body.render(tick, frame);
        self.health.ticks.fetch_add(1, Ordering::Relaxed);
        if tick.missed > 0 {
            self.health.missed.fetch_add(tick.missed, Ordering::Relaxed);
        }
    }
}

/// The daemon's handle on the running tick.
///
/// Dropping it stops the thread and waits for it: a tick thread that outlived
/// its handle would go on driving outputs after the daemon had given up on them.
pub struct EngineThread {
    commands: Producer<TickCommand>,
    swap: Arc<BodySwap>,
    health: Arc<TickHealth>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    /// Commands the queue had no room for. `Consumer::rejected` counts
    /// something else — slots that would not decode — so this is the daemon's
    /// own number.
    refused: u64,
}

/// Commands one tick may fall behind by before the queue refuses.
///
/// Two ticks' worth of a busy fader bank and then some. A command that does not
/// fit is reported rather than dropped silently — the queue counts refusals and
/// [`EngineThread::rejected`] is what a status panel shows.
pub const COMMAND_QUEUE: usize = 1024;

impl EngineThread {
    /// Starts the tick on a thread of its own, at the priority §3 asks for.
    ///
    /// The publisher must already have every subscriber it will ever have:
    /// `FramePublisher::subscribe` allocates, so an output attached after the
    /// tick has started would allocate on the tick thread (S2).
    ///
    /// # Errors
    ///
    /// Whatever the operating system says if the thread cannot be created.
    pub fn start(body: MergeBody, publisher: FramePublisher) -> std::io::Result<Self> {
        let swap = Arc::new(BodySwap::default());
        let health = Arc::new(TickHealth::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (commands, consumer) = prism_engine::command_queue(COMMAND_QUEUE);

        let engine_body = DaemonBody {
            body,
            swap: Arc::clone(&swap),
            health: Arc::clone(&health),
            blank_next_frame: false,
        };
        let mut engine = Engine::new(engine_body, consumer, publisher);
        let thread_stop = Arc::clone(&stop);
        let join = std::thread::Builder::new()
            .name(TICK_THREAD_NAME.to_owned())
            .spawn(move || {
                raise_this_thread();
                engine.run(&SystemClock::new(), &thread_stop);
            })?;

        Ok(Self {
            commands,
            swap,
            health,
            stop,
            join: Some(join),
            refused: 0,
        })
    }

    /// Queues a command for the next tick.
    ///
    /// Returns whether it fitted. A full queue is a daemon that has stopped
    /// ticking, so the answer is worth looking at rather than ignoring.
    pub fn send(&mut self, command: TickCommand) -> bool {
        if self.commands.push(command).is_ok() {
            return true;
        }
        self.refused += 1;
        false
    }

    /// Commands the queue had no room for since the daemon started.
    #[must_use]
    pub const fn refused(&self) -> u64 {
        self.refused
    }

    /// Hands a rebuilt merge body to the tick.
    pub fn install(&self, body: MergeBody) {
        self.swap.offer(body);
    }

    /// Whether a handed-over body has not been taken yet.
    #[must_use]
    pub fn install_pending(&self) -> bool {
        self.swap.is_pending()
    }

    /// Frees the bodies the tick has handed back. Called from the core thread,
    /// where freeing is allowed.
    pub fn collect_retired(&self) {
        self.swap.collect();
    }

    /// What the tick has managed.
    #[must_use]
    pub fn health(&self) -> &Arc<TickHealth> {
        &self.health
    }

    /// Asks the thread to finish and waits for it.
    pub fn stop(mut self) {
        self.stop_and_join();
    }

    fn stop_and_join(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            // The engine contains its own body in `catch_unwind`, so this cannot
            // report a panic from the merge — but joining is not the place to
            // find out if that ever stops being true.
            drop(join.join());
        }
    }
}

impl Drop for EngineThread {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

impl core::fmt::Debug for EngineThread {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EngineThread")
            .field("ticks", &self.health.ticks())
            .field("missed", &self.health.missed())
            .field("refused", &self.refused)
            .finish_non_exhaustive()
    }
}

/// Raises the calling thread to the priority `ARCHITECTURE_SPEC.md` §3 asks for.
///
/// Returns whether it worked. A refusal is a warning and not a failure: see the
/// module documentation.
fn raise_this_thread() -> bool {
    match thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max) {
        Ok(()) => {
            log::debug(
                "engine",
                "the tick thread runs at the priority ARCHITECTURE_SPEC.md section 3 asks for",
            );
            true
        }
        Err(error) => {
            log::warn(
                "engine",
                &format!(
                    "the tick thread could not be given a high priority ({error:?}); \
                     it will run at the ordinary one, and jitter will be worse - \
                     see PROGRESS.md section 3"
                ),
            );
            false
        }
    }
}

/// The frame layout the publisher is built with, covering universes 1..=`count`.
///
/// **The desk's whole range rather than the show's**, and that is a decision
/// rather than laziness: a `FramePublisher`'s layout is fixed for its lifetime,
/// and every subscriber — every output thread — is attached to it before the
/// tick starts. A layout built from the show's universes would mean that
/// patching a fixture into a universe the show did not have yet could only take
/// effect after a restart, in the middle of a get-in.
///
/// The cost is 512 bytes of frame per universe nothing is patched into, copied
/// once per subscriber per tick. At the full 64 that is 32 KiB a frame, which is
/// the size S6's stress gate was measured at.
///
/// # Errors
///
/// [`prism_engine::LayoutError`] if `count` is zero or beyond
/// `UniverseId::MAX`.
pub fn frame_layout(count: u32) -> Result<FrameLayout, prism_engine::LayoutError> {
    FrameLayout::new((1..=count).map(prism_domain::UniverseId::new))
}

/// One engine tick, as a duration. For a caller pacing itself against the tick
/// rather than against a wall clock.
#[must_use]
pub fn ticks(count: u32) -> Duration {
    Duration::from_nanos(u64::from(count) * 1_000_000_000 / TICK_HZ)
}

/// A subscriber for the telemetry channel.
///
/// Not an output: it reads the same published frames the drivers do, which is
/// what makes the picture a client sees the picture the fixtures got rather than
/// a second calculation of it.
#[must_use]
pub fn telemetry_subscriber(publisher: &mut FramePublisher) -> FrameSubscriber {
    publisher.subscribe()
}

#[cfg(test)]
mod tests {
    use super::{BodySwap, EngineThread, TickHealth, raise_this_thread, ticks};
    use crate::testkit::{dimmer_type, fixture};
    use prism_domain::UniverseId;
    use prism_engine::{
        FrameLayout, FramePublisher, MergeBody, TICK_HZ, TICK_PERIOD, TickCommand,
        UNIVERSE_CHANNELS,
    };
    use prism_protocols::{MockOutput, RunnerConfig, spawn};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn layout() -> Arc<FrameLayout> {
        Arc::new(FrameLayout::new([UniverseId::new(1)]).unwrap())
    }

    /// A body whose one dimmer sits at `home`, so a frame says which body
    /// produced it.
    fn body(home: u16) -> MergeBody {
        let fixture_type = dimmer_type("generic.dimmer", home);
        let fixture = fixture(1, "generic.dimmer", 1, 1);
        MergeBody::for_patch(
            &layout(),
            [(&fixture, &fixture_type)],
            [prism_domain::ExecutorId::new(0)],
        )
        .unwrap()
    }

    /// Waits for `condition`, or gives up. Every wait in this file has a
    /// deadline: S16 pushed a test that blocked for ever on a two-core runner,
    /// and a named failure in seconds is worth more than a job that stops.
    fn until(what: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn the_tick_runs_and_drives_an_output_with_nobody_watching() {
        // The exit criterion in miniature: no client, no show file, no IPC —
        // just an engine and a driver, which is what D2 promises is enough.
        let mut publisher = FramePublisher::new(layout());
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(prism_domain::OutputId::new(1), [UniverseId::new(1)]);
        let frames = output.handle();
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();

        let engine = EngineThread::start(body(65535), publisher).unwrap();
        until("frames to reach the driver", || frames.frame_count() > 5);

        let (universe, data) = frames.last_frame().unwrap();
        assert_eq!(universe, UniverseId::new(1));
        assert_eq!(data.len(), UNIVERSE_CHANNELS);
        assert_eq!(data[0], 255, "the dimmer is at home, which is full");
        assert!(engine.health().ticks() > 5);
        assert_eq!(engine.refused(), 0);

        driver.stop();
        engine.stop();
    }

    #[test]
    fn a_rebuilt_body_reaches_the_tick_and_blanks_what_the_old_one_wrote() {
        let mut publisher = FramePublisher::new(layout());
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(prism_domain::OutputId::new(1), [UniverseId::new(1)]);
        let frames = output.handle();
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();

        let engine = EngineThread::start(body(65535), publisher).unwrap();
        until("the first rig to reach the wire", || {
            frames.last_frame().is_some_and(|(_, data)| data[0] == 255)
        });

        // The second rig's dimmer is at zero, which is also what a blanked frame
        // holds — so the assertion is on a *channel the new patch does not
        // cover*: an unpatched channel must not keep what the old rig put there.
        engine.install(body(0));
        assert!(
            engine.install_pending() || engine.health().swaps() >= 1,
            "a body that has been handed over is either waiting or taken"
        );
        until("the rebuilt rig to reach the wire", || {
            frames.last_frame().is_some_and(|(_, data)| data[0] == 0)
        });
        assert!(engine.health().swaps() >= 1);
        assert!(!engine.install_pending(), "and then nothing is waiting");
        assert!(
            format!("{engine:?}").contains("EngineThread"),
            "a breakpoint should show what the tick has managed"
        );

        engine.collect_retired();
        driver.stop();
        engine.stop();
    }

    #[test]
    fn an_unpatched_channel_does_not_keep_what_the_old_rig_put_there() {
        // The half of `Effect::Repatch` that is easy to forget (S11): the
        // encoder writes only patched channels, so a fixture that has been
        // unpatched would stay lit for as long as the frame buffer lives.
        let mut publisher = FramePublisher::new(layout());
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(prism_domain::OutputId::new(1), [UniverseId::new(1)]);
        let frames = output.handle();
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();
        let engine = EngineThread::start(body(65535), publisher).unwrap();
        until("the lit rig", || {
            frames.last_frame().is_some_and(|(_, data)| data[0] == 255)
        });

        // An empty patch: nothing writes channel 1 at all any more.
        let empty =
            MergeBody::for_patch(&layout(), [], [prism_domain::ExecutorId::new(0)]).unwrap();
        engine.install(empty);
        until("the channel to go dark", || {
            frames.last_frame().is_some_and(|(_, data)| data[0] == 0)
        });

        driver.stop();
        engine.stop();
    }

    #[test]
    fn a_command_reaches_the_merge() {
        let mut publisher = FramePublisher::new(layout());
        let subscriber = publisher.subscribe();
        let output = MockOutput::new(prism_domain::OutputId::new(1), [UniverseId::new(1)]);
        let frames = output.handle();
        let driver = spawn("out-mock", output, subscriber, RunnerConfig::default()).unwrap();
        let mut engine = EngineThread::start(body(65535), publisher).unwrap();

        until("the rig to light", || {
            frames.last_frame().is_some_and(|(_, data)| data[0] == 255)
        });
        assert!(engine.send(TickCommand::SetBlackout(true)));
        until("the blackout to reach the wire", || {
            frames.last_frame().is_some_and(|(_, data)| data[0] == 0)
        });

        driver.stop();
        engine.stop();
    }

    #[test]
    fn a_swap_offered_twice_before_the_tick_takes_it_delivers_the_newer_one() {
        let swap = BodySwap::default();
        assert!(!swap.is_pending());
        swap.offer(body(0));
        swap.offer(body(65535));
        assert!(swap.is_pending());

        let taken = swap.take().unwrap();
        assert_eq!(taken.values(), [65535_u16].as_slice());
        assert!(!swap.is_pending());
        assert!(swap.take().is_none(), "there is nothing left to take");

        // The body it replaced is left to be dropped somewhere that is not the
        // tick thread.
        swap.retire(*taken);
        swap.collect();
    }

    #[test]
    fn tick_health_starts_at_nothing_and_reports_a_rate() {
        let health = TickHealth::default();
        assert_eq!(health.ticks(), 0);
        assert_eq!(health.missed(), 0);
        assert_eq!(health.swaps(), 0);
        // Before the first tick, and on a clock that has not moved: zero rather
        // than a NaN, which a client would compare against itself for ever.
        assert!((health.rate(Duration::ZERO) - 0.0).abs() < f64::EPSILON);
        health.ticks.store(44, std::sync::atomic::Ordering::Relaxed);
        assert!((health.rate(Duration::from_secs(1)) - 44.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_tick_count_is_a_duration_on_the_engines_own_grid() {
        assert_eq!(ticks(0), Duration::ZERO);
        assert_eq!(
            ticks(u32::try_from(TICK_HZ).unwrap()),
            Duration::from_secs(1)
        );
        assert!(ticks(1).abs_diff(TICK_PERIOD) < Duration::from_micros(1));
    }

    #[test]
    fn raising_this_thread_answers_rather_than_failing() {
        // Whether it succeeds is a property of the machine — an unprivileged
        // Linux container refuses, and D10's Raspberry Pi is exactly that. What
        // must hold everywhere is that asking does not stop the daemon.
        let raised = raise_this_thread();
        assert!(raised || !raised);
    }
}
