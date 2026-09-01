//! The switch S37 wrote down and nobody acted on.
//!
//! `prism_core::Settings::autostart` has existed since S37 with a comment
//! saying *stored here and acted on by the shell (S29), which is the one process
//! that can write a `HKCU\…\Run` entry or a user unit*. This is that.
//!
//! # The three tiers, and which one this is
//!
//! `ARCHITECTURE_SPEC.md` §10.3 has three: the **default**, where the shell
//! spawns the daemon and leaves it running (that is [`crate::spawn`]); the
//! **opt-in autostart**, which is this module and needs no administrator rights;
//! and the **advanced** permanent install, a Windows service, which needs them
//! and is not built. The middle tier is the one a school can actually use, and
//! it is the one the settings window offers a box for.
//!
//! # When the entry is written, and why it is all three moments
//!
//! S29 had to choose between *when the switch is set*, *at start* and *at exit*,
//! and the answer is the first two, for a reason that is easier to see from the
//! failure it prevents. The switch is a **setting**: it lives in `machine.json`,
//! any client can change it — a browser on the same machine, a Web Remote later
//! — and the daemon happily stores it while the shell is not running at all. The
//! entry is a **fact about this Windows account**, and a person can delete it
//! from Task Manager's Start-up tab without this program ever hearing about it.
//! Two states that drift apart on their own need reconciling whenever anybody is
//! in a position to look.
//!
//! So: [`reconcile`] runs **at every start**, which catches an entry deleted by
//! hand, an installation that moved and a switch changed while the shell was
//! shut; and it runs **again the moment the box is ticked**, because a setting
//! that took effect at the next start would be a box that appears to do nothing.
//! It deliberately does **not** run at exit. An exit-time write would fight the
//! box — quit a shell whose switch a second client had just turned off and the
//! last word would belong to whichever of them ran last — and, worse, a shell
//! that is being killed does not run it at all, which is the one case where a
//! stale entry matters most.
//!
//! # A switch that displays a lie is worse than no switch
//!
//! Which is why [`Report`] exists and the panel reads it. The stored flag says
//! what the operator asked for; the report says what this machine actually has.
//! Where they disagree — an entry deleted by hand, a build that cannot write one
//! — the panel says so in the same place rather than drawing a tick beside
//! nothing.
//!
//! # Only Windows writes one, and that is a decision rather than an omission
//!
//! §10.3's table names a systemd user unit and a `LaunchAgent` beside the
//! registry value, and neither is implemented here. **D10 makes Windows the only
//! release target**, and §10.1's own argument applies to this crate as much as
//! to any other: the ARM64 cross-check does not compile the shell, no CI job
//! does on Linux or macOS, so a Linux autostart written today would be code
//! nothing ever builds — which is the definition of the rot that section exists
//! to prevent. What is portable is everything above [`Entry`]: the comparison,
//! the report and the command line an entry carries are one code path
//! everywhere, so the day a Pi grows a screen the work is one implementation of
//! one trait with its tests already written.

use std::path::Path;

/// The name the start-up entry is filed under.
///
/// The product's name rather than the crate's, because this is a string a person
/// reads in Task Manager's Start-up tab and has to recognise.
pub const ENTRY_NAME: &str = "PrismDMX";

/// The argument an entry carries, so a desk that starts with the machine starts
/// **into the tray** rather than throwing a window over whatever is on screen at
/// log-in.
pub const HIDDEN_FLAG: &str = "--hidden";

/// What this machine actually has, as opposed to what the switch says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Whether this build can write a start-up entry at all. `false` on every
    /// platform but Windows — see the module documentation.
    pub supported: bool,
    /// Whether an entry exists under [`ENTRY_NAME`].
    pub installed: bool,
    /// The command line the entry carries, when there is one. Shown to a person
    /// rather than parsed: an entry pointing at a second installation is a thing
    /// to read, not a thing to repair silently.
    pub command: Option<String>,
    /// Whether that command is **this** installation's. `false` with `installed`
    /// true is an entry left behind by another copy of the program, which is the
    /// state an update leaves when it is installed somewhere else.
    pub matches_this_install: bool,
}

impl Report {
    /// The report of a platform that cannot have one.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self {
            supported: false,
            installed: false,
            command: None,
            matches_this_install: false,
        }
    }
}

/// What the shell should do about the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Write it — either because there is none, or because the one there names
    /// something else.
    Install,
    /// Take it away.
    Remove,
    /// The entry already says what the switch says.
    Nothing,
}

/// The command line an entry for this installation carries.
///
/// Quoted, because `HKCU\…\Run` values are parsed by the shell and
/// `C:\Program Files\…` has a space in it. The flag is what makes a start at
/// log-in a tray icon rather than a window.
#[must_use]
pub fn command_line(executable: &Path) -> String {
    format!("\"{}\" {HIDDEN_FLAG}", executable.display())
}

/// The three-way comparison, and the whole of the decision.
///
/// `wanted` is `prism_core::Settings::autostart`, `found` is what the platform
/// says is there, and `mine` is [`command_line`] for this executable.
///
/// The interesting case is the third: **an entry that exists but names something
/// else is reinstalled rather than left alone.** That is what an update looks
/// like from here — the program is at a new path and the old value points at a
/// directory that may not exist — and a desk whose switch is on and whose entry
/// points at last month's installation is exactly the switch that displays a
/// lie.
#[must_use]
pub fn reconcile(wanted: bool, found: Option<&str>, mine: &str) -> Action {
    match (wanted, found) {
        (true, Some(existing)) if existing == mine => Action::Nothing,
        (true, _) => Action::Install,
        (false, Some(_)) => Action::Remove,
        (false, None) => Action::Nothing,
    }
}

/// The report a panel reads, from the same three inputs.
#[must_use]
pub fn report(supported: bool, found: Option<String>, mine: &str) -> Report {
    if !supported {
        return Report::unsupported();
    }
    let matches_this_install = found.as_deref() == Some(mine);
    Report {
        supported: true,
        installed: found.is_some(),
        command: found,
        matches_this_install,
    }
}

/// Where a start-up entry is kept, behind a seam.
///
/// The seam is what makes everything above it testable with no registry: the
/// decision is [`reconcile`]'s, the report is [`report`]'s, and an
/// implementation of this trait does nothing but read, write and delete one
/// string.
pub trait Entry {
    /// The command line the entry carries, or `None` if there is not one.
    ///
    /// # Errors
    ///
    /// Whatever the platform said when asked.
    fn read(&self) -> Result<Option<String>, String>;

    /// Writes the entry, replacing whatever was there.
    ///
    /// # Errors
    ///
    /// Whatever the platform said when told.
    fn write(&self, command: &str) -> Result<(), String>;

    /// Removes the entry. Removing one that is not there **succeeds**: the
    /// caller asked for a machine with no entry on it and that is what it has.
    ///
    /// # Errors
    ///
    /// Whatever the platform said when told.
    fn remove(&self) -> Result<(), String>;
}

/// Brings the entry into line with the switch, and reports what is there
/// afterwards.
///
/// # Errors
///
/// The platform's own words, when the entry could not be read or written. A
/// failure here is **never** fatal to the shell: an autostart that could not be
/// written is a message, and a desk that refused to open a window because of one
/// would be a desk that cannot be used to turn the switch off again.
pub fn apply(entry: &dyn Entry, wanted: bool, mine: &str) -> Result<Report, String> {
    let found = entry.read()?;
    match reconcile(wanted, found.as_deref(), mine) {
        Action::Install => entry.write(mine)?,
        Action::Remove => entry.remove()?,
        Action::Nothing => {}
    }
    Ok(report(true, entry.read()?, mine))
}

/// The Windows registry value under `HKCU\…\Run`, which is §10.3's opt-in tier.
///
/// `HKEY_CURRENT_USER`, and that is the whole point: it is the user's own hive,
/// so writing it needs no elevation — which is what makes the box in the
/// settings window offerable at a school at all.
#[cfg(windows)]
pub mod windows {
    use super::Entry;

    /// Where Windows looks for what to start when this user logs in.
    pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    /// A named value under a key of `HKEY_CURRENT_USER`.
    ///
    /// # Why the key and the value are fields rather than constants
    ///
    /// So that this — the only code in the crate that touches the operating
    /// system — has tests. A test against [`RUN_KEY`] itself would put a real
    /// start-up entry on the machine running it, which is `CLAUDE.md`'s rule
    /// about hardware pointed at a registry: **a test may not need a device, and
    /// it may not leave one changed either.** Pointed at a scratch key it is the
    /// same three calls against the same API, and what it proves is what the
    /// unit tests above cannot: that a value written comes back, that removing
    /// one that is not there succeeds, and that a key that does not exist reads
    /// as *no entry* rather than as an error.
    ///
    /// [`super::platform_entry`] is what supplies the real pair, so nothing but
    /// a test ever names anything else.
    #[derive(Debug, Clone, Copy)]
    pub struct RunKey {
        /// The key under `HKEY_CURRENT_USER`.
        pub key: &'static str,
        /// The value's name.
        pub name: &'static str,
    }

    impl Default for RunKey {
        fn default() -> Self {
            Self {
                key: RUN_KEY,
                name: super::ENTRY_NAME,
            }
        }
    }

    impl Entry for RunKey {
        fn read(&self) -> Result<Option<String>, String> {
            let hive = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let key = match hive.open_subkey(self.key) {
                Ok(key) => key,
                // No such key at all is a clean account, not a fault.
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            match key.get_value::<String, _>(self.name) {
                Ok(value) => Ok(Some(value)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error.to_string()),
            }
        }

        fn write(&self, command: &str) -> Result<(), String> {
            let hive = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let (key, _) = hive
                .create_subkey(self.key)
                .map_err(|error| error.to_string())?;
            key.set_value(self.name, &command.to_owned())
                .map_err(|error| error.to_string())
        }

        fn remove(&self) -> Result<(), String> {
            let hive = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let key = match hive.open_subkey_with_flags(self.key, winreg::enums::KEY_ALL_ACCESS) {
                Ok(key) => key,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error.to_string()),
            };
            match key.delete_value(self.name) {
                Ok(()) => Ok(()),
                // Already gone is the state that was asked for.
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{RUN_KEY, RunKey};
        use crate::autostart::{Entry, apply};

        /// A key of this test's own, under the user's hive and nowhere near the
        /// one Windows reads at log-in.
        ///
        /// **One key per test**, because `cargo test` runs them on threads of a
        /// single process and the registry is global to the account: two tests
        /// sharing a key take turns deleting each other's value, which is
        /// exactly what the first draft of this did.
        const fn scratch(key: &'static str) -> RunKey {
            RunKey {
                key,
                name: "PrismDMX-test",
            }
        }

        /// Cleans up whatever a previous run left, including the key itself.
        ///
        /// **Its own key and no more.** The first draft swept the parent up as
        /// well and the two tests then deleted each other's: `cargo test` runs
        /// them on threads of one process, and the registry is global to the
        /// account. `HKCU\Software\PrismDMX` is left standing and empty, which
        /// is where this program's own keys would go anyway.
        fn forget(entry: &RunKey) {
            let hive = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let _ = entry.remove();
            let _ = hive.delete_subkey_all(entry.key);
        }

        /// The real registry, through the real API, on a key nothing else reads.
        ///
        /// It asserts the three things the fake in the tests above cannot: that
        /// a key which is not there reads as *no entry*, that a value written
        /// comes back verbatim, and that removing one twice succeeds twice.
        #[test]
        fn a_value_written_under_the_users_own_hive_comes_back() {
            let entry = scratch(r"Software\PrismDMX\autostart-test-values");
            forget(&entry);

            assert_eq!(
                entry.read().unwrap(),
                None,
                "a key that does not exist is an account with no entry, not a failure"
            );
            // Removing from a key that is not there is the state that was asked
            // for, and it is the path an operator takes by never ticking the box.
            entry.remove().unwrap();

            let command = "\"C:\\PrismDMX\\PrismDMX.exe\" --hidden";
            entry.write(command).unwrap();
            assert_eq!(entry.read().unwrap().as_deref(), Some(command));

            // Written over rather than duplicated — the update case, at the
            // registry rather than in the comparison.
            let moved = "\"D:\\PrismDMX\\PrismDMX.exe\" --hidden";
            entry.write(moved).unwrap();
            assert_eq!(entry.read().unwrap().as_deref(), Some(moved));

            entry.remove().unwrap();
            assert_eq!(entry.read().unwrap(), None);
            entry.remove().unwrap();

            forget(&entry);
        }

        /// And the whole of `apply` over it, which is the path the shell takes.
        #[test]
        fn the_switch_reaches_the_registry_and_back() {
            let entry = scratch(r"Software\PrismDMX\autostart-test-apply");
            forget(&entry);

            let mine = crate::autostart::command_line(std::path::Path::new(
                r"C:\Users\x\AppData\Local\PrismDMX\PrismDMX.exe",
            ));
            let on = apply(&entry, true, &mine).unwrap();
            assert!(on.supported && on.installed && on.matches_this_install);
            assert_eq!(on.command.as_deref(), Some(mine.as_str()));

            let off = apply(&entry, false, &mine).unwrap();
            assert!(!off.installed);
            assert_eq!(off.command, None);

            forget(&entry);
        }

        /// The real one is the one Windows reads, and it is named here so that
        /// moving it is a change somebody has to make on purpose.
        #[test]
        fn the_key_windows_reads_at_log_in_is_the_users_own() {
            assert_eq!(RUN_KEY, r"Software\Microsoft\Windows\CurrentVersion\Run");
            assert_eq!(RunKey::default().key, RUN_KEY);
            assert_eq!(RunKey::default().name, crate::autostart::ENTRY_NAME);
        }
    }
}

/// The switch, read out of `machine.json` without opening a daemon.
///
/// The shell needs the flag before there is a connection to ask over — the
/// reconciliation happens at start, and at start the daemon may not have been
/// spawned yet — so it reads the same file the daemon reads. **It never writes
/// one.** A shell that created `machine.json` would be a second author of a
/// document `prismd::machine` owns, and the first thing it would write is a desk
/// identity, which S37 makes the daemon's alone.
///
/// A file that is not there, or one that will not parse, answers **off**: an
/// autostart is opt-in (§10.3), and a desk that started with the machine because
/// a configuration file was damaged would be the wrong way round.
#[must_use]
pub fn stored_switch(data_dir: &Path) -> bool {
    std::fs::read_to_string(prismd::paths::machine_config_path(data_dir))
        .ok()
        .and_then(|text| serde_json::from_str::<prism_core::MachineConfig>(&text).ok())
        .is_some_and(|config| config.settings().autostart)
}

/// This platform's entry, or `None` where §10.3's opt-in tier is not built.
#[cfg(windows)]
#[must_use]
pub fn platform_entry() -> Option<Box<dyn Entry>> {
    Some(Box::new(windows::RunKey::default()))
}

/// This platform's entry, or `None` where §10.3's opt-in tier is not built.
#[cfg(not(windows))]
#[must_use]
pub fn platform_entry() -> Option<Box<dyn Entry>> {
    None
}

#[cfg(test)]
mod tests {
    use super::{
        Action, ENTRY_NAME, Entry, HIDDEN_FLAG, Report, apply, command_line, reconcile, report,
        stored_switch,
    };
    use std::cell::RefCell;
    use std::path::Path;

    /// An entry made of a cell, which is every property of a registry value this
    /// module's decisions depend on.
    #[derive(Default)]
    struct Remembered {
        value: RefCell<Option<String>>,
        writes: RefCell<usize>,
        removes: RefCell<usize>,
    }

    impl Entry for Remembered {
        fn read(&self) -> Result<Option<String>, String> {
            Ok(self.value.borrow().clone())
        }

        fn write(&self, command: &str) -> Result<(), String> {
            *self.writes.borrow_mut() += 1;
            *self.value.borrow_mut() = Some(command.to_owned());
            Ok(())
        }

        fn remove(&self) -> Result<(), String> {
            *self.removes.borrow_mut() += 1;
            *self.value.borrow_mut() = None;
            Ok(())
        }
    }

    /// An entry that refuses everything, which is a locked-down account.
    struct Refuses;

    impl Entry for Refuses {
        fn read(&self) -> Result<Option<String>, String> {
            Err("access is denied".to_owned())
        }

        fn write(&self, _command: &str) -> Result<(), String> {
            Err("access is denied".to_owned())
        }

        fn remove(&self) -> Result<(), String> {
            Err("access is denied".to_owned())
        }
    }

    fn mine() -> String {
        command_line(Path::new(r"C:\Users\x\AppData\Local\PrismDMX\PrismDMX.exe"))
    }

    #[test]
    fn the_command_line_is_quoted_and_starts_into_the_tray() {
        let command = command_line(Path::new(r"C:\Program Files\PrismDMX\PrismDMX.exe"));
        assert_eq!(
            command,
            format!("\"C:\\Program Files\\PrismDMX\\PrismDMX.exe\" {HIDDEN_FLAG}")
        );
        assert!(
            command.starts_with('"'),
            "an unquoted path with a space in it starts a different program"
        );
    }

    #[test]
    fn the_four_cases_of_the_comparison() {
        let mine = mine();
        assert_eq!(reconcile(true, None, &mine), Action::Install);
        assert_eq!(reconcile(true, Some(&mine), &mine), Action::Nothing);
        assert_eq!(reconcile(false, Some(&mine), &mine), Action::Remove);
        assert_eq!(reconcile(false, None, &mine), Action::Nothing);
    }

    /// **The update case.** The switch is on and the entry names last month's
    /// installation: reinstalled rather than believed.
    #[test]
    fn an_entry_naming_another_installation_is_written_over() {
        let mine = mine();
        let elsewhere = command_line(Path::new(r"C:\Old\PrismDMX.exe"));
        assert_eq!(reconcile(true, Some(&elsewhere), &mine), Action::Install);
    }

    /// The exit criterion: opt-in autostart installs and uninstalls cleanly.
    #[test]
    fn the_entry_is_installed_and_uninstalled_and_says_so_afterwards() {
        let mine = mine();
        let entry = Remembered::default();

        let on = apply(&entry, true, &mine).unwrap();
        assert_eq!(
            on,
            Report {
                supported: true,
                installed: true,
                command: Some(mine.clone()),
                matches_this_install: true,
            }
        );

        // Applying the same answer again writes nothing: a shell that rewrote
        // the value at every start would be a shell that logged a change every
        // morning.
        let again = apply(&entry, true, &mine).unwrap();
        assert_eq!(again, on);
        assert_eq!(*entry.writes.borrow(), 1);

        let off = apply(&entry, false, &mine).unwrap();
        assert!(!off.installed);
        assert_eq!(off.command, None);
        assert_eq!(*entry.removes.borrow(), 1);

        // …and turning it off when it is already off touches nothing either.
        apply(&entry, false, &mine).unwrap();
        assert_eq!(*entry.removes.borrow(), 1);
    }

    /// **The switch that would otherwise display a lie.** Somebody deleted the
    /// entry in Task Manager while the shell was shut; the report says what is
    /// actually there, and the next start puts it back.
    #[test]
    fn an_entry_deleted_by_hand_is_reported_and_then_restored() {
        let mine = mine();
        let entry = Remembered::default();
        apply(&entry, true, &mine).unwrap();

        entry.remove().unwrap();
        let seen = report(true, entry.read().unwrap(), &mine);
        assert!(
            !seen.installed,
            "the panel must not draw a tick beside nothing"
        );

        let restored = apply(&entry, true, &mine).unwrap();
        assert!(restored.installed);
        assert!(restored.matches_this_install);
    }

    /// An entry belonging to another installation is *reported* as one before
    /// it is repaired — the difference between a panel that explains and one
    /// that merely disagrees.
    #[test]
    fn an_entry_from_another_installation_is_named_rather_than_hidden() {
        let mine = mine();
        let elsewhere = command_line(Path::new(r"C:\Old\PrismDMX.exe"));
        let seen = report(true, Some(elsewhere.clone()), &mine);
        assert!(seen.installed);
        assert!(!seen.matches_this_install);
        assert_eq!(seen.command, Some(elsewhere));
    }

    #[test]
    fn a_platform_that_cannot_have_one_says_so_rather_than_saying_no() {
        let seen = report(false, Some(mine()), &mine());
        assert_eq!(seen, Report::unsupported());
        assert!(!seen.supported);
        // Round-trips, because a settings panel reads it over the Tauri bridge.
        let json = serde_json::to_string(&seen).unwrap();
        assert!(json.contains("\"supported\":false"), "{json}");
        assert_eq!(serde_json::from_str::<Report>(&json).unwrap(), seen);
    }

    /// A locked-down account is a message, not a shell that will not open.
    #[test]
    fn an_entry_that_cannot_be_written_is_an_error_the_caller_can_show() {
        let error = apply(&Refuses, true, &mine()).unwrap_err();
        assert!(error.contains("denied"), "{error}");
    }

    #[test]
    fn the_entry_is_filed_under_a_name_a_person_recognises() {
        assert_eq!(ENTRY_NAME, "PrismDMX");
    }

    /// The switch is read out of the daemon's own file, and a desk that has
    /// never been configured is **off** — which is what *opt-in* means.
    #[test]
    fn the_switch_is_read_from_machine_json_and_defaults_to_off() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!stored_switch(dir.path()), "no file at all is off");

        let path = prismd::paths::machine_config_path(dir.path());
        std::fs::write(&path, b"{ this is not json").unwrap();
        assert!(!stored_switch(dir.path()), "a damaged file is off, not on");

        let mut config = prism_core::MachineConfig::default();
        config
            .apply(&prism_domain::Command::ConfigureMachine {
                change: prism_domain::MachineChange::Autostart { autostart: true },
            })
            .unwrap();
        std::fs::write(&path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        assert!(stored_switch(dir.path()));

        // …and reading it left the file exactly as the daemon wrote it.
        let back: prism_core::MachineConfig =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back, config);
    }
}
