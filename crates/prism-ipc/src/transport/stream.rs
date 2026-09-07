//! The length-prefixed framing of `docs/IPC_PROTOCOL.md` §3, over any byte
//! stream.
//!
//! Both local transports are byte streams — a Windows named pipe and a Unix
//! domain socket differ in how they are opened and in nothing after that — and
//! so is the in-memory duplex the tests use. This module is what turns one into
//! a [`Wire`].
//!
//! # The oversized frame, and where the criterion is actually met
//!
//! `read_loop`, private to this module, reads four bytes, hands them to
//! [`crate::payload_length`], and
//! only builds a body buffer if that answers `Ok`. A peer announcing four
//! gigabytes therefore costs four bytes of stack, one comparison and a
//! disconnection — never a `Vec::with_capacity` it will not fill.
//!
//! The order matters more than it looks. The obvious loop reads the length, then
//! allocates, then checks; it passes every functional test, because the
//! connection does close and the error is right. `tests/oversized_frame.rs`
//! counts what the allocator was asked for, which is the only way to tell the
//! two apart.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::frame::{LENGTH_PREFIX_BYTES, payload_length};
use crate::transport::{Wire, WireError, WirePump};

/// Wraps a byte stream in the framing and returns the wire it now looks like.
///
/// Spawns two tasks, one per direction, so that a client which has stopped
/// reading cannot stop the daemon from reading what it has already sent.
pub fn spawn<S>(stream: S) -> Wire
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (wire, pump) = Wire::pair();
    let (reader, writer) = tokio::io::split(stream);
    let WirePump {
        outbound,
        inbound,
        flushed,
    } = pump;
    tokio::spawn(read_loop(reader, inbound));
    tokio::spawn(async move {
        write_loop(writer, outbound).await;
        let _ = flushed.send(());
    });
    wire
}

/// Reads frames until the stream ends, refuses to make sense, or nobody is
/// listening any more.
///
/// # The third of those is not an optimisation
///
/// A reader that only ever stops at end of stream keeps its half of the socket
/// alive, and a socket is only closed when **both** halves are dropped. On a
/// Windows named pipe, shutting down the writing half of a split handle does not
/// close the pipe — so a client that disconnected while the daemon was saying
/// nothing would leave the daemon reading a handle that will never produce
/// another byte, and the daemon's client list would never lose it. That is a
/// resource leak in the process `docs/IPC_PROTOCOL.md` §8 promises will *free
/// the client's state and carry on*.
///
/// So the reader also stops when the [`Wire`]'s receiving half has been dropped:
/// there is nowhere left to deliver to, the read half is released, and the
/// socket closes on both sides. Found by the transport parity suite, which is
/// what a suite that runs identically over three transports is for.
async fn read_loop<R>(mut reader: R, inbound: mpsc::Sender<Result<Vec<u8>, WireError>>)
where
    R: AsyncRead + Unpin,
{
    loop {
        let mut header = [0_u8; LENGTH_PREFIX_BYTES];
        let read = tokio::select! {
            biased;
            () = inbound.closed() => return,
            result = reader.read_exact(&mut header) => result,
        };
        match read {
            Ok(_) => {}
            // A closed connection is not a failure. It is how every connection
            // ends, including the ones that ended well.
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return,
            Err(error) => {
                let _ = inbound.send(Err(WireError::Io(error.to_string()))).await;
                return;
            }
        }

        // Before any buffer exists. This line is the criterion.
        let length = match payload_length(header) {
            Ok(length) => length,
            Err(error) => {
                let _ = inbound.send(Err(WireError::Frame(error))).await;
                return;
            }
        };

        let mut payload = vec![0_u8; length];
        let read = tokio::select! {
            biased;
            () = inbound.closed() => return,
            result = reader.read_exact(&mut payload) => result,
        };
        if let Err(error) = read {
            let _ = inbound.send(Err(WireError::Io(error.to_string()))).await;
            return;
        }
        if inbound.send(Ok(payload)).await.is_err() {
            return;
        }
    }
}

/// Writes frames until the wire is dropped or the stream fails.
async fn write_loop<W>(mut writer: W, mut outbound: mpsc::Receiver<Vec<u8>>)
where
    W: AsyncWrite + Unpin,
{
    while let Some(payload) = outbound.recv().await {
        // The cast is sound: `encode` refuses anything above MAX_FRAME_BYTES,
        // which is a thousandth of u32::MAX.
        let header = (payload.len() as u32).to_le_bytes();
        if writer.write_all(&header).await.is_err() || writer.write_all(&payload).await.is_err() {
            break;
        }
    }
    let _ = writer.flush().await;
    let _ = writer.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::spawn;
    use crate::frame::{FrameError, MAX_FRAME_BYTES};
    use crate::transport::WireError;
    use prism_domain::Command;
    use tokio::io::AsyncWriteExt;

    /// A duplex pair standing in for a socket, so the framing is tested without
    /// one.
    fn duplex() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
        tokio::io::duplex(64 * 1024)
    }

    #[tokio::test]
    async fn a_payload_arrives_as_it_was_sent() {
        let (here, there) = duplex();
        let left = spawn(here);
        let mut right = spawn(there);

        left.send(vec![1, 2, 3]).await.unwrap();
        assert_eq!(right.recv().await, Some(Ok(vec![1, 2, 3])));
    }

    #[tokio::test]
    async fn an_empty_payload_is_a_payload() {
        let (here, there) = duplex();
        let left = spawn(here);
        let mut right = spawn(there);
        left.send(Vec::new()).await.unwrap();
        assert_eq!(right.recv().await, Some(Ok(Vec::new())));
    }

    #[tokio::test]
    async fn frames_keep_their_order_and_their_boundaries() {
        let (here, there) = duplex();
        let left = spawn(here);
        let mut right = spawn(there);
        for n in 0_u8..32 {
            left.send(vec![n; usize::from(n) + 1]).await.unwrap();
        }
        for n in 0_u8..32 {
            assert_eq!(right.recv().await, Some(Ok(vec![n; usize::from(n) + 1])));
        }
    }

    /// A frame split across reads must still arrive whole: the length prefix is
    /// what makes a stream into messages, and a reader that trusted one read to
    /// be one frame would work on a duplex and fail on a socket.
    #[tokio::test]
    async fn a_frame_split_across_writes_still_arrives_whole() {
        let (here, mut there) = duplex();
        let mut left = spawn(here);

        let payload = vec![7_u8; 1000];
        let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
        frame.extend_from_slice(&payload);
        for chunk in frame.chunks(7) {
            there.write_all(chunk).await.unwrap();
            tokio::task::yield_now().await;
        }
        assert_eq!(left.recv().await, Some(Ok(payload)));
    }

    /// The criterion, at the functional level. The memory half is
    /// `tests/oversized_frame.rs`.
    #[tokio::test]
    async fn an_oversized_frame_closes_the_connection() {
        let (here, mut there) = duplex();
        let mut left = spawn(here);

        there
            .write_all(&u32::MAX.to_le_bytes())
            .await
            .expect("the header fits in the duplex buffer");

        assert_eq!(
            left.recv().await,
            Some(Err(WireError::Frame(FrameError::TooLarge {
                announced: u64::from(u32::MAX),
                limit: MAX_FRAME_BYTES,
            })))
        );
        assert_eq!(left.recv().await, None, "the reader stopped after refusing");
    }

    #[tokio::test]
    async fn a_frame_exactly_at_the_limit_is_carried() {
        let (here, there) = duplex();
        let left = spawn(here);
        let mut right = spawn(there);
        let payload = vec![3_u8; MAX_FRAME_BYTES];
        left.send(payload.clone()).await.unwrap();
        assert_eq!(right.recv().await, Some(Ok(payload)));
    }

    #[tokio::test]
    async fn a_stream_that_ends_mid_frame_is_an_error_rather_than_a_short_message() {
        let (here, mut there) = duplex();
        let mut left = spawn(here);

        there.write_all(&100_u32.to_le_bytes()).await.unwrap();
        there.write_all(&[0; 10]).await.unwrap();
        drop(there);

        assert!(matches!(left.recv().await, Some(Err(WireError::Io(_)))));
    }

    #[tokio::test]
    async fn a_closed_stream_ends_the_wire_without_an_error() {
        let (here, there) = duplex();
        let mut left = spawn(here);
        drop(there);
        assert_eq!(left.recv().await, None);
    }

    #[tokio::test]
    async fn a_message_survives_the_stream() {
        let (here, there) = duplex();
        let left = spawn(here);
        let mut right = spawn(there);
        left.send_message(&Command::ClearProgrammer).await.unwrap();
        assert_eq!(
            right.recv_message::<Command>().await.unwrap().unwrap(),
            Command::ClearProgrammer
        );
    }

    /// `shutdown` exists so a final `Reject` is known to have reached the
    /// socket. This is that promise, on a real stream.
    #[tokio::test]
    async fn a_shutdown_flushes_what_was_already_sent() {
        let (here, there) = duplex();
        let left = spawn(here);
        let mut right = spawn(there);

        left.send(vec![1, 2, 3]).await.unwrap();
        left.shutdown().await;

        assert_eq!(right.recv().await, Some(Ok(vec![1, 2, 3])));
        assert_eq!(right.recv().await, None);
    }
}
