//! The daemon's own tracker receiver on the real network - **S32**, `#[ignore]`d.
//!
//! `prism-protocols`' `psn_multicast` listens with the bare socket and measures
//! the *network*; this runs the receiver the desk actually uses (the thread, the
//! table, the view a panel is answered from) and samples what a panel would show,
//! ten times a second, so a tracker that flaps between *live* and *quiet* shows up
//! as a count and a time. It **only listens** - the sender is a program you start
//! yourself:
//!
//! ```text
//! cargo test -p prismd --test tracking_real -- --ignored --nocapture
//! ```
//!
//! `PSN_SECONDS` (default 20) is how long, `PSN_INTERFACE` names a card.

#![allow(
    clippy::print_stdout,
    reason = "a manual diagnostic has to print what it sees"
)]

use std::time::{Duration, Instant};

use prism_domain::{TrackerHealth, TrackerSettings};
use prismd::tracking::Tracking;

#[test]
#[ignore = "listens on the real network; start a sender first"]
fn the_receiver_the_desk_uses_holds_a_live_tracker() {
    let seconds: u64 = std::env::var("PSN_SECONDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(20);
    let settings = TrackerSettings {
        enabled: true,
        interface: std::env::var("PSN_INTERFACE").ok(),
        ..TrackerSettings::default()
    };
    let mut tracking = Tracking::system();
    tracking.configure(&settings);

    let start = Instant::now();
    let mut state: Option<TrackerHealth> = None;
    let mut flips = 0_u32;
    let mut samples = 0_u32;
    let mut quiet_samples = 0_u32;
    while start.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_millis(100));
        let view = tracking.view();
        let now = Instant::now();
        let Some(row) = view.trackers.iter().find(|row| row.last_seen.is_some()) else {
            continue;
        };
        let health = view.health(row, now);
        samples += 1;
        if health == TrackerHealth::Quiet {
            quiet_samples += 1;
        }
        if state != Some(health) {
            if state.is_some() {
                flips += 1;
            }
            println!("{:?}: tracker {} is {health:?}", start.elapsed(), row.id);
            state = Some(health);
        }
    }
    let view = tracking.view();
    println!(
        "listening {}, error {:?}, joined on {:?}, last from {:?}",
        view.listening, view.error, view.interfaces, view.last_from
    );
    println!(
        "{:?}; {samples} samples, {quiet_samples} quiet, {flips} changes of state",
        view.counters
    );
    tracking.configure(&TrackerSettings::default());
    assert!(view.counters.datagrams > 0, "nothing arrived");
    assert_eq!(flips, 0, "the tracker flapped between live and quiet");
}
