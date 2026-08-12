//! The D2XX access path: FTDI's own driver, on Windows.
//!
//! `ARCHITECTURE_SPEC.md` §7.1 makes this the preferred path, and §10.1 makes
//! this file one of the few places allowed to know what platform it is on.
//! Everything above [`FtdiBackend`] is unaware of it.
//!
//! The library is statically linked (`libftd2xx/static`), so the daemon does
//! not depend on which `ftd2xx.dll` a school's machine happens to have. The
//! `unsafe` that any FFI needs lives inside that crate: `unsafe_code` is denied
//! across this workspace and S8 is not the session to start making exceptions.
//!
//! # Why a write here waits for the wire
//!
//! `FT_Write` hands bytes to the driver and returns; the transmission happens
//! afterwards. That is fine for a serial terminal and wrong for DMX, because
//! the next thing this driver does is assert a break — and a break is a USB
//! *control* transfer, which does not queue behind bulk data. It can therefore
//! land in the middle of the frame that is still going out. So the write here
//! does not return until the device's transmit queue has drained, which makes
//! the break land where it belongs and, incidentally, makes a measured frame
//! rate a measurement of the wire rather than of a buffer.

use std::time::{Duration, Instant};

use libftd2xx::{
    BitsPerWord, FtStatus, Ftdi, FtdiCommon, Parity as FtParity, StopBits as FtStopBits,
    TimeoutError,
};

use crate::device::{AttachedDevice, DeviceDescriptor};
use crate::ftdi::{FtdiBackend, FtdiError, PortConfig, spin_wait, transmission_time};

/// Added to the computed wire time before the next break may be asserted.
///
/// The computation says when the bytes *this* write started have gone; the
/// margin covers whatever the driver was still holding when it began. Measured
/// at about 2.5 ms on the bring-up machine, so 2 ms of margin on top of a
/// 22.6 ms frame is generous without being another millisecond of frame rate
/// thrown away — and once the wait is in place the buffer stops accumulating,
/// which is what makes the computation accurate in the first place.
const WIRE_MARGIN: Duration = Duration::from_millis(2);

/// Read and write timeouts on the port. Nothing is ever read; the write timeout
/// exists so that a cable pulled mid-frame fails rather than blocking a driver
/// thread for ever.
const IO_TIMEOUT: Duration = Duration::from_millis(500);

/// One FTDI device, reached through FTDI's own driver.
#[derive(Default)]
pub struct D2xxBackend {
    device: Option<Ftdi>,
    /// The baud rate this port was configured at, which is what turns a byte
    /// count into a duration. Zero until [`configure`](FtdiBackend::configure)
    /// has run, and a write before that has nothing to wait for.
    baud: u32,
}

impl D2xxBackend {
    /// A backend with nothing open yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            device: None,
            baud: 0,
        }
    }

    /// The open device, or the error for asking anything of a closed one.
    fn device(&mut self) -> Result<&mut Ftdi, FtdiError> {
        self.device.as_mut().ok_or(FtdiError::NotFound)
    }
}

/// Every FTDI device the driver can see.
///
/// # Errors
///
/// [`FtdiError`] if the device list cannot be read.
pub fn list_devices() -> Result<Vec<AttachedDevice>, FtdiError> {
    let devices = libftd2xx::list_devices().map_err(map_status)?;
    Ok(devices
        .into_iter()
        .map(|info| AttachedDevice {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            serial: non_empty(info.serial_number),
            product: non_empty(info.description),
            path: Some(format!("D2XX {:?}", info.device_type)),
        })
        .collect())
}

/// An FTDI string field, with "absent" and "empty" treated as the same thing:
/// D2XX reports an unprogrammed serial number as an empty string, and a
/// descriptor that required `Some("")` would match nothing.
fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

/// What an FTDI status means for the driver above.
///
/// Split the way [`FtdiError::is_link_lost`] is: the errors that mean the cable
/// has gone and the driver should reconnect, against the ones that mean this
/// frame did not go out. A device that has been unplugged answers
/// `INVALID_HANDLE` or `IO_ERROR` depending on when it is noticed, so both are
/// on the reconnect side.
const fn map_status(status: FtStatus) -> FtdiError {
    match status {
        FtStatus::DEVICE_NOT_FOUND | FtStatus::DEVICE_LIST_NOT_READY => FtdiError::NotFound,
        FtStatus::INVALID_HANDLE
        | FtStatus::DEVICE_NOT_OPENED
        | FtStatus::IO_ERROR
        | FtStatus::DEVICE_NOT_OPENED_FOR_WRITE
        | FtStatus::DEVICE_NOT_OPENED_FOR_ERASE
        | FtStatus::FAILED_TO_WRITE_DEVICE => FtdiError::Disconnected,
        FtStatus::INVALID_PARAMETER
        | FtStatus::INVALID_BAUD_RATE
        | FtStatus::INVALID_ARGS
        | FtStatus::NOT_SUPPORTED => FtdiError::Config,
        FtStatus::INSUFFICIENT_RESOURCES
        | FtStatus::OTHER_ERROR
        | FtStatus::EEPROM_READ_FAILED
        | FtStatus::EEPROM_WRITE_FAILED
        | FtStatus::EEPROM_ERASE_FAILED
        | FtStatus::EEPROM_NOT_PRESENT
        | FtStatus::EEPROM_NOT_PROGRAMMED => FtdiError::Io,
    }
}

/// The same, for the writing calls, which can also come back short.
const fn map_timeout(error: TimeoutError) -> FtdiError {
    match error {
        TimeoutError::FtStatus(status) => map_status(status),
        TimeoutError::Timeout { actual, expected } => FtdiError::ShortWrite {
            wrote: actual,
            expected,
        },
    }
}

/// The port's data format in the library's vocabulary.
///
/// A pure translation, and the one place where "8N2" stops being three fields
/// and becomes three calls. Anything the hardware cannot express is a
/// configuration error rather than a silently different port.
const fn data_format(port: &PortConfig) -> Result<(BitsPerWord, FtStopBits, FtParity), FtdiError> {
    let bits = match port.data_bits {
        7 => BitsPerWord::Bits7,
        8 => BitsPerWord::Bits8,
        _ => return Err(FtdiError::Config),
    };
    let stop = match port.stop_bits {
        crate::ftdi::StopBits::One => FtStopBits::Bits1,
        crate::ftdi::StopBits::Two => FtStopBits::Bits2,
    };
    let parity = match port.parity {
        crate::ftdi::Parity::None => FtParity::No,
        crate::ftdi::Parity::Odd => FtParity::Odd,
        crate::ftdi::Parity::Even => FtParity::Even,
    };
    Ok((bits, stop, parity))
}

/// A USB transfer size the driver will accept: 64 to 65536 bytes, in multiples
/// of 64.
///
/// Checked here rather than passed straight through because the library
/// *asserts* on a bad value, and a panic in a driver thread is the one thing
/// `CLAUDE.md` forbids outright. A frame has to fit in one transfer as well, or
/// the break timing is broken up by the USB stack instead of by us.
const fn transfer_size(port: &PortConfig) -> Result<u32, FtdiError> {
    let size = port.write_transfer_size;
    if size < 64 || size > 64 * 1024 || !size.is_multiple_of(64) {
        return Err(FtdiError::Config);
    }
    Ok(size)
}

impl FtdiBackend for D2xxBackend {
    fn open(&mut self, device: &DeviceDescriptor) -> Result<(), FtdiError> {
        self.close();
        let found = list_devices()?
            .into_iter()
            .find(|attached| attached.matches(device))
            .ok_or(FtdiError::NotFound)?;
        // By serial number, because that is what identifies one cable of
        // several. An adapter with no serial programmed is opened by position.
        let opened = match &found.serial {
            Some(serial) => Ftdi::with_serial_number(serial),
            None => Ftdi::with_index(0),
        };
        self.device = Some(opened.map_err(map_status)?);
        Ok(())
    }

    fn configure(&mut self, port: &PortConfig) -> Result<(), FtdiError> {
        let (bits, stop, parity) = data_format(port)?;
        let transfer = transfer_size(port)?;
        let latency = Duration::from_millis(u64::from(port.latency_timer_ms));
        let device = self.device()?;
        device.set_baud_rate(port.baud).map_err(map_status)?;
        device
            .set_data_characteristics(bits, stop, parity)
            .map_err(map_status)?;
        device.set_flow_control_none().map_err(map_status)?;
        // The 16 ms default is the one number in this file that would be fatal
        // on its own: a frame period at 44 Hz is 22.7 ms.
        device.set_latency_timer(latency).map_err(map_status)?;
        device.set_usb_parameters(transfer).map_err(map_status)?;
        device
            .set_timeouts(IO_TIMEOUT, IO_TIMEOUT)
            .map_err(map_status)?;
        // Remembered because it is what turns a frame's byte count into the
        // time that frame occupies the line.
        self.baud = port.baud;
        Ok(())
    }

    fn set_break(&mut self, on: bool) -> Result<(), FtdiError> {
        let device = self.device()?;
        if on {
            device.set_break_on().map_err(map_status)
        } else {
            device.set_break_off().map_err(map_status)
        }
    }

    /// Writes the frame and does not return until it has left the port.
    ///
    /// The obvious implementation — write, then poll `FT_GetStatus` until the
    /// transmit queue is empty — was written first and **measured to be
    /// worthless**: the queue reads zero on the very first poll of every single
    /// frame, while `FT_Write` itself returns after 20.0 ms of a frame that
    /// takes 22.6 ms to transmit. `FT_GetStatus` reports the driver's queue,
    /// not the chip's shift register, so there is nothing there to wait for.
    ///
    /// What is left is arithmetic, which does not lie: the bytes take
    /// [`transmission_time`] to leave and the break may not be asserted until
    /// they have.
    fn write(&mut self, data: &[u8]) -> Result<usize, FtdiError> {
        let baud = self.baud;
        let started = Instant::now();
        self.device()?.write_all(data).map_err(map_timeout)?;
        let on_wire = transmission_time(data.len(), baud) + WIRE_MARGIN;
        if let Some(remaining) = on_wire.checked_sub(started.elapsed()) {
            spin_wait(remaining);
        }
        Ok(data.len())
    }

    fn purge(&mut self) -> Result<(), FtdiError> {
        self.device()?.purge_all().map_err(map_status)
    }

    fn close(&mut self) {
        self.baud = 0;
        if let Some(mut device) = self.device.take() {
            // Nothing useful to do about a close that fails, and the handle is
            // gone either way.
            let _outcome = device.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{D2xxBackend, data_format, map_status, map_timeout, non_empty, transfer_size};
    use crate::ftdi::{FlowControl, FtdiBackend, FtdiError, Parity, PortConfig, StopBits};
    use libftd2xx::{
        BitsPerWord, FtStatus, Parity as FtParity, StopBits as FtStopBits, TimeoutError,
    };

    #[test]
    fn a_lost_cable_is_told_apart_from_a_refused_call() {
        // The mapping decides whether the runner reconnects or sends the next
        // frame, so it is the part of this file most worth getting right - and
        // the only part that can be checked without hardware.
        for status in [FtStatus::DEVICE_NOT_FOUND, FtStatus::DEVICE_LIST_NOT_READY] {
            assert_eq!(map_status(status), FtdiError::NotFound);
            assert!(map_status(status).is_link_lost());
        }
        for status in [
            FtStatus::INVALID_HANDLE,
            FtStatus::DEVICE_NOT_OPENED,
            FtStatus::IO_ERROR,
            FtStatus::DEVICE_NOT_OPENED_FOR_WRITE,
            FtStatus::DEVICE_NOT_OPENED_FOR_ERASE,
            FtStatus::FAILED_TO_WRITE_DEVICE,
        ] {
            assert_eq!(map_status(status), FtdiError::Disconnected);
            assert!(map_status(status).is_link_lost());
        }
        for status in [
            FtStatus::INVALID_PARAMETER,
            FtStatus::INVALID_BAUD_RATE,
            FtStatus::INVALID_ARGS,
            FtStatus::NOT_SUPPORTED,
        ] {
            assert_eq!(map_status(status), FtdiError::Config);
            assert!(!map_status(status).is_link_lost());
        }
        for status in [
            FtStatus::INSUFFICIENT_RESOURCES,
            FtStatus::OTHER_ERROR,
            FtStatus::EEPROM_READ_FAILED,
            FtStatus::EEPROM_WRITE_FAILED,
            FtStatus::EEPROM_ERASE_FAILED,
            FtStatus::EEPROM_NOT_PRESENT,
            FtStatus::EEPROM_NOT_PROGRAMMED,
        ] {
            assert_eq!(map_status(status), FtdiError::Io);
            assert!(!map_status(status).is_link_lost());
        }
    }

    #[test]
    fn a_write_that_moved_less_than_the_frame_keeps_both_numbers() {
        assert_eq!(
            map_timeout(TimeoutError::Timeout {
                actual: 300,
                expected: 513
            }),
            FtdiError::ShortWrite {
                wrote: 300,
                expected: 513
            }
        );
        assert_eq!(
            map_timeout(TimeoutError::FtStatus(FtStatus::IO_ERROR)),
            FtdiError::Disconnected
        );
    }

    #[test]
    fn the_dmx_port_translates_to_eight_bits_two_stops_and_no_parity() {
        let (bits, stop, parity) = data_format(&PortConfig::DMX512).unwrap();
        assert_eq!(u8::from(bits), u8::from(BitsPerWord::Bits8));
        assert_eq!(u8::from(stop), u8::from(FtStopBits::Bits2));
        assert_eq!(u8::from(parity), u8::from(FtParity::No));
    }

    #[test]
    fn every_port_setting_this_hardware_can_express_is_translated() {
        for (bits, expected) in [(7, BitsPerWord::Bits7), (8, BitsPerWord::Bits8)] {
            let port = PortConfig {
                data_bits: bits,
                ..PortConfig::DMX512
            };
            assert_eq!(u8::from(data_format(&port).unwrap().0), u8::from(expected));
        }
        for (stop, expected) in [
            (StopBits::One, FtStopBits::Bits1),
            (StopBits::Two, FtStopBits::Bits2),
        ] {
            let port = PortConfig {
                stop_bits: stop,
                ..PortConfig::DMX512
            };
            assert_eq!(u8::from(data_format(&port).unwrap().1), u8::from(expected));
        }
        for (parity, expected) in [
            (Parity::None, FtParity::No),
            (Parity::Odd, FtParity::Odd),
            (Parity::Even, FtParity::Even),
        ] {
            let port = PortConfig {
                parity,
                ..PortConfig::DMX512
            };
            assert_eq!(u8::from(data_format(&port).unwrap().2), u8::from(expected));
        }
    }

    #[test]
    fn a_port_this_hardware_cannot_express_is_a_configuration_error() {
        // Rather than a port quietly opened at some other word length, which
        // on a live DMX line is noise.
        for bits in [0u8, 5, 6, 9, 16] {
            let port = PortConfig {
                data_bits: bits,
                ..PortConfig::DMX512
            };
            assert_eq!(data_format(&port), Err(FtdiError::Config));
        }
    }

    #[test]
    fn a_transfer_size_the_driver_would_assert_on_is_refused_instead() {
        // The library asserts on a bad value, and a panic on a driver thread is
        // the one thing CLAUDE.md forbids outright.
        assert_eq!(transfer_size(&PortConfig::DMX512), Ok(4096));
        for size in [0u32, 32, 63, 100, 65_536 + 64, 1_000_000] {
            let port = PortConfig {
                write_transfer_size: size,
                ..PortConfig::DMX512
            };
            assert_eq!(transfer_size(&port), Err(FtdiError::Config));
        }
        for size in [64u32, 512, 4096, 65_536] {
            let port = PortConfig {
                write_transfer_size: size,
                ..PortConfig::DMX512
            };
            assert_eq!(transfer_size(&port), Ok(size));
        }
    }

    #[test]
    fn an_unprogrammed_string_is_absent_rather_than_empty() {
        // D2XX reports a serial number that was never programmed as "", and a
        // descriptor asking for `Some("")` would match nothing.
        assert_eq!(non_empty(String::new()), None);
        assert_eq!(
            non_empty("B0037HIY".to_owned()),
            Some("B0037HIY".to_owned())
        );
    }

    #[test]
    fn nothing_can_be_asked_of_a_backend_that_has_not_opened_a_cable() {
        let mut backend = D2xxBackend::new();
        assert_eq!(
            backend.configure(&PortConfig::DMX512),
            Err(FtdiError::NotFound)
        );
        assert_eq!(backend.set_break(true), Err(FtdiError::NotFound));
        assert_eq!(backend.set_break(false), Err(FtdiError::NotFound));
        assert_eq!(backend.purge(), Err(FtdiError::NotFound));
        assert_eq!(backend.write(&[0u8; 513]), Err(FtdiError::NotFound));
        // And closing one is not an error.
        backend.close();
    }

    #[test]
    fn the_port_it_would_configure_is_the_dmx_one() {
        // Guards the constant this file is built around: flow control off and
        // a latency timer of 1 ms rather than the 16 ms default.
        assert_eq!(PortConfig::DMX512.flow_control, FlowControl::None);
        assert_eq!(PortConfig::DMX512.latency_timer_ms, 1);
    }
}
