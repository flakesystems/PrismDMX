//! The real multicast socket, against a real sender - **S32**, and `#[ignore]`d.
//!
//! Everything else in this workspace tests the receiver over a double, because
//! no test may put a multicast datagram on the network it runs on. This is the
//! one thing that cannot be a double: whether `SystemUdpNode::listen_multicast`
//! hears a group on *this* machine's adapters. It **only listens** - the sender
//! is a program you start yourself (OpenFollow, or `tools/psn-send.py`):
//!
//! ```text
//! cargo test -p prism-protocols --test psn_multicast -- --ignored --nocapture
//! ```
//!
//! Set `PSN_GROUP`, `PSN_PORT` and `PSN_INTERFACE` to listen elsewhere. It
//! listens for eight seconds (`PSN_SECONDS` changes that) and prints the first
//! datagrams' source and what the codec made of it, then **the gaps**: the longest
//! silence and how many were longer than 200 ms. That is the number that says a
//! Wi-Fi link is dropping multicast, which a count alone does not.

#![allow(
    clippy::print_stdout,
    reason = "a manual diagnostic has to print what it heard"
)]

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use prism_protocols::psn::{self, Event};
use prism_protocols::{SystemUdpNode, UdpNode};

#[test]
#[ignore = "listens on a real multicast group for eight seconds; start a sender first"]
fn hears_a_sender_on_this_machine() {
    let group: Ipv4Addr = std::env::var("PSN_GROUP")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(psn::GROUP);
    let port: u16 = std::env::var("PSN_PORT")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(psn::PORT);
    let interface: Option<Ipv4Addr> = std::env::var("PSN_INTERFACE")
        .ok()
        .and_then(|text| text.parse().ok());

    let mut node = SystemUdpNode::new();
    node.listen_multicast(group, port, interface)
        .expect("the group could not be joined");
    println!(
        "listening on {group}:{port} (interface {interface:?}); joined on {:?}",
        node.joined_interfaces()
    );

    let mut buffer = [0_u8; psn::MAX_PACKET];
    let (mut datagrams, mut positions) = (0_u32, 0_u32);
    let seconds: u64 = std::env::var("PSN_SECONDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(8);
    let start = Instant::now();
    let until = start + Duration::from_secs(seconds);
    let mut last = start;
    let (mut longest, mut long_gaps) = (Duration::ZERO, 0_u32);
    while Instant::now() < until {
        let Ok(Some((len, from))) = node.recv_from(&mut buffer, Duration::from_millis(200)) else {
            continue;
        };
        let gap = last.elapsed();
        last = Instant::now();
        longest = longest.max(gap);
        if gap > Duration::from_millis(200) {
            long_gaps += 1;
            println!("gap of {gap:?} at {:?}", start.elapsed());
        }
        datagrams += 1;
        let mut seen = Vec::new();
        let result = psn::decode(&buffer[..len], &mut |event| {
            if let Event::Position { tracker, position } = event {
                seen.push((tracker, position));
            }
        });
        positions += u32::try_from(seen.len()).unwrap_or(u32::MAX);
        if datagrams <= 5 {
            println!("{len} bytes from {from}: {result:?}, positions {seen:?}");
        }
    }
    println!(
        "{datagrams} datagrams, {positions} positions in {seconds} s; longest silence {longest:?}, {long_gaps} longer than 200 ms"
    );
    assert!(
        datagrams > 0,
        "nothing arrived - see the notes in the manual"
    );
}
