//! Starting the daemon: where its executable is, and what it is told.
//!
//! `ARCHITECTURE_SPEC.md` §10.3's default tier in one module — *shell spawns
//! `prismd` as a child, detaches it, leaves it running on close*, with no
//! administrator rights anywhere in it.
//!
//! # The arguments are almost none, and that is S37's doing
//!
//! Before S37 a shell would have had to pass the whole rig, the listener, the
//! log level and the exit action on a command line. It passes **nothing** of the
//! sort now: every one of those is a field of `prism_core::MachineConfig`, and a
//! flag on the command line is the value *for that run* with the stored setting
//! neither read nor written. A shell that passed flags would therefore be a
//! shell that made the settings window's rows read *held by a flag* for ever —
//! which is precisely the state S37 built that machinery to describe, and
//! precisely the wrong state for a desk somebody is configuring.
//!
//! So the daemon a shell starts is told two things at most: **which data
//! directory**, and only when this process was told one itself; and nothing
//! else. It reads its own configuration from there, exactly as it would if a
//! person had started it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The daemon's executable name, as it is installed beside the shell.
#[cfg(windows)]
pub const DAEMON_EXECUTABLE: &str = "prismd.exe";
/// The daemon's executable name, as it is installed beside the shell.
#[cfg(not(windows))]
pub const DAEMON_EXECUTABLE: &str = "prismd";

/// Where the daemon is, given where this executable is.
///
/// Beside it first, which is where an installed desk has it — the installer puts
/// `prismd.exe` in the same directory as `PrismDMX.exe`, because that is also
/// what `prismd::paths::installed_library_dir` needs in order to find
/// `profiles/fixtures/` — and then up the tree, which is what finds
/// `target/debug/prismd.exe` when the shell is a `cargo run` inside a checkout.
///
/// The same shape as `installed_library_dir`, deliberately: two searches with
/// two different ideas of where an installation is would be two ways for a desk
/// to come up half-configured.
#[must_use]
pub fn daemon_beside(executable: &Path) -> Option<PathBuf> {
    let mut directory = executable.parent()?;
    for _ in 0..3 {
        let candidate = directory.join(DAEMON_EXECUTABLE);
        if candidate.is_file() {
            return Some(candidate);
        }
        directory = directory.parent()?;
    }
    None
}

/// What a daemon this shell starts is told.
///
/// `data_dir` is `Some` only when this process was itself pointed at one —
/// `PRISMD_DATA_DIR` in the environment, or a `--data-dir` the shell was given.
/// Passing the resolved default would be passing an argument the daemon would
/// have worked out identically, and it would turn a setting into a flag.
#[must_use]
pub fn arguments(data_dir: Option<&Path>) -> Vec<OsString> {
    let mut arguments = Vec::new();
    if let Some(directory) = data_dir {
        arguments.push(OsString::from("--data-dir"));
        arguments.push(directory.as_os_str().to_owned());
    }
    arguments
}

/// Whether this process was pointed at a data directory rather than working one
/// out.
///
/// Read from the environment through a function so a test can describe a machine
/// this one is not — `prismd::paths::data_dir`'s reason exactly, and the same
/// reason: `std::env::set_var` is `unsafe` in edition 2024 and this workspace
/// forbids `unsafe_code`.
#[must_use]
pub fn explicit_data_dir(read: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    read(prismd::paths::DATA_DIR_VARIABLE)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{DAEMON_EXECUTABLE, arguments, daemon_beside, explicit_data_dir};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    #[test]
    fn the_daemon_is_found_beside_the_shell_first() {
        let dir = tempfile::tempdir().unwrap();
        let beside = dir.path().join(DAEMON_EXECUTABLE);
        std::fs::write(&beside, b"not really a daemon").unwrap();
        assert_eq!(
            daemon_beside(&dir.path().join("PrismDMX.exe")),
            Some(beside)
        );
    }

    /// The checkout case: `target/debug/PrismDMX.exe` has the daemon in the same
    /// directory, and a bundle laid out with the shell one level down still
    /// finds it.
    #[test]
    fn the_daemon_is_found_one_level_up_as_well() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("bin");
        std::fs::create_dir(&nested).unwrap();
        let above = dir.path().join(DAEMON_EXECUTABLE);
        std::fs::write(&above, b"not really a daemon").unwrap();
        assert_eq!(
            daemon_beside(&nested.join("PrismDMX.exe")),
            Some(above),
            "a shell in a subdirectory of the installation still finds the daemon"
        );
    }

    #[test]
    fn a_shell_with_no_daemon_anywhere_near_it_finds_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(daemon_beside(&dir.path().join("PrismDMX.exe")), None);
    }

    /// **The rule S37 left for this session.** A shell that passed the rig, the
    /// listener or the log level would make every one of those rows read *held
    /// by a flag* in the settings window for as long as the desk ran.
    #[test]
    fn a_daemon_started_by_the_shell_is_told_nothing_it_could_work_out_itself() {
        assert!(arguments(None).is_empty());
    }

    #[test]
    fn a_data_directory_this_process_was_given_is_passed_on() {
        assert_eq!(
            arguments(Some(Path::new("/srv/prism"))),
            vec![OsString::from("--data-dir"), OsString::from("/srv/prism")]
        );
    }

    #[test]
    fn a_variable_set_to_nothing_is_a_variable_that_is_not_set() {
        let env = |value: &'static str| move |_: &str| Some(value.to_owned());
        assert_eq!(
            explicit_data_dir(env("/srv/prism")),
            Some(PathBuf::from("/srv/prism"))
        );
        assert_eq!(explicit_data_dir(env("   ")), None);
        assert_eq!(explicit_data_dir(|_| None), None);
    }
}
