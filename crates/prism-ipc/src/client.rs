//! The client's half — the other end of everything in [`crate::server`].
//!
//! One type, used by all three clients of `docs/IPC_PROTOCOL.md` §1: the
//! desktop shell, the Web Remote, and any test or script. There is no
//! privileged client and no back door for the console, so there is no second
//! client type either.
//!
//! # It sends intent and receives facts, and the types say so
//!
//! [`Client::send`] takes a `prism_domain::Command` and nothing else.
//! [`Client::next_event`] hands back a [`ClientEvent`], which carries deltas,
//! telemetry and answers. There is no method here that lets a client tell the
//! daemon what the state now is — that is D3 expressed as an API rather than as
//! a rule to remember.
//!
//! # The snapshot is not an event
//!
//! [`Client::connect`] answers with the [`Snapshot`] or with an error. A client
//! that has a `Client` at all has already been given the world, so there is no
//! "not yet synchronised" state for the UI to render and no ordering question
//! about the first delta.

use core::fmt;

use prism_domain::{Answer, Command, Delta, Query};

use crate::message::{ClientMessage, Hello, RejectReason, ServerMessage, Snapshot};
use crate::telemetry::{TelemetryError, TelemetryFrame};
use crate::transport::{Endpoint, Wire, WireError, WireReceiver, WireSender, local, websocket};

/// Why a client could not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// The connection failed, or the endpoint was not there. This is how a
    /// client discovers the daemon is not running (§8).
    Wire(WireError),
    /// The daemon turned the connection away. §4.2's version mismatch, §2.1's
    /// token, or a daemon that is shutting down.
    Rejected {
        /// Which refusal.
        reason: RejectReason,
        /// What to show the operator.
        message: String,
    },
    /// The daemon answered the handshake with something that is not a snapshot.
    /// A daemon that did this would be broken, and guessing would be worse.
    UnexpectedAnswer(String),
    /// A telemetry frame could not be read. Telemetry is droppable, so this does
    /// not end anything — it is reported so it can be counted rather than
    /// silently ignored.
    Telemetry(TelemetryError),
    /// The daemon closed the connection.
    Closed,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => fmt::Display::fmt(error, f),
            Self::Rejected { reason, message } => {
                write!(
                    f,
                    "the daemon refused the connection ({reason:?}): {message}"
                )
            }
            Self::UnexpectedAnswer(what) => {
                write!(f, "the daemon answered the handshake with {what}")
            }
            Self::Telemetry(error) => write!(f, "a telemetry frame was dropped: {error}"),
            Self::Closed => f.write_str("the daemon closed the connection"),
        }
    }
}

impl core::error::Error for ClientError {}

impl From<WireError> for ClientError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

/// Something the daemon said.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    /// A change that has already been applied. Apply it to the mirror without
    /// validating it: the daemon has already decided (§6).
    Delta(Delta),
    /// A telemetry frame. §7: write it to a ref and render it on a canvas —
    /// never into reactive state.
    Telemetry(TelemetryFrame),
    /// A command was applied.
    Ack {
        /// The `seq` [`Client::send`] answered with.
        seq: u64,
    },
    /// A question was answered (§5.2). Changes nothing by construction.
    Answered {
        /// The `seq` [`Client::ask`] answered with.
        seq: u64,
        /// What the daemon says.
        answer: Answer,
    },
    /// A command was refused, and it changed nothing (§5).
    Refused {
        /// The `seq` [`Client::send`] answered with.
        seq: Option<u64>,
        /// Which refusal.
        reason: RejectReason,
        /// What to show the operator.
        message: String,
    },
}

/// A connection to `prismd`.
#[derive(Debug)]
pub struct Client {
    sender: WireSender,
    receiver: WireReceiver,
    next_seq: u64,
}

impl Client {
    /// Connects over whichever transport `endpoint` names, and completes the
    /// handshake.
    ///
    /// This is the one place a client chooses a transport, and it is four lines
    /// long. Everything after it is identical whichever branch was taken, which
    /// is what `tests/transport_parity.rs` demonstrates by running the same
    /// suite through both.
    ///
    /// # Errors
    ///
    /// [`ClientError::Wire`] if the daemon is not there, and
    /// [`ClientError::Rejected`] if it is there and turned the connection away.
    pub async fn connect(
        endpoint: &Endpoint,
        hello: Hello,
    ) -> Result<(Self, Snapshot), ClientError> {
        let wire = match endpoint {
            Endpoint::Local(address) => local::connect(address)
                .await
                .map_err(|error| ClientError::Wire(WireError::Io(error.to_string())))?,
            Endpoint::WebSocket(addr) => {
                websocket::connect(&format!("ws://{addr}{}", websocket::IPC_PATH)).await?
            }
        };
        Self::handshake(wire, hello).await
    }

    /// Completes the handshake over a wire that is already open.
    ///
    /// # Errors
    ///
    /// As [`Self::connect`].
    pub async fn handshake(wire: Wire, hello: Hello) -> Result<(Self, Snapshot), ClientError> {
        let (sender, mut receiver) = wire.split();
        sender.send_message(&ClientMessage::Hello { hello }).await?;

        let answer = match receiver.recv_message::<ServerMessage>().await {
            Some(Ok(message)) => message,
            Some(Err(error)) => return Err(error.into()),
            None => return Err(ClientError::Closed),
        };

        match answer {
            ServerMessage::Snapshot { snapshot } => Ok((
                Self {
                    sender,
                    receiver,
                    next_seq: 0,
                },
                *snapshot,
            )),
            ServerMessage::Reject {
                reason, message, ..
            } => Err(ClientError::Rejected { reason, message }),
            other => Err(ClientError::UnexpectedAnswer(describe(&other).to_owned())),
        }
    }

    /// Sends a command, answering with the sequence number the daemon will echo.
    ///
    /// # Errors
    ///
    /// [`ClientError::Wire`] if the connection has gone.
    pub async fn send(&mut self, command: Command) -> Result<u64, ClientError> {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.sender
            .send_message(&ClientMessage::Command { seq, command })
            .await?;
        Ok(seq)
    }

    /// Asks a question, answering with the sequence number the daemon will echo.
    ///
    /// A question changes nothing (§5.2), so there is no `Ack` and no `Reject`
    /// for one: the answer arrives as [`ClientEvent::Answered`] carrying this
    /// number. The numbering is shared with [`Self::send`] deliberately —
    /// commands and queries travel on one ordered channel, and two numberings
    /// would let an answer and an acknowledgement collide.
    ///
    /// # Errors
    ///
    /// [`ClientError::Wire`] if the connection has gone.
    pub async fn ask(&mut self, query: Query) -> Result<u64, ClientError> {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.sender
            .send_message(&ClientMessage::Query { seq, query })
            .await?;
        Ok(seq)
    }

    /// The next thing the daemon said, or `None` when the connection has closed.
    pub async fn next_event(&mut self) -> Option<Result<ClientEvent, ClientError>> {
        let message = match self.receiver.recv_message::<ServerMessage>().await? {
            Ok(message) => message,
            Err(error) => return Some(Err(error.into())),
        };
        Some(match message {
            ServerMessage::Delta { delta } => Ok(ClientEvent::Delta(delta)),
            ServerMessage::Telemetry { data } => match TelemetryFrame::decode(&data) {
                Ok(frame) => Ok(ClientEvent::Telemetry(frame)),
                Err(error) => Err(ClientError::Telemetry(error)),
            },
            ServerMessage::Ack { seq } => Ok(ClientEvent::Ack { seq }),
            ServerMessage::Answer { seq, answer } => Ok(ClientEvent::Answered { seq, answer }),
            ServerMessage::Reject {
                seq,
                reason,
                message,
            } => Ok(ClientEvent::Refused {
                seq,
                reason,
                message,
            }),
            // A second snapshot is not part of this version of the protocol: a
            // client that needs one reconnects, which is what §8 says a client
            // does after every disconnection.
            other @ ServerMessage::Snapshot { .. } => {
                Err(ClientError::UnexpectedAnswer(describe(&other).to_owned()))
            }
        })
    }

    /// Closes the connection, having flushed what was already sent.
    pub async fn disconnect(self) {
        drop(self.receiver);
        self.sender.shutdown().await;
    }
}

/// A message's name, for an error a person has to read.
const fn describe(message: &ServerMessage) -> &'static str {
    match message {
        ServerMessage::Snapshot { .. } => "a snapshot",
        ServerMessage::Delta { .. } => "a delta",
        ServerMessage::Telemetry { .. } => "a telemetry frame",
        ServerMessage::Ack { .. } => "an acknowledgement",
        ServerMessage::Answer { .. } => "an answer",
        ServerMessage::Reject { .. } => "a rejection",
    }
}

#[cfg(test)]
mod tests {
    use super::{Client, ClientError, ClientEvent};
    use crate::message::{ClientKind, DaemonHealth, Hello, RejectReason, ServerMessage, Snapshot};
    use crate::telemetry::{TelemetryFrame, UniverseLevels};
    use crate::transport::{Wire, memory};
    use prism_domain::{
        Answer, Command, Delta, JsonValue, NoticeLevel, ProgrammerState, Query, UniverseId,
    };

    fn snapshot() -> Snapshot {
        Snapshot {
            show: JsonValue::String("show".to_owned()),
            session: JsonValue::String("session".to_owned()),
            programmer: ProgrammerState::default(),
            outputs: Vec::new(),
            health: DaemonHealth::default(),
            fixture_library: 0,
            machine: prism_domain::MachineSettings::default(),
            show_file: prism_domain::ShowFileInfo::default(),
        }
    }

    /// Answers one handshake with `answer` and hands back the daemon's wire.
    async fn daemon_answering(answer: ServerMessage) -> (Client, Snapshot, Wire) {
        let (client_wire, mut daemon) = memory::pair();
        let serving = tokio::spawn(async move {
            let _hello = daemon.recv().await;
            daemon.send_message(&answer).await.unwrap();
            daemon
        });
        let (client, snapshot) = Client::handshake(client_wire, Hello::new(ClientKind::Desktop))
            .await
            .unwrap();
        (client, snapshot, serving.await.unwrap())
    }

    #[tokio::test]
    async fn a_handshake_answers_with_the_world() {
        let (_client, received, _daemon) = daemon_answering(ServerMessage::Snapshot {
            snapshot: Box::new(snapshot()),
        })
        .await;
        assert_eq!(received, snapshot());
    }

    #[tokio::test]
    async fn a_rejected_handshake_says_which_refusal_it_was() {
        let (client_wire, mut daemon) = memory::pair();
        tokio::spawn(async move {
            let _hello = daemon.recv().await;
            daemon
                .send_message(&ServerMessage::Reject {
                    seq: None,
                    reason: RejectReason::ProtocolVersion,
                    message: "different versions".to_owned(),
                })
                .await
                .unwrap();
            daemon.shutdown().await;
        });

        let error = Client::handshake(client_wire, Hello::new(ClientKind::Desktop))
            .await
            .unwrap_err();
        assert_eq!(
            error,
            ClientError::Rejected {
                reason: RejectReason::ProtocolVersion,
                message: "different versions".to_owned(),
            }
        );
        assert!(error.to_string().contains("different versions"));
    }

    #[tokio::test]
    async fn a_daemon_that_answers_the_handshake_with_something_else_is_an_error() {
        let (client_wire, mut daemon) = memory::pair();
        tokio::spawn(async move {
            let _hello = daemon.recv().await;
            daemon
                .send_message(&ServerMessage::Ack { seq: 0 })
                .await
                .unwrap();
            daemon.shutdown().await;
        });
        assert_eq!(
            Client::handshake(client_wire, Hello::new(ClientKind::Desktop))
                .await
                .unwrap_err(),
            ClientError::UnexpectedAnswer("an acknowledgement".to_owned())
        );
    }

    #[tokio::test]
    async fn a_daemon_that_never_answers_is_a_closed_connection() {
        let (client_wire, daemon) = memory::pair();
        drop(daemon);
        assert_eq!(
            Client::handshake(client_wire, Hello::new(ClientKind::Desktop))
                .await
                .unwrap_err(),
            ClientError::Closed
        );
    }

    #[tokio::test]
    async fn commands_are_numbered_from_zero_and_the_number_is_the_answer() {
        let (mut client, _snapshot, mut daemon) = daemon_answering(ServerMessage::Snapshot {
            snapshot: Box::new(snapshot()),
        })
        .await;

        assert_eq!(client.send(Command::ClearProgrammer).await.unwrap(), 0);
        assert_eq!(client.send(Command::Oops).await.unwrap(), 1);

        let first = daemon
            .recv_message::<crate::message::ClientMessage>()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            first,
            crate::message::ClientMessage::Command {
                seq: 0,
                command: Command::ClearProgrammer
            }
        );
    }

    #[tokio::test]
    async fn every_message_the_daemon_sends_becomes_the_event_for_it() {
        let (mut client, _snapshot, daemon) = daemon_answering(ServerMessage::Snapshot {
            snapshot: Box::new(snapshot()),
        })
        .await;

        let mut levels = UniverseLevels::blackout(UniverseId::new(2));
        levels.levels[7] = 200;
        let frame = TelemetryFrame {
            sequence: 3,
            universes: vec![levels],
        };
        let notice = Delta::Notice {
            level: NoticeLevel::Warn,
            message: "careful".to_owned(),
        };

        for message in [
            ServerMessage::Delta {
                delta: notice.clone(),
            },
            ServerMessage::Telemetry {
                data: frame.encode(),
            },
            ServerMessage::Ack { seq: 8 },
            ServerMessage::Reject {
                seq: Some(8),
                reason: RejectReason::CommandRefused,
                message: "nothing to store".to_owned(),
            },
        ] {
            daemon.send_message(&message).await.unwrap();
        }

        assert_eq!(
            client.next_event().await.unwrap().unwrap(),
            ClientEvent::Delta(notice)
        );
        assert_eq!(
            client.next_event().await.unwrap().unwrap(),
            ClientEvent::Telemetry(frame)
        );
        assert_eq!(
            client.next_event().await.unwrap().unwrap(),
            ClientEvent::Ack { seq: 8 }
        );
        assert_eq!(
            client.next_event().await.unwrap().unwrap(),
            ClientEvent::Refused {
                seq: Some(8),
                reason: RejectReason::CommandRefused,
                message: "nothing to store".to_owned(),
            }
        );

        daemon.shutdown().await;
        assert_eq!(client.next_event().await, None);
    }

    /// Telemetry is droppable, so an unreadable frame is reported and the
    /// connection carries on rather than ending over a picture of the lights.
    #[tokio::test]
    async fn an_unreadable_telemetry_frame_is_reported_and_survived() {
        let (mut client, _snapshot, daemon) = daemon_answering(ServerMessage::Snapshot {
            snapshot: Box::new(snapshot()),
        })
        .await;

        daemon
            .send_message(&ServerMessage::Telemetry { data: vec![0; 16] })
            .await
            .unwrap();
        daemon
            .send_message(&ServerMessage::Ack { seq: 1 })
            .await
            .unwrap();

        assert!(matches!(
            client.next_event().await.unwrap(),
            Err(ClientError::Telemetry(_))
        ));
        assert_eq!(
            client.next_event().await.unwrap().unwrap(),
            ClientEvent::Ack { seq: 1 }
        );
    }

    /// A question (§5.2) shares the command numbering, and its answer echoes
    /// that number — which is what lets a client with several in flight know
    /// which draft an answer belongs to.
    #[tokio::test]
    async fn a_question_is_numbered_with_the_commands_and_its_answer_echoes_it() {
        let (mut client, _snapshot, mut daemon) = daemon_answering(ServerMessage::Snapshot {
            snapshot: Box::new(snapshot()),
        })
        .await;

        assert_eq!(client.send(Command::ClearProgrammer).await.unwrap(), 0);
        assert_eq!(client.ask(Query::PatchConflicts).await.unwrap(), 1);
        let sent = daemon
            .recv_message::<crate::message::ClientMessage>()
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            sent,
            crate::message::ClientMessage::Command { seq: 0, .. }
        ));
        let asked = daemon
            .recv_message::<crate::message::ClientMessage>()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            asked,
            crate::message::ClientMessage::Query {
                seq: 1,
                query: Query::PatchConflicts,
            }
        );

        let answer = Answer::PatchConflicts {
            conflicts: Vec::new(),
        };
        daemon
            .send_message(&ServerMessage::Answer {
                seq: 1,
                answer: answer.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            client.next_event().await.unwrap().unwrap(),
            ClientEvent::Answered { seq: 1, answer }
        );
    }

    #[tokio::test]
    async fn a_second_snapshot_is_not_part_of_this_protocol() {
        let (mut client, _snapshot, daemon) = daemon_answering(ServerMessage::Snapshot {
            snapshot: Box::new(snapshot()),
        })
        .await;
        daemon
            .send_message(&ServerMessage::Snapshot {
                snapshot: Box::new(snapshot()),
            })
            .await
            .unwrap();
        assert!(matches!(
            client.next_event().await.unwrap(),
            Err(ClientError::UnexpectedAnswer(_))
        ));
    }

    #[tokio::test]
    async fn a_disconnect_leaves_nothing_behind() {
        let (client, _snapshot, mut daemon) = daemon_answering(ServerMessage::Snapshot {
            snapshot: Box::new(snapshot()),
        })
        .await;
        client.disconnect().await;
        assert_eq!(daemon.recv().await, None);
    }

    #[test]
    fn every_error_says_something_a_person_could_act_on() {
        for error in [
            ClientError::Wire(crate::WireError::Closed),
            ClientError::Rejected {
                reason: RejectReason::Unauthorised,
                message: "no token".to_owned(),
            },
            ClientError::UnexpectedAnswer("a delta".to_owned()),
            ClientError::Telemetry(crate::TelemetryError::NotTelemetry),
            ClientError::Closed,
        ] {
            assert!(error.to_string().len() > 15, "{error:?}");
        }
        assert_eq!(
            ClientError::from(crate::WireError::Closed),
            ClientError::Wire(crate::WireError::Closed)
        );
    }

    #[test]
    fn every_message_has_a_name_for_an_error_message() {
        use super::describe;
        assert_eq!(
            describe(&ServerMessage::Snapshot {
                snapshot: Box::new(snapshot())
            }),
            "a snapshot"
        );
        assert_eq!(
            describe(&ServerMessage::Delta {
                delta: Delta::DirtyFlag {
                    unsaved_changes: false
                }
            }),
            "a delta"
        );
        assert_eq!(
            describe(&ServerMessage::Telemetry { data: vec![] }),
            "a telemetry frame"
        );
        assert_eq!(
            describe(&ServerMessage::Ack { seq: 0 }),
            "an acknowledgement"
        );
        assert_eq!(
            describe(&ServerMessage::Reject {
                seq: None,
                reason: RejectReason::Undecodable,
                message: String::new()
            }),
            "a rejection"
        );
    }
}
