//! The operating system's own file dialogue, for every path an operator types
//! — punch-list **B31**, moved here on the condition the owner named.
//!
//! # Why this could not be done in the browser
//!
//! **A browser cannot name a path on the daemon's machine.**
//! `showOpenFilePicker` hands a page a *handle*, never a path; the two are
//! deliberately not the same thing, and the daemon needs a path because the
//! daemon is what opens the file. On a desk where the browser and the daemon are
//! the same machine that is a wall in the way of something that would obviously
//! work, and the shell is what takes it down: the command below returns the
//! chosen path **as a string**, the settings window puts it in the box it has
//! had since S37, and `Command::OpenShow` travels unchanged.
//!
//! **Nothing about the protocol moves.** That is the measurement that says the
//! shape was right: S37's five file commands each carry a `path` and each of them
//! still does. The box stays too, because the Web Remote (S31) is a browser and
//! always will be — a desk reached from a phone in the auditorium types its
//! paths, and the same panel has to work in both places.
//!
//! **The control map is the exception and it is already done** (S43, punch-list
//! B26). It is exported and imported as a *file the browser holds* rather than a
//! path the daemon resolves, because a binding table belongs to the machine an
//! operator is sitting at and not to the one running the show. That one wants no
//! shell, and this module deliberately has no entry for it.
//!
//! # What is here, and what is in the window
//!
//! The table below — which dialogue each place opens, what it filters, and
//! whether it is an *open* or a *save* — is arithmetic and is tested. The
//! dialogue itself is the platform's, and the row for it is in
//! `ARCHITECTURE_SPEC.md` §14.

use serde::{Deserialize, Serialize};

/// A place in the interface where a path is asked for.
///
/// The five show-file names are `ui/src/settings/showfiles.tsx`'s `Asking`
/// values, spelled identically because that is what travels across the bridge;
/// the two others are the machine panel's boxes. A test in the interface holds
/// the two lists together, and this crate's own asserts that every name here has
/// a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathKind {
    /// A `.prism` file to open.
    OpenShow,
    /// Where to write the show, which is the file that is open from then on.
    SaveShowAs,
    /// Where to make an empty show.
    NewShow,
    /// Where to write the JSON export.
    ExportShow,
    /// A JSON export to read back.
    ImportShow,
    /// The directory the installed fixture library is in.
    FixtureLibrary,
    /// A surface binding profile to read.
    SurfaceProfile,
}

impl PathKind {
    /// Every kind, so a test can walk them and the caller can list them.
    pub const ALL: [Self; 7] = [
        Self::OpenShow,
        Self::SaveShowAs,
        Self::NewShow,
        Self::ExportShow,
        Self::ImportShow,
        Self::FixtureLibrary,
        Self::SurfaceProfile,
    ];
}

/// Which of the platform's three dialogues a place wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Pick a file that is there.
    OpenFile,
    /// Name a file, which may or may not be there.
    SaveFile,
    /// Pick a directory.
    OpenDirectory,
}

/// One file type the dialogue offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filter {
    /// What the drop-down says.
    pub name: &'static str,
    /// The extensions, without the dot.
    pub extensions: &'static [&'static str],
}

/// Everything the platform needs to be told to put the right dialogue up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chooser {
    /// The dialogue's title, which is the sentence an operator reads.
    pub title: &'static str,
    /// Open, save, or a directory.
    pub mode: Mode,
    /// What to offer, most specific first.
    pub filters: &'static [Filter],
    /// The name a save dialogue starts with, so a *Save as* on a fresh desk
    /// suggests something rather than nothing.
    pub suggested: Option<&'static str>,
}

/// A show file, and the extension `prism_core::ShowStore` writes.
const SHOWS: &[Filter] = &[
    Filter {
        name: "PrismDMX show",
        extensions: &["prism"],
    },
    Filter {
        name: "Every file",
        extensions: &["*"],
    },
];

/// The second format S15 built: a whole show as JSON, for a diff or a backup.
const JSON: &[Filter] = &[
    Filter {
        name: "PrismDMX export",
        extensions: &["json"],
    },
    Filter {
        name: "Every file",
        extensions: &["*"],
    },
];

/// The **table**, and it is the whole of this module's logic.
///
/// A `match` rather than a map, so a `PathKind` added without a row is a compile
/// error rather than a dialogue that opens on nothing.
#[must_use]
pub const fn chooser(kind: PathKind) -> Chooser {
    match kind {
        PathKind::OpenShow => Chooser {
            title: "Open a show",
            mode: Mode::OpenFile,
            filters: SHOWS,
            suggested: None,
        },
        PathKind::SaveShowAs => Chooser {
            title: "Save the show as",
            mode: Mode::SaveFile,
            filters: SHOWS,
            suggested: Some("show.prism"),
        },
        // A **save** dialogue for something that must not exist, and that is
        // deliberate: `NewShow` refuses a name something is already at rather
        // than replacing it (S37), so what an operator is doing here is *naming*
        // a file. An open dialogue cannot name one that is not there.
        PathKind::NewShow => Chooser {
            title: "Make a new show",
            mode: Mode::SaveFile,
            filters: SHOWS,
            suggested: Some("show.prism"),
        },
        PathKind::ExportShow => Chooser {
            title: "Export the show as JSON",
            mode: Mode::SaveFile,
            filters: JSON,
            suggested: Some("show.json"),
        },
        PathKind::ImportShow => Chooser {
            title: "Import a JSON export",
            mode: Mode::OpenFile,
            filters: JSON,
            suggested: None,
        },
        // A **directory**: `Settings::fixture_library` names the folder the
        // Open Fixture Library was installed into, not a file in it
        // (`prismd::paths::installed_library_dir`).
        PathKind::FixtureLibrary => Chooser {
            title: "Where the fixture library is",
            mode: Mode::OpenDirectory,
            filters: &[],
            suggested: None,
        },
        PathKind::SurfaceProfile => Chooser {
            title: "Open a surface binding profile",
            mode: Mode::OpenFile,
            filters: &[
                Filter {
                    name: "Binding profile",
                    extensions: &["json"],
                },
                Filter {
                    name: "Every file",
                    extensions: &["*"],
                },
            ],
            suggested: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Mode, PathKind, chooser};

    /// Every kind has a row, every row says something, and a save dialogue is
    /// the only kind that suggests a name.
    #[test]
    fn every_place_a_path_is_asked_for_has_a_dialogue() {
        for kind in PathKind::ALL {
            let chooser = chooser(kind);
            assert!(!chooser.title.is_empty(), "{kind:?}");
            match chooser.mode {
                Mode::SaveFile => assert!(
                    chooser.suggested.is_some(),
                    "a save dialogue with no suggested name opens on an empty box: {kind:?}"
                ),
                Mode::OpenFile | Mode::OpenDirectory => assert_eq!(
                    chooser.suggested, None,
                    "an open dialogue has nothing to suggest: {kind:?}"
                ),
            }
        }
    }

    /// The suggested name carries the extension the filter offers, or a *Save
    /// as* writes `show` and the desk cannot open it again.
    #[test]
    fn a_suggested_name_and_its_filter_agree() {
        for kind in PathKind::ALL {
            let chooser = chooser(kind);
            let Some(name) = chooser.suggested else {
                continue;
            };
            let extension = name.rsplit('.').next().expect("a suggested name");
            assert!(
                chooser
                    .filters
                    .iter()
                    .any(|filter| filter.extensions.contains(&extension)),
                "{kind:?} suggests {name} and no filter offers {extension}"
            );
        }
    }

    /// The three the daemon *resolves* against its own data directory are files;
    /// the library is the one directory in the set.
    #[test]
    fn the_fixture_library_is_the_only_directory() {
        for kind in PathKind::ALL {
            let directory = chooser(kind).mode == Mode::OpenDirectory;
            assert_eq!(directory, kind == PathKind::FixtureLibrary, "{kind:?}");
        }
    }

    /// A file dialogue that offered only one extension is a dialogue that hides
    /// the file an operator is looking at. Every file-shaped row has a way out.
    #[test]
    fn every_file_dialogue_can_be_made_to_show_everything() {
        for kind in PathKind::ALL {
            let chooser = chooser(kind);
            if chooser.mode == Mode::OpenDirectory {
                continue;
            }
            assert!(
                chooser
                    .filters
                    .iter()
                    .any(|filter| filter.extensions.contains(&"*")),
                "{kind:?} has no way to show a file with the wrong extension"
            );
        }
    }

    /// The names travel across the bridge, so they are the interface's own
    /// spelling and a rename is a breakage rather than a silent no-op.
    #[test]
    fn the_kinds_are_spelled_the_way_the_interface_spells_them() {
        let names: Vec<String> = PathKind::ALL
            .iter()
            .map(|kind| serde_json::to_string(kind).expect("a kind serialises"))
            .collect();
        assert_eq!(
            names,
            [
                "\"OpenShow\"",
                "\"SaveShowAs\"",
                "\"NewShow\"",
                "\"ExportShow\"",
                "\"ImportShow\"",
                "\"FixtureLibrary\"",
                "\"SurfaceProfile\"",
            ]
        );
    }

    /// And the five show-file kinds are exactly `Command`'s five file commands,
    /// which is what makes *the chosen path goes in the box the daemon is sent*
    /// true rather than nearly true.
    #[test]
    fn the_five_show_kinds_are_the_protocols_five_file_commands() {
        for (kind, command) in [
            (
                PathKind::OpenShow,
                prism_domain::Command::OpenShow {
                    path: "x".to_owned(),
                },
            ),
            (
                PathKind::SaveShowAs,
                prism_domain::Command::SaveShowAs {
                    path: "x".to_owned(),
                },
            ),
            (
                PathKind::NewShow,
                prism_domain::Command::NewShow {
                    path: "x".to_owned(),
                },
            ),
            (
                PathKind::ExportShow,
                prism_domain::Command::ExportShow {
                    path: "x".to_owned(),
                },
            ),
            (
                PathKind::ImportShow,
                prism_domain::Command::ImportShow {
                    path: "x".to_owned(),
                },
            ),
        ] {
            let tag = serde_json::to_value(&command).expect("a command serialises")["t"]
                .as_str()
                .expect("a tagged command")
                .to_owned();
            let name = serde_json::to_value(kind).expect("a kind serialises");
            assert_eq!(name.as_str(), Some(tag.as_str()), "{kind:?}");
        }
    }
}
