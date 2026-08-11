//! The seam between a network output and a real UDP socket.
//!
//! The same shape as [`FtdiBackend`](crate::FtdiBackend) one protocol along:
//! everything that is *the network* lives behind [`UdpSender`], and everything
//! that is Art-Net or sACN lives above it. What that buys is the thing S8 paid
//! for the hard way — the packet a driver builds can be asserted byte for byte
//! with nothing plugged in, because the byte-building never had to know whether
//! there was a socket underneath it.
//!
//! Unlike the FTDI seam, this one has a second way to be checked that costs
//! nothing: a UDP socket bound to `127.0.0.1` is a real socket, and a datagram
//! sent to one is a real capture. [`SystemUdp`] is therefore exercised by the
//! ordinary test suite rather than only on a machine with hardware attached —
//! see `tests/artnet_wire.rs`. [`MockUdp`] exists for the faults a loopback
//! socket will not produce on demand: a network that has gone away, a refused
//! datagram, a send that moved fewer bytes than it was given.
//!
//! # Why `bind` takes the broadcast flag
//!
//! `ARCHITECTURE_SPEC.md` §7.2 makes broadcast opt-in, and an operating system
//! refuses a broadcast datagram on a socket that has not asked for permission.
//! Putting the flag on `bind` rather than leaving it to the caller means the
//! permission is granted in exactly one place, and that a test can assert the
//! default configuration never asks for it.
//!
//! # Why multicast is one method and not a group membership
//!
//! sACN sends to `239.255.x.x` (`ARCHITECTURE_SPEC.md` §7.2), and a *sender*
//! needs none of what multicast usually implies: joining a group is how a
//! receiver asks to be given datagrams, and this seam has no receive. What a
//! sender does need is the hop limit — [`UdpSender::set_multicast_ttl`] — which
//! defaults to 1 on every platform and therefore confines the show to the local
//! segment unless somebody says otherwise. The outgoing interface is chosen the
//! way it already was: by the address passed to [`UdpSender::bind`].

use core::fmt;
use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Why a datagram did not go out.
///
/// Split by what the driver has to *do* about it, exactly as
/// [`FtdiError`](crate::FtdiError) is: [`is_link_lost`](Self::is_link_lost)
/// separates "the network is not there, reconnect on the backoff" from "this
/// datagram was refused, try the next frame".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpError {
    /// The local address could not be taken: another process holds it, or the
    /// interface it names is not on this machine.
    Bind,
    /// A datagram was offered to a socket that is not open. A driver mistake
    /// rather than a network fault, and treated as a lost link so the runner
    /// reopens the socket instead of sending into nothing for the rest of the
    /// show.
    NotBound,
    /// The network is not reachable: the cable is out, the Wi-Fi has dropped,
    /// or the route to the node has gone. The field failure `ARCHITECTURE_SPEC.md`
    /// §7 asks every driver to survive.
    Unreachable,
    /// The socket is there and refused this datagram.
    Io,
    /// The send moved fewer bytes than the datagram holds, which is a truncated
    /// packet rather than a partial success — the same reasoning as
    /// [`FtdiError::ShortWrite`](crate::FtdiError::ShortWrite): a receiver
    /// would apply half a universe and the output would still look green.
    ShortSend {
        /// Bytes the socket accepted.
        sent: usize,
        /// Bytes the datagram actually holds.
        expected: usize,
    },
}

impl UdpError {
    /// Whether the socket should be closed and reopened rather than used for
    /// another datagram.
    #[must_use]
    pub const fn is_link_lost(self) -> bool {
        matches!(self, Self::Bind | Self::NotBound | Self::Unreachable)
    }
}

impl fmt::Display for UdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind => write!(f, "the local address could not be bound"),
            Self::NotBound => write!(f, "the socket is not open"),
            Self::Unreachable => write!(f, "the network is not reachable"),
            Self::Io => write!(f, "the socket refused the datagram"),
            Self::ShortSend { sent, expected } => {
                write!(f, "the socket took {sent} of {expected} bytes")
            }
        }
    }
}

impl std::error::Error for UdpError {}

/// What an operating system error means for a DMX output.
///
/// A pure function so the classification is a table test rather than something
/// that can only be observed by unplugging a network cable. The four kinds on
/// the left are all "the packet never reached a wire"; everything else is a
/// socket that is present and unhappy, which costs one frame.
#[must_use]
pub fn classify(kind: io::ErrorKind) -> UdpError {
    match kind {
        io::ErrorKind::NetworkUnreachable
        | io::ErrorKind::HostUnreachable
        | io::ErrorKind::NetworkDown
        | io::ErrorKind::AddrNotAvailable => UdpError::Unreachable,
        _ => UdpError::Io,
    }
}

/// One UDP socket, as a network DMX output needs to use it.
///
/// Deliberately small, for the same reason [`FtdiBackend`](crate::FtdiBackend)
/// is: Art-Net output is transmit-only, so there is no receive here and no
/// discovery. `ArtPoll` and node discovery are a later session's problem and
/// will need their own seam rather than a `recv` bolted onto this one.
pub trait UdpSender: Send {
    /// Opens a socket on `local`, asking for broadcast permission if
    /// `broadcast` is set.
    ///
    /// Called again on every reconnect, so it must be safe on a sender that is
    /// already open.
    ///
    /// # Errors
    ///
    /// [`UdpError::Bind`] if the address cannot be taken.
    fn bind(&mut self, local: SocketAddr, broadcast: bool) -> Result<(), UdpError>;

    /// Sends one datagram and answers how many bytes went.
    ///
    /// # Errors
    ///
    /// [`UdpError`] as the operating system reports it, classified by
    /// [`classify`].
    fn send_to(&mut self, datagram: &[u8], target: SocketAddr) -> Result<usize, UdpError>;

    /// Sets how many hops a multicast datagram from this socket may take.
    ///
    /// Transmit-side only, for the reason in this module's documentation. The
    /// operating system's default is 1, which is right for a lighting network
    /// that is one switch and wrong for a venue whose nodes are behind a
    /// router — so it is asked for explicitly rather than inherited.
    ///
    /// # Errors
    ///
    /// [`UdpError::NotBound`] if there is no socket yet, or whatever the
    /// operating system says, classified by [`classify`].
    fn set_multicast_ttl(&mut self, ttl: u32) -> Result<(), UdpError>;

    /// The address datagrams are leaving from, once there is a socket.
    ///
    /// Worth having beyond tests: an output bound to the wrong interface is
    /// invisible on the network and perfectly healthy from here, so the daemon
    /// can show an operator where the packets are actually coming from.
    fn local_addr(&self) -> Option<SocketAddr>;

    /// Closes the socket. Infallible: it runs on the error path and on
    /// shutdown, where there is nothing to do about a failure.
    fn close(&mut self);
}

/// A boxed sender is a sender.
///
/// The same reason [`FtdiBackend`](crate::FtdiBackend) has one: it lets an
/// output hold a socket chosen at run time — a real one in the field, a
/// recording one under `ARCHITECTURE_SPEC.md` §12's mock-output mode — without
/// the choice having to be made at compile time.
impl<S: UdpSender + ?Sized> UdpSender for Box<S> {
    fn bind(&mut self, local: SocketAddr, broadcast: bool) -> Result<(), UdpError> {
        (**self).bind(local, broadcast)
    }

    fn send_to(&mut self, datagram: &[u8], target: SocketAddr) -> Result<usize, UdpError> {
        (**self).send_to(datagram, target)
    }

    fn set_multicast_ttl(&mut self, ttl: u32) -> Result<(), UdpError> {
        (**self).set_multicast_ttl(ttl)
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        (**self).local_addr()
    }

    fn close(&mut self) {
        (**self).close();
    }
}

/// The real socket: `std::net::UdpSocket`, and nothing else.
///
/// There is no platform code here and no `#[cfg]`. That is worth stating
/// because §7.1's cable needed both: the whole of the network output side is
/// portable, so the Linux CI job and the ARM64 cross-check test the same code
/// the Windows job does.
#[derive(Debug, Default)]
pub struct SystemUdp {
    socket: Option<UdpSocket>,
}

impl SystemUdp {
    /// A sender with no socket open yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { socket: None }
    }
}

impl UdpSender for SystemUdp {
    fn bind(&mut self, local: SocketAddr, broadcast: bool) -> Result<(), UdpError> {
        // Dropped before the new one is opened: a reconnect that left the old
        // socket behind would leak a port per retry, and the backoff retries
        // every five seconds for as long as the network is down.
        self.socket = None;
        let socket = UdpSocket::bind(local).map_err(|_| UdpError::Bind)?;
        // A socket that will not take the broadcast permission cannot send the
        // datagrams this output was configured for, so it is not usable.
        if broadcast {
            socket.set_broadcast(true).map_err(|_| UdpError::Bind)?;
        }
        self.socket = Some(socket);
        Ok(())
    }

    fn send_to(&mut self, datagram: &[u8], target: SocketAddr) -> Result<usize, UdpError> {
        let Some(socket) = self.socket.as_ref() else {
            return Err(UdpError::NotBound);
        };
        socket
            .send_to(datagram, target)
            .map_err(|error| classify(error.kind()))
    }

    fn set_multicast_ttl(&mut self, ttl: u32) -> Result<(), UdpError> {
        let Some(socket) = self.socket.as_ref() else {
            return Err(UdpError::NotBound);
        };
        socket
            .set_multicast_ttl_v4(ttl)
            .map_err(|error| classify(error.kind()))
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.socket
            .as_ref()
            .and_then(|socket| socket.local_addr().ok())
    }

    fn close(&mut self) {
        self.socket = None;
    }
}

/// A socket that records every datagram and fails wherever a test asks.
///
/// The loopback tests cover what a real socket does; this covers what one does
/// on a bad day. Both matter, and only one of them can be arranged on demand.
#[derive(Debug)]
pub struct MockUdp {
    state: Arc<Mutex<MockUdpState>>,
}

/// A test's view of a [`MockUdp`], usable after the sender has been moved into
/// an output and the output onto its thread.
#[derive(Debug, Clone)]
pub struct MockUdpHandle {
    state: Arc<Mutex<MockUdpState>>,
}

#[derive(Debug, Default)]
struct MockUdpState {
    datagrams: Vec<(SocketAddr, Vec<u8>)>,
    binds: Vec<(SocketAddr, bool)>,
    multicast_ttls: Vec<u32>,
    open: bool,
    closes: usize,
    bind_faults: VecDeque<UdpError>,
    send_faults: VecDeque<UdpError>,
    ttl_faults: VecDeque<UdpError>,
}

/// Takes a lock without caring whether a previous holder panicked: the runner's
/// panic tests unwind through code holding it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl MockUdp {
    /// A sender with nothing recorded and no socket open.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockUdpState::default())),
        }
    }

    /// A handle onto this sender's recording.
    #[must_use]
    pub fn handle(&self) -> MockUdpHandle {
        MockUdpHandle {
            state: Arc::clone(&self.state),
        }
    }
}

impl Default for MockUdp {
    fn default() -> Self {
        Self::new()
    }
}

impl MockUdpHandle {
    /// Every datagram sent so far, with the address it went to, in order.
    #[must_use]
    pub fn datagrams(&self) -> Vec<(SocketAddr, Vec<u8>)> {
        lock(&self.state).datagrams.clone()
    }

    /// How many datagrams have been sent.
    #[must_use]
    pub fn datagram_count(&self) -> usize {
        lock(&self.state).datagrams.len()
    }

    /// The most recent datagram.
    #[must_use]
    pub fn last_datagram(&self) -> Option<(SocketAddr, Vec<u8>)> {
        lock(&self.state).datagrams.last().cloned()
    }

    /// Forgets the datagrams recorded so far, so a test can assert on what
    /// happens next rather than on an offset into everything.
    pub fn clear(&self) {
        lock(&self.state).datagrams.clear();
    }

    /// Every `bind`, with the address and whether broadcast was asked for.
    ///
    /// This is where "broadcast is opt-in" is asserted at the socket: the flag
    /// the output passes is the permission the operating system is asked for.
    #[must_use]
    pub fn binds(&self) -> Vec<(SocketAddr, bool)> {
        lock(&self.state).binds.clone()
    }

    /// Every multicast hop limit this socket has been asked for, in order.
    ///
    /// An empty list is an assertion in its own right: an output that unicasts
    /// has no business touching a multicast option, and one that multicasts
    /// must not leave the hop limit to whatever the machine happens to default
    /// to.
    #[must_use]
    pub fn multicast_ttls(&self) -> Vec<u32> {
        lock(&self.state).multicast_ttls.clone()
    }

    /// Whether a socket is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        lock(&self.state).open
    }

    /// How many times the socket has been closed.
    #[must_use]
    pub fn closes(&self) -> usize {
        lock(&self.state).closes
    }

    /// Makes the next `times` binds fail.
    pub fn fail_bind(&self, times: usize, error: UdpError) {
        lock(&self.state)
            .bind_faults
            .extend(std::iter::repeat_n(error, times));
    }

    /// Makes the next `times` sends fail.
    pub fn fail_send(&self, times: usize, error: UdpError) {
        lock(&self.state)
            .send_faults
            .extend(std::iter::repeat_n(error, times));
    }

    /// Makes the next `times` attempts to set the multicast hop limit fail.
    pub fn fail_multicast_ttl(&self, times: usize, error: UdpError) {
        lock(&self.state)
            .ttl_faults
            .extend(std::iter::repeat_n(error, times));
    }
}

impl UdpSender for MockUdp {
    fn bind(&mut self, local: SocketAddr, broadcast: bool) -> Result<(), UdpError> {
        let mut state = lock(&self.state);
        state.binds.push((local, broadcast));
        match state.bind_faults.pop_front() {
            Some(error) => {
                state.open = false;
                Err(error)
            }
            None => {
                state.open = true;
                Ok(())
            }
        }
    }

    fn send_to(&mut self, datagram: &[u8], target: SocketAddr) -> Result<usize, UdpError> {
        let mut state = lock(&self.state);
        if !state.open {
            return Err(UdpError::NotBound);
        }
        match state.send_faults.pop_front() {
            Some(UdpError::ShortSend { sent, expected }) => Ok(sent.min(expected)),
            Some(error) => Err(error),
            None => {
                state.datagrams.push((target, datagram.to_vec()));
                Ok(datagram.len())
            }
        }
    }

    fn set_multicast_ttl(&mut self, ttl: u32) -> Result<(), UdpError> {
        let mut state = lock(&self.state);
        if !state.open {
            return Err(UdpError::NotBound);
        }
        state.multicast_ttls.push(ttl);
        state.ttl_faults.pop_front().map_or(Ok(()), Err)
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        let state = lock(&self.state);
        if !state.open {
            return None;
        }
        state.binds.last().map(|&(local, _)| local)
    }

    fn close(&mut self) {
        let mut state = lock(&self.state);
        state.open = false;
        state.closes += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{MockUdp, SystemUdp, UdpError, UdpSender, classify};
    use std::io;
    use std::net::{SocketAddr, UdpSocket};
    use std::time::Duration;

    /// Loopback only, and never `0.0.0.0`: a test suite has no business
    /// listening on every interface of the machine it runs on.
    fn loopback() -> SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    /// A bound receiver and the address to send to it.
    fn receiver() -> (UdpSocket, SocketAddr) {
        let socket = UdpSocket::bind(loopback()).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        (socket, address)
    }

    #[test]
    fn a_real_socket_delivers_the_bytes_it_was_given() {
        let (receiver, address) = receiver();
        let mut sender = SystemUdp::new();
        sender.bind(loopback(), false).unwrap();
        assert_eq!(sender.send_to(b"the bytes", address), Ok(9));

        let mut buffer = [0u8; 32];
        let (len, from) = receiver.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..len], b"the bytes");
        assert_eq!(Some(from), sender.local_addr());
    }

    #[test]
    fn a_real_socket_can_be_given_broadcast_permission() {
        // Asked for, not exercised: nothing in this repository's tests sends to
        // a broadcast address, because a test suite that floods the network it
        // runs on is the fault ARCHITECTURE_SPEC.md §7.2 is about.
        let (receiver, address) = receiver();
        let mut sender = SystemUdp::new();
        sender.bind(loopback(), true).unwrap();
        sender.send_to(b"still unicast", address).unwrap();
        let mut buffer = [0u8; 32];
        let (len, _) = receiver.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..len], b"still unicast");
    }

    #[test]
    fn a_real_socket_that_is_not_open_says_so_rather_than_pretending() {
        let mut sender = SystemUdp::default();
        assert_eq!(sender.local_addr(), None);
        assert_eq!(
            sender.send_to(b"nowhere", loopback()),
            Err(UdpError::NotBound)
        );
        sender.bind(loopback(), false).unwrap();
        assert!(sender.local_addr().is_some());
        sender.close();
        assert_eq!(sender.local_addr(), None);
        assert_eq!(
            sender.send_to(b"nowhere", loopback()),
            Err(UdpError::NotBound)
        );
    }

    #[test]
    fn a_real_socket_takes_a_multicast_hop_limit() {
        // Set, never exercised: nothing in this repository's tests sends a
        // multicast datagram, because a test suite that puts sACN on the
        // network it runs on is the same fault as one that broadcasts. What is
        // checked here is that the option reaches a real socket at all.
        let mut sender = SystemUdp::new();
        assert_eq!(sender.set_multicast_ttl(1), Err(UdpError::NotBound));
        sender.bind(loopback(), false).unwrap();
        assert_eq!(sender.set_multicast_ttl(1), Ok(()));
        assert_eq!(sender.set_multicast_ttl(16), Ok(()));
        sender.close();
        assert_eq!(sender.set_multicast_ttl(1), Err(UdpError::NotBound));
    }

    #[test]
    fn a_real_socket_refuses_an_address_this_machine_does_not_have() {
        // TEST-NET-3 (RFC 5737): documentation space, so it is not on this
        // machine and will not be on the CI runner either.
        let mut sender = SystemUdp::new();
        let elsewhere: SocketAddr = "203.0.113.1:0".parse().unwrap();
        assert_eq!(sender.bind(elsewhere, false), Err(UdpError::Bind));
        assert_eq!(sender.local_addr(), None);
    }

    #[test]
    fn rebinding_replaces_the_socket_rather_than_leaking_it() {
        // The reconnect path calls `bind` again on every retry, and the backoff
        // retries for as long as the network is down.
        let mut sender = SystemUdp::new();
        sender.bind(loopback(), false).unwrap();
        let first = sender.local_addr().unwrap();
        sender.bind(loopback(), false).unwrap();
        let second = sender.local_addr().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn the_errors_that_mean_the_network_is_gone_are_named() {
        for kind in [
            io::ErrorKind::NetworkUnreachable,
            io::ErrorKind::HostUnreachable,
            io::ErrorKind::NetworkDown,
            io::ErrorKind::AddrNotAvailable,
        ] {
            assert_eq!(classify(kind), UdpError::Unreachable);
            assert!(classify(kind).is_link_lost());
        }
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::WouldBlock,
            io::ErrorKind::Other,
        ] {
            assert_eq!(classify(kind), UdpError::Io);
            assert!(!classify(kind).is_link_lost());
        }
    }

    #[test]
    fn a_lost_link_is_the_one_that_reconnects() {
        assert!(UdpError::Bind.is_link_lost());
        assert!(UdpError::NotBound.is_link_lost());
        assert!(UdpError::Unreachable.is_link_lost());
        assert!(!UdpError::Io.is_link_lost());
        assert!(
            !UdpError::ShortSend {
                sent: 1,
                expected: 2
            }
            .is_link_lost()
        );
    }

    #[test]
    fn a_udp_error_says_what_went_wrong_in_words() {
        let errors = [
            (UdpError::Bind, "the local address could not be bound"),
            (UdpError::NotBound, "the socket is not open"),
            (UdpError::Unreachable, "the network is not reachable"),
            (UdpError::Io, "the socket refused the datagram"),
            (
                UdpError::ShortSend {
                    sent: 12,
                    expected: 530,
                },
                "the socket took 12 of 530 bytes",
            ),
        ];
        for (error, text) in errors {
            assert_eq!(error.to_string(), text);
            let as_error: &dyn std::error::Error = &error;
            assert_eq!(as_error.to_string(), text);
        }
    }

    #[test]
    fn a_mock_socket_records_every_datagram_and_where_it_went() {
        let mut sender = MockUdp::new();
        let handle = sender.handle();
        assert!(!handle.is_open());
        // Nothing can be asked of a socket that is not open, options included.
        assert_eq!(sender.set_multicast_ttl(1), Err(UdpError::NotBound));
        sender.bind(loopback(), false).unwrap();
        assert!(handle.is_open());
        assert_eq!(handle.binds(), vec![(loopback(), false)]);
        assert!(handle.multicast_ttls().is_empty());
        sender.set_multicast_ttl(4).unwrap();
        assert_eq!(handle.multicast_ttls(), vec![4]);

        let target: SocketAddr = "127.0.0.1:6454".parse().unwrap();
        assert_eq!(sender.send_to(&[1, 2, 3], target), Ok(3));
        assert_eq!(handle.datagram_count(), 1);
        assert_eq!(handle.last_datagram(), Some((target, vec![1, 2, 3])));
        assert_eq!(handle.datagrams(), vec![(target, vec![1, 2, 3])]);
        assert_eq!(sender.local_addr(), Some(loopback()));

        handle.clear();
        assert_eq!(handle.datagram_count(), 0);
    }

    #[test]
    fn a_mock_socket_can_be_told_to_fail() {
        let mut sender = MockUdp::new();
        let handle = sender.handle();
        handle.fail_bind(1, UdpError::Bind);
        assert_eq!(sender.bind(loopback(), true), Err(UdpError::Bind));
        assert!(!handle.is_open());
        assert_eq!(handle.binds(), vec![(loopback(), true)]);
        assert_eq!(sender.local_addr(), None);

        sender.bind(loopback(), true).unwrap();
        handle.fail_send(2, UdpError::Unreachable);
        let target: SocketAddr = "127.0.0.1:6454".parse().unwrap();
        assert_eq!(sender.send_to(&[0; 4], target), Err(UdpError::Unreachable));
        assert_eq!(sender.send_to(&[0; 4], target), Err(UdpError::Unreachable));
        assert_eq!(sender.send_to(&[0; 4], target), Ok(4));
        assert_eq!(handle.datagram_count(), 1);
    }

    #[test]
    fn a_mock_socket_can_take_fewer_bytes_than_it_was_given() {
        // A real socket does not do this on demand, and a driver that treated a
        // short send as success would put half a universe on the wire under a
        // green light.
        let mut sender = MockUdp::new();
        let handle = sender.handle();
        sender.bind(loopback(), false).unwrap();
        handle.fail_send(
            1,
            UdpError::ShortSend {
                sent: 4,
                expected: 530,
            },
        );
        assert_eq!(sender.send_to(&[0; 530], loopback()), Ok(4));
        assert_eq!(handle.datagram_count(), 0);
    }

    #[test]
    fn a_mock_socket_that_is_closed_refuses_datagrams() {
        let mut sender = MockUdp::new();
        let handle = sender.handle();
        sender.bind(loopback(), false).unwrap();
        sender.close();
        assert!(!handle.is_open());
        assert_eq!(handle.closes(), 1);
        assert_eq!(sender.send_to(&[0; 4], loopback()), Err(UdpError::NotBound));
    }

    #[test]
    fn a_boxed_sender_is_a_sender() {
        // What lets an output hold a socket chosen at run time.
        let inner = MockUdp::default();
        let handle = inner.handle();
        let mut sender: Box<dyn UdpSender> = Box::new(inner);
        sender.bind(loopback(), false).unwrap();
        assert_eq!(sender.send_to(&[7; 8], loopback()), Ok(8));
        assert_eq!(sender.set_multicast_ttl(2), Ok(()));
        assert_eq!(handle.multicast_ttls(), vec![2]);
        assert_eq!(sender.local_addr(), Some(loopback()));
        sender.close();
        assert_eq!(handle.closes(), 1);
        assert_eq!(handle.datagram_count(), 1);
    }
}
