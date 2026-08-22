//! What is known about the cable, in one table.
//!
//! Two of the numbers below had never been read off a real device: the USB
//! product ID and the achievable frame rate. `PROGRESS.md` §5 and
//! `ARCHITECTURE_SPEC.md` §14 recorded them as open, and both said the same
//! thing about how they should be held — as plain data, so that verifying them
//! is an edit to a table rather than a refactor. That is what this module is,
//! and **S8 turned out the way that design hoped**: the whole verification was
//! three fields and one test.
//!
//! Measured on 2026-08-11 against the adapter itself, serial `B0037HIY`:
//! `0403:6001`, product string `FT232R USB UART`, and **35.5 Hz sustained over
//! sixty seconds** through D2XX — with the frame timing corrected, see
//! [`DmxTiming`]. The product ID matched the guess; the rate did not, and the
//! reason it did not is the most useful thing S8 found.

use core::fmt;
use std::borrow::Cow;
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
///
/// **Not `Copy` since S33**, and the serial is why: the output patch is
/// configuration now, so the serial that pins one cable comes off a
/// `machine.json` an operator wrote rather than out of a constant in this
/// crate. `Cow` is what lets [`SH_RS09B`] stay a `const` with a borrowed string
/// in it while a configured output carries an owned one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// USB vendor ID. FTDI is `0x0403`.
    pub vendor_id: u16,
    /// USB product ID. An FT232R is *typically* `0x6001` — unverified for this
    /// adapter, see the module documentation.
    pub product_id: u16,
    /// Product string to require, or `None` to accept any.
    pub product: Option<&'static str>,
    /// Serial number to require, or `None` to accept the first match.
    pub serial: Option<Cow<'static, str>>,
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
        let wanted_serial = self
            .serial
            .as_deref()
            .is_none_or(|want| serial == Some(want));
        wanted_product && wanted_serial
    }

    /// The same descriptor pinned to one physical cable by its serial number.
    ///
    /// Takes anything that can be a `Cow` so that the serial can come from a
    /// constant in this crate *or* from the machine configuration an operator
    /// wrote — S33's `OutputKind::OpenDmx { serial }` is the second.
    #[must_use]
    pub fn with_serial(self, serial: impl Into<Cow<'static, str>>) -> Self {
        Self {
            serial: Some(serial.into()),
            ..self
        }
    }
}

impl fmt::Display for DeviceDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor_id, self.product_id)?;
        if let Some(serial) = &self.serial {
            write!(f, " serial {serial}")?;
        }
        Ok(())
    }
}

/// A device found on the bus, as a backend reports it.
///
/// Platform-neutral on purpose: D2XX answers with an index and a description,
/// a COM port with a name, libftdi with a bus address. What a caller wants to
/// know is the same in all three cases, so the shape is the same and only the
/// `path` differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedDevice {
    /// USB vendor ID as read from the device.
    pub vendor_id: u16,
    /// USB product ID as read from the device.
    pub product_id: u16,
    /// Serial number string, if the device has one.
    pub serial: Option<String>,
    /// Product or description string, if the device has one.
    pub product: Option<String>,
    /// How this platform names the device — a COM port, a `/dev` node, a D2XX
    /// index. For display and for logs; matching goes by the fields above.
    pub path: Option<String>,
}

impl AttachedDevice {
    /// Whether this device is the one a descriptor asks for.
    #[must_use]
    pub fn matches(&self, descriptor: &DeviceDescriptor) -> bool {
        descriptor.matches(
            self.vendor_id,
            self.product_id,
            self.product.as_deref(),
            self.serial.as_deref(),
        )
    }
}

impl fmt::Display for AttachedDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor_id, self.product_id)?;
        if let Some(serial) = &self.serial {
            write!(f, " serial {serial}")?;
        }
        if let Some(product) = &self.product {
            write!(f, " \"{product}\"")?;
        }
        if let Some(path) = &self.path {
            write!(f, " at {path}")?;
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
    ///
    /// **Measured, not chosen.** A frame on this adapter costs 22.6 ms of data,
    /// about 3 ms for the two USB control transfers that make the break, and
    /// the safety margin the D2XX backend adds before it lets the next break
    /// through. That comes to 28 ms, and the sustained rate over a minute is
    /// 35.5 Hz.
    pub min_frame_interval: Duration,
}

/// Everything the driver needs to know about one kind of adapter.
///
/// **Not `Copy` since S33** — see [`DeviceDescriptor`] for why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Human-readable name, as the UI shows it when the output is created.
    pub name: &'static str,
    /// How to find it on the bus.
    pub device: DeviceDescriptor,
    /// How to set the port up once it is open.
    pub port: PortConfig,
    /// How to time a frame on it.
    pub timing: DmxTiming,
    /// The frame rate this adapter sustains, in whole hertz, as the two access
    /// paths measured it.
    ///
    /// `ARCHITECTURE_SPEC.md` §7.1 and §3.2: for Open DMX USB this is *below*
    /// the engine's 44 Hz and that is a property of the hardware rather than a
    /// fault. The UI states it when the output is created rather than hiding
    /// it.
    pub expected_rate_hz: (u32, u32),
    /// Whether the numbers above have been read off a real device.
    ///
    /// `true` since S8 measured them. Kept as a field rather than as a comment
    /// so that a UI, a log line or a later test can ask — a profile for an
    /// adapter nobody has held should be able to say so.
    pub verified: bool,
}

impl DeviceProfile {
    /// The adapter `ARCHITECTURE_SPEC.md` §7.1 makes mandatory for V1,
    /// **as measured on 2026-08-11** (S8).
    ///
    /// The strings are deliberately left as "any": `FT232R USB UART` is what
    /// FTDI's own chip reports, not something DSD TECH programmed, so
    /// requiring it would match every FT232R in the world and exclude a cable
    /// somebody has relabelled. The serial number is what tells two adapters
    /// apart, and it belongs to the cable rather than to the model — see
    /// [`DeviceDescriptor::with_serial`].
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
            min_frame_interval: Duration::from_millis(28),
        },
        expected_rate_hz: (35, 38),
        verified: true,
    };
}

/// The DSD TECH SH-RS09B, at the name this crate refers to it by.
pub const SH_RS09B: DeviceProfile = DeviceProfile::SH_RS09B;

#[cfg(test)]
mod tests {
    use super::{AccessPath, AttachedDevice, DeviceDescriptor, SH_RS09B};
    use std::time::Duration;

    #[test]
    fn the_adapter_profile_is_what_the_adapter_reported() {
        // Read off the device on 2026-08-11 (S8), serial B0037HIY, and not
        // copied from ARCHITECTURE_SPEC.md §7.1 - which guessed the product ID
        // right and the frame rate wrong.
        assert_eq!(SH_RS09B.name, "DSD TECH SH-RS09B");
        assert_eq!(SH_RS09B.device.vendor_id, 0x0403);
        assert_eq!(SH_RS09B.device.product_id, 0x6001);
        assert_eq!(SH_RS09B.timing.break_time, Duration::from_micros(110));
        assert_eq!(SH_RS09B.timing.mark_after_break, Duration::from_micros(16));
    }

    #[test]
    fn the_adapter_profile_carries_measurements_rather_than_estimates() {
        // The assertion S7 left behind said the opposite, on purpose: nothing
        // could claim these were known until somebody had the cable. This is
        // the same guard from the other side - 35.5 Hz sustained over sixty
        // seconds through D2XX and 38.4 Hz through the virtual COM port, so
        // the recorded band is 35 to 38 and it is a measurement.
        let shipped = SH_RS09B;
        assert!(shipped.verified);
        assert_eq!(shipped.expected_rate_hz, (35, 38));
        assert_eq!(shipped.timing.min_frame_interval, Duration::from_millis(28));
    }

    #[test]
    fn the_recorded_rate_and_the_recorded_frame_interval_agree() {
        // Two numbers for the same fact, so they are checked against each
        // other: 28 ms between frame starts is 35.7 Hz, which has to land
        // inside the band the profile claims.
        let (low, high) = SH_RS09B.expected_rate_hz;
        let interval = SH_RS09B.timing.min_frame_interval.as_secs_f64();
        let implied = 1.0 / interval;
        assert!(
            implied >= f64::from(low) - 1.0 && implied <= f64::from(high) + 1.0,
            "a {interval:?} interval implies {implied:.1} Hz, outside {low}..={high}"
        );
    }

    #[test]
    fn the_profile_matches_any_adapter_of_this_type_and_not_one_cable() {
        // The strings the chip reports are FTDI's, not DSD TECH's: requiring
        // "FT232R USB UART" would match every FT232R ever made and exclude a
        // relabelled one. Pinning a particular cable is `with_serial`, and it
        // is the operator's decision rather than the profile's.
        assert_eq!(SH_RS09B.device.product, None);
        assert_eq!(SH_RS09B.device.serial, None);
        assert!(
            SH_RS09B
                .device
                .matches(0x0403, 0x6001, Some("FT232R USB UART"), Some("B0037HIY"))
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
    fn an_attached_device_is_matched_against_the_descriptor() {
        let found = AttachedDevice {
            vendor_id: 0x0403,
            product_id: 0x6001,
            serial: Some("B0037HIY".to_owned()),
            product: Some("FT232R USB UART".to_owned()),
            path: Some("COM3".to_owned()),
        };
        assert!(found.matches(&SH_RS09B.device));
        assert!(found.matches(&SH_RS09B.device.with_serial("B0037HIY")));
        assert!(!found.matches(&SH_RS09B.device.with_serial("OTHER")));

        let other = AttachedDevice {
            product_id: 0x6015,
            ..found.clone()
        };
        assert!(!other.matches(&SH_RS09B.device));
        // A device with no strings at all still matches a descriptor that asks
        // for none, which is how a first bring-up finds an adapter.
        let bare = AttachedDevice {
            serial: None,
            product: None,
            path: None,
            ..found
        };
        assert!(bare.matches(&SH_RS09B.device));
    }

    #[test]
    fn an_attached_device_prints_everything_that_identifies_it() {
        let found = AttachedDevice {
            vendor_id: 0x0403,
            product_id: 0x6001,
            serial: Some("B0037HIY".to_owned()),
            product: Some("FT232R USB UART".to_owned()),
            path: Some("COM3".to_owned()),
        };
        assert_eq!(
            found.to_string(),
            "0403:6001 serial B0037HIY \"FT232R USB UART\" at COM3"
        );
        assert_eq!(
            AttachedDevice {
                serial: None,
                product: None,
                path: None,
                ..found
            }
            .to_string(),
            "0403:6001"
        );
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
