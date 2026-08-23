//! This machine's settings, and the five commands that name a file — S37.
//!
//! S33 made the rig data and S36 made the surface data. This target is the rest
//! of what `prismd` used to be told on a command line, plus the half of the
//! *Show files* panel that belongs in this crate: **which paths a file command
//! will accept**, decided from the string alone, because whether the file is
//! there needs a disk and this crate deliberately has none.
//!
//! The claim every test here rests on is the one `desk.rs` has been making since
//! S11 and that S33 and S36 each made again: **none of it is show content.** A
//! show carried to another hall on a stick must not bring the first hall's
//! network exposure, its log level or its access token with it, any more than it
//! brings the cabling.

use prism_core::{
    DEFAULT_UNIVERSES, DEFAULT_WEBSOCKET_PORT, DeskId, MachineConfig, MachineError, RECENT_SHOWS,
    Show, ShowError, ShowFile,
};
use prism_domain::{Command, ExitAction, LogLevel, MachineChange};

mod common;

/// A desk with an identity, so a refusal is about the setting rather than about
/// the desk.
fn configured() -> MachineConfig {
    MachineConfig::new(DeskId::parse("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap())
}

/// Applies one setting and unwraps, which is what most of these want.
fn set(config: &mut MachineConfig, change: MachineChange) {
    config
        .apply(&Command::ConfigureMachine {
            change: change.clone(),
        })
        .unwrap_or_else(|error| panic!("{change:?} was refused: {error}"));
}

/// The listener is **on** and it is on loopback, which is S37's change to what a
/// desk out of the box is.
///
/// Before it the WebSocket listener was opt-in, so a browser could not reach a
/// desk nobody had passed a flag to — and the settings window this file is part
/// of, the Web Remote (S31) and the whole end-to-end suite all speak WebSocket.
/// Loopback is what makes having it on safe: `docs/IPC_PROTOCOL.md` §2.1's rule
/// is about reaching *off* the machine, and that still needs both an address and
/// a token.
#[test]
fn a_desk_out_of_the_box_listens_on_loopback_and_nowhere_else() {
    let config = configured();
    let settings = config.settings();
    assert!(settings.local, "the desktop shell's transport is open");
    assert_eq!(
        settings.websocket.map(|address| address.to_string()),
        Some(format!("127.0.0.1:{DEFAULT_WEBSOCKET_PORT}"))
    );
    assert!(
        settings
            .websocket
            .is_some_and(|address| address.ip().is_loopback()),
        "and it reaches nothing off this machine"
    );
    assert_eq!(settings.token, None);
    assert_eq!(settings.log_level, LogLevel::Info);
    assert_eq!(settings.universes, DEFAULT_UNIVERSES);
    assert_eq!(settings.exit_action, ExitAction::Hold);
    assert!(!settings.autostart);
    assert_eq!(settings.fixture_library, None);
    assert_eq!(settings.surface_profile, None);
}

/// Every setting, written and read back.
#[test]
fn each_setting_is_written_on_its_own() {
    let mut config = configured();
    set(&mut config, MachineChange::Local { local: false });
    assert!(!config.settings().local);

    set(
        &mut config,
        MachineChange::LogLevel {
            level: LogLevel::Debug,
        },
    );
    assert_eq!(config.settings().log_level, LogLevel::Debug);

    set(&mut config, MachineChange::Universes { universes: 12 });
    assert_eq!(config.settings().universes, 12);

    set(
        &mut config,
        MachineChange::ExitAction {
            action: ExitAction::Blackout,
        },
    );
    assert_eq!(config.settings().exit_action, ExitAction::Blackout);

    set(&mut config, MachineChange::Autostart { autostart: true });
    assert!(config.settings().autostart);

    set(
        &mut config,
        MachineChange::FixtureLibrary {
            path: Some("D:/fixtures".to_owned()),
        },
    );
    assert_eq!(
        config.settings().fixture_library.as_deref(),
        Some("D:/fixtures")
    );

    set(
        &mut config,
        MachineChange::SurfaceProfile {
            path: Some("profiles/surface/xtouch.json".to_owned()),
        },
    );
    assert_eq!(
        config.settings().surface_profile.as_deref(),
        Some("profiles/surface/xtouch.json")
    );

    // …and one setting at a time is the whole point of `MachineChange`: nothing
    // written above was disturbed by anything written after it.
    assert!(!config.settings().local);
    assert_eq!(config.settings().log_level, LogLevel::Debug);
    assert_eq!(config.settings().universes, 12);
}

/// A blank path is *no path*, which is `set_surface_port`'s rule one field
/// along: a stored `""` would be a file nothing can open that still counts as a
/// setting.
#[test]
fn a_cleared_box_means_nothing_rather_than_an_empty_string() {
    let mut config = configured();
    for blank in [None, Some(String::new()), Some("   ".to_owned())] {
        set(
            &mut config,
            MachineChange::FixtureLibrary {
                path: blank.clone(),
            },
        );
        assert_eq!(config.settings().fixture_library, None, "{blank:?}");
        set(
            &mut config,
            MachineChange::SurfaceProfile {
                path: blank.clone(),
            },
        );
        assert_eq!(config.settings().surface_profile, None, "{blank:?}");
        set(
            &mut config,
            MachineChange::Token {
                token: blank.clone(),
            },
        );
        assert_eq!(config.settings().token, None, "{blank:?}");
    }
}

/// `docs/IPC_PROTOCOL.md` §2.1, and it is refused **here** rather than in an
/// interface.
///
/// The reason it is not an interface's rule: a second client could otherwise put
/// an unauthenticated lighting console on a school's network, which is the exact
/// thing §2.1 exists to prevent. What the daemon does instead is name the
/// remedy, which is one command.
#[test]
fn a_listener_off_loopback_needs_a_token_and_says_so() {
    let mut config = configured();
    let public: std::net::SocketAddr = "0.0.0.0:7373".parse().unwrap();

    let refused = config
        .apply(&Command::ConfigureMachine {
            change: MachineChange::Websocket {
                address: Some(public),
            },
        })
        .unwrap_err();
    assert_eq!(refused, MachineError::NoTokenForNetwork(public));
    assert!(
        refused.to_string().contains("access token"),
        "{refused} does not say what to do"
    );
    // And nothing moved: the listener is where it was.
    assert_eq!(
        config
            .settings()
            .websocket
            .map(|address| address.to_string()),
        Some(format!("127.0.0.1:{DEFAULT_WEBSOCKET_PORT}"))
    );

    // Loopback needs none, because the rule is about reaching *off* the
    // machine and a password on an inside door protects nothing.
    set(
        &mut config,
        MachineChange::Websocket {
            address: Some("127.0.0.1:9000".parse().unwrap()),
        },
    );
    assert_eq!(
        config
            .settings()
            .websocket
            .map(|address| address.to_string()),
        Some("127.0.0.1:9000".to_owned())
    );

    // With a token it is accepted.
    set(
        &mut config,
        MachineChange::Token {
            token: Some("hunter2".to_owned()),
        },
    );
    set(
        &mut config,
        MachineChange::Websocket {
            address: Some(public),
        },
    );
    assert_eq!(config.settings().websocket, Some(public));
}

/// The other side of the same door, and it is the case a rule stated once would
/// have missed.
///
/// A configuration with a listener off loopback and **no** token is a state the
/// check above refuses to reach; taking the token away afterwards would reach it
/// from behind. So clearing the token puts the listener back on loopback, on the
/// port it was on, and the operator finds out from the delta that follows.
#[test]
fn taking_the_token_away_closes_the_door_it_was_holding_open() {
    let mut config = configured();
    set(
        &mut config,
        MachineChange::Token {
            token: Some("hunter2".to_owned()),
        },
    );
    set(
        &mut config,
        MachineChange::Websocket {
            address: Some("192.168.1.20:7373".parse().unwrap()),
        },
    );

    set(&mut config, MachineChange::Token { token: None });
    assert_eq!(config.settings().token, None);
    assert_eq!(
        config
            .settings()
            .websocket
            .map(|address| address.to_string()),
        Some("127.0.0.1:7373".to_owned()),
        "the port is kept and the reach is not"
    );

    // A listener that was already on loopback is left exactly where it was, and
    // so is one that is off altogether.
    set(&mut config, MachineChange::Token { token: None });
    assert_eq!(
        config
            .settings()
            .websocket
            .map(|address| address.to_string()),
        Some("127.0.0.1:7373".to_owned())
    );
    set(&mut config, MachineChange::Websocket { address: None });
    set(&mut config, MachineChange::Token { token: None });
    assert_eq!(config.settings().websocket, None);
}

/// The desk carries `1..=64` universes and a settings window can ask for none of
/// the others.
#[test]
fn a_universe_count_outside_the_desks_range_is_refused() {
    let mut config = configured();
    for universes in [0, DEFAULT_UNIVERSES + 1, 1000] {
        let refused = config
            .apply(&Command::ConfigureMachine {
                change: MachineChange::Universes { universes },
            })
            .unwrap_err();
        assert_eq!(
            refused,
            MachineError::UniverseCountOutOfRange(universes),
            "{universes}"
        );
        // In words an operator can act on, which is what every `MachineError`
        // promises: the range, and the number they typed.
        assert_eq!(
            refused.to_string(),
            format!("a desk carries between 1 and {DEFAULT_UNIVERSES} universes, not {universes}")
        );
        assert_eq!(config.settings().universes, DEFAULT_UNIVERSES);
    }
    for universes in [1, 12, DEFAULT_UNIVERSES] {
        set(&mut config, MachineChange::Universes { universes });
        assert_eq!(config.settings().universes, universes);
    }
}

/// The two changes whose value the daemon has to make, and neither writes
/// anything here.
///
/// A UUID and a token both need entropy, which this crate is not allowed to
/// have — and a **client** must not supply either: one would be choosing this
/// desk's password and the other would be able to give two desks one sACN CID.
/// So the applier answers with an effect and `prismd` fills it in.
#[test]
fn a_token_and_an_identity_are_asked_for_rather_than_sent() {
    let mut config = configured();
    let identity = config.desk_id();

    let applied = config
        .apply(&Command::ConfigureMachine {
            change: MachineChange::NewIdentity,
        })
        .unwrap();
    assert_eq!(
        applied.effects,
        vec![
            prism_core::Effect::NewDeskIdentity,
            prism_core::Effect::Machine
        ]
    );
    assert!(applied.deltas.is_empty(), "the daemon says what changed");
    assert_eq!(config.desk_id(), identity, "and nothing has changed yet");

    let applied = config
        .apply(&Command::ConfigureMachine {
            change: MachineChange::NewToken,
        })
        .unwrap();
    assert_eq!(
        applied.effects,
        vec![prism_core::Effect::NewToken, prism_core::Effect::Machine]
    );
    assert_eq!(config.settings().token, None);

    // And the second half, which is the daemon's: the value arrives through a
    // door of its own rather than through a command.
    config.set_token("made-by-the-daemon");
    assert_eq!(
        config.settings().token.as_deref(),
        Some("made-by-the-daemon")
    );
    config.set_desk_id(DeskId::from_bytes([7; 16]));
    assert_ne!(config.desk_id(), identity);
}

/// Every other machine command answers with the rig or the port; this one
/// answers with an effect and no delta at all, because half of what
/// `Delta::MachineChanged` carries is the daemon's knowledge.
#[test]
fn a_setting_change_leaves_the_delta_to_the_daemon() {
    let mut config = configured();
    let applied = config
        .apply(&Command::ConfigureMachine {
            change: MachineChange::Autostart { autostart: true },
        })
        .unwrap();
    assert!(applied.deltas.is_empty());
    assert_eq!(applied.effects, vec![prism_core::Effect::Machine]);
}

/// S33's and S36's claim, made for the third time and on the same evidence: a
/// show has nowhere to put any of this.
#[test]
fn the_settings_are_not_show_content() {
    let mut show = Show::new();
    show.embed_fixture_type(common::dimmer_type("generic.dimmer", 65_535))
        .unwrap();
    let mut config = configured();
    set(
        &mut config,
        MachineChange::Token {
            token: Some("classroom-token".to_owned()),
        },
    );
    set(
        &mut config,
        MachineChange::Websocket {
            address: Some("192.168.1.20:7373".parse().unwrap()),
        },
    );
    config.remember_show("D:/shows/aula.prism");

    let machine_json = serde_json::to_string(&config).unwrap();
    assert!(machine_json.contains("classroom-token"), "{machine_json}");
    assert!(machine_json.contains("192.168.1.20:7373"), "{machine_json}");
    assert!(
        machine_json.contains("D:/shows/aula.prism"),
        "{machine_json}"
    );

    let show_json = serde_json::to_string(&show).unwrap();
    for word in ["classroom-token", "192.168.1.20", "websocket", "logLevel"] {
        assert!(!show_json.contains(word), "the show learned {word}");
    }
}

/// Every machine configuration ever written is missing this whole block, so
/// every one of them has to open — and open with the *new* defaults rather than
/// with zeroes.
///
/// The distinction matters: a `#[serde(default)]` that produced
/// `websocket: None` would silently close a listener the operator never turned
/// off, on the day they updated.
#[test]
fn a_configuration_written_before_the_settings_existed_still_opens() {
    const TEXT: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";
    let config: MachineConfig = serde_json::from_str(&format!(r#"{{"deskId":"{TEXT}"}}"#)).unwrap();
    assert_eq!(config.desk_id(), DeskId::parse(TEXT).unwrap());
    assert!(config.settings().local);
    assert_eq!(
        config
            .settings()
            .websocket
            .map(|address| address.to_string()),
        Some(format!("127.0.0.1:{DEFAULT_WEBSOCKET_PORT}")),
        "an update must not close a listener nobody turned off"
    );
    assert_eq!(config.settings().universes, DEFAULT_UNIVERSES);
    assert!(config.shows().paths.is_empty());

    // And one written by S36, which has the rig and the surface and none of
    // this.
    let config: MachineConfig = serde_json::from_str(&format!(
        r#"{{"deskId":"{TEXT}","outputs":[],"surfacePort":"X-Touch"}}"#
    ))
    .unwrap();
    assert_eq!(config.surface_port(), Some("X-Touch"));
    assert_eq!(config.settings().log_level, LogLevel::Info);
}

/// The recent list: most recent first, no repeats, and bounded.
#[test]
fn the_shows_this_desk_has_opened_are_remembered_newest_first() {
    let mut config = configured();
    for index in 0..RECENT_SHOWS + 3 {
        config.remember_show(&format!("show{index}.prism"));
    }
    assert_eq!(config.shows().paths.len(), RECENT_SHOWS);
    assert_eq!(config.shows().paths[0], "show10.prism");

    // Opening one that is already in the list moves it rather than repeating
    // it — a menu with one file in it twice is a menu with a bug in it.
    config.remember_show("show6.prism");
    assert_eq!(config.shows().paths[0], "show6.prism");
    assert_eq!(
        config
            .shows()
            .paths
            .iter()
            .filter(|path| path.as_str() == "show6.prism")
            .count(),
        1
    );

    // What a panel draws is *the others*: the show that is open is named
    // separately and a list that repeated it would offer an operator a way to
    // open the file they are in.
    let others = config.shows().without("show6.prism");
    assert!(!others.contains(&"show6.prism".to_owned()));
    assert_eq!(others.len(), RECENT_SHOWS - 1);
}

/// What the five file commands will accept, decided from the string alone.
///
/// The extension is the whole check and it is not cosmetic: a `.prism` file is a
/// SQLite database and a `.json` export is text, so a path with the wrong one is
/// a command that would either fail obscurely or write one format under the
/// other's name.
#[test]
fn a_file_command_takes_a_path_that_could_hold_what_it_writes() {
    let mut show = common::populated_show();

    for path in [
        "aula.prism",
        "D:/shows/aula.prism",
        "/srv/shows/next term.prism",
        "AULA.PRISM",
    ] {
        for command in [
            Command::SaveShowAs {
                path: path.to_owned(),
            },
            Command::OpenShow {
                path: path.to_owned(),
            },
            Command::NewShow {
                path: path.to_owned(),
            },
        ] {
            assert!(show.apply(&command).is_ok(), "{command:?}");
        }
    }

    for path in ["", "   ", "aula", "aula.prism.bak", "aula.json", ".prism"] {
        let command = Command::OpenShow {
            path: path.to_owned(),
        };
        let refused = show.apply(&command).unwrap_err();
        assert_eq!(
            refused,
            ShowError::NotAShowPath {
                path: path.to_owned(),
                wanted: ".prism",
            },
            "{path:?} was accepted"
        );
        assert_eq!(
            refused.to_string(),
            format!("{path:?} is not a .prism file")
        );
    }

    // And the export's own extension, which is the other half of the same rule.
    assert!(
        show.apply(&Command::ExportShow {
            path: "aula.json".to_owned()
        })
        .is_ok()
    );
    assert_eq!(
        show.apply(&Command::ImportShow {
            path: "aula.prism".to_owned()
        }),
        Err(ShowError::NotAShowPath {
            path: "aula.prism".to_owned(),
            wanted: ".json",
        })
    );
}

/// None of the five touches the show, and the two that name a JSON file do not
/// even claim to.
///
/// Asserted on the serialised bytes rather than on the claim, which is the rule
/// `command_application.rs` follows for every refusal.
#[test]
fn a_file_command_changes_nothing_in_the_show_it_names() {
    let mut file = ShowFile::new();
    file.show = common::populated_show();
    let before = rmp_serde::to_vec_named(&file.show).unwrap();
    let revision = file.show.patch_revision();

    for command in [
        Command::SaveShowAs {
            path: "aula.prism".to_owned(),
        },
        Command::OpenShow {
            path: "aula.prism".to_owned(),
        },
        Command::NewShow {
            path: "aula.prism".to_owned(),
        },
        Command::ExportShow {
            path: "aula.json".to_owned(),
        },
        Command::ImportShow {
            path: "aula.json".to_owned(),
        },
    ] {
        let applied = file
            .apply(&command)
            .unwrap_or_else(|error| panic!("{command:?}: {error}"));
        assert!(applied.deltas.is_empty(), "{command:?} broadcast something");
        assert_eq!(applied.effects.len(), 1, "{command:?}");
        assert_eq!(
            rmp_serde::to_vec_named(&file.show).unwrap(),
            before,
            "{command:?} moved the show"
        );
        assert_eq!(file.show.patch_revision(), revision, "{command:?}");
    }
}

/// **None of the five is undoable**, and three of them are the reason the other
/// two are not either.
///
/// `OpenShow`, `NewShow` and `ImportShow` replace the show, which empties the
/// journal (`prism_core::journal`) — so a record filed against the show that was
/// open would describe a show that is gone. `SaveShowAs` and `ExportShow` change
/// no show state at all, so there is nothing to take back.
#[test]
fn no_file_command_is_journalled() {
    let mut file = ShowFile::new();
    file.show = common::populated_show();
    // One real edit, so the journal has something in it that must not move.
    file.apply(&Command::Label {
        target: prism_domain::ObjectRef::Sequence {
            sequence_id: prism_domain::SequenceId::new(1),
        },
        name: "Act one".to_owned(),
    })
    .unwrap();
    let depth = file.journal.len();
    assert_eq!(depth, 1);

    for command in [
        Command::SaveShow,
        Command::SaveShowAs {
            path: "aula.prism".to_owned(),
        },
        Command::OpenShow {
            path: "aula.prism".to_owned(),
        },
        Command::NewShow {
            path: "aula.prism".to_owned(),
        },
        Command::ExportShow {
            path: "aula.json".to_owned(),
        },
        Command::ImportShow {
            path: "aula.json".to_owned(),
        },
    ] {
        assert!(!command.is_undoable(), "{command:?}");
        file.apply(&command).unwrap();
        assert_eq!(file.journal.len(), depth, "{command:?} filed a step");
    }
}
