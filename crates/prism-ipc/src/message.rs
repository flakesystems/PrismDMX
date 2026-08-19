//! The message types of `docs/IPC_PROTOCOL.md` §4 — what actually travels.
//!
//! | Message | Direction | Reliability |
//! |---|---|---|
//! | [`ClientMessage::Hello`] | client → daemon | reliable, first message |
//! | [`ServerMessage::Snapshot`] | daemon → client | reliable, answer to `Hello` |
//! | [`ClientMessage::Command`] | client → daemon | reliable, ordered — never dropped |
//! | [`ClientMessage::Query`] | client → daemon | reliable, ordered, changes nothing |
//! | [`ServerMessage::Delta`] | daemon → client | reliable, ordered |
//! | [`ServerMessage::Answer`] | daemon → client | reliable, to the one client that asked |
//! | [`ServerMessage::Telemetry`] | daemon → client | **droppable**, coalesced |
//! | [`ServerMessage::Ack`] / [`ServerMessage::Reject`] | daemon → client | reliable |
//!
//! Two envelopes rather than one, because the protocol is asymmetric by design
//! (D3): a client sends intent and a daemon sends facts, and a type that could
//! carry either would let a client send a `Delta`. There is no message in this
//! module that lets one.
//!
//! # Three additions to the specification, and the reasons
//!
//! **The snapshot carries the programmer.** §4.1 lists `{ show, session,
//! outputs, health }`, and S13 recorded the gap: the programmer is a third
//! document with its own delta, so a client connecting mid-programming would see
//! an empty one. It could be closed by sending a `ProgrammerChanged` straight
//! after the snapshot; it is closed here instead, because §9's snapshot
//! completeness test — *a fresh client's snapshot equals the state an existing
//! client reached by accumulating deltas* — is false for the programmer under
//! the other reading. The world arrives in one message or the criterion has to
//! be rewritten.
//!
//! **A command carries a sequence number, and `Ack`/`Reject` echo it.** §4 lists
//! both as reliable daemon-to-client messages and §5 says a command that cannot
//! be applied yields a `Reject`. With several commands in flight — which is the
//! normal case for a fader bank — an unaddressed rejection tells a client that
//! *something* failed. [`ClientMessage::Command::seq`] is that address; it is
//! per connection, and the daemon never interprets it.
//!
//! **Telemetry travels inside the envelope as an opaque byte string.** §7 says
//! telemetry is "binary, fixed layout — not MessagePack maps", and §3 says
//! message types are distinguished by the tagged enum in the payload rather than
//! by a header byte. Both hold: the envelope is one MessagePack map with a `t`
//! and a `bin` field, and the [`crate::TelemetryFrame`] inside it is fixed-layout
//! binary. The cost is about ten bytes per telemetry frame; the alternative is a
//! second channel discriminator in the framing, which §3 rules out.

use prism_domain::{
    Answer, Command, Delta, JsonValue, OutputHealth, OutputId, ProgrammerState, Query,
};
use serde::{Deserialize, Serialize};

/// The protocol version, incremented on any breaking change.
///
/// `docs/IPC_PROTOCOL.md` §4.2: a mismatch produces an explicit [`ServerMessage::Reject`]
/// with a human-readable reason. Undefined behaviour from a silent mismatch is
/// not acceptable in software that controls a show.
pub const PROTOCOL_VERSION: u32 = 1;

/// What kind of client is connecting. Informational: there is no privileged
/// client and no back door for the console (D11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ClientKind {
    /// The Tauri desktop shell.
    #[default]
    Desktop,
    /// The browser-based Web Remote.
    WebRemote,
    /// Something else — a test harness, a script, a future surface.
    Other,
}

/// The first message on every connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    /// The version the client speaks. Must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u32,
    /// What kind of client this is.
    pub client_kind: ClientKind,
    /// The token from §2.1, required when the listener is not on loopback.
    pub token: Option<String>,
}

impl Hello {
    /// A hello for this build, with no token.
    #[must_use]
    pub const fn new(client_kind: ClientKind) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            client_kind,
            token: None,
        }
    }

    /// The same hello, carrying a token.
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }
}

/// Everything a client may send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum ClientMessage {
    /// The handshake. Must be first; anything else on a connection that has not
    /// said hello is refused.
    Hello {
        /// The client's half of the handshake.
        hello: Hello,
    },
    /// An intent. The daemon validates, applies and broadcasts the result.
    Command {
        /// This connection's own numbering, echoed in the answer. The daemon
        /// stores it and never interprets it.
        seq: u64,
        /// What the client wants done.
        command: Command,
    },
    /// A question. **Changes nothing**, and is answered to this client alone.
    ///
    /// Added in S27, for the reason `prism_domain::query` gives: an address
    /// conflict has to be shown *before* it is committed, and neither a command
    /// nor a delta can say what would happen. It shares the command's numbering
    /// because it travels on the same ordered channel and the answer has to be
    /// addressable when several are in flight — which, for a patch form
    /// answering keystrokes, is the ordinary case.
    Query {
        /// This connection's own numbering, echoed in the answer.
        seq: u64,
        /// What the client is asking.
        query: Query,
    },
}

/// Everything the daemon may send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    /// The world, in answer to a [`ClientMessage::Hello`].
    Snapshot {
        /// The whole of it.
        snapshot: Box<Snapshot>,
    },
    /// A change that has already been applied.
    Delta {
        /// The change.
        delta: Delta,
    },
    /// A fixed-layout binary telemetry frame. Droppable — see the module
    /// documentation for why it travels inside the envelope.
    Telemetry {
        /// The encoded [`crate::TelemetryFrame`].
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    /// A command was applied.
    Ack {
        /// The `seq` of the command that was applied.
        seq: u64,
    },
    /// The answer to a [`ClientMessage::Query`], to the client that asked.
    ///
    /// Not a `Delta`: a delta is broadcast, and what one operator is typing
    /// into a patch form is nobody else's business (`ARCHITECTURE_SPEC.md`
    /// §4.2). Not droppable either — a client that asked a question and never
    /// heard back would wait for ever.
    Answer {
        /// The `seq` of the query this answers.
        seq: u64,
        /// What the daemon says.
        answer: Answer,
    },
    /// Something was refused. Carries `seq` when it was a command, and `None`
    /// when it was the connection itself — a version mismatch, a missing token,
    /// or a control queue that filled.
    Reject {
        /// The command this refers to, if it was one.
        seq: Option<u64>,
        /// Which kind of refusal.
        reason: RejectReason,
        /// Something a person can read, surfaced in the UI.
        message: String,
    },
}

impl ServerMessage {
    /// Whether this message may be dropped when a client cannot keep up.
    ///
    /// `docs/IPC_PROTOCOL.md` §8: telemetry is coalesced and then dropped;
    /// control messages are **never** dropped. This is the single place that
    /// classification is made, so [`crate::Outbound`] cannot disagree with the
    /// protocol about which is which.
    #[must_use]
    pub const fn is_droppable(&self) -> bool {
        matches!(self, Self::Telemetry { .. })
    }

    /// Whether the connection ends after this message.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        match self {
            Self::Reject { reason, .. } => reason.closes_the_connection(),
            Self::Snapshot { .. }
            | Self::Delta { .. }
            | Self::Telemetry { .. }
            | Self::Ack { .. }
            | Self::Answer { .. } => false,
        }
    }
}

/// Why something was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RejectReason {
    /// The client speaks another version of this protocol (§4.2).
    ProtocolVersion,
    /// The listener is not on loopback and the token was absent or wrong (§2.1).
    Unauthorised,
    /// A message arrived before the handshake, or a second handshake arrived.
    OutOfOrder,
    /// The daemon would not apply the command. The message says why.
    CommandRefused,
    /// The frame arrived intact and was not a message the daemon could read.
    /// The connection survives — the framing found the boundary, so the next
    /// message will be read normally.
    Undecodable,
    /// The client's control queue filled (§8). The connection closes; the client
    /// reconnects and re-snapshots.
    Backpressure,
    /// The daemon is shutting down.
    ShuttingDown,
}

impl RejectReason {
    /// Whether a rejection for this reason ends the connection.
    ///
    /// A refused *command* leaves the connection perfectly usable — that is the
    /// ordinary case, and §5 says the command changes nothing. Everything else
    /// here is a statement about the connection rather than about one message.
    #[must_use]
    pub const fn closes_the_connection(self) -> bool {
        match self {
            Self::ProtocolVersion
            | Self::Unauthorised
            | Self::OutOfOrder
            | Self::Backpressure
            | Self::ShuttingDown => true,
            Self::CommandRefused | Self::Undecodable => false,
        }
    }
}

/// The world, as of the moment a client connected.
///
/// `docs/IPC_PROTOCOL.md` §4.1. This is what makes a UI restart an ordinary
/// reconnect rather than a special case, and it is what makes **D11** work: a
/// view switched from the X-Touch while the UI was closed is simply part of what
/// the UI receives when it comes back.
///
/// The show and the session are carried as documents rather than as models. That
/// is not a convenience: `Delta::ShowPatch` and `Delta::SessionPatch` are RFC
/// 6902 operations, and an operation is only meaningful against a document root.
/// `prism_core::ShowMirror` and `SessionMirror` apply them to exactly these two
/// values. It is also what keeps this crate on `prism-domain` alone — a snapshot
/// typed as `prism_core::Show` would put the whole show model behind the wire
/// format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The show document: patch, groups, presets, sequences, executors.
    pub show: JsonValue,
    /// The session document: `{ session, views }`, per S12.
    pub session: JsonValue,
    /// The programmer. Not in §4.1's list — see the module documentation.
    pub programmer: ProgrammerState,
    /// One entry per configured DMX output.
    pub outputs: Vec<OutputSnapshot>,
    /// How the daemon itself is doing.
    pub health: DaemonHealth,
    /// How many profiles **this desk** can embed into a show.
    ///
    /// A number and not the profiles. S27 carried the whole list here, which was
    /// right for the four built-in ones and impossible for the two thousand S44
    /// brought: a snapshot has to fit in a frame (§3), and a menu of two thousand
    /// entries is not a menu. So a client **searches** — `Query::SearchLibrary` —
    /// and this is only what it needs to say *2 157 profiles* beside the box.
    ///
    /// Zero is an ordinary state: it means this desk found no library and is
    /// offering the built-in profiles only, which the daemon logs on the way up.
    pub fixture_library: u32,
}

/// One DMX output, as the status panel shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputSnapshot {
    /// Which output.
    pub id: OutputId,
    /// What the operator calls it.
    pub name: String,
    /// Whether frames are reaching the fixtures.
    pub health: OutputHealth,
}

/// The daemon's own state at snapshot time.
///
/// `unsaved_changes` is here rather than only in `Delta::DirtyFlag` because a
/// delta describes a *transition*: a client that connected after the last one
/// would otherwise have no idea whether the Save LED should be lit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonHealth {
    /// The version the daemon speaks, so a client can show it beside its own.
    pub protocol_version: u32,
    /// Measured tick rate. 44 Hz when all is well (`ARCHITECTURE_SPEC.md` §3.2).
    ///
    /// Guarded like every `f64` in the domain (S1): MessagePack carries NaN and
    /// infinity faithfully, and a client comparing a NaN tick rate against
    /// itself would redraw its status panel forever. The rule is the domain's
    /// and this is the one `f64` the protocol adds to it.
    #[serde(with = "prism_domain::finite")]
    pub tick_hz: f64,
    /// Ticks that missed their deadline since start-up.
    pub missed_ticks: u64,
    /// Whether the show has unsaved changes.
    pub unsaved_changes: bool,
}

impl Default for DaemonHealth {
    fn default() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            tick_hz: 0.0,
            missed_ticks: 0,
            unsaved_changes: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClientKind, ClientMessage, DaemonHealth, Hello, OutputSnapshot, PROTOCOL_VERSION,
        RejectReason, ServerMessage, Snapshot,
    };
    use crate::{decode, encode};
    use prism_domain::{
        Answer, Command, Delta, FixtureId, JsonValue, NoticeLevel, OutputHealth, OutputId,
        ParamDirection, PatchConflict, PatchPreview, ProgrammerState, Query, UniverseId, ViewId,
    };

    fn snapshot() -> Snapshot {
        Snapshot {
            show: JsonValue::Object(
                [("fixtures".to_owned(), JsonValue::Array(vec![]))]
                    .into_iter()
                    .collect(),
            ),
            session: JsonValue::Object(
                [("session".to_owned(), JsonValue::Int(1))]
                    .into_iter()
                    .collect(),
            ),
            programmer: ProgrammerState {
                selection: vec![FixtureId::new(3)],
                ..ProgrammerState::default()
            },
            outputs: vec![OutputSnapshot {
                id: OutputId::new(1),
                name: "Open DMX".to_owned(),
                health: OutputHealth::Degraded,
            }],
            health: DaemonHealth {
                protocol_version: PROTOCOL_VERSION,
                tick_hz: 44.0,
                missed_ticks: 2,
                unsaved_changes: true,
            },
            fixture_library: 2157,
        }
    }

    #[test]
    fn a_hello_is_this_version_unless_it_is_told_otherwise() {
        let hello = Hello::new(ClientKind::WebRemote);
        assert_eq!(hello.protocol_version, PROTOCOL_VERSION);
        assert_eq!(hello.token, None);
        assert_eq!(
            hello.with_token("abc").token.as_deref(),
            Some("abc"),
            "a token has to survive being attached"
        );
    }

    #[test]
    fn every_client_message_survives_the_wire() {
        let messages = [
            ClientMessage::Hello {
                hello: Hello::new(ClientKind::Desktop).with_token("t"),
            },
            ClientMessage::Command {
                seq: u64::MAX,
                command: Command::ClearProgrammer,
            },
            // S35's three. A command carrying a `String` and one carrying an
            // enum, through the envelope rather than only through `Command`'s
            // own round trip: the name an operator typed is the part that would
            // survive `serde_json` and not MessagePack if the tag were wrong.
            ClientMessage::Command {
                seq: 5,
                command: Command::RenameView {
                    view_id: ViewId::new(2),
                    name: "Busking — front of house".to_owned(),
                },
            },
            ClientMessage::Command {
                seq: 6,
                command: Command::DeleteView {
                    view_id: ViewId::new(2),
                },
            },
            ClientMessage::Command {
                seq: 7,
                command: Command::MoveView {
                    view_id: ViewId::new(2),
                    direction: ParamDirection::Prev,
                },
            },
            ClientMessage::Query {
                seq: 3,
                query: Query::PatchConflicts,
            },
            ClientMessage::Query {
                seq: 4,
                query: Query::PatchPreview {
                    id: FixtureId::new(1),
                    type_id: "generic.dimmer".to_owned(),
                    universe: UniverseId::new(2),
                    address: 5,
                },
            },
        ];
        for message in messages {
            let bytes = encode(&message).unwrap();
            assert_eq!(decode::<ClientMessage>(&bytes).unwrap(), message);
        }
    }

    #[test]
    fn every_server_message_survives_the_wire() {
        let messages = [
            ServerMessage::Snapshot {
                snapshot: Box::new(snapshot()),
            },
            ServerMessage::Delta {
                delta: Delta::Notice {
                    level: NoticeLevel::Error,
                    message: "no".to_owned(),
                },
            },
            ServerMessage::Telemetry {
                data: vec![0, 1, 2, 255],
            },
            ServerMessage::Ack { seq: 9 },
            ServerMessage::Answer {
                seq: 9,
                answer: Answer::PatchPreview {
                    preview: PatchPreview {
                        accepted: true,
                        refusal: None,
                        footprint: 4,
                        last_address: Some(8),
                        conflicts: vec![PatchConflict {
                            universe: UniverseId::new(1),
                            from: 5,
                            to: 8,
                            first: FixtureId::new(1),
                            second: FixtureId::new(2),
                        }],
                    },
                },
            },
            ServerMessage::Reject {
                seq: Some(9),
                reason: RejectReason::CommandRefused,
                message: "nothing to store".to_owned(),
            },
            ServerMessage::Reject {
                seq: None,
                reason: RejectReason::ProtocolVersion,
                message: "the interface and the engine are different versions".to_owned(),
            },
        ];
        for message in messages {
            let bytes = encode(&message).unwrap();
            assert_eq!(decode::<ServerMessage>(&bytes).unwrap(), message);
        }
    }

    /// The snapshot is the message the fixture rule of S14 applies hardest to: a
    /// default-valued one cannot tell "carried across" from "never filled in".
    #[test]
    fn the_snapshot_carries_every_document_it_claims_to() {
        let bytes = encode(&snapshot()).unwrap();
        let back: Snapshot = decode(&bytes).unwrap();
        assert_eq!(back.show, snapshot().show);
        assert_eq!(back.session, snapshot().session);
        assert_eq!(back.programmer.selection, vec![FixtureId::new(3)]);
        assert_eq!(back.outputs.len(), 1);
        assert_eq!(back.outputs[0].name, "Open DMX");
        assert_eq!(back.outputs[0].health, OutputHealth::Degraded);
        assert_eq!(back.fixture_library, 2157);
        assert!((back.health.tick_hz - 44.0).abs() < f64::EPSILON);
        assert_eq!(back.health.missed_ticks, 2);
        assert!(back.health.unsaved_changes);
    }

    /// The point of the byte-string encoding: telemetry must not cost one
    /// MessagePack integer per channel.
    #[test]
    fn telemetry_travels_as_bytes_rather_than_as_a_list_of_numbers() {
        let payload = vec![200_u8; 512];
        let bytes = encode(&ServerMessage::Telemetry {
            data: payload.clone(),
        })
        .unwrap();
        assert!(
            bytes.len() < payload.len() + 64,
            "512 channels cost {} bytes on the wire",
            bytes.len()
        );
        assert_eq!(
            decode::<ServerMessage>(&bytes).unwrap(),
            ServerMessage::Telemetry { data: payload }
        );
    }

    #[test]
    fn only_telemetry_may_be_dropped() {
        assert!(ServerMessage::Telemetry { data: vec![] }.is_droppable());
        for message in [
            ServerMessage::Snapshot {
                snapshot: Box::new(snapshot()),
            },
            ServerMessage::Delta {
                delta: Delta::DirtyFlag {
                    unsaved_changes: false,
                },
            },
            ServerMessage::Ack { seq: 0 },
            // An answer is not droppable either: a client that asked a question
            // and never heard back would wait for ever.
            ServerMessage::Answer {
                seq: 0,
                answer: Answer::PatchConflicts {
                    conflicts: Vec::new(),
                },
            },
            ServerMessage::Reject {
                seq: None,
                reason: RejectReason::Backpressure,
                message: String::new(),
            },
        ] {
            assert!(!message.is_droppable(), "{message:?} is not droppable");
        }
    }

    #[test]
    fn a_refused_command_leaves_the_connection_open_and_everything_else_does_not() {
        assert!(!RejectReason::CommandRefused.closes_the_connection());
        assert!(!RejectReason::Undecodable.closes_the_connection());
        for reason in [
            RejectReason::ProtocolVersion,
            RejectReason::Unauthorised,
            RejectReason::OutOfOrder,
            RejectReason::Backpressure,
            RejectReason::ShuttingDown,
        ] {
            assert!(reason.closes_the_connection(), "{reason:?}");
        }

        assert!(
            !ServerMessage::Reject {
                seq: Some(1),
                reason: RejectReason::CommandRefused,
                message: String::new(),
            }
            .is_terminal()
        );
        assert!(
            ServerMessage::Reject {
                seq: None,
                reason: RejectReason::ShuttingDown,
                message: String::new(),
            }
            .is_terminal()
        );
        assert!(!ServerMessage::Ack { seq: 1 }.is_terminal());
        assert!(!ServerMessage::Telemetry { data: vec![] }.is_terminal());
        assert!(
            !ServerMessage::Delta {
                delta: Delta::DirtyFlag {
                    unsaved_changes: true
                }
            }
            .is_terminal()
        );
        assert!(
            !ServerMessage::Snapshot {
                snapshot: Box::new(snapshot())
            }
            .is_terminal()
        );
    }

    /// The asymmetry of D3, asserted rather than described: a client's envelope
    /// has no variant that carries a fact.
    #[test]
    fn a_client_cannot_send_a_delta() {
        let delta = encode(&ServerMessage::Delta {
            delta: Delta::DirtyFlag {
                unsaved_changes: true,
            },
        })
        .unwrap();
        assert!(decode::<ClientMessage>(&delta).is_err());

        let hello = encode(&ClientMessage::Hello {
            hello: Hello::new(ClientKind::Other),
        })
        .unwrap();
        assert!(decode::<ServerMessage>(&hello).is_err());
    }

    #[test]
    fn a_fresh_health_claims_nothing_it_has_not_measured() {
        let health = DaemonHealth::default();
        assert_eq!(health.protocol_version, PROTOCOL_VERSION);
        assert_eq!(health.tick_hz, 0.0);
        assert_eq!(health.missed_ticks, 0);
        assert!(!health.unsaved_changes);
        assert_eq!(ClientKind::default(), ClientKind::Desktop);
    }
}
