//! **A venue's own profiles survive a library re-download** — punch-list entry
//! **B43**.
//!
//! # This runs the installer, and that is the point
//!
//! The entry is about a *destructive* step: `tools/fetch-fixtures` empties its
//! destination on every run, deliberately, because a half-replaced copy of
//! somebody else's data is worse than none. Whether a venue's own profiles
//! survive that is a question about the two directories being different ones,
//! and a test that reimplemented the wipe would be asserting against its own
//! copy of the script rather than against the script.
//!
//! So the real script runs — `fetch-fixtures.sh` here, `fetch-fixtures.ps1` on
//! Windows, whichever this platform has — with `PRISMDMX_OFL_ARCHIVE` pointing
//! at an archive this test builds. Nothing is downloaded: `CLAUDE.md` says a
//! test may not need a device, and a test that needs a network is the same
//! promise broken a different way.
//!
//! If the interpreter is not there the test says so and returns rather than
//! passing quietly — the same rule `crates/prism-core/tests/fixture_library.rs`
//! follows for the corpus it may not have.

// This target prints why it skipped, for a person to read.
#![allow(clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// One OFL fixture, as the venue would write one for a light nobody has a
/// profile for.
const VENUE_FIXTURE: &str = r#"{
  "name": "The one in the roof",
  "availableChannels": {
    "Dimmer": { "capability": { "type": "Intensity" } },
    "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } }
  },
  "modes": [{ "shortName": "2ch", "channels": ["Dimmer", "Red"] }]
}"#;

/// One OFL fixture, as the upstream library has it.
const VENDORED_FIXTURE: &str = r#"{
  "name": "Wash 7Q5",
  "availableChannels": {
    "Dimmer": { "capability": { "type": "Intensity" } },
    "Green": { "capability": { "type": "ColorIntensity", "color": "Green" } }
  },
  "modes": [{ "shortName": "2ch", "channels": ["Dimmer", "Green"] }]
}"#;

/// The repository this test is part of.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository is on disk")
}

/// Builds an archive shaped exactly as the one the installer downloads: a
/// single top-level `open-fixture-library-<revision>/` holding `fixtures/` and
/// `LICENSE`.
///
/// `tar` is what both scripts already require, so using it here adds no
/// dependency the installer has not got.
fn archive(work: &Path, revision: &str, fixtures: &[(&str, &str)]) -> Option<PathBuf> {
    let prefix = work.join(format!("open-fixture-library-{revision}"));
    std::fs::create_dir_all(prefix.join("fixtures")).expect("the tree is made");
    std::fs::write(prefix.join("LICENSE"), "MIT").expect("a licence");
    std::fs::write(
        prefix.join("fixtures").join("manufacturers.json"),
        r#"{ "robe": { "name": "Robe" } }"#,
    )
    .expect("a names table");
    for (path, source) in fixtures {
        let full = prefix.join("fixtures").join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("the tree is made");
        std::fs::write(full, source).expect("a fixture");
    }
    let path = work.join("ofl.tar.gz");
    let made = Command::new("tar")
        .arg("--create")
        .arg("--gzip")
        .arg("--file")
        .arg(&path)
        .arg("--directory")
        .arg(work)
        .arg(format!("open-fixture-library-{revision}"))
        .status();
    match made {
        Ok(status) if status.success() => Some(path),
        _ => {
            println!("skipping: this machine has no working `tar`");
            None
        }
    }
}

/// Runs the installer this platform has, against `destination`.
///
/// `None` when there is no interpreter for it here, which is printed rather
/// than passed over: a silent skip cannot be told from a pass.
fn install(destination: &Path, archive: &Path, revision: &str) -> Option<bool> {
    let root = repository();
    let script = if cfg!(windows) {
        root.join("tools/fetch-fixtures/fetch-fixtures.ps1")
    } else {
        root.join("tools/fetch-fixtures/fetch-fixtures.sh")
    };
    let mut command = if cfg!(windows) {
        let mut command = Command::new("pwsh");
        command
            .arg("-NoProfile")
            .arg("-File")
            .arg(&script)
            .arg("-Destination")
            .arg(destination);
        command
    } else {
        let mut command = Command::new("bash");
        command.arg(&script).arg(destination);
        command
    };
    command
        .env("PRISMDMX_OFL_ARCHIVE", archive)
        .env("PRISMDMX_OFL_REVISION", revision);
    match command.output() {
        Ok(output) => Some(output.status.success()),
        Err(error) => {
            println!("skipping: the installer could not be run here ({error})");
            None
        }
    }
}

/// **The exit criterion, in one run.**
///
/// A venue's own profile is in the data directory and a vendored one is in the
/// library directory. The installer runs — the wipe included — replacing the
/// library with a different upstream. Afterwards the venue's profile is still
/// there, still wins the key it corrects, and is still marked as the venue's
/// own; the new vendored profiles are there too.
#[test]
fn a_venues_own_profiles_survive_a_library_re_download() {
    let work = tempfile::tempdir().expect("a temporary directory");
    let data_dir = work.path().join("data");
    let library = work.path().join("library");
    std::fs::create_dir_all(&library).expect("the library directory");

    // The venue's own: one light nobody has a profile for, and one **correction**
    // to a vendored profile — the case that decides whether the override works.
    let own = prismd::paths::fixtures_dir(&data_dir);
    std::fs::create_dir_all(own.join("robe")).expect("the venue's directory");
    std::fs::write(own.join("in-the-roof.json"), VENUE_FIXTURE).expect("a venue profile");
    std::fs::write(own.join("robe").join("wash-7q5.json"), VENUE_FIXTURE)
        .expect("a venue correction");

    // A file the installer will delete, so this test can tell a run that
    // happened from one that did not.
    std::fs::write(library.join("SOURCE.md"), "committed").expect("the source note");
    std::fs::write(library.join("stale.json"), "{}").expect("a stale file");

    let Some(archive) = archive(
        work.path(),
        "abc123",
        &[("robe/wash-7q5.json", VENDORED_FIXTURE)],
    ) else {
        return;
    };
    let Some(ran) = install(&library, &archive, "abc123") else {
        return;
    };
    assert!(ran, "the installer did not finish");

    // The installer did what it does: its destination is now the archive's, and
    // the committed note is the one thing it left.
    assert!(
        library.join("SOURCE.md").is_file(),
        "SOURCE.md is committed"
    );
    assert!(
        !library.join("stale.json").exists(),
        "the wipe did not happen"
    );
    assert!(library.join("robe").join("wash-7q5.json").is_file());

    // And the venue's directory is untouched, because it is somewhere else.
    assert!(own.join("in-the-roof.json").is_file());
    assert!(own.join("robe").join("wash-7q5.json").is_file());

    // The library the daemon would build out of the two, in the daemon's own
    // order: the venue's first, so a correction wins.
    let built = prismd::daemon::load_library(&data_dir, Some(&library));
    let entry = |id: &str| built.entries().iter().find(|entry| entry.id == id).cloned();

    let roof = entry("custom/in-the-roof/2ch").expect("the venue's own light is in the library");
    assert!(roof.own, "the picker cannot tell it is the venue's");
    assert_eq!(roof.name, "The one in the roof");

    let corrected = entry("robe/wash-7q5/2ch").expect("the corrected profile is in the library");
    assert!(
        corrected.own,
        "the vendored copy came back over the venue's correction"
    );
    assert_eq!(corrected.name, "The one in the roof");
}
