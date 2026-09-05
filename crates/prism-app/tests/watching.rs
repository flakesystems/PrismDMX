//! The shell noticing that its desk has gone — **punch-list entry B39**.
//!
//! # Why this is a test and not a hand-check
//!
//! B39 was reported by an operator, not by the suite, and the reason it was
//! never caught is written in the entry: nobody had killed a daemon under a
//! running shell. So the thing that has to be exercised is the *mechanism* the
//! shell watches — an advisory lock on `prismd.guard` — and it can be, without
//! a window and without a second process, because the operating system releases
//! that lock when the holder goes and `DaemonLock`'s `Drop` is the same release
//! a kill produces (`prismd::lock`'s module documentation says so, and it is
//! why the guard is a lock rather than a process-id probe).
//!
//! `CLAUDE.md`: no test may need a device, and none here needs a window either.
//! What is asserted is the reading the shell acts on, over real files.

use prism_app::attach::{Standing, standing};
use prismd::lock::{DaemonLock, Presence, look};

/// The pid the shell believes it attached to, in the tests where a document is
/// written by hand.
const ATTACHED_TO: u32 = 4711;

/// **A desk that goes while the shell is watching is seen to have gone.**
///
/// The lock is taken, the shell's reading says *holding*, the holder goes — and
/// the very next reading says *gone*. That is the entry's first half: the icon
/// has something to report, because the shell has something to notice.
#[test]
fn a_desk_that_stops_under_the_shell_is_noticed() {
    let dir = tempfile::tempdir().expect("a temporary data directory");
    let lock = DaemonLock::acquire(dir.path()).expect("nobody else is holding it");
    let mine = std::process::id();

    let presence = look(dir.path()).expect("the directory can be read");
    assert!(matches!(presence, Presence::Running(_)));
    assert_eq!(standing(mine, &presence), Standing::Holding);

    // What a task manager does, as far as the file system is concerned: the
    // process ends and the operating system lets the lock go.
    drop(lock);

    let presence = look(dir.path()).expect("the directory can be read");
    assert!(matches!(presence, Presence::Nobody));
    assert_eq!(
        standing(mine, &presence),
        Standing::Gone,
        "the shell went on believing in a desk that had stopped"
    );
}

/// **A second start does not leave a second icon**, which is the half of B39
/// that has to be decided rather than observed.
///
/// A shell whose desk has been replaced sees a guard held by a *different*
/// process, and that is the moment at which a dead icon would otherwise sit
/// beside a live one. The verdict is `Replaced`, which is what closes the old
/// shell — so the notification area holds one desk, and it is the one that is
/// running.
#[test]
fn a_second_start_replaces_the_desk_rather_than_standing_beside_it() {
    let dir = tempfile::tempdir().expect("a temporary data directory");
    let _lock = DaemonLock::acquire(dir.path()).expect("nobody else is holding it");

    // The guard is held and the document names somebody else: a daemon started
    // after the one this shell attached to. Written rather than spawned,
    // because what the shell reads is the document and a second process would
    // make this a test of `std::process` rather than of the reading.
    let document = dir.path().join("prismd.lock");
    std::fs::write(
        &document,
        serde_json::json!({ "pid": 5150, "websocket": "127.0.0.1:7373" }).to_string(),
    )
    .expect("the discovery document can be written");

    let presence = look(dir.path()).expect("the directory can be read");
    assert_eq!(
        standing(ATTACHED_TO, &presence),
        Standing::Replaced { pid: 5150 }
    );

    // And the shell attached to *that* one stays exactly where it is. A verdict
    // that fired on every shell would close the live one too.
    assert_eq!(standing(5150, &presence), Standing::Holding);
}

/// A directory that cannot be read is **not** a verdict.
///
/// The watcher skips a reading it could not take, and this is the reading:
/// there is no guard file at all in a directory nothing has ever run in, which
/// `look` answers as nobody rather than as an error — so the one thing this
/// asserts is that a shell watching a directory with no daemon in it is told
/// *gone* rather than being told nothing.
#[test]
fn a_directory_with_no_desk_in_it_reads_as_gone() {
    let dir = tempfile::tempdir().expect("a temporary data directory");
    let presence = look(dir.path()).expect("an empty directory is not an error");
    assert_eq!(standing(ATTACHED_TO, &presence), Standing::Gone);
}
