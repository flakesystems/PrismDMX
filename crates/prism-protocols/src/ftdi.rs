//! The seam between the Open DMX driver and a real FTDI cable.
//!
//! `ARCHITECTURE_SPEC.md` §7.1: the SH-RS09B is an FT232R with no
//! microcontroller, so there is no widget firmware to generate break and
//! mark-after-break and the **host** owns DMX timing. Everything that differs
//! per platform — D2XX on Windows, libftdi on Linux — lives behind
//! [`FtdiBackend`]; everything that is DMX512 rather than USB lives above it, in
//! [`OpenDmxUsb`](crate::OpenDmxUsb), and is therefore testable on any machine
//! with no cable attached.
//!
//! # Why the wait is a backend call
//!
//! The break sequence is `SetBreakOn` → ~110 µs → `SetBreakOff` → ~16 µs →
//! write. The delays are not incidental to it: a break that is not held long
//! enough is not a break, and a receiver will not resynchronise. A driver that
//! slept behind the backend's back would be untestable in exactly the place the
//! specification is most specific, so [`FtdiBackend::wait`] is part of the
//! trait, with the real spin as its default implementation and the mock
//! recording it into the same call log as everything else. The call *sequence*
//! is then one thing to assert rather than two interleaved ones.

use core::fmt;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::device::DeviceDescriptor;

/// Stop bits on the serial line. DMX512 is two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    /// One stop bit — the usual serial default, and wrong for DMX.
    One,
    /// Two stop bits, as DMX512 requires.
    Two,
}

/// Parity on the serial line. DMX512 has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    /// No parity bit.
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

/// Hardware or software flow control. DMX512 has neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    /// No flow control: the line is driven continuously and never paused.
    None,
    /// RTS/CTS hardware handshaking.
    RtsCts,
    /// XON/XOFF software handshaking.
    XonXoff,
}

/// How the port is set up before a single byte goes out.
///
/// Every field is stated rather than left to the driver's default, because two
/// of the defaults are actively harmful: a serial port opens at one stop bit,
/// and an FTDI device opens with a **16 ms** latency timer. At 44 Hz a frame is
/// 22.7 ms, so a 16 ms latency timer alone would put the achievable rate into
/// the twenties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortConfig {
    /// Bit rate. DMX512 is 250 000 baud exactly.
    pub baud: u32,
    /// Data bits per slot. DMX512 is 8.
    pub data_bits: u8,
    /// Stop bits per slot. DMX512 is [`StopBits::Two`].
    pub stop_bits: StopBits,
    /// Parity. DMX512 has [`Parity::None`].
    pub parity: Parity,
    /// Flow control. DMX512 has [`FlowControl::None`].
    pub flow_control: FlowControl,
    /// FTDI latency timer in milliseconds. The 16 ms default would be fatal.
    pub latency_timer_ms: u8,
    /// USB read transfer size in bytes. Read back is not used, but the value is
    /// set rather than inherited.
    pub read_transfer_size: u32,
    /// USB write transfer size in bytes. A whole 513-byte packet has to fit
    /// inside one transfer, or the break timing is broken by the USB stack
    /// rather than by us.
    pub write_transfer_size: u32,
}

impl PortConfig {
    /// The DMX512 port: 250 000 baud, 8N2, no flow control, latency timer 1.
    ///
    /// `ARCHITECTURE_SPEC.md` §7.1, row "Port setup", verbatim.
    pub const DMX512: Self = Self {
        baud: 250_000,
        data_bits: 8,
        stop_bits: StopBits::Two,
        parity: Parity::None,
        flow_control: FlowControl::None,
        latency_timer_ms: 1,
        read_transfer_size: 4096,
        write_transfer_size: 4096,
    };
}

/// Why an operation on the cable failed.
///
/// Split by what the driver has to *do* about it rather than by which USB call
/// returned it: [`is_link_lost`](Self::is_link_lost) separates the errors that
/// mean "reconnect" from the ones that mean "this frame did not go out cleanly,
/// try the next one".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FtdiError {
    /// No device matching the descriptor is attached.
    NotFound,
    /// The device was there and is not any more: unplugged, reset, or the
    /// machine suspended. `ARCHITECTURE_SPEC.md` §7.1 calls this the expected
    /// failure in the field, not an exception.
    Disconnected,
    /// The device is present and refused the operation.
    Io,
    /// The port could not be configured. Refusing to send is the only safe
    /// answer: a port at the wrong baud rate puts noise on a live DMX line.
    Config,
    /// This platform has no backend for FTDI hardware yet.
    ///
    /// `ARCHITECTURE_SPEC.md` §7.1 names libftdi as the Linux path, and S8
    /// verified the Windows one against a real cable. Until a machine exists to
    /// verify the Linux one on, asking for a cable there is answered rather
    /// than pretended: the crate builds and its logic is tested everywhere, and
    /// only the last inch is missing.
    Unsupported,
    /// A write returned having moved fewer bytes than it was given, which on a
    /// DMX line is a truncated frame rather than a partial success.
    ShortWrite {
        /// Bytes the device accepted.
        wrote: usize,
        /// Bytes the frame actually needs.
        expected: usize,
    },
}

impl FtdiError {
    /// Whether the cable has gone, so the driver should close down and
    /// reconnect with backoff rather than send another frame at it.
    #[must_use]
    pub const fn is_link_lost(self) -> bool {
        matches!(self, Self::NotFound | Self::Disconnected)
    }
}

impl fmt::Display for FtdiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "no FTDI device matching the descriptor is attached"),
            Self::Disconnected => write!(f, "the FTDI device is no longer attached"),
            Self::Io => write!(f, "the FTDI device refused the operation"),
            Self::Config => write!(f, "the FTDI port could not be configured"),
            Self::Unsupported => write!(f, "this platform has no FTDI backend"),
            Self::ShortWrite { wrote, expected } => {
                write!(f, "the FTDI device took {wrote} of {expected} bytes")
            }
        }
    }
}

impl std::error::Error for FtdiError {}

/// One FTDI cable, as the Open DMX driver needs to use it.
///
/// Deliberately small. Everything a DMX frame needs is here and nothing else is:
/// no read-back, no RDM, no enumeration. `ARCHITECTURE_SPEC.md` §7.1 lists
/// transmit-only as a property of this hardware, so it is a property of this
/// trait too, and a backend cannot be asked for something the cable cannot do.
pub trait FtdiBackend: Send {
    /// Opens the first device matching the descriptor.
    ///
    /// # Errors
    ///
    /// [`FtdiError::NotFound`] if nothing matches, [`FtdiError::Io`] if the
    /// device is there and will not open.
    fn open(&mut self, device: &DeviceDescriptor) -> Result<(), FtdiError>;

    /// Applies the port parameters. Called once per connection, before any
    /// frame goes out.
    ///
    /// # Errors
    ///
    /// [`FtdiError::Config`] if the device will not take the settings.
    fn configure(&mut self, port: &PortConfig) -> Result<(), FtdiError>;

    /// Drives the line to the break state, or releases it.
    ///
    /// # Errors
    ///
    /// [`FtdiError::Disconnected`] if the cable has gone, [`FtdiError::Io`]
    /// otherwise.
    fn set_break(&mut self, on: bool) -> Result<(), FtdiError>;

    /// Writes bytes to the line and answers how many it moved.
    ///
    /// **Must not return before the bytes have left the port.** The next thing
    /// the driver does is assert a break, and a break is a USB control
    /// transfer: it does not queue behind bulk data, so one asserted while the
    /// frame is still going out lands *inside* it. Every implementation
    /// therefore either waits for the hardware or waits out
    /// [`transmission_time`] — see `d2xx`, where measurement showed the
    /// hardware's own answer to be no answer at all.
    ///
    /// # Errors
    ///
    /// [`FtdiError::Disconnected`] if the cable has gone, [`FtdiError::Io`]
    /// otherwise.
    fn write(&mut self, data: &[u8]) -> Result<usize, FtdiError>;

    /// Discards whatever is queued in the device's buffers.
    ///
    /// Done on connect and after a fault: a reconnect that resumed halfway
    /// through the previous frame would put a truncated packet on the wire.
    ///
    /// # Errors
    ///
    /// [`FtdiError::Disconnected`] if the cable has gone, [`FtdiError::Io`]
    /// otherwise.
    fn purge(&mut self) -> Result<(), FtdiError>;

    /// Closes the device. Infallible on purpose — it runs on the error path and
    /// on shutdown, and there is nothing useful to do about a failure in
    /// either place.
    fn close(&mut self);

    /// Holds the line in its current state for `duration`.
    ///
    /// Part of the trait because the break and the mark-after-break are timed
    /// on the wire, so the delay is as much a step of the frame as the two
    /// `set_break` calls around it — see this module's documentation. The
    /// default is what a real cable wants; a test backend overrides it to
    /// record the wait instead of taking it.
    fn wait(&mut self, duration: Duration) {
        spin_wait(duration);
    }
}

/// A boxed backend is a backend.
///
/// What this buys is a driver whose cable is chosen at run time:
/// `OpenDmxUsb<Box<dyn FtdiBackend>>` can hold D2XX on one machine and the
/// virtual COM port on the next, which is exactly the fallback
/// `ARCHITECTURE_SPEC.md` §7.1 asks for. Without it the choice would have to be
/// made at compile time, which is the one place it cannot be made.
impl<B: FtdiBackend + ?Sized> FtdiBackend for Box<B> {
    fn open(&mut self, device: &DeviceDescriptor) -> Result<(), FtdiError> {
        (**self).open(device)
    }

    fn configure(&mut self, port: &PortConfig) -> Result<(), FtdiError> {
        (**self).configure(port)
    }

    fn set_break(&mut self, on: bool) -> Result<(), FtdiError> {
        (**self).set_break(on)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, FtdiError> {
        (**self).write(data)
    }

    fn purge(&mut self) -> Result<(), FtdiError> {
        (**self).purge()
    }

    fn close(&mut self) {
        (**self).close();
    }

    fn wait(&mut self, duration: Duration) {
        (**self).wait(duration);
    }
}

/// Bits on the wire per byte at DMX512's framing: one start bit, eight data
/// bits, two stop bits.
pub const BITS_PER_SLOT: u32 = 11;

/// How long `bytes` take to leave a port at `baud`, exactly.
///
/// This is the number a backend needs and cannot get from the operating
/// system. A write call returns when the *driver* has accepted the bytes,
/// which on Windows measured a good two milliseconds before the last of them
/// had left the port — and the next thing an Open DMX driver does is assert a
/// break, which is a USB control transfer and does not queue behind bulk data.
/// A break asserted early lands inside the frame still going out.
///
/// So the wire time is computed rather than asked for: 513 bytes at 250 000
/// baud is 22.572 ms, and no amount of buffering changes that.
#[must_use]
pub fn transmission_time(bytes: usize, baud: u32) -> Duration {
    let bits = u64::try_from(bytes)
        .unwrap_or(u64::MAX)
        .saturating_mul(u64::from(BITS_PER_SLOT));
    // `checked_div` rather than `/`: a port that has been opened but not
    // configured yet has no baud rate, and dividing by zero on a driver thread
    // is not a way to find that out.
    let nanos = bits
        .saturating_mul(1_000_000_000)
        .checked_div(u64::from(baud))
        .unwrap_or(0);
    Duration::from_nanos(nanos)
}

/// Waits without giving the thread up for anything under a millisecond.
///
/// `thread::sleep` is the obvious implementation and it is not usable here: the
/// Windows scheduler's default granularity is between 1 ms and 15.6 ms, so
/// sleeping for the 110 µs break would cost most of a frame period and cap the
/// output somewhere in the teens of hertz. The break and the mark-after-break
/// are 126 µs together, which is a spin. Anything longer than a millisecond —
/// DMX512 permits a break of up to a second — sleeps for all but the last
/// millisecond first, so a slow break does not burn a core.
pub fn spin_wait(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    let start = Instant::now();
    if let Some(coarse) = duration.checked_sub(Duration::from_millis(1)) {
        std::thread::sleep(coarse);
    }
    while start.elapsed() < duration {
        core::hint::spin_loop();
    }
}

/// One recorded call to a [`MockFtdi`].
///
/// The break sequence is an *order*, so what a test asserts is a list of these
/// and not a set of counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FtdiCall {
    /// [`FtdiBackend::open`] with the descriptor it was given.
    Open(DeviceDescriptor),
    /// [`FtdiBackend::configure`] with the port parameters it was given.
    Configure(PortConfig),
    /// `set_break(true)` — the line driven low.
    SetBreakOn,
    /// `set_break(false)` — the line released.
    SetBreakOff,
    /// [`FtdiBackend::wait`], with the duration asked for.
    Wait(Duration),
    /// [`FtdiBackend::write`], with the bytes it was given.
    Write(Vec<u8>),
    /// [`FtdiBackend::purge`].
    Purge,
    /// [`FtdiBackend::close`].
    Close,
}

/// A cable that is not there: records what it was asked to do and answers
/// however the test told it to.
///
/// Public rather than test-only. `CLAUDE.md` requires every hardware interface
/// to be mockable and every test to run with no hardware attached, and S8 will
/// want to drive the same sequences against the same assertions while a real
/// cable is on the bench.
pub struct MockFtdi {
    state: Arc<Mutex<MockState>>,
}

/// A test's view of a [`MockFtdi`], usable after the mock has been moved into a
/// driver or onto a thread.
#[derive(Clone)]
pub struct MockFtdiHandle {
    state: Arc<Mutex<MockState>>,
}

#[derive(Default)]
struct MockState {
    calls: Vec<FtdiCall>,
    open: bool,
    open_faults: VecDeque<FtdiError>,
    configure_faults: VecDeque<FtdiError>,
    /// Results rather than faults, because the two `set_break` calls of one
    /// frame have to be failable separately: a break that cannot be *released*
    /// is a different fault from one that cannot be driven.
    break_results: VecDeque<Result<(), FtdiError>>,
    purge_faults: VecDeque<FtdiError>,
    write_results: VecDeque<Result<usize, FtdiError>>,
}

/// Takes a lock without caring whether a previous holder panicked.
///
/// A poisoned mock must not be the thing that takes an output thread down: the
/// panic tests deliberately unwind through code that holds this lock, and the
/// runner is required to survive that.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl MockFtdi {
    /// A mock that answers every call successfully.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState::default())),
        }
    }

    /// A handle onto this mock's recording, cloneable and usable from another
    /// thread once the mock itself has been moved into a driver.
    #[must_use]
    pub fn handle(&self) -> MockFtdiHandle {
        MockFtdiHandle {
            state: Arc::clone(&self.state),
        }
    }
}

impl Default for MockFtdi {
    fn default() -> Self {
        Self::new()
    }
}

impl MockFtdiHandle {
    /// Every call so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<FtdiCall> {
        lock(&self.state).calls.clone()
    }

    /// Forgets the calls so far, so a test can assert on one frame rather than
    /// on a connection followed by a frame.
    pub fn clear_calls(&self) {
        lock(&self.state).calls.clear();
    }

    /// Just the payloads that reached the line, in order.
    #[must_use]
    pub fn writes(&self) -> Vec<Vec<u8>> {
        lock(&self.state)
            .calls
            .iter()
            .filter_map(|call| match call {
                FtdiCall::Write(bytes) => Some(bytes.clone()),
                _ => None,
            })
            .collect()
    }

    /// Whether the device is currently open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        lock(&self.state).open
    }

    /// Makes the next `times` calls to `open` fail.
    pub fn fail_open(&self, times: usize, error: FtdiError) {
        let mut state = lock(&self.state);
        state.open_faults.extend(std::iter::repeat_n(error, times));
    }

    /// Makes the next `times` calls to `configure` fail.
    pub fn fail_configure(&self, times: usize, error: FtdiError) {
        let mut state = lock(&self.state);
        state
            .configure_faults
            .extend(std::iter::repeat_n(error, times));
    }

    /// Makes the next `times` calls to `set_break` fail, whichever direction
    /// they are.
    pub fn fail_break(&self, times: usize, error: FtdiError) {
        let mut state = lock(&self.state);
        state
            .break_results
            .extend(std::iter::repeat_n(Err(error), times));
    }

    /// Lets the next `times` calls to `set_break` through, so that a fault
    /// queued after them lands on the *release* rather than on the assertion.
    pub fn pass_break(&self, times: usize) {
        let mut state = lock(&self.state);
        state
            .break_results
            .extend(std::iter::repeat_n(Ok(()), times));
    }

    /// Makes the next `times` calls to `purge` fail.
    pub fn fail_purge(&self, times: usize, error: FtdiError) {
        let mut state = lock(&self.state);
        state.purge_faults.extend(std::iter::repeat_n(error, times));
    }

    /// Makes the next `times` writes fail — the mid-frame case, because a write
    /// only happens once the break has already gone out.
    pub fn fail_write(&self, times: usize, error: FtdiError) {
        let mut state = lock(&self.state);
        state
            .write_results
            .extend(std::iter::repeat_n(Err(error), times));
    }

    /// Makes the next write succeed having moved only `bytes` bytes.
    pub fn short_write(&self, bytes: usize) {
        lock(&self.state).write_results.push_back(Ok(bytes));
    }

    /// Forgets every queued fault: the cable is plugged back in.
    ///
    /// An outage is queued as "the next hundred opens fail", because a test
    /// cannot know in advance how many times a backoff will try. Ending one
    /// therefore has to be its own operation rather than a matter of counting.
    pub fn clear_faults(&self) {
        let mut state = lock(&self.state);
        state.open_faults.clear();
        state.configure_faults.clear();
        state.break_results.clear();
        state.purge_faults.clear();
        state.write_results.clear();
    }
}

impl MockState {
    fn record(&mut self, call: FtdiCall) {
        self.calls.push(call);
    }

    /// A fault if one is queued, and the link state it leaves behind.
    fn fault(open: &mut bool, queue: &mut VecDeque<FtdiError>) -> Result<(), FtdiError> {
        match queue.pop_front() {
            Some(error) => {
                if error.is_link_lost() {
                    *open = false;
                }
                Err(error)
            }
            None => Ok(()),
        }
    }
}

impl FtdiBackend for MockFtdi {
    fn open(&mut self, device: &DeviceDescriptor) -> Result<(), FtdiError> {
        let mut state = lock(&self.state);
        state.record(FtdiCall::Open(*device));
        let MockState {
            open, open_faults, ..
        } = &mut *state;
        MockState::fault(open, open_faults)?;
        state.open = true;
        Ok(())
    }

    fn configure(&mut self, port: &PortConfig) -> Result<(), FtdiError> {
        let mut state = lock(&self.state);
        state.record(FtdiCall::Configure(*port));
        let MockState {
            open,
            configure_faults,
            ..
        } = &mut *state;
        MockState::fault(open, configure_faults)
    }

    fn set_break(&mut self, on: bool) -> Result<(), FtdiError> {
        let mut state = lock(&self.state);
        let call = if on {
            FtdiCall::SetBreakOn
        } else {
            FtdiCall::SetBreakOff
        };
        state.record(call);
        match state.break_results.pop_front() {
            Some(Err(error)) => {
                if error.is_link_lost() {
                    state.open = false;
                }
                Err(error)
            }
            Some(Ok(())) | None => Ok(()),
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, FtdiError> {
        let mut state = lock(&self.state);
        state.record(FtdiCall::Write(data.to_vec()));
        match state.write_results.pop_front() {
            Some(Err(error)) => {
                if error.is_link_lost() {
                    state.open = false;
                }
                Err(error)
            }
            Some(Ok(bytes)) => Ok(bytes),
            None => Ok(data.len()),
        }
    }

    fn purge(&mut self) -> Result<(), FtdiError> {
        let mut state = lock(&self.state);
        state.record(FtdiCall::Purge);
        let MockState {
            open, purge_faults, ..
        } = &mut *state;
        MockState::fault(open, purge_faults)
    }

    fn close(&mut self) {
        let mut state = lock(&self.state);
        state.record(FtdiCall::Close);
        state.open = false;
    }

    fn wait(&mut self, duration: Duration) {
        lock(&self.state).record(FtdiCall::Wait(duration));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FlowControl, FtdiBackend, FtdiCall, FtdiError, MockFtdi, Parity, PortConfig, StopBits,
        spin_wait, transmission_time,
    };
    use crate::device::SH_RS09B;
    use std::time::{Duration, Instant};

    #[test]
    fn the_dmx_port_is_the_one_the_specification_gives() {
        // ARCHITECTURE_SPEC.md §7.1: 250 000 baud, 8 data bits, 2 stop bits, no
        // parity, no flow control, latency timer 1. Every one of these is a
        // deviation from a default, which is why they are all asserted.
        let port = PortConfig::DMX512;
        assert_eq!(port.baud, 250_000);
        assert_eq!(port.data_bits, 8);
        assert_eq!(port.stop_bits, StopBits::Two);
        assert_eq!(port.parity, Parity::None);
        assert_eq!(port.flow_control, FlowControl::None);
        assert_eq!(port.latency_timer_ms, 1);
    }

    #[test]
    fn the_latency_timer_is_not_the_sixteen_millisecond_default() {
        // A frame period at 44 Hz is 22.7 ms. A 16 ms latency timer would cost
        // most of one, so this is the single setting most likely to be
        // forgotten and most expensive to forget.
        let port = PortConfig::DMX512;
        assert!(u32::from(port.latency_timer_ms) < 16);
        // And a whole packet fits in one USB transfer, so the break timing is
        // not broken up by the USB stack instead of by us.
        assert!(port.write_transfer_size >= 513);
    }

    #[test]
    fn a_cable_error_says_what_went_wrong_in_words() {
        // These reach an operator through the interface status light, so
        // "output error" is not good enough.
        let errors = [
            (
                FtdiError::NotFound,
                "no FTDI device matching the descriptor is attached",
            ),
            (
                FtdiError::Disconnected,
                "the FTDI device is no longer attached",
            ),
            (FtdiError::Io, "the FTDI device refused the operation"),
            (FtdiError::Config, "the FTDI port could not be configured"),
            (FtdiError::Unsupported, "this platform has no FTDI backend"),
            (
                FtdiError::ShortWrite {
                    wrote: 12,
                    expected: 513,
                },
                "the FTDI device took 12 of 513 bytes",
            ),
        ];
        for (error, text) in errors {
            assert_eq!(error.to_string(), text);
            let as_error: &dyn std::error::Error = &error;
            assert_eq!(as_error.to_string(), text);
        }
    }

    #[test]
    fn losing_the_cable_is_a_different_answer_from_refusing_a_call() {
        // The first means reconnect with backoff; the second means this frame
        // did not go out and the next one might.
        assert!(FtdiError::NotFound.is_link_lost());
        assert!(FtdiError::Disconnected.is_link_lost());
        assert!(!FtdiError::Io.is_link_lost());
        assert!(!FtdiError::Config.is_link_lost());
        // A platform with no backend is not a cable that fell out, and a
        // runner must not spend its life reconnecting to one.
        assert!(!FtdiError::Unsupported.is_link_lost());
        assert!(
            !FtdiError::ShortWrite {
                wrote: 1,
                expected: 513
            }
            .is_link_lost()
        );
    }

    #[test]
    fn the_mock_records_every_call_in_order() {
        let mut ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        ftdi.open(&SH_RS09B.device).unwrap();
        ftdi.configure(&PortConfig::DMX512).unwrap();
        ftdi.purge().unwrap();
        ftdi.set_break(true).unwrap();
        ftdi.wait(Duration::from_micros(110));
        ftdi.set_break(false).unwrap();
        ftdi.wait(Duration::from_micros(16));
        assert_eq!(ftdi.write(&[0u8, 1, 2]).unwrap(), 3);
        ftdi.close();

        assert_eq!(
            handle.calls(),
            vec![
                FtdiCall::Open(SH_RS09B.device),
                FtdiCall::Configure(PortConfig::DMX512),
                FtdiCall::Purge,
                FtdiCall::SetBreakOn,
                FtdiCall::Wait(Duration::from_micros(110)),
                FtdiCall::SetBreakOff,
                FtdiCall::Wait(Duration::from_micros(16)),
                FtdiCall::Write(vec![0, 1, 2]),
                FtdiCall::Close,
            ]
        );
        assert_eq!(handle.writes(), vec![vec![0u8, 1, 2]]);
    }

    #[test]
    fn the_mock_tracks_whether_the_device_is_open() {
        let mut ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        assert!(!handle.is_open());
        ftdi.open(&SH_RS09B.device).unwrap();
        assert!(handle.is_open());
        ftdi.close();
        assert!(!handle.is_open());
    }

    #[test]
    fn the_mock_fails_the_operations_it_was_told_to_fail() {
        let mut ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        handle.fail_open(2, FtdiError::NotFound);
        handle.fail_configure(1, FtdiError::Config);
        handle.fail_break(1, FtdiError::Io);
        handle.fail_purge(1, FtdiError::Io);
        handle.fail_write(1, FtdiError::Disconnected);

        assert_eq!(ftdi.open(&SH_RS09B.device), Err(FtdiError::NotFound));
        assert_eq!(ftdi.open(&SH_RS09B.device), Err(FtdiError::NotFound));
        assert_eq!(ftdi.open(&SH_RS09B.device), Ok(()));
        assert_eq!(ftdi.configure(&PortConfig::DMX512), Err(FtdiError::Config));
        assert_eq!(ftdi.configure(&PortConfig::DMX512), Ok(()));
        assert_eq!(ftdi.set_break(true), Err(FtdiError::Io));
        assert_eq!(ftdi.set_break(false), Ok(()));
        assert_eq!(ftdi.purge(), Err(FtdiError::Io));
        assert_eq!(ftdi.purge(), Ok(()));
        assert_eq!(ftdi.write(&[0]), Err(FtdiError::Disconnected));
        // A lost link closes the device, exactly as the real one does.
        assert!(!handle.is_open());
    }

    #[test]
    fn a_lost_link_closes_the_mock_whichever_call_reports_it() {
        for (fail, call) in [(0, "break"), (1, "purge"), (2, "configure")] {
            let mut ftdi = MockFtdi::new();
            let handle = ftdi.handle();
            ftdi.open(&SH_RS09B.device).unwrap();
            match fail {
                0 => handle.fail_break(1, FtdiError::Disconnected),
                1 => handle.fail_purge(1, FtdiError::Disconnected),
                _ => handle.fail_configure(1, FtdiError::Disconnected),
            }
            let outcome = match fail {
                0 => ftdi.set_break(true),
                1 => ftdi.purge(),
                _ => ftdi.configure(&PortConfig::DMX512),
            };
            assert_eq!(outcome, Err(FtdiError::Disconnected), "{call}");
            assert!(!handle.is_open(), "{call}");
        }
    }

    #[test]
    fn a_short_write_reports_the_count_that_actually_moved() {
        let mut ftdi = MockFtdi::new();
        ftdi.handle().short_write(7);
        assert_eq!(ftdi.write(&[0u8; 513]), Ok(7));
        // And the next one is back to normal.
        assert_eq!(ftdi.write(&[0u8; 513]), Ok(513));
    }

    #[test]
    fn plugging_the_cable_back_in_forgets_every_queued_fault() {
        let mut ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        handle.fail_open(100, FtdiError::NotFound);
        handle.fail_configure(100, FtdiError::Config);
        handle.fail_break(100, FtdiError::Io);
        handle.fail_purge(100, FtdiError::Io);
        handle.fail_write(100, FtdiError::Disconnected);
        assert_eq!(ftdi.open(&SH_RS09B.device), Err(FtdiError::NotFound));

        handle.clear_faults();
        assert_eq!(ftdi.open(&SH_RS09B.device), Ok(()));
        assert_eq!(ftdi.configure(&PortConfig::DMX512), Ok(()));
        assert_eq!(ftdi.purge(), Ok(()));
        assert_eq!(ftdi.set_break(true), Ok(()));
        assert_eq!(ftdi.write(&[0u8; 513]), Ok(513));
    }

    #[test]
    fn the_recording_can_be_cleared_between_phases() {
        let mut ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        ftdi.open(&SH_RS09B.device).unwrap();
        handle.clear_calls();
        ftdi.purge().unwrap();
        assert_eq!(handle.calls(), vec![FtdiCall::Purge]);
    }

    #[test]
    fn a_dmx_frame_takes_twenty_two_and_a_half_milliseconds_on_the_wire() {
        // The number the whole break sequence hangs on. 513 bytes at 250 000
        // baud, eleven bits each: 5643 bits, 22.572 ms. A write that returns
        // before this has elapsed has left data in flight, and a break
        // asserted on top of it lands inside the frame.
        let frame = transmission_time(513, 250_000);
        assert_eq!(frame, Duration::from_nanos(22_572_000));
        // A shorter universe is proportionally quicker; the arithmetic is not
        // special-cased for full frames.
        assert_eq!(transmission_time(1, 250_000), Duration::from_nanos(44_000));
        assert_eq!(transmission_time(0, 250_000), Duration::ZERO);
        assert_eq!(transmission_time(513, 500_000), frame / 2);
    }

    #[test]
    fn a_port_at_no_baud_rate_has_nothing_to_wait_for() {
        // A backend that has opened a device but not configured it yet has no
        // baud rate to divide by, and dividing by zero on a driver thread is
        // not an option.
        assert_eq!(transmission_time(513, 0), Duration::ZERO);
    }

    #[test]
    fn the_real_wait_does_not_return_early() {
        // Loose on purpose: this asserts the floor, not the ceiling. There is no
        // jitter measurement anywhere in this crate, so nothing here can be made
        // to fail by a busy machine.
        let asked = Duration::from_micros(500);
        let start = Instant::now();
        spin_wait(asked);
        assert!(start.elapsed() >= asked);
    }

    #[test]
    fn a_wait_longer_than_a_millisecond_sleeps_for_most_of_it() {
        let asked = Duration::from_millis(3);
        let start = Instant::now();
        spin_wait(asked);
        assert!(start.elapsed() >= asked);
    }

    #[test]
    fn waiting_for_no_time_at_all_returns_at_once() {
        let start = Instant::now();
        spin_wait(Duration::ZERO);
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[test]
    fn the_default_wait_is_the_real_one() {
        // A backend that does not override `wait` gets the spin, so a real
        // driver cannot accidentally inherit a test's instant one.
        struct Bare;
        impl FtdiBackend for Bare {
            fn open(&mut self, _: &crate::device::DeviceDescriptor) -> Result<(), FtdiError> {
                Ok(())
            }
            fn configure(&mut self, _: &PortConfig) -> Result<(), FtdiError> {
                Ok(())
            }
            fn set_break(&mut self, _: bool) -> Result<(), FtdiError> {
                Ok(())
            }
            fn write(&mut self, data: &[u8]) -> Result<usize, FtdiError> {
                Ok(data.len())
            }
            fn purge(&mut self) -> Result<(), FtdiError> {
                Ok(())
            }
            fn close(&mut self) {}
        }
        let asked = Duration::from_micros(300);
        let start = Instant::now();
        let mut bare = Bare;
        bare.wait(asked);
        assert!(start.elapsed() >= asked);

        // And a backend that implements only the six required methods is a
        // usable one: `wait` is the only thing the trait supplies itself, which
        // is what a real D2XX or libftdi backend will rely on.
        assert_eq!(bare.open(&SH_RS09B.device), Ok(()));
        assert_eq!(bare.configure(&PortConfig::DMX512), Ok(()));
        assert_eq!(bare.set_break(true), Ok(()));
        assert_eq!(bare.purge(), Ok(()));
        assert_eq!(bare.write(&[0u8; 513]), Ok(513));
        bare.close();
    }

    #[test]
    fn a_boxed_backend_is_a_backend() {
        // Which cable a driver holds is decided at run time — D2XX on one
        // machine, the virtual COM port on the next — so the driver has to be
        // able to hold a boxed one.
        let ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        let mut boxed: Box<dyn FtdiBackend> = Box::new(ftdi);
        boxed.open(&SH_RS09B.device).unwrap();
        boxed.configure(&PortConfig::DMX512).unwrap();
        boxed.purge().unwrap();
        boxed.set_break(true).unwrap();
        boxed.wait(Duration::from_micros(110));
        boxed.set_break(false).unwrap();
        assert_eq!(boxed.write(&[0u8; 513]), Ok(513));
        boxed.close();
        assert_eq!(
            handle.calls(),
            vec![
                FtdiCall::Open(SH_RS09B.device),
                FtdiCall::Configure(PortConfig::DMX512),
                FtdiCall::Purge,
                FtdiCall::SetBreakOn,
                FtdiCall::Wait(Duration::from_micros(110)),
                FtdiCall::SetBreakOff,
                FtdiCall::Write(vec![0u8; 513]),
                FtdiCall::Close,
            ]
        );
    }

    #[test]
    fn a_default_mock_is_a_new_one() {
        let mut ftdi = MockFtdi::default();
        let handle = ftdi.handle();
        ftdi.purge().unwrap();
        assert_eq!(handle.calls(), vec![FtdiCall::Purge]);
    }
}
