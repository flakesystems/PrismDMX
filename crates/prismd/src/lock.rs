//! The single-instance guarantee, and the discovery file that comes with it.
//!
//! `ARCHITECTURE_SPEC.md` §10.3: *two daemons driving the same output would be
//! the worst possible failure mode, so it is prevented structurally rather than
//! by convention.* `docs/IPC_PROTOCOL.md` §2.2 gives the same file a second job:
//! it holds the daemon's process id and its IPC endpoints, and clients read it
//! to find the daemon.
//!
//! # The liveness check is a file lock, not a process id
//!
//! §10.3 says stale files "are detected through a PID liveness check". This is
//! that check, done a way that is both stronger and portable:
//! [`std::fs::File::try_lock`] takes an advisory lock the operating system
//! releases when the process ends — including when it is killed, which is the
//! case the criterion is about. So a second daemon does not ask *is process 4711
//! alive*; it asks *is anybody holding this*, and the answer cannot be wrong.
//!
//! Two reasons this is not merely a convenient substitute:
//!
//! - **A process id is reusable.** A daemon killed at three o'clock and a text
//!   editor started at four can have the same number, and a liveness probe
//!   would then report a stale lock as live for ever — a desk that refuses to
//!   start and cannot say why.
//! - **A liveness probe is platform code**, and §10.1 does not allow this crate
//!   any. `kill(pid, 0)` and `OpenProcess` are two implementations of one
//!   question, and both need `unsafe` or a `#[cfg]`.
//!
//! The process id is still written into the file, because §2.2 says so and
//! because it is what a person looks at when something is wrong. It is
//! reported, not believed.
//!
//! # Why there are two files
//!
//! An exclusive lock on Windows stops **other processes reading the locked
//! bytes**, and the whole point of §2.2's file is that clients read it. So the
//! lock is held on a file of its own — [`crate::paths::guard_path`], which is
//! empty and exists only to be locked — and the discovery document is an
//! ordinary readable file beside it.
//!
//! # What a takeover has to clean up
//!
//! S16 left `LocalListener::bind` deliberately *not* removing a socket file it
//! finds, because deciding a previous daemon is dead is a liveness question and
//! belongs here. So it is done here: once the guard has been acquired — which
//! is proof that no daemon is running — a Unix domain socket left behind by the
//! previous one is removed, using the address out of the stale document rather
//! than by guessing.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use prism_ipc::Endpoint;
use serde::{Deserialize, Serialize};

/// What a daemon publishes about itself, and what a client reads to find it.
///
/// The endpoints are two `Option`s rather than a list because a client asks a
/// specific question — *is there a local transport?* — and a list would make it
/// search. A daemon with neither is a daemon nothing can reach, which is a
/// legitimate headless configuration and says so by carrying no endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockDocument {
    /// The daemon's process id. For a person and for a status panel — the
    /// decision about whether it is running is the guard's, not this number's.
    pub pid: u32,
    /// The named pipe or Unix domain socket, if the daemon opened one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    /// The WebSocket address, if the daemon opened one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub websocket: Option<String>,
    /// The token §2.1 requires when the WebSocket listener is reachable from
    /// another machine. Absent on loopback, where §2.1 does not ask for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl LockDocument {
    /// A document for this process, carrying no endpoints yet.
    #[must_use]
    pub fn for_this_process() -> Self {
        Self {
            pid: std::process::id(),
            ..Self::default()
        }
    }

    /// Records an endpoint the daemon is listening on.
    pub fn listening_on(&mut self, endpoint: &Endpoint) {
        match endpoint {
            Endpoint::Local(address) => self.local = Some(address.clone()),
            Endpoint::WebSocket(address) => self.websocket = Some(address.to_string()),
        }
    }

    /// What to tell somebody who wants to know where the daemon is.
    #[must_use]
    pub fn endpoints(&self) -> String {
        let mut named: Vec<String> = Vec::new();
        if let Some(local) = &self.local {
            named.push(local.clone());
        }
        if let Some(websocket) = &self.websocket {
            named.push(format!("ws://{websocket}/ipc"));
        }
        if named.is_empty() {
            return "no endpoint".to_owned();
        }
        named.join(", ")
    }
}

/// Why the daemon could not take the lock.
#[derive(Debug)]
pub enum LockError {
    /// Another daemon holds it. `ARCHITECTURE_SPEC.md` §10.3: the shell attaches
    /// to that one instead of starting a second.
    AlreadyRunning(Box<LockDocument>),
    /// The file system said no.
    Io(io::Error),
}

impl core::fmt::Display for LockError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AlreadyRunning(document) => write!(
                f,
                "a PrismDMX daemon is already running (process {}, at {})",
                document.pid,
                document.endpoints()
            ),
            Self::Io(error) => write!(f, "the lock file could not be taken: {error}"),
        }
    }
}

impl core::error::Error for LockError {}

impl From<io::Error> for LockError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// The lock this daemon holds for as long as it is running.
///
/// Dropping it releases the operating system's lock and removes the discovery
/// file. Both also happen if the process is killed — the first because the
/// operating system does it, the second because the next daemon treats a
/// document it can lock the guard beside as stale.
#[derive(Debug)]
pub struct DaemonLock {
    guard: File,
    guard_path: PathBuf,
    lock_path: PathBuf,
    document: LockDocument,
    /// Whether the daemon that started before this one had left a file behind.
    took_over: bool,
}

impl DaemonLock {
    /// Takes the lock in `data_dir`, creating the directory if it is not there.
    ///
    /// # Errors
    ///
    /// [`LockError::AlreadyRunning`] if another daemon holds the guard — with
    /// its document, so the caller can say where it is — or [`LockError::Io`]
    /// if the directory or the files cannot be opened.
    pub fn acquire(data_dir: &Path) -> Result<Self, LockError> {
        std::fs::create_dir_all(data_dir)?;
        let guard_path = crate::paths::guard_path(data_dir);
        let lock_path = crate::paths::lock_path(data_dir);

        let guard = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&guard_path)?;
        match guard.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                // Somebody is holding it. The document beside it describes them,
                // and if it cannot be read the refusal still stands — an
                // unreadable document is a reason to say less, not to start a
                // second daemon.
                let document = read_document(&lock_path).unwrap_or_default();
                return Err(LockError::AlreadyRunning(Box::new(document)));
            }
            Err(TryLockError::Error(error)) => return Err(LockError::Io(error)),
        }

        // The guard is ours, so no daemon is running: anything the previous one
        // left behind is rubbish, and this is the only moment at which that can
        // be said safely. The *file* is what makes it a takeover — a document
        // that does not parse was still written by a daemon that has gone.
        let took_over = lock_path.exists();
        if let Some(stale) = read_document(&lock_path) {
            remove_stale_socket(stale.local.as_deref());
        }

        let lock = Self {
            guard,
            guard_path,
            lock_path,
            document: LockDocument::for_this_process(),
            took_over,
        };
        // Published straight away, before a single endpoint is known. A daemon
        // with no listener at all — the headless configuration
        // `ARCHITECTURE_SPEC.md` §10.2 describes for a Pi — is still a daemon
        // whose process id somebody has to be able to read, and a file that
        // appeared only once a listener bound would leave the previous
        // daemon's document in place for as long as the new one was starting.
        lock.write()?;
        Ok(lock)
    }

    /// Whether a previous daemon had left a lock file behind — a crash, a
    /// killed process, a machine that lost power mid-show.
    #[must_use]
    pub const fn took_over_a_stale_lock(&self) -> bool {
        self.took_over
    }

    /// This daemon's document, as it will be published.
    #[must_use]
    pub const fn document(&self) -> &LockDocument {
        &self.document
    }

    /// Where the discovery file is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.lock_path
    }

    /// Records an endpoint and rewrites the discovery file.
    ///
    /// Called once per listener, after it has bound: an endpoint written before
    /// the listener exists is an address clients would fail to reach.
    ///
    /// # Errors
    ///
    /// [`io::Error`] if the file cannot be written.
    pub fn publish(&mut self, endpoint: &Endpoint) -> io::Result<()> {
        self.document.listening_on(endpoint);
        self.write()
    }

    /// Records the §2.1 token and rewrites the discovery file.
    ///
    /// # Errors
    ///
    /// As [`Self::publish`].
    pub fn publish_token(&mut self, token: &str) -> io::Result<()> {
        self.document.token = Some(token.to_owned());
        self.write()
    }

    fn write(&self) -> io::Result<()> {
        let text = serde_json::to_string_pretty(&self.document)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        std::fs::write(&self.lock_path, text)
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        // The document goes first: a client that reads it between these two
        // steps should find nothing rather than an address that is closing.
        let _ = std::fs::remove_file(&self.lock_path);
        let _ = self.guard.unlock();
        // The guard file itself is left in place. Removing it races with a
        // second daemon that has already opened it and would leave that daemon
        // holding a lock on a file nobody can find, which is the one way to get
        // two daemons past a mutual exclusion built out of one.
        let _ = &self.guard_path;
    }
}

/// Reads the discovery document, or `None` if there is not one to read.
///
/// A document that will not parse is treated as absent rather than as an error:
/// it was written by a daemon that is no longer running, and refusing to start
/// because a dead process left a damaged file would be the fault this whole
/// module exists to prevent.
fn read_document(path: &Path) -> Option<LockDocument> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Removes a Unix domain socket the previous daemon left behind.
///
/// Nothing to do on Windows, where a named pipe has no file: the path names
/// nothing in the file system, so the removal simply fails and is ignored. That
/// is what makes this one code path on every target rather than a `#[cfg]`.
fn remove_stale_socket(address: Option<&str>) {
    let Some(address) = address else { return };
    let path = Path::new(address);
    if path.is_file() || path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::{DaemonLock, LockDocument, LockError, read_document};
    use prism_ipc::Endpoint;
    use std::net::SocketAddr;

    fn websocket() -> Endpoint {
        Endpoint::WebSocket("127.0.0.1:7373".parse::<SocketAddr>().unwrap())
    }

    #[test]
    fn a_lock_publishes_the_pid_and_the_endpoints_where_a_client_can_read_them() {
        let dir = tempfile::tempdir().unwrap();
        let mut lock = DaemonLock::acquire(dir.path()).unwrap();
        assert!(!lock.took_over_a_stale_lock());

        lock.publish(&Endpoint::Local(r"\\.\pipe\prismd-test".to_owned()))
            .unwrap();
        lock.publish(&websocket()).unwrap();
        lock.publish_token("hunter2").unwrap();

        // Read the way a client reads it: from the file, while the daemon holds
        // the lock. That is the whole reason the guard is a second file.
        let document = read_document(lock.path()).unwrap();
        assert_eq!(document.pid, std::process::id());
        assert_eq!(document.local.as_deref(), Some(r"\\.\pipe\prismd-test"));
        assert_eq!(document.websocket.as_deref(), Some("127.0.0.1:7373"));
        assert_eq!(document.token.as_deref(), Some("hunter2"));
        assert_eq!(
            document.endpoints(),
            r"\\.\pipe\prismd-test, ws://127.0.0.1:7373/ipc"
        );
    }

    /// The exit criterion, asserted rather than described: a second instance
    /// detects the first and refuses to start.
    #[test]
    fn a_second_daemon_is_refused_and_told_where_the_first_one_is() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = DaemonLock::acquire(dir.path()).unwrap();
        first.publish(&websocket()).unwrap();

        let refusal = DaemonLock::acquire(dir.path()).unwrap_err();
        let LockError::AlreadyRunning(document) = refusal else {
            panic!("a second daemon must be refused, not {refusal:?}");
        };
        assert_eq!(document.pid, std::process::id());
        assert_eq!(document.websocket.as_deref(), Some("127.0.0.1:7373"));
        assert!(
            LockError::AlreadyRunning(document)
                .to_string()
                .contains("already running")
        );

        // And once the first one has finished, the address is free again.
        drop(first);
        let third = DaemonLock::acquire(dir.path()).unwrap();
        assert!(
            !third.took_over_a_stale_lock(),
            "a daemon that shut down cleanly leaves nothing to take over"
        );
    }

    /// The other exit criterion: a lock file left by a killed process is
    /// recognised and taken over.
    ///
    /// The kill is simulated the only way it can be inside one process — by
    /// writing the document a dead daemon would have left and *not* holding the
    /// guard, which is exactly the state the operating system puts the file
    /// system in when it reaps a process.
    #[test]
    fn a_lock_file_left_by_a_dead_daemon_is_taken_over() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("prismd.sock");
        std::fs::write(&socket, b"a socket file the dead daemon left").unwrap();
        let corpse = LockDocument {
            pid: 4711,
            local: Some(socket.to_string_lossy().into_owned()),
            websocket: Some("127.0.0.1:7373".to_owned()),
            token: None,
        };
        std::fs::write(
            crate::paths::lock_path(dir.path()),
            serde_json::to_string(&corpse).unwrap(),
        )
        .unwrap();

        let lock = DaemonLock::acquire(dir.path()).unwrap();
        assert!(lock.took_over_a_stale_lock());
        assert_eq!(lock.document().pid, std::process::id());
        assert!(
            !socket.exists(),
            "the socket file the dead daemon left has to go, or the next bind fails"
        );
    }

    #[test]
    fn a_damaged_lock_file_is_taken_over_rather_than_believed() {
        // Written by a daemon that died mid-write. Refusing to start over it
        // would be the failure this module exists to prevent.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(crate::paths::lock_path(dir.path()), b"{\"pid\": ").unwrap();
        let lock = DaemonLock::acquire(dir.path()).unwrap();
        assert!(lock.took_over_a_stale_lock());
        assert_eq!(lock.document().pid, std::process::id());
    }

    #[test]
    fn the_data_directory_is_created_if_it_is_not_there() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("one").join("two");
        let lock = DaemonLock::acquire(&nested).unwrap();
        assert!(nested.is_dir());
        assert_eq!(lock.path(), crate::paths::lock_path(&nested));
    }

    #[test]
    fn a_finished_daemon_leaves_no_discovery_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let mut lock = DaemonLock::acquire(dir.path()).unwrap();
        lock.publish(&websocket()).unwrap();
        let path = lock.path().to_path_buf();
        assert!(path.is_file());
        drop(lock);
        assert!(
            !path.exists(),
            "a client must not find the address of a daemon that has stopped"
        );
    }

    #[test]
    fn a_document_with_no_endpoints_says_so() {
        let document = LockDocument::for_this_process();
        assert_eq!(document.endpoints(), "no endpoint");
        assert_eq!(document.pid, std::process::id());
    }

    #[test]
    fn a_lock_file_names_a_socket_that_is_not_there_and_nothing_happens() {
        // The Windows case, where the recorded address is a pipe name and not a
        // path at all — and the case of a Unix daemon whose socket was already
        // cleaned up.
        super::remove_stale_socket(Some(r"\\.\pipe\prismd-nothing"));
        super::remove_stale_socket(None);
    }

    #[test]
    fn a_lock_error_from_the_file_system_says_what_happened() {
        let error = LockError::from(std::io::Error::other("no room"));
        assert!(error.to_string().contains("no room"));
        let as_error: &dyn core::error::Error = &error;
        assert!(!as_error.to_string().is_empty());
    }
}
