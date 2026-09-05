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

/// The repository this test is part of, in a form **another program can
/// open**.
///
/// `canonicalize` on Windows returns a *verbatim* path — `\\?\D:\a\…` — which
/// the Rust file API is happy with and which **PowerShell will not open**. So
/// the prefix is taken off again. Reading the script here worked either way and
/// running it did not, which is why this only ever failed on the one job that
/// spawns `pwsh`.
fn repository() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository is on disk");
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => path,
    }
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
/// than passed over: a silent skip cannot be told from a pass. `Some` carries
/// whether it finished and what it said on the way — the first line names the
/// branch it took, and that is worth asserting: a script that ignored the
/// override would download the real library and pass this test by accident.
fn install(destination: &Path, archive: &Path, revision: &str) -> Option<(bool, String)> {
    // What the archive actually holds, in case the script cannot find a member
    // in it. Printed rather than asserted: it is a diagnosis and not a claim.
    if let Ok(listing) = Command::new("tar")
        .arg("--list")
        .arg("--file")
        .arg(archive)
        .output()
    {
        println!(
            "the archive holds:\n{}",
            String::from_utf8_lossy(&listing.stdout)
        );
    }
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
        Ok(output) => Some((
            output.status.success(),
            // **Both streams.** A script that fails says why on stderr, and an
            // assertion that threw that away would leave a red build with
            // nothing in it but *the installer did not finish*.
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        )),
        Err(error) => {
            println!("skipping: the installer could not be run here ({error})");
            None
        }
    }
}

/// **No local variable of the PowerShell script shadows one of its parameters.**
///
/// A guard against the fault the commit that added `-Archive` shipped and CI
/// caught: **PowerShell variable names are case-insensitive**, so a local
/// `$archive` *is* the `$Archive` parameter. Assigning the work directory's
/// tarball path to it made `if ($Archive)` true on every run, and a machine that
/// had been told nothing went down the *local archive* branch to copy a file
/// that was not there — every Windows install, broken by a name.
///
/// Held as a check on the text because that is what the fault is: the two names
/// are the same name, and no execution of the script with the override set can
/// see it. The download branch itself is exercised by `.github/workflows/ci.yml`,
/// which is where it was found.
#[test]
fn the_powershell_script_has_no_local_that_is_also_a_parameter() {
    let script =
        std::fs::read_to_string(repository().join("tools/fetch-fixtures/fetch-fixtures.ps1"))
            .expect("the installer is in the repository");

    // The `param(...)` block, which is the first one in the file. Its closing
    // bracket is found by **counting**, not by the first `)`: a parameter's
    // default is an expression and carries brackets of its own.
    let opened = script
        .split_once("param(")
        .map(|(_, rest)| rest)
        .expect("the script takes parameters");
    let mut depth = 1_i32;
    let mut end = opened.len();
    for (at, character) in opened.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = at;
                    break;
                }
            }
            _ => {}
        }
    }
    let block = &opened[..end];
    let parameters: Vec<String> = block
        .split('$')
        .skip(1)
        .filter_map(|piece| {
            let name: String = piece
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty()).then_some(name.to_lowercase())
        })
        .collect();
    assert!(
        parameters.contains(&"archive".to_owned()),
        "the parameters were not found; this guard is reading the wrong thing: {parameters:?}"
    );

    for line in script.lines() {
        let Some((left, _)) = line.split_once('=') else {
            continue;
        };
        let trimmed = left.trim();
        let Some(name) = trimmed.strip_prefix('$') else {
            continue;
        };
        // An assignment, not a comparison or a parameter default.
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') || left.starts_with("    [") {
            continue;
        }
        assert!(
            !parameters.contains(&name.to_lowercase()),
            "`${name}` is a local and a parameter at once, and PowerShell cannot tell them \
             apart: {line}"
        );
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
    let Some((ran, said)) = install(&library, &archive, "abc123") else {
        return;
    };
    assert!(ran, "the installer did not finish, and said:\n{said}");
    assert!(
        said.contains("installing the Open Fixture Library from"),
        "the installer ignored the archive it was given and said: {said}"
    );

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
