//! The tracker receiver thread, and the table it keeps - **S32**.
//!
//! `prism_protocols::TrackerReceiver` is the conversation (PSN in, mapped
//! positions out); this is where it runs, and where what it has heard is kept for
//! a client to ask about. It is [`crate::discovery`]'s shape one protocol along,
//! and for the same reasons.
//!
//! # Two tables, and why
//!
//! A tracker's position goes **twice**, to two readers who want different things.
//! The **`TrackerTable`** (`prism_engine`) is what the *tick* reads: one atomic
//! word per tracker, the latest position and nothing else, built to be read
//! without a lock by a thread that may not wait. The **rows** here are what a
//! *person* reads: names, ages, how many packets, behind a lock the settings
//! panel takes while it draws. Merging them would give the tick a lock or the
//! panel an atomic word, and each reader would be the worse for it.
//!
//! # When it runs, and when it does not
//!
//! Only when the machine says so (`TrackerSettings::enabled`), and never under
//! `--mock-devices`: a daemon that opens a multicast socket nobody asked for
//! surprises a firewall, and every test in this workspace that starts a daemon
//! must touch no network. [`Tracking::idle`] is the state they are all in.
//!
//! # A socket that will not open degrades alone
//!
//! The rest of the desk is untouched and the table says `listening: false` with
//! the reason in words - another program already holding the port without
//! sharing it, or an interface that is not on this machine. A show that follows
//! nothing does not notice; one that does has its heads **hold where they are**,
//! which is the specification's answer to a tracker that goes quiet.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use prism_domain::{TrackerHealth, TrackerSettings, Vec3};
use prism_engine::TrackerTable;
use prism_protocols::{Listen, TrackerReceiver, TrackingConfig, TrackingCounters};

use crate::discovery::{SocketSource, SystemSockets};
use crate::log;

/// One tracker as the daemon last saw it, in the form a client is answered in.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackerView {
    /// The tracker's number.
    pub id: u16,
    /// What the system calls it.
    pub name: Option<String>,
    /// Where it last was, in show space.
    pub position: Vec3,
    /// When it last spoke, on this machine's clock - `None` for a tracker that
    /// has been named and has never moved.
    pub last_seen: Option<Instant>,
}

/// What the receiver has heard.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackingView {
    /// Every tracker heard, lowest number first.
    pub trackers: Vec<TrackerView>,
    /// Whether the socket is open.
    pub listening: bool,
    /// Why not, in words, or `None`.
    pub error: Option<String>,
    /// What has been read and what was done with it.
    pub counters: TrackingCounters,
    /// How long a tracker may be quiet before it reads as such.
    pub timeout: Duration,
}

impl TrackingView {
    /// Whether a tracker is being heard, at `now`.
    ///
    /// A tracker that was named and has not moved is **quiet**, not live: nothing
    /// is being heard from it, and a head following it would be holding nothing.
    #[must_use]
    pub fn health(&self, tracker: &TrackerView, now: Instant) -> TrackerHealth {
        match tracker.last_seen {
            Some(at) if now.saturating_duration_since(at) <= self.timeout => TrackerHealth::Live,
            _ => TrackerHealth::Quiet,
        }
    }

    /// One tracker, by number.
    #[must_use]
    pub fn tracker(&self, id: u16) -> Option<&TrackerView> {
        self.trackers.iter().find(|tracker| tracker.id == id)
    }
}

/// State the thread writes and the daemon reads.
#[derive(Debug, Default)]
struct Shared {
    view: Mutex<TrackingView>,
    running: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The daemon's handle on the tracker receiver.
pub struct Tracking {
    shared: Arc<Shared>,
    source: Option<Arc<dyn SocketSource>>,
    table: Arc<TrackerTable>,
    /// Why this run is **not allowed** to listen, if it is not - the reason a
    /// panel shows instead of a receiver. Never cleared by a setting: a desk told
    /// to listen in a run that may not has not been given a socket by being told.
    disabled: Option<String>,
    /// What the thread is running with, so a setting that did not change does
    /// not restart it.
    running_with: Option<TrackerSettings>,
    thread: Option<JoinHandle<()>>,
}

impl core::fmt::Debug for Tracking {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Tracking")
            .field("running", &self.thread.is_some())
            .field("enabled", &self.source.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for Tracking {
    fn default() -> Self {
        Self::idle()
    }
}

impl Tracking {
    /// A receiver that will never open a socket.
    #[must_use]
    pub fn idle() -> Self {
        Self {
            shared: Arc::new(Shared::default()),
            source: None,
            table: Arc::new(TrackerTable::new()),
            disabled: None,
            running_with: None,
            thread: None,
        }
    }

    /// A receiver that will never open a socket, and **says why** - the
    /// difference from [`idle`](Self::idle) being [`crate::discovery::Discovery::disabled`]'s.
    #[must_use]
    pub fn disabled(reason: impl Into<String>) -> Self {
        let mut tracking = Self::idle();
        tracking.disabled = Some(reason.into());
        tracking
    }

    /// A receiver that will open a socket from `source` once it is switched on.
    #[must_use]
    pub fn with_source(source: Arc<dyn SocketSource>) -> Self {
        let mut tracking = Self::idle();
        tracking.source = Some(source);
        tracking
    }

    /// The real one.
    #[must_use]
    pub fn system() -> Self {
        Self::with_source(Arc::new(SystemSockets))
    }

    /// The table the tick reads. **One table for the daemon's whole life**: every
    /// merge body's follow layer holds a handle on it, so a rebuild or a restart
    /// of the receiver never leaves the tick looking at a table nobody writes.
    #[must_use]
    pub const fn table(&self) -> &Arc<TrackerTable> {
        &self.table
    }

    /// Whether a thread is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.thread.is_some()
    }

    /// What has been heard.
    #[must_use]
    pub fn view(&self) -> TrackingView {
        let mut view = lock(&self.shared.view).clone();
        if view.error.is_none() {
            view.error.clone_from(&self.disabled);
        }
        view
    }

    /// Makes the receiver match `settings`: starts it, stops it, or restarts it
    /// on a different address - and does nothing at all if nothing it reads
    /// changed.
    ///
    /// The group and the interface come out of the settings as text, which the
    /// machine applier has already checked; one that is not an address here is a
    /// hand-edited `machine.json`, and the receiver reports it and stays off.
    pub fn configure(&mut self, settings: &TrackerSettings) {
        if self.running_with.as_ref() == Some(settings) && self.thread.is_some() {
            return;
        }
        self.stop();
        if !settings.enabled {
            lock(&self.shared.view).error = None;
            return;
        }
        let Some(source) = self.source.clone() else {
            return;
        };
        let listen = match listen_from(settings) {
            Ok(listen) => listen,
            Err(reason) => {
                log::warn(
                    "tracking",
                    &format!("trackers are not being listened for: {reason}"),
                );
                let mut view = lock(&self.shared.view);
                view.listening = false;
                view.error = Some(reason);
                return;
            }
        };
        let mut config = TrackingConfig::new(
            listen,
            settings.mapping,
            Duration::from_millis(u64::from(settings.timeout_ms)),
        );
        config.wait = Duration::from_millis(40);
        // A new source is not the old one holding its last position for ever.
        self.table.clear();
        *lock(&self.shared.view) = TrackingView {
            timeout: config.timeout,
            ..TrackingView::default()
        };
        self.shared.running.store(true, Ordering::Release);
        let shared = Arc::clone(&self.shared);
        let table = Arc::clone(&self.table);
        match std::thread::Builder::new()
            .name("psn-in".to_owned())
            .spawn(move || run(&source, config, &table, &shared))
        {
            Ok(thread) => {
                self.thread = Some(thread);
                self.running_with = Some(settings.clone());
            }
            Err(error) => {
                self.shared.running.store(false, Ordering::Release);
                log::error(
                    "tracking",
                    &format!("the tracker thread could not be started: {error}"),
                );
                let mut view = lock(&self.shared.view);
                view.listening = false;
                view.error = Some(format!("the tracker thread could not be started: {error}"));
            }
        }
    }

    /// Stops the thread and gives the socket back. Safe on one that is not
    /// running.
    pub fn stop(&mut self) {
        self.shared.running.store(false, Ordering::Release);
        self.running_with = None;
        if let Some(thread) = self.thread.take() {
            // Joined, so the port is free before a restart tries to take it.
            let _ = thread.join();
            lock(&self.shared.view).listening = false;
            log::info("tracking", "tracker receiver stopped");
        }
    }
}

impl Drop for Tracking {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Where the settings say to listen.
fn listen_from(settings: &TrackerSettings) -> Result<Listen, String> {
    let group = settings
        .group_address()
        .ok_or_else(|| format!("\"{}\" is not an IPv4 address", settings.group))?;
    let interface = settings.interface_address().map_err(|_| {
        format!(
            "\"{}\" is not an IPv4 address",
            settings.interface.clone().unwrap_or_default()
        )
    })?;
    Ok(if group.is_multicast() {
        Listen::Multicast {
            group,
            port: settings.port,
            interface,
        }
    } else {
        // Not a group: an address to listen on, which is how a tracker that sends
        // to this machine alone is received - and the only way a test receives
        // one without putting a multicast datagram on the network.
        Listen::Unicast(SocketAddr::from((group, settings.port)))
    })
}

/// The thread body: open, then read until told to stop.
fn run(
    source: &Arc<dyn SocketSource>,
    config: TrackingConfig,
    table: &Arc<TrackerTable>,
    shared: &Arc<Shared>,
) {
    let listen = config.listen;
    let mut receiver = TrackerReceiver::new(source.open(), config, Arc::clone(table));
    if let Err(error) = receiver.open() {
        let reason = match listen {
            Listen::Multicast { group, port, .. } => {
                format!("could not join {group}:{port}: {error}")
            }
            Listen::Unicast(address) => format!("could not listen on {address}: {error}"),
        };
        log::warn(
            "tracking",
            &format!(
                "trackers are not being heard - {reason}. Heads that follow one will hold where they are"
            ),
        );
        let mut view = lock(&shared.view);
        view.listening = false;
        view.error = Some(reason);
        shared.running.store(false, Ordering::Release);
        return;
    }
    log::info(
        "tracking",
        &format!(
            "listening for trackers on {}",
            receiver
                .local_addr()
                .map_or_else(|| describe(listen), |address| address.to_string())
        ),
    );
    publish(&receiver, shared, true);

    let mut last_publish = Instant::now();
    let mut heard = 0_usize;
    let mut announced = false;
    while shared.running.load(Ordering::Acquire) {
        let taken = receiver.service();
        let counters = receiver.counters();
        // Said once, because it is the line an installer needs and one line is
        // all it needs: nothing is reaching the receiver that is a position.
        if !announced && counters.rejected >= 8 && counters.positions == 0 {
            announced = true;
            log::warn(
                "tracking",
                "datagrams are arriving on the tracker address that are not PSN version 2 - \
                 check the tracking system's protocol and version",
            );
        }
        if receiver.rows().len() != heard {
            heard = receiver.rows().len();
            if let Some(row) = receiver.rows().last() {
                log::info(
                    "tracking",
                    &format!(
                        "tracker {} heard{}",
                        row.id,
                        row.name
                            .as_deref()
                            .map_or_else(String::new, |name| format!(": \"{name}\""))
                    ),
                );
            }
        }
        // **When something arrived, and otherwise every hundred milliseconds.**
        // A pass that took a position publishes it, so the view never lags the
        // table the tick reads by more than a pass - which is what stops a
        // tracker that has just been heard being reported as never heard. The
        // ages still need refreshing while nothing arrives, which is the
        // second half of the condition; a panel redraws a few times a second at
        // most, so neither costs anything that matters.
        if taken > 0 || last_publish.elapsed() >= Duration::from_millis(100) {
            last_publish = Instant::now();
            publish(&receiver, shared, true);
        }
    }
    receiver.close();
    publish(&receiver, shared, false);
}

fn describe(listen: Listen) -> String {
    match listen {
        Listen::Multicast { group, port, .. } => format!("{group}:{port}"),
        Listen::Unicast(address) => address.to_string(),
    }
}

/// Copies what the receiver holds into the view a client is answered from.
fn publish<S: prism_protocols::UdpNode, C: prism_engine::Clock>(
    receiver: &TrackerReceiver<S, C>,
    shared: &Arc<Shared>,
    listening: bool,
) {
    // The receiver's clock is monotonic from its own start; the view carries
    // `Instant`s so that every age a client is told is measured at the moment the
    // question is answered.
    let now = Instant::now();
    let reference = receiver.now();
    let mut trackers: Vec<TrackerView> = receiver
        .rows()
        .iter()
        .map(|row| TrackerView {
            id: row.id,
            name: row.name.clone(),
            position: row.position,
            last_seen: (row.positions > 0).then(|| {
                now.checked_sub(reference.saturating_sub(row.last_seen))
                    .unwrap_or(now)
            }),
        })
        .collect();
    trackers.sort_by_key(|tracker| tracker.id);
    let mut view = lock(&shared.view);
    view.listening = listening;
    view.trackers = trackers;
    view.counters = receiver.counters();
    view.timeout = receiver.config().timeout;
    if listening {
        view.error = None;
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use std::time::Duration;

    use prism_domain::{TrackerHealth, TrackerSettings, Vec3};
    use prism_protocols::psn::{encode_data, encode_info};
    use prism_protocols::{MockUdpNode, MockUdpNodeHandle, UdpNode};

    use super::Tracking;
    use crate::discovery::SocketSource;

    /// A source that hands out one mock socket and keeps the handle.
    struct Mock {
        socket: std::sync::Mutex<Option<MockUdpNode>>,
    }

    impl SocketSource for Mock {
        fn open(&self) -> Box<dyn UdpNode> {
            let taken = self
                .socket
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            Box::new(taken.unwrap_or_default())
        }
    }

    fn mocked() -> (Tracking, MockUdpNodeHandle) {
        let socket = MockUdpNode::new();
        let handle = socket.handle();
        let source = Arc::new(Mock {
            socket: std::sync::Mutex::new(Some(socket)),
        });
        (Tracking::with_source(source), handle)
    }

    fn on() -> TrackerSettings {
        TrackerSettings {
            enabled: true,
            ..TrackerSettings::default()
        }
    }

    fn from() -> std::net::SocketAddr {
        std::net::SocketAddr::from(([10, 0, 0, 9], 56_565))
    }

    fn until(tracking: &Tracking, check: impl Fn(&super::TrackingView) -> bool) -> bool {
        for _ in 0..400 {
            if check(&tracking.view()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn an_idle_receiver_opens_nothing_whatever_it_is_told() {
        let mut tracking = Tracking::idle();
        tracking.configure(&on());
        assert!(!tracking.is_running());
        assert!(!tracking.view().listening);
    }

    #[test]
    fn a_receiver_that_is_switched_off_does_not_start_and_one_switched_on_does() {
        let (mut tracking, socket) = mocked();
        tracking.configure(&TrackerSettings::default());
        assert!(!tracking.is_running());
        assert!(socket.joins().is_empty());
        tracking.configure(&on());
        assert!(until(&tracking, |view| view.listening));
        assert_eq!(
            socket.joins(),
            vec![(Ipv4Addr::new(236, 10, 10, 10), 56_565, None)]
        );
        tracking.configure(&TrackerSettings::default());
        assert!(!tracking.is_running());
        assert!(!tracking.view().listening);
    }

    #[test]
    fn a_position_reaches_both_the_table_the_tick_reads_and_the_rows_a_person_reads() {
        let (mut tracking, socket) = mocked();
        tracking.configure(&on());
        assert!(until(&tracking, |view| view.listening));
        socket.deliver(from(), &encode_info(0, 0, "Sim", &[(3, "Anna")]));
        socket.deliver(from(), &encode_data(0, 0, &[(3, [1.0, 2.0, 3.0])]));
        assert!(until(&tracking, |view| view
            .tracker(3)
            .is_some_and(|row| row.last_seen.is_some())));
        // The default mapping swaps y and z.
        let at = Vec3 {
            x: 1.0,
            y: 3.0,
            z: 2.0,
        };
        assert_eq!(tracking.table().read(3), Some(at));
        let view = tracking.view();
        let row = view.tracker(3).unwrap();
        assert_eq!((row.name.as_deref(), row.position), (Some("Anna"), at));
        assert_eq!(
            view.health(row, std::time::Instant::now()),
            TrackerHealth::Live
        );
    }

    #[test]
    fn a_tracker_that_stops_reads_quiet_and_keeps_its_position() {
        let (mut tracking, socket) = mocked();
        tracking.configure(&TrackerSettings {
            timeout_ms: 100,
            ..on()
        });
        assert!(until(&tracking, |view| view.listening));
        socket.deliver(from(), &encode_data(0, 0, &[(1, [1.0, 1.0, 1.0])]));
        assert!(until(&tracking, |view| view.tracker(1).is_some()));
        std::thread::sleep(Duration::from_millis(250));
        let view = tracking.view();
        let row = view.tracker(1).unwrap();
        assert_eq!(
            view.health(row, std::time::Instant::now()),
            TrackerHealth::Quiet
        );
        assert!(
            tracking.table().read(1).is_some(),
            "the last position is held"
        );
    }

    #[test]
    fn a_name_alone_is_a_tracker_that_has_never_spoken() {
        let (mut tracking, socket) = mocked();
        tracking.configure(&on());
        assert!(until(&tracking, |view| view.listening));
        socket.deliver(from(), &encode_info(0, 0, "Sim", &[(9, "Ben")]));
        assert!(until(&tracking, |view| view.tracker(9).is_some()));
        let view = tracking.view();
        let row = view.tracker(9).unwrap();
        assert_eq!(row.last_seen, None);
        assert_eq!(
            view.health(row, std::time::Instant::now()),
            TrackerHealth::Quiet
        );
    }

    #[test]
    fn changing_a_setting_restarts_the_receiver_and_forgets_the_old_one() {
        let (mut tracking, socket) = mocked();
        tracking.configure(&on());
        assert!(until(&tracking, |view| view.listening));
        socket.deliver(from(), &encode_data(0, 0, &[(2, [1.0, 1.0, 1.0])]));
        assert!(until(&tracking, |view| view.tracker(2).is_some()));
        // The same settings again are not a restart.
        tracking.configure(&on());
        assert!(tracking.view().tracker(2).is_some());
        tracking.configure(&TrackerSettings {
            port: 6_000,
            ..on()
        });
        assert_eq!(
            tracking.table().read(2),
            None,
            "a new source starts from nothing"
        );
        assert!(tracking.view().tracker(2).is_none());
    }

    #[test]
    fn a_socket_that_will_not_open_says_why_and_leaves_no_thread_pretending() {
        let (mut tracking, socket) = mocked();
        socket.fail_bind(1, prism_protocols::UdpError::Bind);
        tracking.configure(&on());
        assert!(until(&tracking, |view| view.error.is_some()));
        let view = tracking.view();
        assert!(!view.listening);
        assert!(view.error.unwrap().contains("236.10.10.10:56565"));
    }

    #[test]
    fn an_address_that_is_not_an_address_is_said_so_without_a_thread() {
        let (mut tracking, _) = mocked();
        tracking.configure(&TrackerSettings {
            group: "not-an-address".to_owned(),
            ..on()
        });
        assert!(!tracking.is_running());
        assert!(tracking.view().error.unwrap().contains("not-an-address"));
    }

    #[test]
    fn an_address_that_is_not_a_group_is_listened_on_directly() {
        let (mut tracking, socket) = mocked();
        tracking.configure(&TrackerSettings {
            group: "127.0.0.1".to_owned(),
            port: 0,
            ..on()
        });
        assert!(until(&tracking, |view| view.listening));
        assert_eq!(
            socket.binds(),
            vec![(std::net::SocketAddr::from(([127, 0, 0, 1], 0)), false)]
        );
        assert!(socket.joins().is_empty());
    }

    /// **Told to listen, in a run that may not.** `--mock-devices` touches no
    /// network, so the receiver stays shut and *says why* - and the reason must
    /// survive being configured, which is what a panel opening does on the way
    /// in.
    #[test]
    fn a_run_that_may_not_listen_says_why_whatever_it_is_told() {
        let mut tracking = Tracking::disabled("off for this run");
        tracking.configure(&on());
        tracking.configure(&TrackerSettings::default());
        assert!(!tracking.is_running());
        assert_eq!(tracking.view().error.as_deref(), Some("off for this run"));
    }

    #[test]
    fn stopping_one_that_never_started_is_not_an_error() {
        let mut tracking = Tracking::idle();
        tracking.stop();
        tracking.stop();
    }
}
