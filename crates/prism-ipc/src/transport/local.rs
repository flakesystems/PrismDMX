//! The local transport: a named pipe on Windows, a Unix domain socket
//! elsewhere.
//!
//! `docs/IPC_PROTOCOL.md` §2 gives the reasons — lowest latency, no TCP stack,
//! no port to firewall, and no accidental network exposure. The last is the one
//! that matters in a school: a local transport cannot be reached from another
//! machine, whatever anybody configures.
//!
//! # The only `#[cfg]` in this crate, and why it is allowed to be here
//!
//! `ARCHITECTURE_SPEC.md` §10.1 confines `#[cfg(target_os = …)]` to
//! `prism-protocols` and `prism-app`. This module is a third place, and the case
//! for it is the same one §10.1 makes for the FTDI backend: the *selection* of a
//! platform primitive is platform code, and everything above it must not be.
//!
//! So the split is drawn as narrowly as it can be. Two `#[cfg]` blocks, both in
//! this file, both choosing a type: `NamedPipeServer` or `UnixListener`,
//! `ClientOptions::open` or `UnixStream::connect`. Both hand back the same
//! [`Wire`] through the same [`super::stream::spawn`], and the framing, the
//! handshake, the backpressure policy, the server and the client are one code
//! path on every target. CI runs this crate's tests in the **Linux** job as well
//! as the Windows one, which is what keeps that claim honest.
//!
//! # A stale socket file is not this module's problem
//!
//! On Unix a listener leaves a file behind, and binding over an existing one
//! fails. That is deliberate: §2.2 gives `prismd` a lock file with a PID, and
//! deciding that a previous daemon is dead is a liveness question, not a file
//! system question. A transport that silently unlinked whatever it found would
//! defeat the single-instance guarantee it exists under
//! (`ARCHITECTURE_SPEC.md` §10.3) — two daemons driving the same rig is the
//! worst failure mode this project has. **S17 owns the takeover.**

use std::io;

use crate::transport::{Endpoint, Wire, stream};

/// A listener for the local transport.
#[derive(Debug)]
pub struct LocalListener {
    address: String,
    #[cfg(windows)]
    server: Option<tokio::net::windows::named_pipe::NamedPipeServer>,
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
}

impl LocalListener {
    /// Starts listening at `address`.
    ///
    /// The address is the operating system's own name for the endpoint: a pipe
    /// name like `\\.\pipe\prismd` on Windows, a file system path elsewhere.
    ///
    /// # Errors
    ///
    /// Whatever the operating system says. On Unix an existing socket file at
    /// `address` is an error rather than something to remove — see the module
    /// documentation.
    #[cfg(windows)]
    pub fn bind(address: &str) -> io::Result<Self> {
        use tokio::net::windows::named_pipe::ServerOptions;

        // `first_pipe_instance` is the single-instance guarantee at the level
        // the operating system can enforce it: a second daemon asking for the
        // same pipe name is refused by Windows rather than quietly sharing it.
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(address)?;
        Ok(Self {
            address: address.to_owned(),
            server: Some(server),
        })
    }

    /// Starts listening at `address`.
    ///
    /// # Errors
    ///
    /// Whatever the operating system says, including an existing socket file at
    /// `address` — see the module documentation for why that is not swept away.
    #[cfg(unix)]
    pub fn bind(address: &str) -> io::Result<Self> {
        Ok(Self {
            address: address.to_owned(),
            listener: tokio::net::UnixListener::bind(address)?,
        })
    }

    /// Waits for the next client.
    ///
    /// # Errors
    ///
    /// Whatever the operating system says.
    #[cfg(windows)]
    pub async fn accept(&mut self) -> io::Result<Wire> {
        use tokio::net::windows::named_pipe::ServerOptions;

        let server = match self.server.take() {
            Some(server) => server,
            None => ServerOptions::new().create(&self.address)?,
        };
        if let Err(error) = server.connect().await {
            self.server = Some(server);
            return Err(error);
        }
        // The next instance is created only once this one has a client, which is
        // the order Windows wants: a client arriving in the gap is told the pipe
        // is busy and retries, which is what `connect` below does.
        self.server = Some(ServerOptions::new().create(&self.address)?);
        Ok(stream::spawn(server))
    }

    /// Waits for the next client.
    ///
    /// # Errors
    ///
    /// Whatever the operating system says.
    #[cfg(unix)]
    pub async fn accept(&mut self) -> io::Result<Wire> {
        let (socket, _) = self.listener.accept().await?;
        Ok(stream::spawn(socket))
    }

    /// The address this listener is on.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The address as an endpoint, for the lock file of §2.2.
    #[must_use]
    pub fn endpoint(&self) -> Endpoint {
        Endpoint::Local(self.address.clone())
    }
}

#[cfg(unix)]
impl Drop for LocalListener {
    fn drop(&mut self) {
        // The socket file outlives the listener otherwise, and the next daemon
        // would refuse to bind over it. Failure is ignored on purpose: there is
        // nothing useful to do about it while unwinding, and S17's stale-lock
        // takeover is the path that has to cope with the file being there.
        let _ = std::fs::remove_file(&self.address);
    }
}

/// How long a client keeps retrying a pipe that is busy.
#[cfg(windows)]
const BUSY_RETRY_LIMIT: std::time::Duration = std::time::Duration::from_secs(2);

/// `ERROR_PIPE_BUSY`. Every instance of the pipe already has a client, and the
/// daemon has not yet created the next one — the gap in `accept` above.
#[cfg(windows)]
const ERROR_PIPE_BUSY: i32 = 231;

/// Connects to a daemon listening on the local transport.
///
/// # Errors
///
/// Whatever the operating system says, including "there is nothing there",
/// which is how a client discovers the daemon is not running.
#[cfg(windows)]
pub async fn connect(address: &str) -> io::Result<Wire> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let deadline = std::time::Instant::now() + BUSY_RETRY_LIMIT;
    loop {
        match ClientOptions::new().open(address) {
            Ok(client) => return Ok(stream::spawn(client)),
            // Busy means the daemon is there and between instances, so retrying
            // is right. Anything else — no such pipe, access denied — is an
            // answer, and repeating the question would not change it.
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                if std::time::Instant::now() >= deadline {
                    return Err(error);
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Connects to a daemon listening on the local transport.
///
/// # Errors
///
/// Whatever the operating system says, including "there is nothing there",
/// which is how a client discovers the daemon is not running.
#[cfg(unix)]
pub async fn connect(address: &str) -> io::Result<Wire> {
    Ok(stream::spawn(
        tokio::net::UnixStream::connect(address).await?,
    ))
}

/// The address a daemon listens on, for a caller that has a label to
/// distinguish its instance by.
///
/// The platform split of this module, in the one place a *findable* address is
/// needed: a pipe name under Windows, a socket path elsewhere. The label is
/// what stops two daemons on one machine — two user accounts, each with their
/// own data directory — from asking for the same name; `prismd` derives it from
/// its data directory so that a client which knows the directory can work out
/// the address without reading anything.
///
/// It is still written into the lock file of §2.2, because that is what makes
/// discovery a matter of reading one file rather than of two programs agreeing
/// about a hash.
///
/// The socket goes under the temporary directory rather than beside the show,
/// for the reason [`scratch_address`] gives: the kernel limits a Unix socket
/// path to about a hundred bytes, and a user's home directory can be most of
/// that on its own.
#[must_use]
pub fn daemon_address(label: &str) -> String {
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\prismdmx-{label}")
    }
    #[cfg(unix)]
    {
        std::env::temp_dir()
            .join(format!("prismdmx-{label}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

/// An address nothing else is using.
///
/// Named for what it is: a scratch endpoint. Tests need one per test, because a
/// pipe name and a socket path are both global to the machine and a suite that
/// shared one would fail only when two of its tests happened to overlap. S17's
/// tests need the same thing, which is why this is here rather than duplicated
/// in each target's scenery.
///
/// The real daemon does **not** use this: its endpoint goes into the lock file
/// of §2.2 so clients can find it, and a name with a process id in it could not
/// be found by anybody.
#[must_use]
pub fn scratch_address(label: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();

    #[cfg(windows)]
    {
        format!(r"\\.\pipe\prismdmx-{pid}-{unique}-{label}")
    }
    #[cfg(unix)]
    {
        // Under the temporary directory rather than beside the show: a Unix
        // socket path is limited to about a hundred bytes by the kernel, and a
        // user's home directory can be most of that on its own.
        std::env::temp_dir()
            .join(format!("prismdmx-{pid}-{unique}-{label}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{LocalListener, connect, scratch_address};
    use crate::message::{ClientKind, ClientMessage, Hello};
    use crate::transport::Endpoint;

    #[tokio::test]
    async fn a_client_reaches_a_listener_and_messages_travel_both_ways() {
        let address = scratch_address("both-ways");
        let mut listener = LocalListener::bind(&address).unwrap();

        let dialling = tokio::spawn({
            let address = address.clone();
            async move { connect(&address).await.unwrap() }
        });
        let mut server_side = listener.accept().await.unwrap();
        let mut client_side = dialling.await.unwrap();

        let hello = ClientMessage::Hello {
            hello: Hello::new(ClientKind::Desktop),
        };
        client_side.send_message(&hello).await.unwrap();
        assert_eq!(
            server_side
                .recv_message::<ClientMessage>()
                .await
                .unwrap()
                .unwrap(),
            hello
        );

        server_side.send(vec![9, 9]).await.unwrap();
        assert_eq!(client_side.recv().await, Some(Ok(vec![9, 9])));
    }

    #[tokio::test]
    async fn a_listener_serves_one_client_after_another() {
        let address = scratch_address("serial");
        let mut listener = LocalListener::bind(&address).unwrap();
        assert_eq!(listener.address(), address);
        assert_eq!(listener.endpoint(), Endpoint::Local(address.clone()));

        for n in 0_u8..3 {
            let dialling = tokio::spawn({
                let address = address.clone();
                async move { connect(&address).await.unwrap() }
            });
            let mut server_side = listener.accept().await.unwrap();
            let client_side = dialling.await.unwrap();

            client_side.send(vec![n]).await.unwrap();
            assert_eq!(server_side.recv().await, Some(Ok(vec![n])));
        }
    }

    #[tokio::test]
    async fn connecting_to_nothing_fails_rather_than_waiting() {
        let error = connect(&scratch_address("absent")).await.unwrap_err();
        assert!(
            error.kind() == std::io::ErrorKind::NotFound
                || error.kind() == std::io::ErrorKind::ConnectionRefused,
            "a daemon that is not running should look like one: {error:?}"
        );
    }

    #[tokio::test]
    async fn two_listeners_cannot_share_one_address() {
        // The transport-level half of the single-instance guarantee: the second
        // daemon is refused by the operating system, before anything of ours has
        // to decide.
        let address = scratch_address("single");
        let first = LocalListener::bind(&address).unwrap();
        assert!(LocalListener::bind(&address).is_err());
        drop(first);
    }

    /// The address a daemon publishes has to be the same one every time it
    /// starts, or the lock file of §2.2 is the only way to find it and a client
    /// that has lost the file has lost the daemon.
    #[tokio::test]
    async fn a_daemon_address_is_stable_for_a_label_and_different_between_labels() {
        use super::daemon_address;

        assert_eq!(daemon_address("aula"), daemon_address("aula"));
        assert_ne!(daemon_address("aula"), daemon_address("studio"));
        assert!(daemon_address("aula").contains("aula"));
        assert!(!daemon_address("aula").contains(&std::process::id().to_string()));

        // And it is an address a listener actually takes.
        let mut listener = LocalListener::bind(&daemon_address("prismd-test-label")).unwrap();
        let address = listener.address().to_owned();
        let dialling = tokio::spawn(async move { connect(&address).await.unwrap() });
        let mut server_side = listener.accept().await.unwrap();
        let client_side = dialling.await.unwrap();
        client_side.send(vec![4]).await.unwrap();
        assert_eq!(server_side.recv().await, Some(Ok(vec![4])));
    }

    #[test]
    fn a_scratch_address_is_different_every_time() {
        let first = scratch_address("a");
        let second = scratch_address("a");
        assert_ne!(first, second);
        assert!(first.contains(&std::process::id().to_string()));
        assert!(first.contains('a'));
    }
}
