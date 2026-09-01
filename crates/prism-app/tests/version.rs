//! **One version, and this is what keeps it one** — S29.
//!
//! A release has a version number, and this tree has three places it could live:
//! `Cargo.toml`'s `[workspace.package]`, `tauri.conf.json`, and
//! `ui/package.json`. A number written in two of them is a number that
//! disagrees with itself the first time somebody bumps one and forgets the
//! other — and the failure is silent, because an installer built from a stale
//! configuration installs happily and reports the wrong version to whoever asks.
//!
//! **The workspace manifest is the source**, for two reasons. It is where every
//! crate in this repository already gets its version from (`version.workspace =
//! true`), so `prismd --version` and the shell's own resource block are already
//! two readings of it. And the Tauri bundler falls back to the crate's version
//! when its configuration does not name one — so the way to make the installer
//! agree is to **write nothing** in `tauri.conf.json`, which is a thing that
//! cannot go stale.
//!
//! `ui/package.json` is `"private": true` and its version is not a version of
//! anything: nothing publishes it and nothing reads it. It is asserted here to
//! be `0.0.0` rather than made to match, because a second number that tracked
//! the first would be the second number this file exists to prevent.

use std::path::{Path, PathBuf};

/// The repository root, from this crate's directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate is two levels under the repository root")
        .to_path_buf()
}

/// The Tauri configuration, as JSON.
fn tauri_config() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let text = std::fs::read_to_string(&path).expect("tauri.conf.json is beside the crate");
    serde_json::from_str(&text).expect("tauri.conf.json is JSON")
}

/// The bundler must find no version of its own, or there are two.
#[test]
fn the_bundler_is_given_no_version_to_disagree_with() {
    let config = tauri_config();
    assert!(
        config.get("version").is_none(),
        "tauri.conf.json must carry no `version`: the bundler falls back to the crate's, which is \
         the workspace's, and a number written here is a second one to keep in step"
    );
    // …and the crate's own is the workspace's, which is the other half of the
    // same claim.
    assert_eq!(prism_app::VERSION, env!("CARGO_PKG_VERSION"));
    assert!(
        prism_app::VERSION.split('.').count() == 3,
        "a release tag is built from this: {}",
        prism_app::VERSION
    );
}

/// The daemon and the shell are one program, so they are one version.
#[test]
fn the_shell_and_the_daemon_are_the_same_version() {
    // Both inherit `[workspace.package] version`, and this is the assertion that
    // says somebody has not given one of them a version of its own.
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).expect("the workspace root");
    let declared = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = \""))
        .and_then(|rest| rest.split('"').next())
        .expect("the workspace declares a version");
    assert_eq!(declared, prism_app::VERSION);

    for crate_name in ["prismd", "prism-core", "prism-domain"] {
        let manifest =
            std::fs::read_to_string(root().join("crates").join(crate_name).join("Cargo.toml"))
                .unwrap_or_else(|_| panic!("{crate_name} has a manifest"));
        assert!(
            manifest.contains("version.workspace = true"),
            "{crate_name} must take the workspace's version, or a release ships two numbers"
        );
    }
}

/// The interface is not a published package and does not carry a version.
#[test]
fn the_interface_carries_no_version_of_its_own() {
    let text = std::fs::read_to_string(root().join("ui").join("package.json"))
        .expect("the interface has a package.json");
    let package: serde_json::Value = serde_json::from_str(&text).expect("package.json is JSON");
    assert_eq!(package["private"], serde_json::Value::Bool(true));
    assert_eq!(
        package["version"],
        serde_json::Value::String("0.0.0".to_owned()),
        "nothing publishes the interface, so its version is not a version. Making it track the \
         workspace's would be the second number this file exists to prevent"
    );
}

/// What the installer will be called, and what it installs beside the shell.
///
/// Asserted because the release workflow attaches a file by name and the shell
/// looks for the daemon beside itself: three claims in three files that a
/// release only finds out about at the end.
#[test]
fn the_bundle_carries_the_daemon_and_the_profiles_beside_the_shell() {
    let config = tauri_config();
    assert_eq!(config["productName"], "PrismDMX");
    assert_eq!(config["bundle"]["targets"][0], "nsis");
    // §10.3: opt-in autostart works **without administrator rights**, so an
    // installer that asked for them would have chosen the wrong tier.
    assert_eq!(
        config["bundle"]["windows"]["nsis"]["installMode"], "currentUser",
        "a per-machine install needs administrator rights, which §10.3 says this desk must not"
    );

    let resources = &config["bundle"]["resources"];
    let engine = resources
        .as_object()
        .expect("the resources are a map")
        .values()
        .any(|target| target == "prismd.exe");
    assert!(
        engine,
        "the daemon has to land beside the shell: `prism_app::spawn::daemon_beside` looks there, \
         and so does `prismd::paths::installed_library_dir`"
    );
}
