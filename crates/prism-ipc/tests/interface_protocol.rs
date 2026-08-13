//! The transcription check: `ui/src/ipc/protocol.ts` against this crate.
//!
//! # Why there is a hand-written TypeScript file at all
//!
//! `ui/src/bindings/` is generated from `prism-domain`, which owns the
//! *vocabulary* — `Command`, `Delta`, `Session`. The **envelope** is this
//! crate's: `Hello`, `Snapshot`, `ServerMessage`, `RejectReason`,
//! `PROTOCOL_VERSION`. `prism-ipc` exports no TypeScript, because it is the
//! wire rather than the vocabulary, and putting a `ts-rs` export here would
//! mean two crates writing one directory — one of which deletes what it finds.
//!
//! So the envelope is transcribed by hand in `ui`, and this target is what
//! makes that safe. It reads the TypeScript back with `include_str!` — compiled
//! in, so a file that has been moved or deleted fails the *build* — and checks
//! that every name this crate has appears there, in the spelling that travels.
//!
//! The same shape as S22's `the_shipped_profile_is_the_built_in_default_table`,
//! and for the same reason: a copy nobody compares is a second source of truth,
//! and D3 says there is one.

use prism_ipc::{ClientKind, PROTOCOL_VERSION, RejectReason};

/// The interface's transcription of `crates/prism-ipc/src/message.rs`.
const PROTOCOL_TS: &str = include_str!("../../../ui/src/ipc/protocol.ts");

/// Every [`RejectReason`], with the spelling `serde` gives it.
///
/// The match has no wildcard, so a variant added to the enum will not compile
/// until it is listed here — and once it is listed, the test below requires it
/// in the TypeScript as well.
fn reject_reasons() -> Vec<(RejectReason, &'static str)> {
    let all = [
        RejectReason::ProtocolVersion,
        RejectReason::Unauthorised,
        RejectReason::OutOfOrder,
        RejectReason::CommandRefused,
        RejectReason::Undecodable,
        RejectReason::Backpressure,
        RejectReason::ShuttingDown,
    ];
    all.into_iter()
        .map(|reason| {
            let name = match reason {
                RejectReason::ProtocolVersion => "ProtocolVersion",
                RejectReason::Unauthorised => "Unauthorised",
                RejectReason::OutOfOrder => "OutOfOrder",
                RejectReason::CommandRefused => "CommandRefused",
                RejectReason::Undecodable => "Undecodable",
                RejectReason::Backpressure => "Backpressure",
                RejectReason::ShuttingDown => "ShuttingDown",
            };
            (reason, name)
        })
        .collect()
}

/// Every [`ClientKind`], likewise.
fn client_kinds() -> Vec<&'static str> {
    [
        ClientKind::Desktop,
        ClientKind::WebRemote,
        ClientKind::Other,
    ]
    .into_iter()
    .map(|kind| match kind {
        ClientKind::Desktop => "Desktop",
        ClientKind::WebRemote => "WebRemote",
        ClientKind::Other => "Other",
    })
    .collect()
}

#[test]
fn the_interface_speaks_this_protocol_version() {
    assert!(
        PROTOCOL_TS.contains(&format!(
            "export const PROTOCOL_VERSION = {PROTOCOL_VERSION};"
        )),
        "ui/src/ipc/protocol.ts does not speak version {PROTOCOL_VERSION}"
    );
}

#[test]
fn the_interface_knows_every_refusal() {
    for (_, name) in reject_reasons() {
        assert!(
            PROTOCOL_TS.contains(&format!("\"{name}\",")),
            "ui/src/ipc/protocol.ts has no RejectReason::{name}, so a daemon that sent one \
             would be reported to the operator as an unreadable message"
        );
    }
    // And the complement, which is the half a list can get wrong quietly: the
    // TypeScript union has exactly as many members as the Rust enum, so a
    // reason removed here does not linger there.
    let listed = PROTOCOL_TS
        .lines()
        .skip_while(|line| !line.contains("export const REJECT_REASONS"))
        .take_while(|line| !line.contains("] as const;"))
        .filter(|line| line.trim_start().starts_with('"'))
        .count();
    assert_eq!(
        listed,
        reject_reasons().len(),
        "REJECT_REASONS lists {listed} reasons and this crate has {}",
        reject_reasons().len()
    );
}

/// The distinction the interface has to be able to make (§8, last paragraph).
///
/// A refused *command* and an undecodable *message* both leave the connection
/// open; everything else is a statement about the connection. An interface that
/// had this the wrong way round would either reconnect on every refused command
/// or sit for ever on a socket the daemon has already closed.
#[test]
fn the_interface_agrees_about_which_refusals_end_a_connection() {
    let staying: Vec<&str> = reject_reasons()
        .into_iter()
        .filter(|(reason, _)| !reason.closes_the_connection())
        .map(|(_, name)| name)
        .collect();
    assert_eq!(staying, vec!["CommandRefused", "Undecodable"]);

    let expected = format!(
        "return reason !== \"{}\" && reason !== \"{}\";",
        staying[0], staying[1]
    );
    assert!(
        PROTOCOL_TS.contains(&expected),
        "closesTheConnection does not read `{expected}`"
    );
}

#[test]
fn the_interface_knows_every_client_kind_and_every_message() {
    for kind in client_kinds() {
        assert!(
            PROTOCOL_TS.contains(&format!("\"{kind}\"")),
            "ui/src/ipc/protocol.ts has no ClientKind::{kind}"
        );
    }

    // The message tags, both directions. Written out here rather than derived
    // from the enums, because a tag is what `#[serde(tag = "t")]` puts on the
    // wire and reading it off the type would be asking the code under test.
    for tag in ["Hello", "Command"] {
        assert!(
            PROTOCOL_TS.contains(&format!("readonly t: \"{tag}\"")),
            "the interface cannot send a {tag}"
        );
    }
    for tag in ["Snapshot", "Delta", "Telemetry", "Ack", "Reject"] {
        assert!(
            PROTOCOL_TS.contains(&format!("case \"{tag}\":")),
            "the interface cannot read a {tag}"
        );
    }
}

/// The fields a `Hello` carries, in the camel case the wire uses.
///
/// `Hello` is the one message that must be right before anything else can be:
/// a client whose handshake is misspelt is refused with `Undecodable` and never
/// finds out why.
#[test]
fn the_interface_says_hello_with_the_fields_this_crate_reads() {
    for field in ["protocolVersion", "clientKind", "token"] {
        assert!(
            PROTOCOL_TS.contains(&format!("readonly {field}:")),
            "the interface's Hello has no {field}"
        );
    }
    // And the snapshot's three documents, which is what §4.1 promises.
    for field in ["show", "session", "programmer", "outputs", "health"] {
        assert!(
            PROTOCOL_TS.contains(&format!("readonly {field}:")),
            "the interface's Snapshot has no {field}"
        );
    }
}
