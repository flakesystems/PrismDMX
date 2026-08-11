//! Open DMX USB: one FTDI cable, one universe, and the host generating DMX512
//! timing by hand.
//!
//! `ARCHITECTURE_SPEC.md` §7.1. There is no microcontroller in this adapter, so
//! every frame is four operations rather than one write:
//!
//! ```text
//!   SetBreakOn ──110 µs──▶ SetBreakOff ──16 µs──▶ write(513 bytes)
//!   │                      │                      │
//!   line held low          mark after break       start code 0x00
//!   (≥ 92 µs required)     (≥ 12 µs required)     + 512 channels
//! ```
//!
//! and the 513th byte leaves the port 22.6 ms after the first, because that is
//! what 250 000 baud means. Together with two USB control transfers for the
//! break, that is the whole reason this adapter runs at 30–40 Hz rather than
//! 44 — `ARCHITECTURE_SPEC.md` §3.2 treats the difference as a property of the
//! hardware, not as a fault, and so does this driver: it never reports degraded
//! for being slow.
//!
//! Nothing here is platform-specific. The cable is a [`FtdiBackend`], the
//! adapter's numbers are a [`DeviceProfile`], and both are parameters — which
//! is what lets the sequence above be asserted call by call with no hardware
//! present.

use prism_domain::{OutputHealth, OutputId, UniverseId};
use prism_engine::UNIVERSE_CHANNELS;

use crate::device::{DeviceProfile, SH_RS09B};
use crate::ftdi::{FtdiBackend, FtdiError};
use crate::output::{DmxOutput, OutputError};

/// The first byte of a DMX512 packet. `0x00` is the null start code: ordinary
/// dimmer data, as opposed to RDM or a text packet.
pub const START_CODE: u8 = 0x00;

/// Bytes in one DMX512 packet: the start code and 512 channels.
pub const DMX_PACKET_BYTES: usize = 1 + UNIVERSE_CHANNELS;

/// One Open DMX USB adapter, driving one universe.
///
/// The type parameter is the cable. In the field it is a D2XX or libftdi
/// backend; in every test in this repository it is
/// [`MockFtdi`](crate::MockFtdi), which is why the break timing and the packet
/// layout are checked on every commit rather than on the days someone has the
/// hardware to hand.
pub struct OpenDmxUsb<B: FtdiBackend> {
    id: OutputId,
    /// One universe, held as a slice so [`DmxOutput::universes`] can return it.
    /// §7.1: exactly one per adapter, and that is a hardware limit rather than
    /// a simplification.
    universes: [UniverseId; 1],
    profile: DeviceProfile,
    backend: B,
    open: bool,
    health: OutputHealth,
    /// The packet buffer, allocated once. A driver thread has no reason to
    /// touch the allocator per frame, and at 40 Hz per adapter it would be
    /// doing so 40 times a second for no gain.
    packet: [u8; DMX_PACKET_BYTES],
}

impl<B: FtdiBackend> OpenDmxUsb<B> {
    /// A driver for the [`SH_RS09B`], disconnected until
    /// [`connect`](DmxOutput::connect) is called.
    #[must_use]
    pub fn new(id: OutputId, universe: UniverseId, backend: B) -> Self {
        Self::with_profile(id, universe, backend, SH_RS09B)
    }

    /// A driver for another adapter of the same kind — or for this one once S8
    /// has measured it, since the profile is data rather than code.
    #[must_use]
    pub fn with_profile(
        id: OutputId,
        universe: UniverseId,
        backend: B,
        profile: DeviceProfile,
    ) -> Self {
        Self {
            id,
            universes: [universe],
            profile,
            backend,
            open: false,
            health: OutputHealth::Disconnected,
            packet: [START_CODE; DMX_PACKET_BYTES],
        }
    }

    /// The universe this adapter carries.
    #[must_use]
    pub const fn universe(&self) -> UniverseId {
        self.universes[0]
    }

    /// The adapter's profile: descriptor, port parameters and timing.
    #[must_use]
    pub const fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    /// Closes the cable and records the output as silent.
    fn drop_link(&mut self) {
        if self.open {
            self.backend.close();
            self.open = false;
        }
        self.health = OutputHealth::Disconnected;
    }

    /// Turns a cable error into an output error, and leaves the driver in the
    /// state that error implies.
    ///
    /// The split is the one [`FtdiError::is_link_lost`] makes: a cable that has
    /// gone means close down and let the runner reconnect on its backoff; a
    /// refused transfer means this frame did not go out and the next one will
    /// be attempted.
    ///
    /// A failure to *release* the break is deliberately not special-cased. It
    /// leaves the line low, which is a dark universe — but the next frame
    /// begins by asserting the break and releasing it again, so the retry the
    /// ordinary path already performs is exactly the recovery a special case
    /// would have to make. Closing the device instead would not guarantee the
    /// pin comes back up either.
    fn fault(&mut self, error: FtdiError) -> OutputError {
        if error.is_link_lost() {
            self.drop_link();
            OutputError::Disconnected
        } else {
            self.health = OutputHealth::Degraded;
            OutputError::Faulted
        }
    }
}

impl<B: FtdiBackend> DmxOutput for OpenDmxUsb<B> {
    fn id(&self) -> OutputId {
        self.id
    }

    fn universes(&self) -> &[UniverseId] {
        &self.universes
    }

    /// Opens the cable and sets the port up: 250 000 baud, 8N2, no flow
    /// control, latency timer 1.
    ///
    /// The purge at the end is what makes this safe to call as a *re*connect:
    /// §7.1's failure mode in the field is an unplugged cable, and a device
    /// reopened with the tail of the last frame still queued would put a
    /// truncated packet on the wire the moment it came back.
    fn connect(&mut self) -> Result<(), OutputError> {
        // Called again on every reconnect, so a handle must not be left behind
        // each time the cable is replugged.
        if self.open {
            self.backend.close();
            self.open = false;
        }
        if self.backend.open(&self.profile.device).is_err() {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }
        self.open = true;
        // A port that will not take the DMX parameters is not usable: sending
        // at the wrong baud rate would put noise onto a live line.
        if self.backend.configure(&self.profile.port).is_err() || self.backend.purge().is_err() {
            self.drop_link();
            return Err(OutputError::Disconnected);
        }
        self.health = OutputHealth::Ok;
        Ok(())
    }

    fn send_frame(
        &mut self,
        universe: UniverseId,
        data: &[u8; UNIVERSE_CHANNELS],
    ) -> Result<(), OutputError> {
        if universe != self.universe() {
            let error = OutputError::UniverseNotCarried(universe);
            // Visible as an amber light rather than a silent drop, but not
            // enough to claim the cable is sending if it is not.
            if self.health.is_sending() {
                self.health = error.health();
            }
            return Err(error);
        }
        if !self.open {
            self.health = OutputHealth::Disconnected;
            return Err(OutputError::Disconnected);
        }

        let (start_code, channels) = self.packet.split_at_mut(1);
        start_code.fill(START_CODE);
        channels.copy_from_slice(data);

        if let Err(error) = self.backend.set_break(true) {
            return Err(self.fault(error));
        }
        self.backend.wait(self.profile.timing.break_time);
        if let Err(error) = self.backend.set_break(false) {
            return Err(self.fault(error));
        }
        self.backend.wait(self.profile.timing.mark_after_break);

        match self.backend.write(&self.packet) {
            Ok(DMX_PACKET_BYTES) => {
                self.health = OutputHealth::Ok;
                Ok(())
            }
            // Fewer bytes than the packet is a frame with its tail missing, not
            // a partial success: the channels that did not go out keep whatever
            // the receiver had, while the output claims to be sending.
            Ok(wrote) => Err(self.fault(FtdiError::ShortWrite {
                wrote,
                expected: DMX_PACKET_BYTES,
            })),
            Err(error) => Err(self.fault(error)),
        }
    }

    fn health(&self) -> OutputHealth {
        self.health
    }

    fn shutdown(&mut self) {
        self.drop_link();
    }
}

#[cfg(test)]
mod tests {
    use super::{DMX_PACKET_BYTES, OpenDmxUsb, START_CODE};
    use crate::device::SH_RS09B;
    use crate::ftdi::{FtdiCall, FtdiError, MockFtdi, MockFtdiHandle, PortConfig};
    use crate::output::{DmxOutput, OutputError};
    use prism_domain::{OutputHealth, OutputId, UniverseId};
    use proptest::prelude::*;
    use std::time::Duration;

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    /// A driver on universe 1 with a mock cable, plus the handle onto it.
    fn driver() -> (OpenDmxUsb<MockFtdi>, MockFtdiHandle) {
        let ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        (OpenDmxUsb::new(OutputId::new(1), universe(1), ftdi), handle)
    }

    /// A connected driver, with the connection's calls already forgotten so a
    /// test asserts on one frame and not on the set-up in front of it.
    fn connected() -> (OpenDmxUsb<MockFtdi>, MockFtdiHandle) {
        let (mut driver, handle) = driver();
        driver.connect().unwrap();
        handle.clear_calls();
        (driver, handle)
    }

    #[test]
    fn connecting_opens_the_named_device_configures_it_and_purges_it() {
        let (mut driver, handle) = driver();
        assert_eq!(driver.health(), OutputHealth::Disconnected);
        driver.connect().unwrap();

        // ARCHITECTURE_SPEC.md §7.1: the port parameters are applied before any
        // byte goes out, and the buffers are cleared so a reconnect cannot
        // resume half way through the previous frame.
        assert_eq!(
            handle.calls(),
            vec![
                FtdiCall::Open(SH_RS09B.device),
                FtdiCall::Configure(PortConfig::DMX512),
                FtdiCall::Purge,
            ]
        );
        assert_eq!(driver.health(), OutputHealth::Ok);
        assert!(handle.is_open());
    }

    #[test]
    fn the_port_is_configured_at_two_hundred_and_fifty_thousand_baud_eight_n_two() {
        // The exit criterion names these four, so they are asserted on what the
        // cable was actually told, not on the constant.
        let (mut driver, handle) = driver();
        driver.connect().unwrap();
        let configured = handle
            .calls()
            .into_iter()
            .find_map(|call| match call {
                FtdiCall::Configure(port) => Some(port),
                _ => None,
            })
            .expect("the driver must configure the port");
        assert_eq!(configured.baud, 250_000);
        assert_eq!(configured.data_bits, 8);
        assert_eq!(configured.stop_bits, crate::ftdi::StopBits::Two);
        assert_eq!(configured.parity, crate::ftdi::Parity::None);
        assert_eq!(configured.flow_control, crate::ftdi::FlowControl::None);
        assert_eq!(configured.latency_timer_ms, 1);
    }

    #[test]
    fn a_frame_is_break_mark_and_five_hundred_and_thirteen_bytes_in_that_order() {
        // The headline exit criterion of S7, and the reason this crate exists
        // in this shape: SetBreakOn → delay → SetBreakOff → delay → write.
        let (mut driver, handle) = connected();
        let mut channels = [0u8; 512];
        channels[0] = 255;
        channels[511] = 42;
        driver.send_frame(universe(1), &channels).unwrap();

        let mut packet = vec![START_CODE];
        packet.extend_from_slice(&channels);
        assert_eq!(
            handle.calls(),
            vec![
                FtdiCall::SetBreakOn,
                FtdiCall::Wait(SH_RS09B.timing.break_time),
                FtdiCall::SetBreakOff,
                FtdiCall::Wait(SH_RS09B.timing.mark_after_break),
                FtdiCall::Write(packet),
            ]
        );
        assert_eq!(driver.health(), OutputHealth::Ok);
    }

    #[test]
    fn the_packet_is_the_start_code_and_then_the_channels_in_order() {
        let (mut driver, handle) = connected();
        let channels: [u8; 512] = std::array::from_fn(|index| (index % 251) as u8);
        driver.send_frame(universe(1), &channels).unwrap();

        let written = handle.writes();
        assert_eq!(written.len(), 1);
        let packet = &written[0];
        assert_eq!(packet.len(), DMX_PACKET_BYTES);
        assert_eq!(packet.len(), 513);
        assert_eq!(packet[0], 0x00, "the DMX start code is 0x00");
        assert_eq!(&packet[1..], &channels[..]);
    }

    #[test]
    fn the_break_is_held_for_the_time_the_profile_asks_for() {
        let (mut driver, handle) = connected();
        driver.send_frame(universe(1), &[0; 512]).unwrap();
        let waits: Vec<Duration> = handle
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                FtdiCall::Wait(duration) => Some(duration),
                _ => None,
            })
            .collect();
        assert_eq!(
            waits,
            vec![SH_RS09B.timing.break_time, SH_RS09B.timing.mark_after_break]
        );
        assert!(waits[0] >= Duration::from_micros(92));
        assert!(waits[1] >= Duration::from_micros(12));
    }

    #[test]
    fn nothing_goes_out_before_the_cable_is_open() {
        let (mut driver, handle) = driver();
        assert_eq!(
            driver.send_frame(universe(1), &[255; 512]),
            Err(OutputError::Disconnected)
        );
        assert!(handle.calls().is_empty(), "not even a break");
    }

    #[test]
    fn one_adapter_carries_exactly_one_universe() {
        // ARCHITECTURE_SPEC.md §7.1, row "Limitations".
        let (mut driver, handle) = connected();
        assert_eq!(driver.universes(), [universe(1)]);
        assert_eq!(
            driver.send_frame(universe(2), &[0; 512]),
            Err(OutputError::UniverseNotCarried(universe(2)))
        );
        assert!(handle.calls().is_empty());
        // And the mistake does not take the output down: the cable is fine.
        assert_eq!(driver.health(), OutputHealth::Degraded);
    }

    #[test]
    fn a_cable_pulled_mid_frame_reports_disconnected_and_gives_the_link_up() {
        // The write is the mid-frame moment: the break has already gone out.
        let (mut driver, handle) = connected();
        handle.fail_write(1, FtdiError::Disconnected);
        assert_eq!(
            driver.send_frame(universe(1), &[7; 512]),
            Err(OutputError::Disconnected)
        );
        assert_eq!(driver.health(), OutputHealth::Disconnected);
        assert!(!handle.is_open());
        // The driver closed the device rather than leaving it half open.
        assert_eq!(handle.calls().last(), Some(&FtdiCall::Close));
        // And it refuses to send again until something reconnects it.
        assert_eq!(
            driver.send_frame(universe(1), &[7; 512]),
            Err(OutputError::Disconnected)
        );
    }

    #[test]
    fn a_cable_pulled_while_the_break_is_asserted_gives_the_link_up_too() {
        let (mut driver, handle) = connected();
        handle.fail_break(1, FtdiError::Disconnected);
        assert_eq!(
            driver.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Disconnected)
        );
        assert_eq!(driver.health(), OutputHealth::Disconnected);
        assert!(handle.writes().is_empty(), "no packet after a failed break");
    }

    #[test]
    fn a_break_that_cannot_be_driven_stops_the_frame_before_any_byte_goes_out() {
        let (mut driver, handle) = connected();
        handle.fail_break(1, FtdiError::Io);
        assert_eq!(
            driver.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Faulted)
        );
        assert_eq!(driver.health(), OutputHealth::Degraded);
        assert_eq!(handle.calls(), vec![FtdiCall::SetBreakOn]);
        assert!(handle.writes().is_empty());
    }

    #[test]
    fn a_break_that_cannot_be_released_leaves_no_packet_on_the_line() {
        // The line stays low, which is a dark universe — and the recovery is
        // the next frame, which asserts and releases the break again. See
        // `OpenDmxUsb::fault` for why that is not special-cased.
        let (mut driver, handle) = connected();
        handle.pass_break(1);
        handle.fail_break(1, FtdiError::Io);
        assert_eq!(
            driver.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Faulted)
        );
        assert_eq!(driver.health(), OutputHealth::Degraded);
        assert_eq!(
            handle.calls(),
            vec![
                FtdiCall::SetBreakOn,
                FtdiCall::Wait(SH_RS09B.timing.break_time),
                FtdiCall::SetBreakOff,
            ]
        );
        assert!(handle.writes().is_empty());
        // The next frame goes out normally.
        assert_eq!(driver.send_frame(universe(1), &[0; 512]), Ok(()));
        assert_eq!(driver.health(), OutputHealth::Ok);
    }

    #[test]
    fn a_refused_write_leaves_the_cable_up_and_the_output_degraded() {
        let (mut driver, handle) = connected();
        handle.fail_write(1, FtdiError::Io);
        assert_eq!(
            driver.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Faulted)
        );
        assert_eq!(driver.health(), OutputHealth::Degraded);
        assert!(handle.is_open(), "an Io error is not a lost cable");
        // The next frame is attempted and recovers the output.
        assert_eq!(driver.send_frame(universe(1), &[0; 512]), Ok(()));
        assert_eq!(driver.health(), OutputHealth::Ok);
    }

    #[test]
    fn a_short_write_is_a_truncated_frame_and_not_a_success() {
        // 512 of 513 bytes is a frame with no last channel in it. A driver that
        // called that "sent" would show green while a fixture sat at the wrong
        // value.
        let (mut driver, handle) = connected();
        handle.short_write(512);
        assert_eq!(
            driver.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Faulted)
        );
        assert_eq!(driver.health(), OutputHealth::Degraded);
        assert!(handle.is_open());
    }

    #[test]
    fn a_device_that_is_not_there_reports_disconnected() {
        let (mut driver, handle) = driver();
        handle.fail_open(1, FtdiError::NotFound);
        assert_eq!(driver.connect(), Err(OutputError::Disconnected));
        assert_eq!(driver.health(), OutputHealth::Disconnected);
        assert_eq!(handle.calls(), vec![FtdiCall::Open(SH_RS09B.device)]);
    }

    #[test]
    fn a_port_that_will_not_configure_does_not_send_at_all() {
        // A port left at the wrong baud rate would put noise on a live DMX
        // line, so a failed configuration closes the device instead.
        let (mut driver, handle) = driver();
        handle.fail_configure(1, FtdiError::Config);
        assert_eq!(driver.connect(), Err(OutputError::Disconnected));
        assert_eq!(driver.health(), OutputHealth::Disconnected);
        assert_eq!(handle.calls().last(), Some(&FtdiCall::Close));
        assert!(!handle.is_open());
        assert_eq!(
            driver.send_frame(universe(1), &[0; 512]),
            Err(OutputError::Disconnected)
        );
    }

    #[test]
    fn a_purge_that_fails_is_a_failed_connection() {
        let (mut driver, handle) = driver();
        handle.fail_purge(1, FtdiError::Io);
        assert_eq!(driver.connect(), Err(OutputError::Disconnected));
        assert_eq!(handle.calls().last(), Some(&FtdiCall::Close));
    }

    #[test]
    fn reconnecting_after_a_disconnect_opens_a_fresh_port() {
        let (mut driver, handle) = connected();
        handle.fail_write(1, FtdiError::Disconnected);
        driver.send_frame(universe(1), &[0; 512]).unwrap_err();
        handle.clear_calls();

        driver.connect().unwrap();
        assert_eq!(
            handle.calls(),
            vec![
                FtdiCall::Open(SH_RS09B.device),
                FtdiCall::Configure(PortConfig::DMX512),
                FtdiCall::Purge,
            ]
        );
        assert_eq!(driver.health(), OutputHealth::Ok);
        assert_eq!(driver.send_frame(universe(1), &[3; 512]), Ok(()));
        assert_eq!(handle.writes().len(), 1);
    }

    #[test]
    fn connecting_an_already_open_driver_closes_the_old_device_first() {
        // Reconnect is the same call as connect, so it has to be safe on a
        // driver that thinks it is still up — otherwise a device handle leaks
        // every time the cable is replugged.
        let (mut driver, handle) = connected();
        driver.connect().unwrap();
        assert_eq!(
            handle.calls(),
            vec![
                FtdiCall::Close,
                FtdiCall::Open(SH_RS09B.device),
                FtdiCall::Configure(PortConfig::DMX512),
                FtdiCall::Purge,
            ]
        );
    }

    #[test]
    fn shutting_the_driver_down_closes_the_cable() {
        let (mut driver, handle) = connected();
        driver.shutdown();
        assert_eq!(driver.health(), OutputHealth::Disconnected);
        assert_eq!(handle.calls(), vec![FtdiCall::Close]);
        assert!(!handle.is_open());
        // Shutting down twice is not an error and does not close twice.
        driver.shutdown();
        assert_eq!(handle.calls(), vec![FtdiCall::Close]);
    }

    #[test]
    fn the_driver_reports_the_output_and_the_profile_it_was_built_with() {
        let (driver, _) = driver();
        assert_eq!(driver.id(), OutputId::new(1));
        assert_eq!(driver.universe(), universe(1));
        assert_eq!(driver.profile().name, "DSD TECH SH-RS09B");
        assert_eq!(driver.profile(), &SH_RS09B);
    }

    #[test]
    fn a_driver_can_be_built_for_another_profile() {
        // The profile is data (see `device`), so a different adapter — or S8's
        // measured version of this one — is a different constant and not a
        // different driver.
        let mut profile = SH_RS09B;
        profile.timing.break_time = Duration::from_micros(200);
        let ftdi = MockFtdi::new();
        let handle = ftdi.handle();
        let mut driver = OpenDmxUsb::with_profile(OutputId::new(2), universe(5), ftdi, profile);
        driver.connect().unwrap();
        handle.clear_calls();
        driver.send_frame(universe(5), &[0; 512]).unwrap();
        assert_eq!(
            handle.calls()[1],
            FtdiCall::Wait(Duration::from_micros(200))
        );
    }

    proptest! {
        #[test]
        fn every_channel_reaches_the_wire_unchanged_behind_a_zero_start_code(
            channels in proptest::collection::vec(any::<u8>(), 512..=512)
        ) {
            let (mut driver, handle) = connected();
            let mut data = [0u8; 512];
            data.copy_from_slice(&channels);
            driver.send_frame(universe(1), &data).unwrap();

            let written = handle.writes();
            prop_assert_eq!(written.len(), 1);
            prop_assert_eq!(written[0].len(), DMX_PACKET_BYTES);
            prop_assert_eq!(written[0][0], START_CODE);
            prop_assert_eq!(&written[0][1..], &channels[..]);
        }
    }
}
