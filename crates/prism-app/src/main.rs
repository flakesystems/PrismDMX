//! The PrismDMX desktop shell: find the desk, start one if there is not one,
//! and put a window on it.
//!
//! Everything below is sequence. The decisions are in the library beside this
//! file, where a test can call them without a window — `CLAUDE.md`'s testing
//! policy as it applies to a shell.
//!
//! Wired up in session **S29**.

// No console window on Windows: this is a desktop program, and a black box
// appearing behind it at every start is the mark of one built by accident.
// `debug_assertions` keeps the console in a development build, where the
// messages below are the fastest way to see what a start-up did.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// The one place this program writes to a terminal: why it could not start.
// Everything a running desk has to say goes through the daemon's logger, which
// is what `CLAUDE.md` means by a structured logger with levels.
#![allow(
    clippy::print_stderr,
    reason = "a program that cannot start says why, on stderr"
)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use prism_app::attach::{Approach, approach};
use prism_app::{autostart, spawn};
use prismd::lock::{Presence, look};

/// How long a daemon this shell started is given to take the guard and publish
/// its endpoints.
///
/// Generous on purpose: the daemon opens a show file, builds a frame layout for
/// up to sixty-four universes and starts its output threads before it binds
/// anything, and the machine it is doing that on may be a school laptop that has
/// just finished logging somebody in.
const START_DEADLINE: Duration = Duration::from_secs(20);

/// How often the lock is looked at while waiting.
const LOOK_EVERY: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    let hidden = std::env::args().skip(1).any(|argument| {
        argument == autostart::HIDDEN_FLAG || argument == "--minimised" || argument == "--minimized"
    });

    let data_dir = match resolve_data_dir() {
        Ok(directory) => directory,
        Err(error) => {
            eprintln!("PrismDMX: {error}");
            return ExitCode::FAILURE;
        }
    };

    // **Spawn or attach**, and this is the whole of it: look, decide, and — in
    // the one case that needs it — start a daemon and look again. The second
    // look is what makes the two cases one path: whatever happens next, the
    // window is opened against a daemon that is holding the guard and has
    // published where it is.
    let presence = match look(&data_dir) {
        Ok(presence) => presence,
        Err(error) => {
            eprintln!(
                "PrismDMX: the desk's own directory could not be read ({}): {error}",
                data_dir.display()
            );
            return ExitCode::FAILURE;
        }
    };
    // **A refusal from here is a window rather than an exit code.** A release
    // build is `windows_subsystem = "windows"` and has no terminal, so a shell
    // that gave up quietly would be a program that did nothing at all when the
    // engine failed to start — which is the one moment a person most needs to be
    // told something. `shell::run` shows the sentence and stops.
    let (desk, presence) = match approach(&presence) {
        Approach::Spawn => match start_a_daemon(&data_dir) {
            // Looked at again rather than assumed: the daemon this shell just
            // started is the one it attaches to, and its endpoints are the ones
            // it published while starting.
            Ok(found) => (Ok(approach(&found)), Some(found)),
            Err(error) => (Err(error), None),
        },
        found => (Ok(found), Some(presence)),
    };
    if let Err(error) = &desk {
        eprintln!("PrismDMX: {error}");
    }

    // The local endpoint, which is what the tray's *Stop the desk* travels on.
    let local = presence.and_then(|presence| match presence {
        Presence::Running(document) => document.local,
        Presence::Nobody => None,
    });
    prism_app::shell::run(desk, data_dir, local, hidden);
    ExitCode::SUCCESS
}

/// Where this desk's files are.
///
/// The daemon's own answer, asked the daemon's own way (`prismd::paths`), so a
/// shell and the daemon it starts can never disagree about which directory the
/// lock file is in — which would be two desks that cannot see each other.
fn resolve_data_dir() -> Result<PathBuf, String> {
    prismd::paths::system_data_dir().map_err(|error| error.to_string())
}

/// Starts `prismd`, detached, and waits for it to take the guard.
///
/// Detached is §10.3's default tier: the daemon outlives this process, so
/// closing the window — or quitting the shell entirely — leaves the show
/// running. On Windows that is what happens to a child anyway once its parent
/// exits; what would tie the two together is a job object, and there is not one.
fn start_a_daemon(data_dir: &std::path::Path) -> Result<Presence, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("this program's own path could not be read: {error}"))?;
    let daemon = spawn::daemon_beside(&executable).ok_or_else(|| {
        format!(
            "the engine ({}) is not installed beside this program. It should be in {}.",
            spawn::DAEMON_EXECUTABLE,
            executable.parent().unwrap_or(&executable).display()
        )
    })?;
    let explicit = spawn::explicit_data_dir(|name| std::env::var(name).ok());

    std::process::Command::new(&daemon)
        .args(spawn::arguments(explicit.as_deref()))
        .spawn()
        .map_err(|error| {
            format!(
                "the engine could not be started ({}): {error}",
                daemon.display()
            )
        })?;

    let deadline = Instant::now() + START_DEADLINE;
    loop {
        let presence = look(data_dir).map_err(|error| error.to_string())?;
        // The guard **and** an endpoint: a daemon that has taken the lock but
        // not yet bound a listener is a daemon the window cannot reach yet, and
        // waiting one more interval costs nothing where showing an error costs
        // an operator an evening.
        if let Presence::Running(document) = &presence
            && document.websocket.is_some()
        {
            return Ok(presence);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "the engine was started but did not come up within {} seconds. Its own \
                 directory is {}.",
                START_DEADLINE.as_secs(),
                data_dir.display()
            ));
        }
        std::thread::sleep(LOOK_EVERY);
    }
}
