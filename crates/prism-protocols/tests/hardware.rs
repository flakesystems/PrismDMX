//! The bring-up target: the only code in this repository that needs a cable.
//!
//! `CLAUDE.md` does not allow a test to require hardware, so every test here is
//! `#[ignore]`d and has to be asked for by name. That is not a way of hiding
//! them — it is what makes the rest of the suite runnable on a build server,
//! and `PROGRESS.md` §3.1 carries the commands so that anybody with the adapter
//! can reproduce every number S8 recorded.
//!
//! What each test needs:
//!
//! | Test | Needs |
//! |---|---|
//! | `the_adapter_on_this_machine_is_the_one_the_profile_describes` | the adapter |
//! | `a_fixture_follows_a_value_ramp` | the adapter and a fixture on the line |
//! | `the_sustained_frame_rate_*` | the adapter |
//! | `unplugging_the_cable_reconnects_without_restarting_the_process` | the adapter and a pair of hands |
//!
//! Channels are chosen with `PRISM_DMX_CHANNELS`: a single channel (`5`), a
//! range (`1-16`), `all` for every channel of the universe, or `sweep` to walk
//! one channel at a time — which is how one finds the dimmer of a fixture whose
//! patch nobody remembers.

// A measurement nobody can read is not a measurement. This target exists to
// print numbers at a person, which is the case the workspace lint has in mind
// when it says "crates that legitimately write to a terminal opt out".
#![allow(clippy::print_stdout)]

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use prism_domain::{OutputHealth, OutputId, UniverseId};
use prism_protocols::{
    DmxOutput, FtdiBackend, OpenDmxUsb, OutputError, SH_RS09B, list_devices, system_backend,
};

/// The universe the bring-up drives. One adapter carries exactly one.
const UNIVERSE: UniverseId = UniverseId::new(1);

/// There is one cable, and `cargo test` runs the tests of a target on several
/// threads.
///
/// Held for the whole of every test that touches the adapter, so they take
/// turns instead of racing to open the same device — four of them failed
/// inside a third of a second the first time this target was run without it.
/// `prism-engine` learned the same lesson in S6 for the same reason, and the
/// remedy is the same: a mutex, not a note in a README telling people to pass
/// `--test-threads=1`.
static ADAPTER: Mutex<()> = Mutex::new(());

/// Takes the adapter, whether or not a previous test panicked while holding it.
fn adapter() -> MutexGuard<'static, ()> {
    ADAPTER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Which channels the ramp moves, read from `PRISM_DMX_CHANNELS`.
enum Channels {
    /// One ramp applied to every channel in the range at once.
    Range(u16, u16),
    /// One channel at a time, so an unknown fixture can be found.
    Sweep(u16, u16),
}

impl Channels {
    /// Reads the environment, defaulting to channel 1.
    fn from_env() -> Self {
        let spec = std::env::var("PRISM_DMX_CHANNELS").unwrap_or_else(|_| "1".to_owned());
        match spec.as_str() {
            "all" => Self::Range(1, 512),
            "sweep" => Self::Sweep(1, 32),
            other => match other.split_once('-') {
                Some((from, to)) => Self::Range(parse(from, 1), parse(to, 512)),
                None => {
                    let single = parse(other, 1);
                    Self::Range(single, single)
                }
            },
        }
    }

    /// The successive sets of channels one run of the ramp moves.
    fn steps(&self) -> Vec<(u16, u16)> {
        match *self {
            Self::Range(from, to) => vec![(from, to)],
            Self::Sweep(from, to) => (from..=to).map(|channel| (channel, channel)).collect(),
        }
    }
}

fn parse(value: &str, fallback: u16) -> u16 {
    value.trim().parse().unwrap_or(fallback).clamp(1, 512)
}

fn seconds_from_env(name: &str, fallback: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

/// The adapter profile, with the break timing overridable from the
/// environment.
///
/// A bring-up is exactly the situation in which the numbers in the profile are
/// a guess: `PRISM_DMX_BREAK_US` and `PRISM_DMX_MAB_US` make the two delays a
/// knob, so "does this fixture need a longer break?" is a question that can be
/// answered by turning it rather than argued about.
fn profile() -> prism_protocols::DeviceProfile {
    let mut profile = SH_RS09B;
    if let Some(micros) = micros_from_env("PRISM_DMX_BREAK_US") {
        profile.timing.break_time = Duration::from_micros(micros);
    }
    if let Some(micros) = micros_from_env("PRISM_DMX_MAB_US") {
        profile.timing.mark_after_break = Duration::from_micros(micros);
    }
    profile
}

fn micros_from_env(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

/// The cable to drive, from `PRISM_DMX_PATH`: `d2xx`, `vcp`, or the default
/// `system`, which is D2XX with the COM port behind it.
///
/// The choice exists because "D2XX or the virtual COM port?" is a question S8
/// was asked to answer *for this machine*, and an answer that rests on a rate
/// measurement alone is half an answer — a path that is quick and puts a
/// malformed frame on the line is not the faster one, it is the wrong one.
fn cable() -> Box<dyn FtdiBackend> {
    match std::env::var("PRISM_DMX_PATH").unwrap_or_default().as_str() {
        #[cfg(windows)]
        "d2xx" => Box::new(prism_protocols::D2xxBackend::new()),
        #[cfg(windows)]
        "vcp" => Box::new(prism_protocols::VcpBackend::new()),
        _ => system_backend(),
    }
}

/// A connected driver on the adapter the profile describes, over whichever
/// access path this machine has.
fn connected() -> OpenDmxUsb<Box<dyn FtdiBackend>> {
    let profile = profile();
    println!(
        "break {:?}, mark-after-break {:?}",
        profile.timing.break_time, profile.timing.mark_after_break
    );
    let mut driver = OpenDmxUsb::with_profile(OutputId::new(1), UNIVERSE, cable(), profile);
    driver
        .connect()
        .expect("no SH-RS09B could be opened - is the adapter plugged in?");
    assert_eq!(driver.health(), OutputHealth::Ok);
    driver
}

/// Sends one frame and fails loudly, since every caller here is a person
/// watching a light.
fn send(driver: &mut OpenDmxUsb<Box<dyn FtdiBackend>>, channels: &[u8; 512]) {
    match driver.send_frame(UNIVERSE, channels) {
        Ok(()) => {}
        Err(error) => panic!("the frame did not go out: {error}"),
    }
}

/// Holds `value` on `from..=to`, zero everywhere else, and then applies
/// whatever `PRISM_DMX_HOLD` pins on top.
///
/// The held channels are what make a real fixture testable. A wash light will
/// not show its red channel with its master at zero, and blind-ramping every
/// channel instead is worse than useless: cheap fixtures put a mode selector on
/// one of them, so a ramp walks the device into its own auto programme and out
/// of DMX control half way up. `PRISM_DMX_HOLD="6=255"` opens the master and
/// leaves the rest of the frame alone.
fn frame(from: u16, to: u16, value: u8) -> [u8; 512] {
    let mut channels = [0u8; 512];
    for channel in from..=to {
        if let Some(slot) = channels.get_mut(usize::from(channel - 1)) {
            *slot = value;
        }
    }
    for (channel, held) in held_channels() {
        if let Some(slot) = channels.get_mut(usize::from(channel - 1)) {
            *slot = held;
        }
    }
    channels
}

/// `PRISM_DMX_HOLD="6=255,10=128"` as pairs.
fn held_channels() -> Vec<(u16, u8)> {
    let Ok(spec) = std::env::var("PRISM_DMX_HOLD") else {
        return Vec::new();
    };
    spec.split(',')
        .filter_map(|pair| {
            let (channel, value) = pair.split_once('=')?;
            Some((parse(channel, 1), value.trim().parse().ok()?))
        })
        .collect()
}

#[test]
#[ignore = "needs the SH-RS09B attached"]
fn the_adapter_on_this_machine_is_the_one_the_profile_describes() {
    let _adapter = adapter();
    // The first question of a bring-up, and the one that replaces an
    // assumption: ARCHITECTURE_SPEC.md §7.1 records 0403:6001 as *typical* for
    // an FT232R, not as read off this cable.
    let found = list_devices().expect("the FTDI device list could not be read");
    println!("\n{} FTDI device(s) attached:", found.len());
    for device in &found {
        println!("  {device}");
    }

    let adapter = found
        .iter()
        .find(|device| device.matches(&SH_RS09B.device))
        .expect("no device matching the SH-RS09B profile is attached");
    println!("\nmatched the profile: {adapter}");
    println!("  vendor  0x{:04x}", adapter.vendor_id);
    println!("  product 0x{:04x}", adapter.product_id);
    println!("  serial  {:?}", adapter.serial);
    println!("  string  {:?}", adapter.product);
    assert_eq!(adapter.vendor_id, SH_RS09B.device.vendor_id);
    assert_eq!(adapter.product_id, SH_RS09B.device.product_id);
}

#[test]
#[ignore = "needs the SH-RS09B and a fixture on the line"]
fn a_fixture_follows_a_value_ramp() {
    let _adapter = adapter();
    // The exit criterion no software can check: somebody has to watch the
    // light. What this asserts is that every frame of the ramp reached the
    // port; what it prints is what the watcher should have seen.
    let channels = Channels::from_env();
    let hold = Duration::from_millis(seconds_from_env("PRISM_DMX_STEP_MS", 2_000));
    let mut driver = connected();

    for (from, to) in channels.steps() {
        println!("\nramping channel(s) {from}..={to} - 0 to 255 and back");
        let started = Instant::now();
        let mut frames = 0u32;
        while started.elapsed() < hold {
            // A triangle: up over the first half of the hold, down over the
            // second, so a watcher sees a fade rather than a flash.
            let phase = started.elapsed().as_secs_f32() / hold.as_secs_f32();
            let level = if phase < 0.5 {
                phase * 2.0
            } else {
                (1.0 - phase) * 2.0
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let value = (level.clamp(0.0, 1.0) * 255.0) as u8;
            send(&mut driver, &frame(from, to, value));
            frames += 1;
        }
        println!("  {frames} frames sent");
    }

    // Left dark, so a fixture is not abandoned at full.
    send(&mut driver, &[0u8; 512]);
    driver.shutdown();
    println!("\nblacked out and closed.");
}

#[test]
#[ignore = "needs the SH-RS09B and a fixture on the line"]
fn a_fixture_holds_a_static_level() {
    let _adapter = adapter();
    // The simplest stimulus there is, and the one to reach for when a ramp
    // produces nothing: one value, held, for long enough that a receiver has
    // seen hundreds of identical frames. If this does not light a fixture, the
    // problem is not the values - it is the frame around them.
    let channels = Channels::from_env();
    let level = std::env::var("PRISM_DMX_LEVEL")
        .ok()
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(255);
    let hold = Duration::from_millis(seconds_from_env("PRISM_DMX_STEP_MS", 8_000));
    let mut driver = connected();

    for (from, to) in channels.steps() {
        println!("holding channel(s) {from}..={to} at {level} for {hold:?}");
        let started = Instant::now();
        let mut frames = 0u32;
        while started.elapsed() < hold {
            send(&mut driver, &frame(from, to, level));
            frames += 1;
        }
        #[allow(clippy::cast_precision_loss)]
        let rate = f64::from(frames) / started.elapsed().as_secs_f64();
        println!("  {frames} frames at {rate:.1} Hz");
    }

    send(&mut driver, &[0u8; 512]);
    driver.shutdown();
    println!("blacked out and closed.");
}

/// Measures completed frames per second over `seconds`, on an already
/// connected driver.
fn measure_rate(driver: &mut OpenDmxUsb<Box<dyn FtdiBackend>>, seconds: u64) -> f64 {
    let duration = Duration::from_secs(seconds);
    // A short warm-up first: the driver's transmit queue takes a few frames to
    // reach steady state, and until it has, a write returns before its bytes
    // are on the wire. Counting those would measure the buffer.
    for _ in 0..10 {
        send(driver, &[0u8; 512]);
    }

    let started = Instant::now();
    let mut frames = 0u64;
    while started.elapsed() < duration {
        send(driver, &[0u8; 512]);
        frames += 1;
    }
    let elapsed = started.elapsed().as_secs_f64();
    let rate = frames as f64 / elapsed;
    println!("  {frames} frames of 513 bytes in {elapsed:.2} s = {rate:.2} Hz");
    rate
}

#[test]
#[ignore = "needs the SH-RS09B attached"]
fn the_sustained_frame_rate_over_sixty_seconds() {
    let _adapter = adapter();
    // ARCHITECTURE_SPEC.md §7.1 estimates 30-40 Hz. This is the number that
    // replaces the estimate, and it is measured on completed 513-byte writes -
    // the backends do not return until the bytes have left the port, so this
    // counts the wire and not a queue.
    let seconds = seconds_from_env("PRISM_DMX_SECONDS", 60);
    let mut driver = connected();
    println!("\nmeasuring the sustained frame rate over {seconds} s");
    let rate = measure_rate(&mut driver, seconds);
    driver.shutdown();

    // Pure data time for 513 bytes at 250 000 baud with 8N2 is 22.6 ms, so
    // nothing on this hardware can exceed 44 Hz, and a figure above it would
    // mean the measurement is counting a buffer rather than the line.
    assert!(rate > 0.0);
    assert!(rate < 45.0, "{rate} Hz is faster than the wire allows");
}

#[cfg(windows)]
#[test]
#[ignore = "needs the SH-RS09B attached"]
fn the_sustained_frame_rate_on_each_access_path() {
    let _adapter = adapter();
    // The other half of the S8 question: D2XX or the virtual COM port. The
    // difference is not academic - the FTDI latency timer is reachable from
    // one of them and not from the other.
    use prism_protocols::{D2xxBackend, VcpBackend};

    let seconds = seconds_from_env("PRISM_DMX_SECONDS", 60);

    println!("\nD2XX:");
    let mut d2xx: OpenDmxUsb<Box<dyn FtdiBackend>> =
        OpenDmxUsb::new(OutputId::new(1), UNIVERSE, Box::new(D2xxBackend::new()));
    d2xx.connect()
        .expect("the D2XX path could not open the cable");
    let d2xx_rate = measure_rate(&mut d2xx, seconds);
    d2xx.shutdown();

    println!("\nvirtual COM port:");
    let mut vcp: OpenDmxUsb<Box<dyn FtdiBackend>> =
        OpenDmxUsb::new(OutputId::new(1), UNIVERSE, Box::new(VcpBackend::new()));
    vcp.connect()
        .expect("the VCP path could not open the cable");
    let vcp_rate = measure_rate(&mut vcp, seconds);
    vcp.shutdown();

    println!("\nD2XX {d2xx_rate:.2} Hz against virtual COM port {vcp_rate:.2} Hz");
    assert!(d2xx_rate > 0.0 && vcp_rate > 0.0);
}

#[cfg(windows)]
#[test]
#[ignore = "needs the SH-RS09B attached"]
fn where_the_time_in_a_frame_actually_goes() {
    let _adapter = adapter();
    // The diagnostic that settles what a fixture cannot tell us: of the ~23 ms
    // a frame takes, how much is FT_Write, how much is waiting for the device's
    // transmit queue to drain, and how much are the two break transfers?
    //
    // It matters because the break is a USB *control* transfer: it does not
    // queue behind bulk data. If the queue is not empty when the break is
    // asserted, the break lands inside the frame that is still going out, and
    // every frame after it is misaligned - which looks, to a fixture, exactly
    // like noise.
    use libftd2xx::{BitsPerWord, Ftdi, FtdiCommon, Parity, StopBits};

    let mut device = Ftdi::with_serial_number(SH_RS09B.device.serial.unwrap_or("B0037HIY"))
        .or_else(|_| Ftdi::new())
        .expect("no FTDI device could be opened");
    device.set_baud_rate(250_000).unwrap();
    device
        .set_data_characteristics(BitsPerWord::Bits8, StopBits::Bits2, Parity::No)
        .unwrap();
    device.set_flow_control_none().unwrap();
    device.set_latency_timer(Duration::from_millis(1)).unwrap();
    device.set_usb_parameters(4096).unwrap();
    device
        .set_timeouts(Duration::from_millis(500), Duration::from_millis(500))
        .unwrap();
    device.purge_all().unwrap();

    let packet = [0u8; 513];
    println!("\n frame |  break |  write | queue after write |  drain | polls |  total");
    for frame in 0..12 {
        let started = Instant::now();
        device.set_break_on().unwrap();
        std::thread::sleep(Duration::from_micros(110));
        device.set_break_off().unwrap();
        let after_break = started.elapsed();

        let write_started = Instant::now();
        device.write_all(&packet).unwrap();
        let write = write_started.elapsed();
        let queued = device.status().unwrap().ammount_in_tx_queue;

        let drain_started = Instant::now();
        let mut polls = 0u32;
        while device.status().unwrap().ammount_in_tx_queue != 0 {
            polls += 1;
            std::thread::sleep(Duration::from_millis(1));
        }
        let drain = drain_started.elapsed();
        println!(
            "  {frame:4} | {after_break:6.2?} | {write:6.2?} | {queued:17} | {drain:6.2?} | {polls:5} | {:6.2?}",
            started.elapsed()
        );
    }
    let _ = device.close();
}

#[test]
#[ignore = "needs the SH-RS09B and somebody to pull the cable out"]
fn unplugging_the_cable_reconnects_without_restarting_the_process() {
    let _adapter = adapter();
    // ARCHITECTURE_SPEC.md §7.1, "failure mode in the field": USB unplugged or
    // the machine suspended. S7 asserted this against a mock; this is the same
    // claim against a cable somebody actually pulls.
    if std::env::var("PRISM_DMX_INTERACTIVE").is_err() {
        println!("set PRISM_DMX_INTERACTIVE=1 to run this - it needs a pair of hands");
        return;
    }
    let mut driver = connected();
    println!("\nsending. PULL THE CABLE OUT.");

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut lost = None;
    while Instant::now() < deadline {
        match driver.send_frame(UNIVERSE, &[0u8; 512]) {
            Ok(()) => {}
            Err(OutputError::Disconnected) => {
                lost = Some(Instant::now());
                break;
            }
            Err(error) => panic!("unexpected while waiting for the unplug: {error}"),
        }
    }
    let lost = lost.expect("the cable was never unplugged");
    assert_eq!(driver.health(), OutputHealth::Disconnected);
    println!("lost the cable, as expected. PLUG IT BACK IN.");

    // The runner's backoff is asserted in the unit tests; what is being
    // checked here is that a real cable can be opened again by the same
    // process, with no restart and no leaked handle.
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if driver.connect().is_ok() {
            let away = lost.elapsed();
            println!("reconnected after {away:.1?} without restarting");
            send(&mut driver, &[0u8; 512]);
            assert_eq!(driver.health(), OutputHealth::Ok);
            driver.shutdown();
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("the cable never came back");
}
