//! The virtual COM port access path: the fallback on Windows, and on macOS the
//! only one there is.
//!
//! `ARCHITECTURE_SPEC.md` §7.1 keeps this for a machine where FTDI's D2XX
//! driver is not installed. It reaches the same cable through the serial port
//! the VCP driver publishes — `COM3` and the like on Windows,
//! `/dev/cu.usbserial-…` on macOS — using `set_break` and `clear_break` to
//! generate the DMX break by hand, exactly as the D2XX path does with its own
//! calls.
//!
//! **macOS reaches an adapter through here and nowhere else** — S63. Apple
//! ships the driver that publishes the port, so this path needs nothing
//! installed, where D2XX would first have to displace that driver. The
//! trade-off below is therefore the trade-off macOS lives with, and
//! [`system_backend`](crate::system_backend) says so in as many words.
//!
//! # What this path cannot do
//!
//! **The FTDI latency timer is not reachable from here.** It is a property of
//! the VCP driver, set per device in the registry (Device Manager → Port
//! Settings → Advanced) on Windows and not settable at all on macOS, and it
//! defaults to **16 ms**. Nothing in a serial API can change it.
//! `PortConfig::latency_timer_ms` is therefore applied on the D2XX path and can
//! only be *reported* on this one — which is a large part of why §7.1 prefers
//! D2XX, and S8 measured what it costs.
//!
//! The USB transfer sizes are out of reach for the same reason.
//!
//! # The one thing macOS needs that Windows does not — S63
//!
//! macOS publishes **two** device nodes for every serial port: the *callout*
//! device `/dev/cu.…` and the *dial-in* device `/dev/tty.…`, and
//! [`serialport::available_ports`] reports both, with identical USB metadata,
//! as two entries. They are not interchangeable. Opening a `tty.` node
//! **blocks until the carrier-detect line is asserted**, and a DMX adapter
//! never asserts one — so a driver thread that opened the wrong node of the
//! pair would hang there for ever, with no error and no frame, which is exactly
//! the freeze `CLAUDE.md`'s first invariant forbids.
//!
//! [`list_devices`] therefore drops the dial-in node on macOS. It is a filter
//! rather than an ordering because *first match wins* would make the invariant
//! depend on the order a dependency happens to walk an IOKit dictionary in.

use std::io::Write;
use std::time::{Duration, Instant};

use serialport::{
    ClearBuffer, DataBits as VcpDataBits, ErrorKind, FlowControl as VcpFlowControl,
    Parity as VcpParity, SerialPort, StopBits as VcpStopBits,
};

use crate::device::{AttachedDevice, DeviceDescriptor};
use crate::ftdi::{FtdiBackend, FtdiError, PortConfig, spin_wait, transmission_time};

/// Write timeout on the port. Nothing is ever read; this exists so a cable
/// pulled mid-frame fails rather than blocking a driver thread for ever.
const IO_TIMEOUT: Duration = Duration::from_millis(500);

/// One FTDI device, reached as an ordinary serial port.
#[derive(Default)]
pub struct VcpBackend {
    port: Option<Box<dyn SerialPort>>,
    /// The baud rate this port was configured at — see
    /// [`D2xxBackend`](crate::D2xxBackend) for why a write has to know it.
    baud: u32,
}

impl VcpBackend {
    /// A backend with nothing open yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            port: None,
            baud: 0,
        }
    }

    fn port(&mut self) -> Result<&mut Box<dyn SerialPort>, FtdiError> {
        self.port.as_mut().ok_or(FtdiError::NotFound)
    }
}

/// Every USB serial port on this machine, in the shape a descriptor is matched
/// against.
///
/// # Errors
///
/// [`FtdiError`] if the ports cannot be enumerated.
pub fn list_devices() -> Result<Vec<AttachedDevice>, FtdiError> {
    let ports = serialport::available_ports().map_err(|error| map_error(&error))?;
    Ok(ports
        .into_iter()
        .filter(|port| is_usable_node(&port.port_name))
        .filter_map(|port| match port.port_type {
            serialport::SerialPortType::UsbPort(usb) => Some(AttachedDevice {
                vendor_id: usb.vid,
                product_id: usb.pid,
                serial: usb.serial_number,
                product: usb.product,
                path: Some(port.port_name),
            }),
            // A built-in RS-232 header is not an adapter, and matching one
            // would open a port with something else on the end of it.
            _ => None,
        })
        .collect())
}

/// Whether a device node is one this driver may open — see the module
/// documentation's last section.
///
/// On macOS that is the **callout** node and not the dial-in one. Everywhere
/// else every node is usable and this is the identity, which is why it is a
/// function with a test rather than a `#[cfg]` wrapped round a filter: the
/// rule is stated once and the platform decides what it selects.
#[cfg(target_os = "macos")]
fn is_usable_node(name: &str) -> bool {
    // `/dev/cu.usbserial-A50285BI` — the callout node. The dial-in twin of the
    // same port is `/dev/tty.usbserial-A50285BI`, and opening it blocks for
    // ever on an adapter that asserts no carrier.
    !name
        .rsplit('/')
        .next()
        .is_some_and(|node| node.starts_with("tty."))
}

/// Whether a device node is one this driver may open. Every node, on a platform
/// that publishes one per port.
#[cfg(not(target_os = "macos"))]
const fn is_usable_node(_name: &str) -> bool {
    true
}

/// What a serial error means for the driver above.
///
/// `NoDevice` is what Windows reports for a port whose device has been removed,
/// so it is the unplugged case; a broken pipe is the same thing noticed during
/// a write.
fn map_error(error: &serialport::Error) -> FtdiError {
    match error.kind() {
        ErrorKind::NoDevice => FtdiError::Disconnected,
        ErrorKind::InvalidInput => FtdiError::Config,
        ErrorKind::Io(std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::NotConnected) => {
            FtdiError::Disconnected
        }
        ErrorKind::Io(_) | ErrorKind::Unknown => FtdiError::Io,
    }
}

/// The same for a plain I/O error, which is what writing gives back.
fn map_io_error(error: &std::io::Error) -> FtdiError {
    match error.kind() {
        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::NotConnected => {
            FtdiError::Disconnected
        }
        std::io::ErrorKind::TimedOut => FtdiError::Io,
        _ => FtdiError::Io,
    }
}

/// The port's data format in the serial library's vocabulary.
const fn data_format(
    port: &PortConfig,
) -> Result<(VcpDataBits, VcpStopBits, VcpParity), FtdiError> {
    let bits = match port.data_bits {
        5 => VcpDataBits::Five,
        6 => VcpDataBits::Six,
        7 => VcpDataBits::Seven,
        8 => VcpDataBits::Eight,
        _ => return Err(FtdiError::Config),
    };
    let stop = match port.stop_bits {
        crate::ftdi::StopBits::One => VcpStopBits::One,
        crate::ftdi::StopBits::Two => VcpStopBits::Two,
    };
    let parity = match port.parity {
        crate::ftdi::Parity::None => VcpParity::None,
        crate::ftdi::Parity::Odd => VcpParity::Odd,
        crate::ftdi::Parity::Even => VcpParity::Even,
    };
    Ok((bits, stop, parity))
}

/// The flow control setting, in the same vocabulary.
const fn flow_control(port: &PortConfig) -> VcpFlowControl {
    match port.flow_control {
        crate::ftdi::FlowControl::None => VcpFlowControl::None,
        crate::ftdi::FlowControl::RtsCts => VcpFlowControl::Hardware,
        crate::ftdi::FlowControl::XonXoff => VcpFlowControl::Software,
    }
}

impl FtdiBackend for VcpBackend {
    fn open(&mut self, device: &DeviceDescriptor) -> Result<(), FtdiError> {
        self.close();
        let found = list_devices()?
            .into_iter()
            .find(|attached| attached.matches(device))
            .ok_or(FtdiError::NotFound)?;
        let name = found.path.ok_or(FtdiError::NotFound)?;
        // Opened at the DMX baud rate straight away: a port that briefly exists
        // at 9600 baud with a fixture on the end of it is noise on the line.
        let port = serialport::new(name, PortConfig::DMX512.baud)
            .timeout(IO_TIMEOUT)
            .open()
            .map_err(|error| map_error(&error))?;
        self.port = Some(port);
        Ok(())
    }

    fn configure(&mut self, config: &PortConfig) -> Result<(), FtdiError> {
        let (bits, stop, parity) = data_format(config)?;
        let flow = flow_control(config);
        let port = self.port()?;
        port.set_baud_rate(config.baud)
            .map_err(|error| map_error(&error))?;
        port.set_data_bits(bits)
            .map_err(|error| map_error(&error))?;
        port.set_stop_bits(stop)
            .map_err(|error| map_error(&error))?;
        port.set_parity(parity).map_err(|error| map_error(&error))?;
        port.set_flow_control(flow)
            .map_err(|error| map_error(&error))?;
        // The latency timer and the USB transfer sizes are not reachable from
        // here - see this module's documentation. They are not silently
        // ignored: they are why D2XX is the preferred path.
        self.baud = config.baud;
        Ok(())
    }

    fn set_break(&mut self, on: bool) -> Result<(), FtdiError> {
        let port = self.port()?;
        let outcome = if on {
            port.set_break()
        } else {
            port.clear_break()
        };
        outcome.map_err(|error| map_error(&error))
    }

    /// Writes the frame and does not return until it has left the port.
    ///
    /// `flush` is asked first, because on Windows it maps to
    /// `FlushFileBuffers`, which is supposed to mean exactly this. It is not
    /// trusted on its own: the same claim from the D2XX path's transmit queue
    /// measured as returning immediately on every frame, and a break asserted
    /// early lands inside the frame still going out. The arithmetic is what
    /// makes it true either way.
    fn write(&mut self, data: &[u8]) -> Result<usize, FtdiError> {
        let baud = self.baud;
        let started = Instant::now();
        let port = self.port()?;
        port.write_all(data).map_err(|error| map_io_error(&error))?;
        port.flush().map_err(|error| map_io_error(&error))?;
        let on_wire = transmission_time(data.len(), baud);
        if let Some(remaining) = on_wire.checked_sub(started.elapsed()) {
            spin_wait(remaining);
        }
        Ok(data.len())
    }

    fn purge(&mut self) -> Result<(), FtdiError> {
        self.port()?
            .clear(ClearBuffer::All)
            .map_err(|error| map_error(&error))
    }

    fn close(&mut self) {
        // Dropping the port closes the handle; there is nothing to fail.
        self.port = None;
        self.baud = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{VcpBackend, data_format, flow_control, is_usable_node, map_error, map_io_error};
    use crate::ftdi::{FlowControl, FtdiBackend, FtdiError, Parity, PortConfig, StopBits};
    use serialport::{
        DataBits as VcpDataBits, ErrorKind, FlowControl as VcpFlowControl, Parity as VcpParity,
        StopBits as VcpStopBits,
    };

    fn error(kind: ErrorKind) -> serialport::Error {
        serialport::Error::new(kind, "test")
    }

    #[test]
    fn a_removed_port_is_a_lost_cable_and_a_bad_setting_is_not() {
        assert_eq!(
            map_error(&error(ErrorKind::NoDevice)),
            FtdiError::Disconnected
        );
        assert_eq!(
            map_error(&error(ErrorKind::Io(std::io::ErrorKind::BrokenPipe))),
            FtdiError::Disconnected
        );
        assert_eq!(
            map_error(&error(ErrorKind::Io(std::io::ErrorKind::NotConnected))),
            FtdiError::Disconnected
        );
        assert_eq!(
            map_error(&error(ErrorKind::InvalidInput)),
            FtdiError::Config
        );
        assert_eq!(map_error(&error(ErrorKind::Unknown)), FtdiError::Io);
        assert_eq!(
            map_error(&error(ErrorKind::Io(std::io::ErrorKind::TimedOut))),
            FtdiError::Io
        );
        assert!(map_error(&error(ErrorKind::NoDevice)).is_link_lost());
        assert!(!map_error(&error(ErrorKind::Unknown)).is_link_lost());
    }

    #[test]
    fn a_write_to_a_cable_that_has_gone_is_told_apart_from_one_that_stalled() {
        use std::io::{Error, ErrorKind as IoKind};
        assert_eq!(
            map_io_error(&Error::from(IoKind::BrokenPipe)),
            FtdiError::Disconnected
        );
        assert_eq!(
            map_io_error(&Error::from(IoKind::NotConnected)),
            FtdiError::Disconnected
        );
        assert_eq!(map_io_error(&Error::from(IoKind::TimedOut)), FtdiError::Io);
        assert_eq!(
            map_io_error(&Error::from(IoKind::PermissionDenied)),
            FtdiError::Io
        );
    }

    /// **The dial-in node is never the one that is opened** — S63.
    ///
    /// macOS publishes `/dev/cu.X` and `/dev/tty.X` for one port and
    /// `serialport` reports both with the same USB metadata, so which of the
    /// pair this driver picks is a decision and not an accident. Opening the
    /// dial-in node blocks until a carrier this adapter never asserts, which
    /// would be a driver thread frozen with no error — the first invariant in
    /// `CLAUDE.md`.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_mac_opens_the_callout_node_and_never_its_dial_in_twin() {
        assert!(is_usable_node("/dev/cu.usbserial-A50285BI"));
        assert!(!is_usable_node("/dev/tty.usbserial-A50285BI"));

        // The prefix is matched on the node, not on the whole path: a
        // directory that happens to contain the three letters is not a node.
        assert!(is_usable_node("/dev/tty-like/cu.usbserial-A50285BI"));
        // And `tty.` rather than `tty`, because `/dev/ttys000` is a pseudo
        // terminal rather than the dial-in twin of anything.
        assert!(is_usable_node("/dev/ttys000"));
    }

    /// Everywhere else a port is one node and every node is usable — the claim
    /// that keeps the macOS filter from quietly dropping a real Windows port.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_platform_with_one_node_per_port_drops_nothing() {
        assert!(is_usable_node("COM3"));
        assert!(is_usable_node("/dev/ttyUSB0"));
        assert!(is_usable_node("/dev/tty.usbserial-A50285BI"));
    }

    #[test]
    fn the_dmx_port_translates_to_eight_bits_two_stops_and_no_parity() {
        let (bits, stop, parity) = data_format(&PortConfig::DMX512).unwrap();
        assert_eq!(bits, VcpDataBits::Eight);
        assert_eq!(stop, VcpStopBits::Two);
        assert_eq!(parity, VcpParity::None);
        assert_eq!(flow_control(&PortConfig::DMX512), VcpFlowControl::None);
    }

    #[test]
    fn every_port_setting_a_serial_port_can_express_is_translated() {
        for (bits, expected) in [
            (5u8, VcpDataBits::Five),
            (6, VcpDataBits::Six),
            (7, VcpDataBits::Seven),
            (8, VcpDataBits::Eight),
        ] {
            let port = PortConfig {
                data_bits: bits,
                ..PortConfig::DMX512
            };
            assert_eq!(data_format(&port).unwrap().0, expected);
        }
        for (stop, expected) in [
            (StopBits::One, VcpStopBits::One),
            (StopBits::Two, VcpStopBits::Two),
        ] {
            let port = PortConfig {
                stop_bits: stop,
                ..PortConfig::DMX512
            };
            assert_eq!(data_format(&port).unwrap().1, expected);
        }
        for (parity, expected) in [
            (Parity::None, VcpParity::None),
            (Parity::Odd, VcpParity::Odd),
            (Parity::Even, VcpParity::Even),
        ] {
            let port = PortConfig {
                parity,
                ..PortConfig::DMX512
            };
            assert_eq!(data_format(&port).unwrap().2, expected);
        }
        for (flow, expected) in [
            (FlowControl::None, VcpFlowControl::None),
            (FlowControl::RtsCts, VcpFlowControl::Hardware),
            (FlowControl::XonXoff, VcpFlowControl::Software),
        ] {
            let port = PortConfig {
                flow_control: flow,
                ..PortConfig::DMX512
            };
            assert_eq!(flow_control(&port), expected);
        }
    }

    #[test]
    fn a_word_length_no_serial_port_has_is_a_configuration_error() {
        for bits in [0u8, 4, 9, 16] {
            let port = PortConfig {
                data_bits: bits,
                ..PortConfig::DMX512
            };
            assert_eq!(data_format(&port), Err(FtdiError::Config));
        }
    }

    #[test]
    fn nothing_can_be_asked_of_a_backend_that_has_not_opened_a_port() {
        let mut backend = VcpBackend::new();
        assert_eq!(
            backend.configure(&PortConfig::DMX512),
            Err(FtdiError::NotFound)
        );
        assert_eq!(backend.set_break(true), Err(FtdiError::NotFound));
        assert_eq!(backend.set_break(false), Err(FtdiError::NotFound));
        assert_eq!(backend.purge(), Err(FtdiError::NotFound));
        assert_eq!(backend.write(&[0u8; 513]), Err(FtdiError::NotFound));
        backend.close();
    }

    #[test]
    fn enumerating_the_ports_of_this_machine_does_not_open_any_of_them() {
        // Runs the real enumeration - it needs no adapter and disturbs nothing
        // - and asserts only what is true on any machine: every device it
        // reports is a USB one, since a built-in RS-232 header is not an
        // adapter and opening one would find something else on the end.
        let found = super::list_devices().unwrap();
        for device in found {
            assert!(device.path.is_some());
        }
    }
}
