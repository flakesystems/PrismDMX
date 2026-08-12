//! Where the daemon keeps what belongs to this machine.
//!
//! `ARCHITECTURE_SPEC.md` §10.3 puts the lock file "into the user data
//! directory", and `docs/IPC_PROTOCOL.md` §2.2 says clients read it from there.
//! `prism_core::MachineConfig` lives beside it — the desk identity is a
//! property of the machine, not of the show (S11).
//!
//! # Why this is not `#[cfg(windows)]`
//!
//! `ARCHITECTURE_SPEC.md` §10.1 does not allow this crate platform code, and
//! the user data directory is the most obviously platform-shaped thing a daemon
//! needs. It is resolved from the **environment** instead, which is what the
//! platform conventions are actually written in: `%APPDATA%` on Windows,
//! `$XDG_DATA_HOME` or `$HOME/.local/share` elsewhere. A machine has one or the
//! other, so asking for all of them in order is one code path that gets the
//! right answer on every target.
//!
//! It is also what makes this testable at all. [`data_dir`] takes the
//! environment as a function rather than reading it, because `std::env::set_var`
//! is `unsafe` in edition 2024 and this workspace forbids `unsafe_code` — so a
//! test that set `APPDATA` could not be written, and one that passes a lookup
//! can.

use std::path::{Path, PathBuf};

/// The directory name under the platform's own data directory.
const APPLICATION_DIRECTORY: &str = "PrismDMX";

/// The variable that overrides everything, for a second daemon on one machine
/// and for every test in this crate.
pub const DATA_DIR_VARIABLE: &str = "PRISMD_DATA_DIR";

/// Why the data directory could not be worked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoDataDirectory;

impl core::fmt::Display for NoDataDirectory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "no data directory: none of {DATA_DIR_VARIABLE}, APPDATA, XDG_DATA_HOME or HOME is set"
        )
    }
}

impl core::error::Error for NoDataDirectory {}

/// The directory the lock file, the machine configuration and the default show
/// live in, given a way of reading the environment.
///
/// In order: the override, the Windows convention, the XDG convention, and the
/// XDG fallback. Empty values are treated as absent, because an environment
/// variable set to nothing is how a launcher unsets one.
///
/// # Errors
///
/// [`NoDataDirectory`] if the environment says nothing about where the user's
/// files are, which is a machine nothing else would work on either.
pub fn data_dir(read: impl Fn(&str) -> Option<String>) -> Result<PathBuf, NoDataDirectory> {
    let named = |name: &str| read(name).filter(|value| !value.trim().is_empty());
    if let Some(explicit) = named(DATA_DIR_VARIABLE) {
        // Taken exactly as given: an operator who names a directory means that
        // directory, not a subdirectory of it.
        return Ok(PathBuf::from(explicit));
    }
    if let Some(appdata) = named("APPDATA") {
        return Ok(Path::new(&appdata).join(APPLICATION_DIRECTORY));
    }
    if let Some(xdg) = named("XDG_DATA_HOME") {
        return Ok(Path::new(&xdg).join(APPLICATION_DIRECTORY));
    }
    if let Some(home) = named("HOME") {
        return Ok(Path::new(&home)
            .join(".local")
            .join("share")
            .join(APPLICATION_DIRECTORY));
    }
    Err(NoDataDirectory)
}

/// The data directory of the machine this process is running on.
///
/// # Errors
///
/// As [`data_dir`].
pub fn system_data_dir() -> Result<PathBuf, NoDataDirectory> {
    data_dir(|name| std::env::var(name).ok())
}

/// The show a daemon opens when nobody named one.
///
/// A file rather than an empty show held in memory, because a daemon that
/// cannot save what an operator built is a daemon that loses it.
#[must_use]
pub fn default_show_path(data_dir: &Path) -> PathBuf {
    data_dir.join("default.prism")
}

/// The discovery file of `docs/IPC_PROTOCOL.md` §2.2.
#[must_use]
pub fn lock_path(data_dir: &Path) -> PathBuf {
    data_dir.join("prismd.lock")
}

/// The file whose operating-system lock is the single-instance guarantee.
#[must_use]
pub fn guard_path(data_dir: &Path) -> PathBuf {
    data_dir.join("prismd.guard")
}

/// Where `prism_core::MachineConfig` is written.
#[must_use]
pub fn machine_config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("machine.json")
}

#[cfg(test)]
mod tests {
    use super::{
        DATA_DIR_VARIABLE, NoDataDirectory, data_dir, default_show_path, guard_path, lock_path,
        machine_config_path, system_data_dir,
    };
    use std::path::{Path, PathBuf};

    /// An environment made of pairs, so a test can describe a machine that does
    /// not exist here.
    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn the_override_wins_and_is_taken_exactly_as_given() {
        let found = data_dir(env(&[
            (DATA_DIR_VARIABLE, "/srv/prism"),
            ("APPDATA", r"C:\Users\x\AppData\Roaming"),
            ("HOME", "/home/x"),
        ]))
        .unwrap();
        assert_eq!(found, PathBuf::from("/srv/prism"));
    }

    #[test]
    fn a_windows_machine_uses_appdata() {
        let found = data_dir(env(&[("APPDATA", r"C:\Users\x\AppData\Roaming")])).unwrap();
        assert_eq!(
            found,
            Path::new(r"C:\Users\x\AppData\Roaming").join("PrismDMX")
        );
    }

    #[test]
    fn an_xdg_machine_uses_the_data_home_and_then_the_fallback() {
        assert_eq!(
            data_dir(env(&[
                ("XDG_DATA_HOME", "/home/x/.data"),
                ("HOME", "/home/x")
            ]))
            .unwrap(),
            Path::new("/home/x/.data").join("PrismDMX")
        );
        assert_eq!(
            data_dir(env(&[("HOME", "/home/x")])).unwrap(),
            Path::new("/home/x/.local/share").join("PrismDMX")
        );
    }

    #[test]
    fn a_variable_set_to_nothing_is_a_variable_that_is_not_set() {
        // How a launcher unsets one, and the case that would otherwise put
        // every file in the root of the file system.
        assert_eq!(
            data_dir(env(&[
                (DATA_DIR_VARIABLE, "   "),
                ("APPDATA", ""),
                ("HOME", "/home/x"),
            ]))
            .unwrap(),
            Path::new("/home/x/.local/share").join("PrismDMX")
        );
    }

    #[test]
    fn a_machine_that_says_nothing_is_an_error_with_a_reason() {
        assert_eq!(data_dir(env(&[])), Err(NoDataDirectory));
        assert!(NoDataDirectory.to_string().contains(DATA_DIR_VARIABLE));
        let as_error: &dyn core::error::Error = &NoDataDirectory;
        assert!(!as_error.to_string().is_empty());
    }

    #[test]
    fn every_file_the_daemon_owns_is_under_the_data_directory() {
        let dir = Path::new("/data");
        for path in [
            default_show_path(dir),
            lock_path(dir),
            guard_path(dir),
            machine_config_path(dir),
        ] {
            assert_eq!(path.parent(), Some(dir), "{path:?}");
        }
        assert_ne!(lock_path(dir), guard_path(dir));
    }

    #[test]
    fn this_machine_has_a_data_directory() {
        // Not an assertion about which one: a Windows runner answers APPDATA
        // and a Linux one answers HOME, and the point is that both answer.
        assert!(system_data_dir().is_ok());
    }
}
