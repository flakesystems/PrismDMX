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

/// Where the **operator's own** fixture profiles go — S44.
///
/// Inside the data directory, so it survives an update: the installed library
/// (`profiles/fixtures/`) is replaced wholesale by the next install, and a
/// venue's correction to a profile must not go with it. Files here are read in
/// the Open Fixture Library's own format and a key here **wins**.
#[must_use]
pub fn fixtures_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("fixtures")
}

/// What the fixture library read at the last start, so the next need not read
/// it again (2026-09-22) — a cache beside the show, always safe to delete. See
/// `prism_core::library::index`.
#[must_use]
pub fn library_index(data_dir: &Path) -> PathBuf {
    data_dir.join("library-index.json")
}

/// The note written into a freshly made [`fixtures_dir`].
///
/// Plain text rather than Markdown because the person who opens this folder is
/// using Explorer, not a repository browser. The library reads `.json` only,
/// so the note is never mistaken for a profile.
pub const FIXTURES_README: &str = "\
PrismDMX - your own fixture profiles

Put fixture profiles here. Nothing that installs or updates PrismDMX
touches this folder. Two formats are read.

GDTF, which is what the desk's own library is made of:

  anything.gdtf                 a .gdtf file from the manufacturer or from
                                gdtf-share.com. The name does not matter:
                                the file says which fixture it is, and if
                                that is one the library already has, yours
                                replaces it.

Open Fixture Library JSON, for a light nobody has published a GDTF for -
it is a far kinder thing to write by hand:

  my-light.json                 a light the library has no profile for.
                                It appears under 'Custom' in the Patch window.

  <manufacturer>/<fixture>.json a correction to a profile the library ships.
                                Same folder and file name as the library,
                                and yours replaces it.

A GDTF profile carries the fixture's gobo pictures, its size and where its
beam comes out, so the 3D viewer can draw it; a JSON one carries channels
and names, which is all any desk had before.

Restart the desk to read new files. A show embeds the profiles it uses,
so a patched rig does not depend on this folder afterwards.
";

/// Makes [`fixtures_dir`] if it is not there yet — punch-list **B54**.
///
/// S51 made the folder the place a venue's own profiles go, and the manuals say
/// so; a fresh installation then did not have it, and somebody following the
/// manual found nothing to put the file into. So the daemon makes it at start,
/// with a note inside that says what it is for.
///
/// Returns whether it was made. An existing folder is left exactly as it is —
/// the note is only written into a folder this call created, so an operator who
/// deleted it is not given it back. A failure is the caller's to *report*, not
/// to stop on: a desk that would not start over a folder it only offers is a
/// worse answer than one that says so.
///
/// # Errors
///
/// Whatever the file system says about creating the folder or the note.
pub fn ensure_fixtures_dir(data_dir: &Path) -> std::io::Result<bool> {
    let directory = fixtures_dir(data_dir);
    if directory.is_dir() {
        return Ok(false);
    }
    std::fs::create_dir_all(&directory)?;
    std::fs::write(directory.join("README.txt"), FIXTURES_README)?;
    Ok(true)
}

/// Where the installer put the fixture library, if it can be found.
///
/// Looked for beside the executable first — which is where an installed desk
/// has it — and then up the tree from it, which is what finds
/// `profiles/fixtures/` when the daemon is a `cargo run` inside a checkout.
/// `None` is an ordinary answer: the desk starts with its built-in profiles and
/// says so.
///
/// A path rather than a compiled-in constant, because the library is
/// **downloaded at install time and not committed** — see
/// `profiles/fixtures/SOURCE.md`.
///
/// # What makes a directory the library
///
/// It holds something a fixture library holds: a `.gdtf` file, an unpacked
/// GDTF's `description.xml`, or an Open Fixture Library `manufacturers.json`.
/// **Three tests rather than one since S61**, because the installed library is
/// GDTF now and a GDTF library has no `manufacturers.json` in it — a desk whose
/// library was installed before S61 still has one, and both are found.
///
/// An *empty* `profiles/fixtures/` is deliberately not it: the directory is
/// committed with only its `SOURCE.md` in it, so a checkout nobody has run the
/// installer in would otherwise look like a library with nothing in it rather
/// than like no library at all, and the daemon would stop saying so.
#[must_use]
pub fn installed_library_dir() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let mut directory = executable.parent()?;
    // `target/debug/prismd.exe` is three levels below the checkout root, and an
    // installed desk has `profiles/` beside the binary. Six is more than either
    // needs and stops well before the root of the disk.
    for _ in 0..6 {
        let candidate = directory.join("profiles").join("fixtures");
        if holds_a_library(&candidate) {
            return Some(candidate);
        }
        directory = directory.parent()?;
    }
    None
}

/// Whether a directory holds a fixture library — see [`installed_library_dir`].
///
/// One level down as well as at the top, because both layouts put the fixtures
/// in a directory per manufacturer and `manufacturers.json` is the only thing
/// either puts at the root.
fn holds_a_library(root: &Path) -> bool {
    if root.join("manufacturers.json").is_file() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            return path.join("description.xml").is_file() || has_gdtf(&path);
        }
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gdtf"))
    })
}

/// Whether one directory holds a `.gdtf` file.
fn has_gdtf(directory: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .path()
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gdtf"))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        DATA_DIR_VARIABLE, FIXTURES_README, NoDataDirectory, data_dir, default_show_path,
        ensure_fixtures_dir, fixtures_dir, guard_path, lock_path, machine_config_path,
        system_data_dir,
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

    #[test]
    fn a_fresh_data_directory_is_given_a_fixtures_folder_with_a_note_in_it() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("PrismDMX");
        assert!(
            ensure_fixtures_dir(&data).unwrap(),
            "the folder was not made"
        );
        let note = std::fs::read_to_string(fixtures_dir(&data).join("README.txt")).unwrap();
        assert_eq!(note, FIXTURES_README);
        // And it says the two things a venue needs to know.
        assert!(note.contains("my-light.json"));
        assert!(note.contains("<manufacturer>/<fixture>.json"));
    }

    #[test]
    fn a_fixtures_folder_that_exists_is_left_exactly_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let own = fixtures_dir(dir.path());
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join("mine.json"), "{}").unwrap();
        assert!(!ensure_fixtures_dir(dir.path()).unwrap());
        // No note written into somebody's own folder, and nothing taken out.
        assert!(!own.join("README.txt").exists());
        assert!(own.join("mine.json").is_file());
    }

    #[test]
    fn a_fixtures_folder_that_cannot_be_made_is_an_error_and_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        // A *file* where the data directory should be.
        let blocked = dir.path().join("blocked");
        std::fs::write(&blocked, "").unwrap();
        assert!(ensure_fixtures_dir(&blocked).is_err());
    }
}
