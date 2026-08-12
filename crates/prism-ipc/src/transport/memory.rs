//! A connected pair of wires with no socket under them.
//!
//! `CLAUDE.md` requires every test to run deterministically with no hardware
//! attached, and a listening socket is hardware's nearest relative: it needs a
//! free port or a writable directory, it leaves something behind when a test
//! panics, and two tests running at once can collide over it.
//!
//! [`pair`] is the answer, and it is the same move `prism-protocols` made with
//! `MockOutput` in S7 — the abstraction was chosen so the tests could implement
//! it too. It is a real transport, not a stub: the framing of
//! [`super::stream`] runs over it unchanged, so a frame boundary here is
//! computed exactly as it is on a pipe.

use crate::transport::{Wire, stream};

/// Bytes of buffer in each direction.
///
/// Larger than [`crate::MAX_FRAME_BYTES`] so a test can write a whole
/// legitimate frame without a reader present. A test that wants the *pump* to
/// block writes more than this, deliberately.
const DUPLEX_CAPACITY: usize = 2 * crate::MAX_FRAME_BYTES;

/// Two wires connected to each other.
///
/// Conventionally the first is the client's end and the second the daemon's,
/// but nothing here enforces that: a wire carries payloads and does not know
/// which side of the protocol it is on.
#[must_use]
pub fn pair() -> (Wire, Wire) {
    let (here, there) = tokio::io::duplex(DUPLEX_CAPACITY);
    (stream::spawn(here), stream::spawn(there))
}

#[cfg(test)]
mod tests {
    use super::pair;
    use crate::message::{ClientKind, ClientMessage, Hello};

    #[tokio::test]
    async fn the_two_ends_are_connected_to_each_other() {
        let (client, mut daemon) = pair();
        let hello = ClientMessage::Hello {
            hello: Hello::new(ClientKind::WebRemote),
        };
        client.send_message(&hello).await.unwrap();
        assert_eq!(
            daemon
                .recv_message::<ClientMessage>()
                .await
                .unwrap()
                .unwrap(),
            hello
        );
    }

    #[tokio::test]
    async fn it_carries_in_both_directions() {
        let (mut client, daemon) = pair();
        daemon.send(vec![1]).await.unwrap();
        assert_eq!(client.recv().await, Some(Ok(vec![1])));
        client.send(vec![2]).await.unwrap();
        let mut daemon = daemon;
        assert_eq!(daemon.recv().await, Some(Ok(vec![2])));
    }

    #[tokio::test]
    async fn dropping_one_end_closes_the_other() {
        let (client, mut daemon) = pair();
        drop(client);
        assert_eq!(daemon.recv().await, None);
    }
}
