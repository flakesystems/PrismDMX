//! The framing round trip — `IMPLEMENTATION_PLAN.md` S16, first exit criterion,
//! and `docs/IPC_PROTOCOL.md` §9's first row.
//!
//! > Framing round trip — property test over arbitrary messages: encode → decode
//! > → equal.
//!
//! *Arbitrary messages* is read strictly. Every one of the six message types is
//! generated with arbitrary contents, which means all 23 `Command` variants and
//! all 7 `Delta` variants, arbitrary `JsonValue` trees inside the patch deltas,
//! arbitrary programmer states, arbitrary snapshots. `prism-domain`'s `proptest`
//! feature supplies the leaves, so nothing here re-invents a generator that
//! could drift from the type it generates.
//!
//! The round trip goes through the **whole frame**, prefix included, and reads
//! the length back out of the header rather than from the value it already has.
//! Encoding a payload and decoding the same payload would leave the one number
//! the framing actually adds untested.

use prism_domain::{Command, Delta, JsonValue, OutputHealth, OutputId, ProgrammerState, arb};
use prism_ipc::{
    ClientKind, ClientMessage, DaemonHealth, Hello, LENGTH_PREFIX_BYTES, OutputSnapshot,
    PROTOCOL_VERSION, RejectReason, ServerMessage, Snapshot, decode, encode_frame, payload_length,
};
use proptest::prelude::*;

/// Encode into a frame, read the length out of the header, decode what the
/// header pointed at.
fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let frame = encode_frame(value).expect("every message must encode");
    let header: [u8; LENGTH_PREFIX_BYTES] = frame[..LENGTH_PREFIX_BYTES]
        .try_into()
        .expect("a frame begins with its length");
    let length = payload_length(header).expect("a frame this end wrote is not oversized");
    assert_eq!(
        length,
        frame.len() - LENGTH_PREFIX_BYTES,
        "the header does not describe the payload behind it"
    );
    decode(&frame[LENGTH_PREFIX_BYTES..]).expect("a frame this end wrote must decode")
}

fn any_client_kind() -> impl Strategy<Value = ClientKind> {
    prop_oneof![
        Just(ClientKind::Desktop),
        Just(ClientKind::WebRemote),
        Just(ClientKind::Other),
    ]
}

fn any_reject_reason() -> impl Strategy<Value = RejectReason> {
    prop_oneof![
        Just(RejectReason::ProtocolVersion),
        Just(RejectReason::Unauthorised),
        Just(RejectReason::OutOfOrder),
        Just(RejectReason::CommandRefused),
        Just(RejectReason::Undecodable),
        Just(RejectReason::Backpressure),
        Just(RejectReason::ShuttingDown),
    ]
}

fn any_hello() -> impl Strategy<Value = Hello> {
    (
        any::<u32>(),
        any_client_kind(),
        proptest::option::of(".{0,16}"),
    )
        .prop_map(|(protocol_version, client_kind, token)| Hello {
            protocol_version,
            client_kind,
            token,
        })
}

fn any_output() -> impl Strategy<Value = OutputSnapshot> {
    // S33 grew this from three fields to seven; the configured row is generated
    // as well, because it is the one that carries a socket address and an enum
    // and is therefore the one a codec can get wrong.
    (
        any::<u32>(),
        ".{0,16}",
        any::<OutputHealth>(),
        proptest::option::of(any::<prism_domain::OutputInstance>()),
        any::<u64>(),
        proptest::option::of(".{0,16}"),
        proptest::option::of(any::<u64>()),
    )
        .prop_map(
            |(id, name, health, output, frames_sent, last_error, last_error_ago_ms)| {
                OutputSnapshot {
                    id: OutputId::new(id),
                    name,
                    health,
                    output,
                    frames_sent,
                    last_error,
                    last_error_ago_ms,
                }
            },
        )
}

fn any_health() -> impl Strategy<Value = DaemonHealth> {
    (any::<u32>(), arb::seconds(), any::<u64>(), any::<bool>()).prop_map(
        |(protocol_version, tick_hz, missed_ticks, unsaved_changes)| DaemonHealth {
            protocol_version,
            tick_hz,
            missed_ticks,
            unsaved_changes,
        },
    )
}

fn any_snapshot() -> impl Strategy<Value = Snapshot> {
    (
        any::<JsonValue>(),
        any::<JsonValue>(),
        any::<ProgrammerState>(),
        proptest::collection::vec(any_output(), 0..3),
        any_health(),
    )
        .prop_map(|(show, session, programmer, outputs, health)| Snapshot {
            show,
            session,
            programmer,
            outputs,
            health,
            fixture_library: 0,
            // Defaults rather than arbitrary values, and deliberately: the two
            // S37 fields have round-trip properties of their own in
            // `prism-domain`, and adding two more arbitrary trees to this
            // strategy is what the stack budget in `wire.rs` is about.
            machine: prism_domain::MachineSettings::default(),
            show_file: prism_domain::ShowFileInfo::default(),
        })
}

fn any_client_message() -> BoxedStrategy<ClientMessage> {
    // Boxed, and not for tidiness: `prop_oneof!` nests one generic value-tree
    // layer per arm, and with `Command`'s derived tree inside it the
    // two-message property below ran a debug build out of stack when S27 added
    // the third arm. Erasing the type puts that recursion behind a vtable.
    //
    // **`Command` itself is boxed too, for the same reason** (S34). Boxing the
    // outer union leaves the derived tree — one variant's worth of value tree
    // per variant, all in one value — on the stack of whatever holds it, and
    // the thirty-sixth command was what tipped the two-message property over
    // again. A session adding a command with a large payload should reach for
    // `.boxed()` rather than for `RUST_MIN_STACK`; `prism-core`'s
    // `tests/oops.rs` carries the same note.
    prop_oneof![
        any_hello().prop_map(|hello| ClientMessage::Hello { hello }),
        (any::<u64>(), any::<prism_domain::Query>())
            .prop_map(|(seq, query)| ClientMessage::Query { seq, query }),
        (any::<u64>(), arb::command())
            .prop_map(|(seq, command)| ClientMessage::Command { seq, command }),
    ]
    .boxed()
}

fn any_server_message() -> BoxedStrategy<ServerMessage> {
    prop_oneof![
        any_snapshot().prop_map(|snapshot| ServerMessage::Snapshot {
            snapshot: Box::new(snapshot)
        }),
        any::<Delta>().prop_map(|delta| ServerMessage::Delta { delta }),
        proptest::collection::vec(any::<u8>(), 0..600)
            .prop_map(|data| ServerMessage::Telemetry { data }),
        any::<u64>().prop_map(|seq| ServerMessage::Ack { seq }),
        (any::<u64>(), any::<prism_domain::Answer>())
            .prop_map(|(seq, answer)| ServerMessage::Answer { seq, answer }),
        (
            proptest::option::of(any::<u64>()),
            any_reject_reason(),
            ".{0,32}"
        )
            .prop_map(|(seq, reason, message)| ServerMessage::Reject {
                seq,
                reason,
                message
            }),
    ]
    .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn every_client_message_survives_the_frame_it_travels_in(message in any_client_message()) {
        prop_assert_eq!(round_trip(&message), message);
    }

    #[test]
    fn every_server_message_survives_the_frame_it_travels_in(message in any_server_message()) {
        prop_assert_eq!(round_trip(&message), message);
    }

    /// The payloads, on their own, so a failure points at the domain rather than
    /// at the envelope.
    #[test]
    fn every_command_survives_the_frame_it_travels_in(command: Command) {
        prop_assert_eq!(round_trip(&command), command);
    }

    #[test]
    fn every_delta_survives_the_frame_it_travels_in(delta: Delta) {
        prop_assert_eq!(round_trip(&delta), delta);
    }

    /// The length prefix describes exactly the payload and nothing else, at
    /// every size a message reaches.
    #[test]
    fn the_prefix_is_the_length_of_what_follows_it(message in any_server_message()) {
        let frame = encode_frame(&message).unwrap();
        let announced = u32::from_le_bytes(frame[..LENGTH_PREFIX_BYTES].try_into().unwrap());
        prop_assert_eq!(announced as usize, frame.len() - LENGTH_PREFIX_BYTES);
    }

    /// Two frames back to back stay two frames: the boundary is the number in
    /// the header and not a delimiter that could occur in a payload.
    #[test]
    fn frames_concatenate_without_running_into_each_other(
        first in any_client_message(),
        second in any_client_message(),
    ) {
        let mut stream = encode_frame(&first).unwrap();
        let split = stream.len();
        stream.extend_from_slice(&encode_frame(&second).unwrap());

        let head: [u8; LENGTH_PREFIX_BYTES] = stream[..LENGTH_PREFIX_BYTES].try_into().unwrap();
        let first_len = payload_length(head).unwrap();
        prop_assert_eq!(
            decode::<ClientMessage>(&stream[LENGTH_PREFIX_BYTES..][..first_len]).unwrap(),
            first
        );
        prop_assert_eq!(LENGTH_PREFIX_BYTES + first_len, split);

        let head: [u8; LENGTH_PREFIX_BYTES] = stream[split..][..LENGTH_PREFIX_BYTES]
            .try_into()
            .unwrap();
        let second_len = payload_length(head).unwrap();
        prop_assert_eq!(
            decode::<ClientMessage>(&stream[split + LENGTH_PREFIX_BYTES..][..second_len]).unwrap(),
            second
        );
    }
}

/// The version the round trip is measured against is the one this build speaks.
#[test]
fn the_protocol_version_is_the_one_the_handshake_sends() {
    assert_eq!(
        Hello::new(ClientKind::Desktop).protocol_version,
        PROTOCOL_VERSION
    );
    assert_eq!(DaemonHealth::default().protocol_version, PROTOCOL_VERSION);
}
