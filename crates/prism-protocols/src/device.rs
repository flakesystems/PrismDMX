//! What is known about the cable, in one table.
//!
//! Two of the numbers below have never been read off a real device: the USB
//! product ID and the achievable frame rate. `PROGRESS.md` §5 and
//! `ARCHITECTURE_SPEC.md` §14 both record them as open, and both say the same
//! thing about how they should be held — as plain data, so that verifying them
//! is an edit to a table rather than a refactor. That is what this module is.
//! Session **S8** puts the adapter on the bench, replaces the estimates with
//! measurements and flips [`DeviceProfile::verified`]; nothing outside this file
//! should need to change when it does.

use core::fmt;
use std::time::Duration;

use crate::ftdi::PortConfig;

/// How the driver gets at an FTDI device on this platform.
///
/// `ARCHITECTURE_SPEC.md` §10.1 confines `#[cfg(target_os = …)]` to this crate
/// and `prism-app`. This enum is the whole of the confinement: the choice is
/// made here, once, as data, and everything above it — the break sequence, the
/// packet, the reconnect policy — is the same code on every platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPath {
    /// FTDI's own driver (`libftd2xx`). The Windows default: it exposes the
    /// break control and the latency timer directly.
    D2xx,
    /// The virtual COM port (`serialport`, with `set_break`/`clear_break`).
    /// The Windows fallback for a machine where the D2XX driver is not
    /// installed.
    Vcp,
    /// `libftdi` over `rusb`. The Linux and Raspberry Pi path — `ftdi_sio`
    /// claims the device, so the kernel driver has to be detached, and D2XX is
    /// not used there at all.
    LibFtdi,
}

impl AccessPath {
    /// The path to try first on the platform this was compiled for.
    #[must_use]
    pub const fn preferred() -> Self {
        #[cfg(windows)]
        {
            Self::D2xx
        }
        #[cfg(not(windows))]
        {
            Self::LibFtdi
        }
    }

    /// The path to try if [`preferred`](Self::preferred) is unavailable, where
    /// there is one.
    ///
    /// Windows has a real fallback because D2XX depends on which FTDI driver
    /// the machine happens to have installed. Linux has none: the libftdi path
    /// is the only one §7.1 permits there.
    #[must_use]
    pub const fn fallback() -> Option<Self> {
        #[cfg(windows)]
        {
            Some(Self::Vcp)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

impl fmt::Display for AccessPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::D2xx => "D2XX",
            Self::Vcp => "virtual COM port",
            Self::LibFtdi => "libftdi",
        };
        f.write_str(name)
    }
}

/// How a device is recognised on the USB bus.
///
/// A `None` in either string field means "do not care", which is what makes one
/// constant usable for every adapter of a type: the serial number differs per
/// cable and is the field an operator uses to pin one output to one physical
/// port once there are two of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// USB vendor ID. FTDI is `0x0403`.
    pub vendor_id: u16,
    /// USB product ID. An FT232R is *typically* `0x6001` — unverified for this
    /// adapter, see the module documentation.
    pub product_id: u16,
    /// Product string to require, or `None` to accept any.
    pub product: Option<&'static str>,
    /// Serial number to require, or `None` to accept the first match.
    pub serial: Option<&'static str>,
}

impl DeviceDescriptor {
    /// Whether an enumerated device is the one this descriptor asks for.
    ///
    /// The backend implementations call this while walking the bus, so the
    /// matching rule is written once and tested once rather than living inside
    /// three platform-specific loops.
    #[must_use]
    pub fn matches(
        &self,
        vendor_id: u16,
        product_id: u16,
        product: Option<&str>,
        serial: Option<&str>,
    ) -> bool {
        if self.vendor_id != vendor_id || self.product_id != product_id {
            return false;
        }
        let wanted_product = self.product.is_none_or(|want| product == Some(want));
        let wanted_serial = self.serial.is_none_or(|want| serial == Some(want));
        wanted_product && wanted_serial
    }

    /// The same descriptor pinned to one physical cable by its serial number.
    #[must_use]
    pub const fn with_serial(self, serial: &'static str) -> Self {
        Self {
            serial: Some(serial),
            ..self
        }
    }
}

impl fmt::Display for DeviceDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor_id, self.product_id)?;
        if let Some(serial) = self.serial {
            write!(f, " serial {serial}")?;
        }
        Ok(())
    }
}

/// The timing of one DMX512 frame, as the host has to generate it.
///
/// `ARCHITECTURE_SPEC.md` §7.1: there is no microcontroller in this cable, so
/// these are not settings the device honours — they are delays this process
/// takes, between two USB control transfers, on a thread that can be preempted.
/// Longer is safe and shorter is not, which is why the numbers sit at the
/// generous end of the standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmxTiming {
    /// How long the line is held low. DMX512 requires at least 92 µs and
    /// permits up to a second.
    pub break_time: Duration,
    /// Mark after break: the line released before the start code. DMX512
    /// requires at least 12 µs.
    pub mark_after_break: Duration,
    /// The shortest gap between two frame starts this output will attempt.
    ///
    /// Not a limit the hardware enforces — 513 bytes at 250 kBaud is 22.6 ms of
    /// pure data on its own — but the cadence the driver thread wakes on, so it
    /// paces itself instead of spinning next to the engine.
    pub min_frame_interval: Duration,
}

/// Everything the driver needs to know about one kind of adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Human-readable name, as the UI shows it when the output is created.
    pub name: &'static str,
    /// How to find it on the bus.
    pub device: DeviceDescriptor,
    /// How to set the port up once it is open.
    pub port: PortConfig,
    /// How to time a frame on it.
    pub timing: DmxTiming,
    /// The frame rate this adapter is expected to sustain, in whole hertz.
    ///
    /// `ARCHITECTURE_SPEC.md` §7.1 and §3.2: for Open DMX USB this is *below*
    /// the engine's 44 Hz and that is a property of the hardware rather than a
    /// fault. The UI states it when the output is created rather than hiding
    /// it.
    pub expected_rate_hz: (u32, u32),
    /// Whether the two numbers above have been read off a real device.
    ///
    /// `false` until S8 measures them. Kept as a field rather than as a comment
    /// so that a UI, a log line or a later test can ask.
    pub verified: bool,
}

impl DeviceProfile {
    /// The adapter `ARCHITECTURE_SPEC.md` §7.1 makes mandatory for V1.
    ///
    /// The product ID and the rate range are **estimates**; `verified` says so.
    pub const SH_RS09B: Self = Self {
        name: "DSD TECH SH-RS09B",
        device: DeviceDescriptor {
            vendor_id: 0x0403,
            product_id: 0x6001,
            product: None,
            serial: None,
        },
        port: PortConfig::DMX512,
        timing: DmxTiming {
            break_time: Duration::from_micros(110),
            mark_after_break: Duration::from_micros(16),
            min_frame_interval: Duration::from_millis(25),
        },
        expected_rate_hz: (30, 40),
        verified: false,
    };
}

/// The DSD TECH SH-RS09B, at the name this crate refers to it by.
pub const SH_RS09B: DeviceProfile = DeviceProfile::SH_RS09B;

#[cfg(test)]
mod tests {
    use super::{AccessPath, DeviceDescriptor, SH_RS09B};
    use std::time::Duration;

    #[test]
    fn the_adapter_profile_is_the_table_in_the_specification() {
        // ARCHITECTURE_SPEC.md §7.1, rows "Device discovery" and "Frame".
        assert_eq!(SH_RS09B.name, "DSD TECH SH-RS09B");
        assert_eq!(SH_RS09B.device.vendor_id, 0x0403);
        assert_eq!(SH_RS09B.device.product_id, 0x6001);
        assert_eq!(SH_RS09B.timing.break_time, Duration::from_micros(110));
        assert_eq!(SH_RS09B.timing.mark_after_break, Duration::from_micros(16));
        assert_eq!(SH_RS09B.expected_rate_hz, (30, 40));
    }

    #[test]
    fn the_adapter_profile_still_says_it_is_unverified() {
        // PROGRESS.md §5 and ARCHITECTURE_SPEC.md §14: the product ID and the
        // rate are estimates until S8 puts the cable on a bench. This assertion
        // is the thing S8 has to come back and change, which is the point of
        // it: the claim "verified" cannot be made anywhere else.
        let shipped = SH_RS09B;
        assert!(
            !shipped.verified,
            "S8 measures the product ID and the frame rate; until it has, \
             nothing may claim they are known"
        );
    }

    #[test]
    fn the_break_is_longer_than_dmx512_demands_and_the_mark_after_break_too() {
        // 92 µs and 12 µs are the standard's minima; a delay taken by a
        // preemptible thread should not be sitting on them.
        assert!(SH_RS09B.timing.break_time >= Duration::from_micros(92));
        assert!(SH_RS09B.timing.mark_after_break >= Duration::from_micros(12));
    }

    #[test]
    fn the_output_cadence_is_slower_than_the_engine_tick() {
        // ARCHITECTURE_SPEC.md §3.2: 513 bytes at 250 kBaud is 22.6 ms of data
        // before break, mark and two USB control transfers are counted, so this
        // adapter cannot keep up with a 22.7 ms tick and is not expected to.
        assert!(SH_RS09B.timing.min_frame_interval > Duration::from_micros(22_727));
        let (low, high) = SH_RS09B.expected_rate_hz;
        assert!(low <= high);
        assert!(high < 44);
    }

    #[test]
    fn a_descriptor_without_a_serial_takes_the_first_matching_cable() {
        let any = SH_RS09B.device;
        assert!(any.matches(0x0403, 0x6001, Some("FT232R USB UART"), Some("A50285BI")));
        assert!(any.matches(0x0403, 0x6001, None, None));
        assert!(!any.matches(0x0403, 0x6015, None, None));
        assert!(!any.matches(0x1234, 0x6001, None, None));
    }

    #[test]
    fn a_descriptor_with_a_serial_takes_that_cable_and_no_other() {
        // Two adapters in two USB ports is the case this exists for: without it
        // universe 1 and universe 2 swap over when the machine reboots.
        let pinned = SH_RS09B.device.with_serial("A50285BI");
        assert!(pinned.matches(0x0403, 0x6001, None, Some("A50285BI")));
        assert!(!pinned.matches(0x0403, 0x6001, None, Some("B70000ZZ")));
        assert!(!pinned.matches(0x0403, 0x6001, None, None));
    }

    #[test]
    fn a_descriptor_can_require_a_product_string_too() {
        let named = DeviceDescriptor {
            product: Some("SH-RS09B"),
            ..SH_RS09B.device
        };
        assert!(named.matches(0x0403, 0x6001, Some("SH-RS09B"), None));
        assert!(!named.matches(0x0403, 0x6001, Some("FT232R USB UART"), None));
        assert!(!named.matches(0x0403, 0x6001, None, None));
    }

    #[test]
    fn a_descriptor_prints_the_way_a_usb_tool_does() {
        assert_eq!(SH_RS09B.device.to_string(), "0403:6001");
        assert_eq!(
            SH_RS09B.device.with_serial("A50285BI").to_string(),
            "0403:6001 serial A50285BI"
        );
    }

    #[test]
    fn the_access_path_follows_the_platform() {
        // ARCHITECTURE_SPEC.md §7.1: D2XX on Windows with the VCP as a
        // fallback, libftdi on Linux and never D2XX there. This is the only
        // place in the crate that asks what platform it is on.
        #[cfg(windows)]
        {
            assert_eq!(AccessPath::preferred(), AccessPath::D2xx);
            assert_eq!(AccessPath::fallback(), Some(AccessPath::Vcp));
        }
        #[cfg(not(windows))]
        {
            assert_eq!(AccessPath::preferred(), AccessPath::LibFtdi);
            assert_eq!(AccessPath::fallback(), None);
            assert_ne!(AccessPath::preferred(), AccessPath::D2xx);
        }
    }

    #[test]
    fn every_access_path_has_a_name_for_the_log() {
        assert_eq!(AccessPath::D2xx.to_string(), "D2XX");
        assert_eq!(AccessPath::Vcp.to_string(), "virtual COM port");
        assert_eq!(AccessPath::LibFtdi.to_string(), "libftdi");
    }
}
