//! *Spawn or attach* — one sentence, two cases, and the two edges between them.
//!
//! `ARCHITECTURE_SPEC.md` §10.3: *if the shell finds a live daemon it attaches
//! instead of spawning a second one. Two daemons driving the same output would
//! be the worst possible failure mode, so it is prevented structurally rather
//! than by convention.* S17 built the structure — an advisory lock on
//! `prismd.guard`, and `prismd.lock` beside it saying where the daemon is — and
//! this module is its first real caller.
//!
//! # The two edges, and what S29 decided about them
//!
//! **A document that names a daemon this shell cannot reach.** The lock file
//! says there is a WebSocket listener and the window cannot open one, or the
//! daemon has no WebSocket listener at all — a desk configured `--no-websocket`,
//! or one whose listener could not bind, which §10.3 makes a warning rather than
//! a refusal. The shell **does not spawn a second daemon**, and it does not
//! silently show an empty screen either: it says which process is holding the
//! desk, where that process said it could be reached, and which data directory
//! the two of them are arguing about. The reason is the sentence at the top: the
//! only thing standing between a venue and two desks driving one rig is that
//! nobody starts a second, and *I could not reach the first one* is not evidence
//! that there is not one — it is usually evidence that somebody switched the
//! listener off.
//!
//! A shell that spawned anyway would not in fact produce two daemons, because
//! `DaemonLock::acquire` refuses the second. It would produce something worse to
//! diagnose: a program that appears to start, briefly, and then reports a lock
//! error from a process the operator never asked for.
//!
//! **A daemon belonging to a different installation.** The data directory is the
//! identity here, not the executable: two installations sharing one user's
//! `%APPDATA%\PrismDMX` *are* one desk by definition — the show, the machine
//! configuration, the rig and the lock are one set of files — and an installation
//! pointed at a directory of its own (`PRISMD_DATA_DIR`, §10.3) never sees this
//! one's lock at all. So *a different installation* can only mean **a different
//! build**, and the shell deliberately does not compare its own version against
//! the running daemon's. That question already has an owner: the handshake.
//! `docs/IPC_PROTOCOL.md` §4.2 makes `PROTOCOL_VERSION` the compatibility test
//! and refuses a client that does not match, with a message about versions
//! rather than about files. A second check here would be a second opinion, and
//! it is the one that would be wrong first — two builds can share a protocol
//! version, and two versions of one build cannot be told apart by a path.
//!
//! What the shell **never** does is replace a running daemon with the one it
//! shipped with. An update that stopped a desk mid-show to start its own copy
//! would be the fault §10.3's whole shutdown section exists to prevent.

use prismd::lock::{LockDocument, Presence};

/// The path a client upgrades on, matching `prism_ipc::websocket::IPC_PATH` and
/// `ui/src/ipc/endpoint.ts`.
const IPC_PATH: &str = "/ipc";

/// What the shell should do about the desk, having looked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Approach {
    /// Nobody is running here. Start one.
    Spawn,
    /// A desk is running and the window can reach it.
    Attach {
        /// The URL to load the interface against — `ui/src/ipc/endpoint.ts`'s
        /// first source, which is the one the shell was always going to supply.
        url: String,
        /// The §2.1 token, when the running daemon published one. Absent on
        /// loopback, where §2.1 asks for none.
        token: Option<String>,
        /// The process holding the guard, for a person reading a message.
        pid: u32,
    },
    /// A desk is running and this window cannot reach it. Neither attaching nor
    /// spawning is right; saying so is.
    Unreachable {
        /// The process holding the guard.
        pid: u32,
        /// Where the daemon said it could be reached, in the words
        /// `LockDocument::endpoints` uses — *no endpoint* when it published
        /// none.
        endpoints: String,
    },
}

/// Reads what [`prismd::lock::look`] found and says what to do about it.
///
/// Pure, and that is the point: everything about *spawn or attach* that could be
/// wrong is decidable from a presence and a document, so it is decided somewhere
/// a test can call with neither a daemon nor a window.
#[must_use]
pub fn approach(presence: &Presence) -> Approach {
    let Presence::Running(document) = presence else {
        return Approach::Spawn;
    };
    websocket_url(document).map_or_else(
        || Approach::Unreachable {
            pid: document.pid,
            endpoints: document.endpoints(),
        },
        |url| Approach::Attach {
            url,
            token: document.token.clone(),
            pid: document.pid,
        },
    )
}

/// The URL a webview loads the interface against, from a discovery document.
///
/// A **WebSocket** address and never the local one, and that is not a
/// preference: `docs/IPC_PROTOCOL.md` §2 gives the named pipe to the desktop
/// shell, but the thing that has to speak to the daemon is the *page* inside it,
/// and a page has `WebSocket` and no pipe. The shell's own connection — which is
/// what the tray's *Stop the desk* travels on — does use the pipe, and that is
/// [`crate::shell`]'s business.
///
/// # An unspecified address is where a daemon *listens*, not where it is
///
/// A desk whose listener is on `0.0.0.0:7373` — which is what an operator sets
/// when they want a phone in the auditorium to reach it — publishes exactly
/// that, because it is the address it bound. It is not an address anything can
/// connect **to**: `0.0.0.0` means *every interface* to a listener and *no host
/// at all* to a client. So the shell, which is on the daemon's own machine by
/// construction, asks for **loopback** on that port. The alternative is a window
/// that shows nothing on precisely the desks somebody has configured for a
/// remote, and it would fail twice over — once at the socket, and once at the
/// content-security policy in `tauri.conf.json`, which lets this page reach
/// loopback and nothing else.
#[must_use]
pub fn websocket_url(document: &LockDocument) -> Option<String> {
    let address = reachable(document.websocket.as_deref()?);
    Some(format!("ws://{address}{IPC_PATH}"))
}

/// An address a client can connect to, from one a daemon bound.
///
/// Text in and text out: what is published is a string, and parsing it into a
/// `SocketAddr` to put it straight back would be a round trip that can fail on a
/// document written by a future build. What is looked for is the two spellings
/// of *every interface*, and everything else is passed through exactly as the
/// daemon wrote it.
fn reachable(address: &str) -> String {
    match address.rsplit_once(':') {
        Some(("0.0.0.0", port)) => format!("127.0.0.1:{port}"),
        Some(("[::]", port)) => format!("[::1]:{port}"),
        _ => address.to_owned(),
    }
}

/// What a person is told when a desk is running that this window cannot reach.
///
/// A sentence rather than a code, because it is read by somebody whose console
/// has not appeared and who has to decide what to do next — so it names the
/// process, the endpoints and the way out, and it answers the first question
/// before it is asked.
#[must_use]
pub fn unreachable_message(pid: u32, endpoints: &str, data_dir: &std::path::Path) -> String {
    format!(
        "A PrismDMX desk is already running on this machine (process {pid}), but this window \
         cannot reach it: it is listening at {endpoints}, and the interface needs a WebSocket \
         listener.\n\nThat desk is still running and the show is still on stage. Nothing was \
         started and nothing was stopped.\n\nTo get a window back, stop that desk and start this \
         one again, or start it with --websocket. Its files are in {}.",
        data_dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::{Approach, approach, unreachable_message, websocket_url};
    use prismd::lock::{LockDocument, Presence};

    fn running(document: LockDocument) -> Presence {
        Presence::Running(Box::new(document))
    }

    #[test]
    fn nobody_running_means_start_one() {
        assert_eq!(approach(&Presence::Nobody), Approach::Spawn);
    }

    /// The exit criterion: a second shell attaches instead of spawning a second
    /// daemon.
    #[test]
    fn a_running_desk_with_a_listener_is_attached_to() {
        let found = approach(&running(LockDocument {
            pid: 4711,
            local: Some(r"\\.\pipe\prismd".to_owned()),
            websocket: Some("127.0.0.1:7373".to_owned()),
            token: None,
        }));
        assert_eq!(
            found,
            Approach::Attach {
                url: "ws://127.0.0.1:7373/ipc".to_owned(),
                token: None,
                pid: 4711,
            }
        );
    }

    /// The token travels with the address, because a listener that is not on
    /// loopback needs one (§2.1) and the shell is the one client that has read
    /// it off the disk.
    ///
    /// And the address it travels with is **loopback**: a daemon that bound
    /// every interface published the address it bound, which is not one anything
    /// connects to.
    #[test]
    fn a_token_the_daemon_published_is_carried_to_the_window() {
        let found = approach(&running(LockDocument {
            pid: 12,
            local: None,
            websocket: Some("0.0.0.0:7373".to_owned()),
            token: Some("hunter2".to_owned()),
        }));
        assert_eq!(
            found,
            Approach::Attach {
                url: "ws://127.0.0.1:7373/ipc".to_owned(),
                token: Some("hunter2".to_owned()),
                pid: 12,
            }
        );
    }

    /// The two spellings of *every interface*, and everything else untouched.
    ///
    /// This is the case that only appears on the desks somebody has deliberately
    /// configured for a remote — which is to say, on the desks where a window
    /// that showed nothing would be hardest to explain.
    #[test]
    fn an_address_a_daemon_bound_becomes_one_a_client_can_reach() {
        let mut document = LockDocument::default();
        for (bound, reachable) in [
            ("0.0.0.0:7373", "ws://127.0.0.1:7373/ipc"),
            ("[::]:7373", "ws://[::1]:7373/ipc"),
            ("127.0.0.1:7373", "ws://127.0.0.1:7373/ipc"),
            ("192.168.1.9:7373", "ws://192.168.1.9:7373/ipc"),
            ("[::1]:9000", "ws://[::1]:9000/ipc"),
        ] {
            document.websocket = Some(bound.to_owned());
            assert_eq!(
                websocket_url(&document).as_deref(),
                Some(reachable),
                "{bound}"
            );
        }
    }

    /// **The first edge, and the decision this session had to make.** A desk is
    /// running and this window cannot reach it: not a spawn, not an attach, and
    /// not a blank screen either.
    #[test]
    fn a_desk_with_no_listener_is_neither_spawned_over_nor_attached_to() {
        let found = approach(&running(LockDocument {
            pid: 4711,
            local: Some(r"\\.\pipe\prismd".to_owned()),
            websocket: None,
            token: None,
        }));
        let Approach::Unreachable { pid, endpoints } = found else {
            panic!("a desk that is running is never spawned over");
        };
        assert_eq!(pid, 4711);
        assert_eq!(endpoints, r"\\.\pipe\prismd");

        let message = unreachable_message(pid, &endpoints, std::path::Path::new(r"C:\data"));
        assert!(message.contains("4711"), "{message}");
        assert!(message.contains(r"\\.\pipe\prismd"), "{message}");
        assert!(message.contains(r"C:\data"), "{message}");
        assert!(
            message.contains("still running"),
            "the operator's first question is whether the show is still on: {message}"
        );
    }

    /// A daemon holding the guard with nothing published — the moment between
    /// taking the lock and binding a listener, and the state a headless desk
    /// stays in. Still not a spawn.
    #[test]
    fn a_desk_that_published_nothing_at_all_is_still_a_desk() {
        assert_eq!(
            approach(&running(LockDocument::default())),
            Approach::Unreachable {
                pid: 0,
                endpoints: "no endpoint".to_owned(),
            }
        );
    }

    #[test]
    fn the_url_is_the_path_the_interface_upgrades_on() {
        let mut document = LockDocument::default();
        assert_eq!(websocket_url(&document), None);
        document.websocket = Some("127.0.0.1:9000".to_owned());
        assert_eq!(
            websocket_url(&document).as_deref(),
            Some("ws://127.0.0.1:9000/ipc")
        );
    }
}
