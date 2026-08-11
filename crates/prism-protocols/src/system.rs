//! Getting at a real cable: which access path, and what to do when the
//! preferred one is not there.
//!
//! `ARCHITECTURE_SPEC.md` §7.1 gives Windows two ways to reach an FT232R —
//! **D2XX** first, the **virtual COM port** as a fallback for a machine where
//! FTDI's own driver is not installed — and Linux exactly one, libftdi. That is
//! a policy, and a policy is testable: [`FallbackFtdi`] is generic over the two
//! backends, so "try the preferred one, fall back to the other, remember which
//! answered" is checked against two mocks with no cable in the building.
//!
//! What is *not* testable without hardware is each backend's handful of library
//! calls. Those live in [`d2xx`](crate::d2xx) and [`vcp`](crate::vcp), are kept
//! as thin as they can be, and are exercised by `tests/hardware.rs` — which is
//! `#[ignore]`d, because `CLAUDE.md` does not allow a test to require a device.

use crate::device::{AccessPath, AttachedDevice, DeviceDescriptor};
use crate::ftdi::{FtdiBackend, FtdiError, PortConfig};

/// Two ways of reaching the same cable, tried in order.
///
/// Holds whichever one answered, so a driver that reconnects after an unplug
/// goes through the same choice again rather than assuming the machine has not
/// changed — a laptop that had FTDI's driver installed while the show was
/// running is a case worth surviving.
pub struct FallbackFtdi<P: FtdiBackend, F: FtdiBackend> {
    preferred: (AccessPath, P),
    fallback: Option<(AccessPath, F)>,
    in_use: Option<AccessPath>,
}

impl<P: FtdiBackend, F: FtdiBackend> FallbackFtdi<P, F> {
    /// A cable reached through `preferred`, or through `fallback` if the
    /// preferred path cannot open it.
    pub const fn new(preferred: (AccessPath, P), fallback: Option<(AccessPath, F)>) -> Self {
        Self {
            preferred,
            fallback,
            in_use: None,
        }
    }

    /// Which path is open, if either. This is the answer S8 was asked to
    /// record, and it is a property of the machine rather than of the code.
    #[must_use]
    pub const fn in_use(&self) -> Option<AccessPath> {
        self.in_use
    }

    /// The backend that is open, if one is.
    fn active(&mut self) -> Option<&mut (dyn FtdiBackend + '_)> {
        let path = self.in_use?;
        if path == self.preferred.0 {
            return Some(&mut self.preferred.1);
        }
        match &mut self.fallback {
            Some((fallback_path, backend)) if *fallback_path == path => Some(backend),
            _ => None,
        }
    }

    /// The backend that is open, or the error a caller gets for asking
    /// anything of a cable that is not.
    fn require_active(&mut self) -> Result<&mut (dyn FtdiBackend + '_), FtdiError> {
        self.active().ok_or(FtdiError::NotFound)
    }
}

impl<P: FtdiBackend, F: FtdiBackend> FtdiBackend for FallbackFtdi<P, F> {
    /// Tries the preferred path, then the fallback.
    ///
    /// If both fail, the **preferred** path's error is the one reported: it is
    /// the path the machine is expected to have, so its reason is the one worth
    /// putting in front of somebody.
    fn open(&mut self, device: &DeviceDescriptor) -> Result<(), FtdiError> {
        self.in_use = None;
        let preferred = self.preferred.1.open(device);
        if preferred.is_ok() {
            self.in_use = Some(self.preferred.0);
            return Ok(());
        }
        let opened = match &mut self.fallback {
            Some((path, backend)) => backend.open(device).ok().map(|()| *path),
            None => None,
        };
        opened.map_or(preferred, |path| {
            self.in_use = Some(path);
            Ok(())
        })
    }

    fn configure(&mut self, port: &PortConfig) -> Result<(), FtdiError> {
        self.require_active()?.configure(port)
    }

    fn set_break(&mut self, on: bool) -> Result<(), FtdiError> {
        self.require_active()?.set_break(on)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, FtdiError> {
        self.require_active()?.write(data)
    }

    fn purge(&mut self) -> Result<(), FtdiError> {
        self.require_active()?.purge()
    }

    fn close(&mut self) {
        if let Some(active) = self.active() {
            active.close();
        }
        self.in_use = None;
    }

    fn wait(&mut self, duration: std::time::Duration) {
        match self.active() {
            Some(active) => active.wait(duration),
            // Waiting is not I/O: a closed cable still has to hold the break
            // for as long as the frame says.
            None => crate::ftdi::spin_wait(duration),
        }
    }
}

/// A cable on a platform that has no backend for one yet.
///
/// Answers [`FtdiError::Unsupported`] to everything. It exists so that
/// `prism-protocols` compiles, links and behaves the same everywhere — a Linux
/// build gets a driver that says clearly why it cannot send, rather than a
/// crate that will not build or a `todo!()` that takes the process down.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedBackend;

impl FtdiBackend for UnsupportedBackend {
    fn open(&mut self, _device: &DeviceDescriptor) -> Result<(), FtdiError> {
        Err(FtdiError::Unsupported)
    }

    fn configure(&mut self, _port: &PortConfig) -> Result<(), FtdiError> {
        Err(FtdiError::Unsupported)
    }

    fn set_break(&mut self, _on: bool) -> Result<(), FtdiError> {
        Err(FtdiError::Unsupported)
    }

    fn write(&mut self, _data: &[u8]) -> Result<usize, FtdiError> {
        Err(FtdiError::Unsupported)
    }

    fn purge(&mut self) -> Result<(), FtdiError> {
        Err(FtdiError::Unsupported)
    }

    fn close(&mut self) {}
}

/// The cable this platform reaches an adapter through.
///
/// Windows: D2XX, falling back to the virtual COM port. Anywhere else:
/// [`UnsupportedBackend`], until a machine exists to verify a libftdi backend
/// on — `ARCHITECTURE_SPEC.md` §7.1 names the path and D10 names the machine.
#[must_use]
pub fn system_backend() -> Box<dyn FtdiBackend> {
    #[cfg(windows)]
    {
        Box::new(FallbackFtdi::new(
            (AccessPath::D2xx, crate::d2xx::D2xxBackend::new()),
            Some((AccessPath::Vcp, crate::vcp::VcpBackend::new())),
        ))
    }
    #[cfg(not(windows))]
    {
        Box::new(UnsupportedBackend)
    }
}

/// Every FTDI device attached to this machine.
///
/// The bring-up question — "is the adapter there, and what does it call
/// itself?" — asked in a way that does not need a driver to be open. Reports
/// through the preferred access path only; the fallback sees the same devices.
///
/// # Errors
///
/// [`FtdiError`] if the bus cannot be enumerated, and
/// [`FtdiError::Unsupported`] on a platform with no backend.
pub fn list_devices() -> Result<Vec<AttachedDevice>, FtdiError> {
    #[cfg(windows)]
    {
        crate::d2xx::list_devices()
    }
    #[cfg(not(windows))]
    {
        Err(FtdiError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::{FallbackFtdi, UnsupportedBackend, system_backend};
    use crate::device::{AccessPath, DeviceDescriptor, SH_RS09B};
    use crate::ftdi::{FtdiBackend, FtdiCall, FtdiError, MockFtdi, MockFtdiHandle, PortConfig};
    use std::time::{Duration, Instant};

    /// A pair of mock cables behind the same policy the real machine uses.
    fn pair() -> (
        FallbackFtdi<MockFtdi, MockFtdi>,
        MockFtdiHandle,
        MockFtdiHandle,
    ) {
        let preferred = MockFtdi::new();
        let fallback = MockFtdi::new();
        let first = preferred.handle();
        let second = fallback.handle();
        (
            FallbackFtdi::new(
                (AccessPath::D2xx, preferred),
                Some((AccessPath::Vcp, fallback)),
            ),
            first,
            second,
        )
    }

    #[test]
    fn the_preferred_path_is_the_one_that_is_used_when_it_works() {
        let (mut cable, first, second) = pair();
        assert_eq!(cable.in_use(), None);
        cable.open(&SH_RS09B.device).unwrap();
        assert_eq!(cable.in_use(), Some(AccessPath::D2xx));

        cable.configure(&PortConfig::DMX512).unwrap();
        cable.purge().unwrap();
        assert_eq!(cable.write(&[0u8; 513]), Ok(513));
        assert_eq!(
            first.calls(),
            vec![
                FtdiCall::Open(SH_RS09B.device),
                FtdiCall::Configure(PortConfig::DMX512),
                FtdiCall::Purge,
                FtdiCall::Write(vec![0u8; 513]),
            ]
        );
        // The fallback was never touched: it was only asked to open.
        assert!(second.calls().is_empty());
    }

    #[test]
    fn the_fallback_is_used_when_the_preferred_path_cannot_open_the_cable() {
        // The machine that has no D2XX driver installed, which is the case
        // ARCHITECTURE_SPEC.md §7.1 introduces the fallback for.
        let (mut cable, first, second) = pair();
        first.fail_open(1, FtdiError::NotFound);
        cable.open(&SH_RS09B.device).unwrap();
        assert_eq!(cable.in_use(), Some(AccessPath::Vcp));

        cable.set_break(true).unwrap();
        cable.wait(Duration::from_micros(110));
        cable.set_break(false).unwrap();
        assert_eq!(cable.write(&[0u8; 513]), Ok(513));
        assert_eq!(
            second.calls(),
            vec![
                FtdiCall::Open(SH_RS09B.device),
                FtdiCall::SetBreakOn,
                FtdiCall::Wait(Duration::from_micros(110)),
                FtdiCall::SetBreakOff,
                FtdiCall::Write(vec![0u8; 513]),
            ]
        );
        assert_eq!(first.calls(), vec![FtdiCall::Open(SH_RS09B.device)]);
    }

    #[test]
    fn when_both_paths_fail_the_preferred_one_gives_the_reason() {
        let (mut cable, first, second) = pair();
        first.fail_open(1, FtdiError::Io);
        second.fail_open(1, FtdiError::NotFound);
        assert_eq!(cable.open(&SH_RS09B.device), Err(FtdiError::Io));
        assert_eq!(cable.in_use(), None);
        // And with no cable open, nothing else is attempted.
        assert_eq!(
            cable.configure(&PortConfig::DMX512),
            Err(FtdiError::NotFound)
        );
        assert_eq!(cable.set_break(true), Err(FtdiError::NotFound));
        assert_eq!(cable.purge(), Err(FtdiError::NotFound));
        assert_eq!(cable.write(&[0]), Err(FtdiError::NotFound));
    }

    #[test]
    fn with_no_fallback_configured_the_preferred_path_is_all_there_is() {
        let preferred = MockFtdi::new();
        let handle = preferred.handle();
        let mut cable: FallbackFtdi<MockFtdi, MockFtdi> =
            FallbackFtdi::new((AccessPath::LibFtdi, preferred), None);
        handle.fail_open(1, FtdiError::NotFound);
        assert_eq!(cable.open(&SH_RS09B.device), Err(FtdiError::NotFound));

        cable.open(&SH_RS09B.device).unwrap();
        assert_eq!(cable.in_use(), Some(AccessPath::LibFtdi));
    }

    #[test]
    fn closing_the_cable_forgets_which_path_it_was_on() {
        // A reconnect after an unplug goes through the choice again: a machine
        // can gain an FTDI driver between one frame and the next.
        let (mut cable, first, second) = pair();
        first.fail_open(1, FtdiError::NotFound);
        cable.open(&SH_RS09B.device).unwrap();
        assert_eq!(cable.in_use(), Some(AccessPath::Vcp));
        cable.close();
        assert_eq!(cable.in_use(), None);
        assert_eq!(second.calls().last(), Some(&FtdiCall::Close));

        cable.open(&SH_RS09B.device).unwrap();
        assert_eq!(cable.in_use(), Some(AccessPath::D2xx));
        // Closing a cable that is not open is not an error and does nothing.
        cable.close();
        cable.close();
        assert_eq!(first.calls().last(), Some(&FtdiCall::Close));
    }

    #[test]
    fn a_closed_cable_still_holds_the_break_for_the_time_it_was_asked_to() {
        // The wait is timing, not I/O. A driver whose delay silently vanished
        // when the cable dropped would send a frame with no break in it the
        // moment the cable came back.
        let (mut cable, _, _) = pair();
        let asked = Duration::from_micros(400);
        let start = Instant::now();
        cable.wait(asked);
        assert!(start.elapsed() >= asked);
    }

    #[test]
    fn a_platform_with_no_backend_says_so_rather_than_pretending() {
        let mut cable = UnsupportedBackend;
        assert_eq!(cable.open(&SH_RS09B.device), Err(FtdiError::Unsupported));
        assert_eq!(
            cable.configure(&PortConfig::DMX512),
            Err(FtdiError::Unsupported)
        );
        assert_eq!(cable.set_break(true), Err(FtdiError::Unsupported));
        assert_eq!(cable.purge(), Err(FtdiError::Unsupported));
        assert_eq!(cable.write(&[0]), Err(FtdiError::Unsupported));
        cable.close();
        // And it is not a lost cable, so a runner does not sit reconnecting to
        // a platform that will never have one.
        assert!(!FtdiError::Unsupported.is_link_lost());
    }

    #[test]
    fn the_system_backend_exists_on_every_platform() {
        // On Windows it is D2XX with the COM port behind it; elsewhere it is
        // the one that says `Unsupported`. Either way the crate builds, links
        // and answers.
        //
        // Asked for a device that cannot exist, so this runs the real
        // enumeration without opening anything: a unit test must neither need
        // an adapter nor interrupt one that is lighting a room.
        let nothing = DeviceDescriptor {
            vendor_id: 0xFFFF,
            product_id: 0xFFFF,
            product: None,
            serial: None,
        };
        let mut cable = system_backend();
        let outcome = cable.open(&nothing);
        #[cfg(not(windows))]
        assert_eq!(outcome, Err(FtdiError::Unsupported));
        #[cfg(windows)]
        assert_eq!(outcome, Err(FtdiError::NotFound));
        cable.close();
    }
}
