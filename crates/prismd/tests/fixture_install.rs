//! **A venue's own profiles survive a library re-install** — punch-list entry
//! **B43**, and since S60 in both formats.
//!
//! # This runs the installers, and that is the point
//!
//! The entry is about a *destructive* step: `tools/fetch-fixtures` empties its
//! destination on every run, deliberately, because a half-replaced copy of
//! somebody else's data is worse than none. Whether a venue's own profiles
//! survive that is a question about the two directories being different ones,
//! and a test that reimplemented the wipe would be asserting against its own
//! copy of the script rather than against the script.
//!
//! So the real scripts run — `.ps1` on Windows, `.sh` everywhere else,
//! whichever this platform has — with their *local source* overrides pointing
//! at files this test builds. Nothing is downloaded: `CLAUDE.md` says a test may
//! not need a device, and a test that needs a network is the same promise broken
//! a different way. It is also the only way this can be tested at all for GDTF,
//! whose upstream has no anonymous download.
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

/* -------------------------------------------------------------------------- */
/* GDTF — the library the desk installs since S60                             */
/* -------------------------------------------------------------------------- */

/// A `description.xml` for a fixture with one mode of `footprint` dimmers.
fn description(manufacturer: &str, name: &str, footprint: u16) -> String {
    let channels: String = (1..=footprint)
        .map(|offset| {
            format!(
                r#"<DMXChannel Offset="{offset}">
                     <LogicalChannel Attribute="Dimmer">
                       <ChannelFunction Attribute="Dimmer"/>
                     </LogicalChannel>
                   </DMXChannel>"#
            )
        })
        .collect();
    format!(
        r#"<GDTF DataVersion="1.2">
             <FixtureType Name="{name}" Manufacturer="{manufacturer}" FixtureTypeID="GUID">
               <Geometries>
                 <Geometry Name="Body" Position="{{1,0,0,0}}{{0,1,0,0}}{{0,0,1,0}}{{0,0,0,1}}">
                   <Beam Name="Beam" Position="{{1,0,0,0}}{{0,1,0,0}}{{0,0,1,0}}{{0,0,0,1}}"
                         BeamAngle="15"/>
                 </Geometry>
               </Geometries>
               <DMXModes>
                 <DMXMode Name="Standard" Geometry="Body">
                   <DMXChannels>{channels}</DMXChannels>
                 </DMXMode>
               </DMXModes>
             </FixtureType>
           </GDTF>"#
    )
}

/// Writes a `.gdtf` file — a ZIP archive holding one `description.xml`.
///
/// Built here rather than taken from `prism-core`: a `#[cfg(test)]` module is
/// not compiled into the library an integration test links against, which is
/// the same reason `crates/prism-core/src/testkit.rs` says its callers carry
/// their own copy. Stored rather than deflated, so this stays thirty lines of
/// the format and no compression at all.
fn write_gdtf(path: &Path, description: &str) {
    let name = b"description.xml";
    let body = description.as_bytes();
    let crc = crc32(body);
    let mut out: Vec<u8> = Vec::new();

    out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes()); // local header
    out.extend_from_slice(&20_u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0_u16.to_le_bytes()); // flags
    out.extend_from_slice(&0_u16.to_le_bytes()); // stored
    out.extend_from_slice(&0_u32.to_le_bytes()); // time and date
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // extra
    out.extend_from_slice(name);
    out.extend_from_slice(body);

    let directory_at = u32::try_from(out.len()).expect("a small archive");
    let mut central: Vec<u8> = Vec::new();
    central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
    central.extend_from_slice(&20_u16.to_le_bytes()); // made by
    central.extend_from_slice(&20_u16.to_le_bytes()); // needed
    central.extend_from_slice(&0_u16.to_le_bytes()); // flags
    central.extend_from_slice(&0_u16.to_le_bytes()); // stored
    central.extend_from_slice(&0_u32.to_le_bytes()); // time and date
    central.extend_from_slice(&crc.to_le_bytes());
    central.extend_from_slice(&(body.len() as u32).to_le_bytes());
    central.extend_from_slice(&(body.len() as u32).to_le_bytes());
    central.extend_from_slice(&(name.len() as u16).to_le_bytes());
    central.extend_from_slice(&0_u16.to_le_bytes()); // extra
    central.extend_from_slice(&0_u16.to_le_bytes()); // comment
    central.extend_from_slice(&0_u16.to_le_bytes()); // disk
    central.extend_from_slice(&0_u16.to_le_bytes()); // internal
    central.extend_from_slice(&0_u32.to_le_bytes()); // external
    central.extend_from_slice(&0_u32.to_le_bytes()); // local header offset
    central.extend_from_slice(name);

    let size = u32::try_from(central.len()).expect("a small directory");
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes()); // end record
    out.extend_from_slice(&0_u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0_u16.to_le_bytes()); // directory's disk
    out.extend_from_slice(&1_u16.to_le_bytes()); // entries here
    out.extend_from_slice(&1_u16.to_le_bytes()); // entries in all
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes()); // comment

    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the tree is made");
    std::fs::write(path, out).expect("a .gdtf file");
}

/// CRC-32 as ZIP states it, by the table-free definition.
///
/// Thirty lines of the format rather than a dependency on this test target,
/// and it is checked against the one value everybody's CRC-32 agrees on.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let carry = crc & 1;
            crc >>= 1;
            if carry != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

#[test]
fn the_crc_this_test_writes_is_the_one_everybody_else_computes() {
    // The check value published with the algorithm. Without it a wrong CRC
    // here would be a `.gdtf` the desk rejects and a test that says the
    // installer is broken.
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

/// Runs the GDTF installer this platform has, against `destination`.
///
/// `None` when there is no interpreter for it here, which is printed rather
/// than passed over: a silent skip cannot be told from a pass. `Some` carries
/// whether it finished and what it said on the way — the first line names the
/// branch it took, and that is worth asserting: a script that ignored the
/// override would try to reach the network and pass this test by accident.
fn install_gdtf(destination: &Path, source: &Path) -> Option<(bool, String)> {
    let root = repository();
    let mut command = if cfg!(windows) {
        let mut command = Command::new("pwsh");
        command
            .arg("-NoProfile")
            .arg("-File")
            .arg(root.join("tools/fetch-fixtures/fetch-fixtures.ps1"))
            .arg("-Destination")
            .arg(destination);
        command
    } else {
        let mut command = Command::new("bash");
        command
            .arg(root.join("tools/fetch-fixtures/fetch-fixtures.sh"))
            .arg(destination);
        command
    };
    command.env("PRISMDMX_GDTF_SOURCE", source);
    // So that a machine with these in its environment does not send this test
    // at the real service.
    command.env_remove("PRISMDMX_GDTF_USER");
    command.env_remove("PRISMDMX_GDTF_PASSWORD");
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

/// **The exit criterion, in one run** — S60's half of B43.
///
/// A venue's own `.gdtf` is in the data directory and a different revision of
/// the same fixture is in what the installer is about to write. The installer
/// runs — the wipe included. Afterwards the venue's file is still there, still
/// wins the fixture it corrects, and is still marked as the venue's own.
#[test]
fn a_venues_own_gdtf_survives_a_library_re_install() {
    let work = tempfile::tempdir().expect("a temporary directory");
    let data_dir = work.path().join("data");
    let library = work.path().join("library");
    let upstream = work.path().join("upstream");
    std::fs::create_dir_all(&library).expect("the library directory");

    // The venue's own correction: the same fixture the library ships, with
    // four channels instead of two, **under a name of its own** — which is the
    // case a key taken from the file name gets wrong.
    let own = prismd::paths::fixtures_dir(&data_dir);
    write_gdtf(
        &own.join("my-corrected-wash.gdtf"),
        &description("Robe", "Wash 7Q5", 4),
    );

    // What the installer is given, as a directory on a stick would be.
    write_gdtf(
        &upstream.join("Robe@Wash 7Q5@3.gdtf"),
        &description("Robe", "Wash 7Q5", 2),
    );
    write_gdtf(
        &upstream.join("Robe@LEDBeam 150@1.gdtf"),
        &description("Robe", "LEDBeam 150", 1),
    );

    // A file the installer will delete, so this test can tell a run that
    // happened from one that did not.
    std::fs::write(library.join("SOURCE.md"), "committed").expect("the source note");
    std::fs::write(library.join("stale.gdtf"), "{}").expect("a stale file");

    let Some((ran, said)) = install_gdtf(&library, &upstream) else {
        return;
    };
    assert!(ran, "the installer did not finish, and said:\n{said}");
    assert!(
        said.contains("installing GDTF fixtures from"),
        "the installer ignored the source it was given and said: {said}"
    );
    assert!(
        said.contains("installed 2 GDTF fixtures"),
        "the installer did not install what it was given: {said}"
    );

    // The installer did what it does: its destination is now the source's, and
    // the committed note is the one thing it left.
    assert!(
        library.join("SOURCE.md").is_file(),
        "SOURCE.md is committed"
    );
    assert!(
        !library.join("stale.gdtf").exists(),
        "the wipe did not happen"
    );

    // And the venue's directory is untouched, because it is somewhere else.
    assert!(own.join("my-corrected-wash.gdtf").is_file());

    // The library the daemon would build out of the two, in the daemon's own
    // order: the venue's first, so a correction wins.
    let built = prismd::daemon::load_library(&data_dir, Some(&library));
    let entry = |id: &str| built.entries().iter().find(|entry| entry.id == id).cloned();

    let corrected = entry("robe/wash-7q5/standard").expect("the corrected profile is there");
    assert!(
        corrected.own,
        "the installed copy came back over the venue's correction"
    );
    assert!(
        corrected.gdtf,
        "it is a GDTF profile and the picker says so"
    );
    assert_eq!(
        corrected.footprint, 4,
        "the venue's four-channel answer is the one kept"
    );

    let other = entry("robe/ledbeam-150/standard").expect("the rest of the library is there");
    assert!(!other.own);
    assert!(other.gdtf);
}

/// **A `.gdtf` and an Open Fixture Library file live in the venue's folder at
/// once** — S60's promise that custom profiles in the old format keep working.
#[test]
fn a_venue_may_write_either_format() {
    let work = tempfile::tempdir().expect("a temporary directory");
    let data_dir = work.path().join("data");
    let library = work.path().join("library");
    std::fs::create_dir_all(&library).expect("the library directory");

    let own = prismd::paths::fixtures_dir(&data_dir);
    std::fs::create_dir_all(&own).expect("the venue's directory");
    std::fs::write(own.join("in-the-roof.json"), VENUE_FIXTURE).expect("a venue profile");
    write_gdtf(&own.join("published.gdtf"), &description("Robe", "T1", 3));

    let built = prismd::daemon::load_library(&data_dir, Some(&library));
    let entry = |id: &str| built.entries().iter().find(|entry| entry.id == id).cloned();

    let hand_written = entry("custom/in-the-roof/2ch").expect("the hand-written one is there");
    assert!(hand_written.own);
    assert!(
        !hand_written.gdtf,
        "an Open Fixture Library file is not GDTF"
    );

    let published = entry("robe/t1/standard").expect("the published one is there");
    assert!(published.own);
    assert!(published.gdtf);
}

/* -------------------------------------------------------------------------- */
/* The Open Fixture Library corpus, which is a separate script since S60       */
/* -------------------------------------------------------------------------- */

/// Builds an archive shaped exactly as the one `fetch-ofl` downloads: a single
/// top-level `open-fixture-library-<revision>/` holding `fixtures/` and
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

/// Runs the corpus installer this platform has, against `destination`.
fn install_ofl(destination: &Path, archive: &Path, revision: &str) -> Option<(bool, String)> {
    let root = repository();
    let mut command = if cfg!(windows) {
        let mut command = Command::new("pwsh");
        command
            .arg("-NoProfile")
            .arg("-File")
            .arg(root.join("tools/fetch-fixtures/fetch-ofl.ps1"))
            .arg("-Destination")
            .arg(destination);
        command
    } else {
        let mut command = Command::new("bash");
        command
            .arg(root.join("tools/fetch-fixtures/fetch-ofl.sh"))
            .arg(destination);
        command
    };
    command
        .env("PRISMDMX_OFL_ARCHIVE", archive)
        .env("PRISMDMX_OFL_REVISION", revision);
    match command.output() {
        Ok(output) => Some((
            output.status.success(),
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

/// **B43's original case, unchanged**: a venue's own JSON profile survives a
/// re-download of the Open Fixture Library corpus and still wins the key it
/// corrects.
#[test]
fn a_venues_own_profiles_survive_a_corpus_re_download() {
    let work = tempfile::tempdir().expect("a temporary directory");
    let data_dir = work.path().join("data");
    let library = work.path().join("library");
    std::fs::create_dir_all(&library).expect("the library directory");

    // The venue's own: one light nobody has a profile for, and one
    // **correction** to a shipped profile — the case that decides whether the
    // override works.
    let own = prismd::paths::fixtures_dir(&data_dir);
    std::fs::create_dir_all(own.join("robe")).expect("the venue's directory");
    std::fs::write(own.join("in-the-roof.json"), VENUE_FIXTURE).expect("a venue profile");
    std::fs::write(own.join("robe").join("wash-7q5.json"), VENUE_FIXTURE)
        .expect("a venue correction");

    std::fs::write(library.join("SOURCE.md"), "committed").expect("the source note");
    std::fs::write(library.join("stale.json"), "{}").expect("a stale file");

    let Some(archive) = archive(
        work.path(),
        "abc123",
        &[("robe/wash-7q5.json", VENDORED_FIXTURE)],
    ) else {
        return;
    };
    let Some((ran, said)) = install_ofl(&library, &archive, "abc123") else {
        return;
    };
    assert!(ran, "the installer did not finish, and said:\n{said}");
    assert!(
        said.contains("installing the Open Fixture Library from"),
        "the installer ignored the archive it was given and said: {said}"
    );

    assert!(
        library.join("SOURCE.md").is_file(),
        "SOURCE.md is committed"
    );
    assert!(
        !library.join("stale.json").exists(),
        "the wipe did not happen"
    );
    assert!(library.join("robe").join("wash-7q5.json").is_file());

    assert!(own.join("in-the-roof.json").is_file());
    assert!(own.join("robe").join("wash-7q5.json").is_file());

    let built = prismd::daemon::load_library(&data_dir, Some(&library));
    let entry = |id: &str| built.entries().iter().find(|entry| entry.id == id).cloned();

    let roof = entry("custom/in-the-roof/2ch").expect("the venue's own light is in the library");
    assert!(roof.own, "the picker cannot tell it is the venue's");
    assert_eq!(roof.name, "The one in the roof");

    let corrected = entry("robe/wash-7q5/2ch").expect("the corrected profile is in the library");
    assert!(
        corrected.own,
        "the shipped copy came back over the venue's correction"
    );
    assert_eq!(corrected.name, "The one in the roof");
}

/* -------------------------------------------------------------------------- */
/* The PowerShell guard                                                        */
/* -------------------------------------------------------------------------- */

/// **No local variable of a PowerShell script shadows one of its parameters.**
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
/// see it. Run over **both** scripts since S60, and the `-Source` of the GDTF
/// one is the same shape of parameter the fault was found in.
#[test]
fn no_powershell_local_is_also_a_parameter() {
    for (file, expected) in [
        ("tools/fetch-fixtures/fetch-fixtures.ps1", "source"),
        ("tools/fetch-fixtures/fetch-ofl.ps1", "archive"),
    ] {
        let script =
            std::fs::read_to_string(repository().join(file)).expect("the installer is here");

        // The `param(...)` block, which is the first one in the file. Its
        // closing bracket is found by **counting**, not by the first `)`: a
        // parameter's default is an expression and carries brackets of its own.
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
            parameters.contains(&expected.to_owned()),
            "the parameters of {file} were not found; this guard is reading the wrong \
             thing: {parameters:?}"
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
                "`${name}` is a local and a parameter at once in {file}, and PowerShell cannot \
                 tell them apart: {line}"
            );
        }
    }
}
