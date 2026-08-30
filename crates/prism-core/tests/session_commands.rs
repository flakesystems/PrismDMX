//! S12 exit criteria: every session command applies and emits a `SessionPatch`,
//! a rejection leaves the session **byte-identical**, the session survives save
//! and load, and the client-local state of `ARCHITECTURE_SPEC.md` §4.2 is
//! provably absent.
//!
//! "Byte-identical" is taken literally, as in S11: the session is serialised
//! with `rmp_serde::to_vec_named` before and after and the two `Vec<u8>` are
//! compared, together with the dirty flag, which is not part of the bytes and
//! which a rejection could otherwise move invisibly.

mod common;

use common::{
    machine_commands, populated_session, populated_show, session_commands, show_commands,
};
use prism_core::{SessionError, SessionState, ShowFile, session_patch_ops};
use prism_domain::{
    Command, ExecutorId, FeatureGroup, JsonValue, SessionId, ViewId, WindowInstanceId, WindowType,
};
use proptest::prelude::*;
use std::collections::BTreeMap;

/// The session as bytes, plus the flag that is not part of them.
fn snapshot(session: &SessionState) -> (Vec<u8>, bool) {
    (
        rmp_serde::to_vec_named(session).unwrap(),
        session.is_dirty(),
    )
}

#[test]
fn the_session_commands_are_the_session_group() {
    // §4.4's twelve, plus `PlaceWindow` — which a screen needs and a console
    // cannot issue — plus S40's four generic verbs **with a view as their
    // target**, which is the one of their six targets that is session state,
    // plus S43's two: `NewView` (a view that starts empty, B11) and
    // `SetWindowPicker` (the chooser a surface key opens, B9).
    // See `common::session_commands`.
    assert_eq!(session_commands().len(), 19);
    for command in session_commands() {
        assert!(command.is_session_command(), "{command:?}");
    }
    // And the three groups together are still the whole protocol —
    // `command_application.rs` carries why the sum is larger than the number of
    // variants there are. **Three since S33**, which gave this machine's rig an
    // applier of its own, and the machine group grew a fifth in S36: see
    // `common::machine_commands`.
    assert_eq!(
        show_commands().len() + session_commands().len() + machine_commands().len(),
        // Sixty-**four** since S48's `SetCueTracking`, which is a **show**
        // command.
        64
    );
}

/// S33's four are refused here, and they leave the session byte-identical while
/// being refused — the same claim `a_show_command_is_not_a_session_command`
/// makes, for the applier that did not exist when it was written.
#[test]
fn a_machine_command_is_not_a_session_command() {
    for command in machine_commands() {
        let mut session = populated_session();
        let before = snapshot(&session);
        assert!(!command.is_session_command(), "{command:?}");
        assert_eq!(
            session.apply(&command),
            Err(SessionError::NotASessionCommand),
            "{command:?}"
        );
        assert_eq!(snapshot(&session), before, "{command:?}");
    }
}

#[test]
fn every_session_command_applies_and_emits_a_session_patch() {
    for command in session_commands() {
        let mut session = populated_session();
        let applied = session
            .apply(&command)
            .unwrap_or_else(|error| panic!("{command:?} was refused: {error}"));
        assert!(
            !session_patch_ops(&applied.deltas).is_empty(),
            "{command:?} produced no operations"
        );
        // A session command asks nobody else to do anything: the session state
        // *is* the answer, and every client reads it from the delta.
        assert!(applied.effects.is_empty(), "{command:?}");
    }
}

#[test]
fn a_show_command_is_not_a_session_command() {
    for command in show_commands() {
        let mut session = populated_session();
        let before = snapshot(&session);
        assert_eq!(
            session.apply(&command),
            Err(SessionError::NotASessionCommand),
            "{command:?}"
        );
        assert_eq!(snapshot(&session), before, "{command:?}");
    }
}

#[test]
fn every_rejection_leaves_the_session_byte_identical() {
    // One case per way a session command can be refused.
    let cases: Vec<(Command, SessionError)> = vec![
        (
            Command::SelectView {
                view_id: ViewId::new(99),
            },
            SessionError::UnknownView(ViewId::new(99)),
        ),
        (
            Command::CloseWindow {
                instance_id: WindowInstanceId::new(99),
            },
            SessionError::UnknownWindow(WindowInstanceId::new(99)),
        ),
        (
            Command::FocusWindow {
                instance_id: WindowInstanceId::new(99),
            },
            SessionError::UnknownWindow(WindowInstanceId::new(99)),
        ),
        (
            // `ExecutorId::from_page_and_slot` saturates (S1), so a page this
            // large is not a page at all: all eight of its slots would address
            // the same executor.
            Command::SetExecutorPage { page: u32::MAX },
            SessionError::ExecutorPageOutOfRange { page: u32::MAX },
        ),
        (
            Command::OpenWindow {
                window: WindowType::FixtureSheet,
                params: Some(BTreeMap::from([(
                    "scrollTop".to_owned(),
                    JsonValue::Int(420),
                )])),
            },
            SessionError::ClientLocalParam {
                key: "scrollTop".to_owned(),
            },
        ),
    ];

    for (command, expected) in cases {
        let mut session = populated_session();
        // Clean and dirty alike: the flag must not move either way.
        for dirty in [false, true] {
            if dirty {
                session
                    .apply(&Command::StoreView {
                        view_id: ViewId::new(7),
                        name: "Busking".to_owned(),
                    })
                    .unwrap();
                assert!(session.is_dirty());
            }
            let before = snapshot(&session);
            assert_eq!(
                session.apply(&command),
                Err(expected.clone()),
                "{command:?}"
            );
            assert_eq!(snapshot(&session), before, "{command:?}");
        }
    }
}

#[test]
fn the_session_survives_save_and_load() {
    let mut session = populated_session();
    for command in [
        Command::SelectView {
            view_id: ViewId::new(2),
        },
        Command::OpenWindow {
            window: WindowType::PresetPool,
            params: Some(BTreeMap::from([(
                "pool".to_owned(),
                JsonValue::String("Color".to_owned()),
            )])),
        },
        Command::SetExecutorPage { page: 2 },
        Command::SelectExecutor {
            executor_id: ExecutorId::new(19),
        },
        Command::SetEncoderBank {
            group: FeatureGroup::Color,
        },
        Command::SetProgrammerPage { page: 3 },
        Command::SelectProgrammerParam {
            direction: prism_domain::ParamDirection::Next,
        },
        Command::CommandLineInput {
            text: "1 thru 4 at full".to_owned(),
            run: false,
        },
    ] {
        session.apply(&command).unwrap();
    }
    // The window is placed, because geometry is session state (§4.1) and no
    // §4.4 command carries any. **Somewhere clear of the other two**, which
    // S43 made a rule (`prism_core::layout`, punch-list B10): the two windows
    // view 2 restores sit at the top of the canvas, so a move back over them
    // changes nothing at all and this test would be asserting about a window
    // that had not moved.
    session
        .place_window(WindowInstanceId::new(3), 120.0, 512.0, 800.0, 512.0)
        .unwrap();

    let file = ShowFile {
        show: populated_show(),
        session: session.clone(),
        ..ShowFile::new()
    };

    // The wire codec the show file is written with (S1: `to_vec_named`).
    let bytes = rmp_serde::to_vec_named(&file).unwrap();
    let reopened: ShowFile = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(rmp_serde::to_vec_named(&reopened).unwrap(), bytes);
    assert_eq!(reopened.session, session);

    // Every field the exit criterion names, one by one, so a round trip that
    // was equal for the wrong reason cannot pass.
    let restored = reopened.session.session();
    assert_eq!(restored.active_view_id, ViewId::new(2));
    assert_eq!(restored.open_windows, session.session().open_windows);
    assert_eq!(restored.open_windows.len(), 3);
    assert_eq!(restored.focused_window, session.session().focused_window);
    assert_eq!(restored.executor_page, 2);
    assert_eq!(restored.selected_executor, Some(ExecutorId::new(19)));
    assert_eq!(restored.encoder_bank, FeatureGroup::Color);
    assert_eq!(restored.programmer_page, 3);
    assert_eq!(restored.programmer_param_index, 1);
    assert_eq!(restored.command_line, "1 thru 4 at full");
    assert_eq!(
        reopened.session.view(ViewId::new(2)).unwrap().windows.len(),
        2
    );

    let placed = &restored.open_windows[2];
    assert_eq!(
        (placed.x, placed.y, placed.w, placed.h),
        (120.0, 512.0, 800.0, 512.0)
    );

    // And through JSON, which is S15's export path. The geometry above is
    // exactly representable, which S1's finding about JSON floats requires of
    // anything asserted for equality across this codec.
    let text = serde_json::to_string(&file).unwrap();
    let via_json: ShowFile = serde_json::from_str(&text).unwrap();
    assert_eq!(via_json.session, session);
    // The show compares as its serialised form, because a reopened show starts
    // at patch revision 0 and clean — S11's rule that neither is show content.
    assert_eq!(
        rmp_serde::to_vec_named(&via_json.show).unwrap(),
        rmp_serde::to_vec_named(&file.show).unwrap()
    );
}

/// Every object key anywhere in a JSON document.
fn keys(value: &serde_json::Value, into: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(members) => {
            for (key, member) in members {
                into.push(key.clone());
                keys(member, into);
            }
        }
        serde_json::Value::Array(elements) => {
            for element in elements {
                keys(element, into);
            }
        }
        _ => {}
    }
}

#[test]
fn client_local_state_is_absent_from_the_session_document() {
    // §4.1's fifteen members, and nothing else. The session type is
    // `prism_domain::Session`, so this is the assertion that the daemon's
    // session document is that type plus the views it selects between. The two
    // S39 added are `selectedSequence` — the decision S28 marked and §4.4 gave
    // to this session — and `editingCue`, the state an Update key blinks on.
    //
    // **`windowPicker` is S43's, and it is the first member that looks like
    // client-local state and is not.** A chooser over the canvas would be §4.2's
    // category — hover, drag, scroll — except that a *surface key* opens it, and
    // a surface key is resolved by a daemon with no screen. See
    // `prism_domain::Session::window_picker`: on this desk the X-Touch drives the
    // interface too, and the two are never allowed to be out of step.
    //
    // **`commandLineRun` is S43's too, and it is the member that is here under
    // protest.** It is a counter the daemon bumps when a *bound* line asked to
    // be sent, and it is on the wire only because the command-line parser lives
    // in the interface: the daemon writes the line, and the client with the
    // keyboard focus is what turns it into commands. That is a stop-gap, it is
    // written down as one in `SurfaceAction::WriteCommandLine`, and
    // `IMPLEMENTATION_PLAN.md` S49 is the session that moves the parser and
    // takes this member out again. A test that let it in silently would be the
    // reason nobody remembered to.
    let session = populated_session();
    let document = serde_json::to_value(session.to_json().unwrap()).unwrap();
    let members: Vec<&str> = document["session"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        members,
        [
            "activeViewId",
            "commandLine",
            "commandLineRun",
            "editingCue",
            "encoderBank",
            "executorPage",
            "focusedWindow",
            "id",
            "name",
            "openWindows",
            "programmerPage",
            "programmerParamIndex",
            "selectedExecutor",
            "selectedSequence",
            "windowPicker",
        ]
    );
    assert_eq!(
        document.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["session", "views"]
    );

    // And nothing anywhere in the document — window instances and stored views
    // included — names one of §4.2's client-local concerns.
    let mut found = Vec::new();
    keys(&document, &mut found);
    assert!(!found.is_empty());
    for key in found {
        let lowered = key.to_ascii_lowercase();
        for forbidden in [
            "monitor", "screen", "scroll", "hover", "drag", "camera", "zoom",
        ] {
            assert!(
                !lowered.contains(forbidden),
                "{key:?} is {forbidden}, which §4.2 keeps client-local"
            );
        }
    }
}

#[test]
fn window_params_cannot_carry_client_local_state_either() {
    // `params` is the one member of the session that is an open bag, so it is
    // the one place §4.2 could be smuggled past the type. It is checked at the
    // door instead.
    let mut session = populated_session();
    for key in [
        "monitor",
        "screenIndex",
        "scrollTop",
        "hoverTarget",
        "dragging",
        "cameraPosition",
        "uiZoom",
    ] {
        let before = snapshot(&session);
        assert_eq!(
            session.apply(&Command::OpenWindow {
                window: WindowType::Viewer3D,
                params: Some(BTreeMap::from([(key.to_owned(), JsonValue::Bool(true))])),
            }),
            Err(SessionError::ClientLocalParam {
                key: key.to_owned()
            })
        );
        assert_eq!(snapshot(&session), before);
    }
    // What a window legitimately carries still goes through.
    session
        .apply(&Command::OpenWindow {
            window: WindowType::PresetPool,
            params: Some(BTreeMap::from([(
                "pool".to_owned(),
                JsonValue::String("Position".to_owned()),
            )])),
        })
        .unwrap();
}

#[test]
fn the_save_led_is_the_show_and_the_session_together() {
    // One LED, two sources. Storing a view is the one §4.4 command that is
    // authoring rather than operating, and it has to reach the LED.
    let mut file = ShowFile {
        show: populated_show(),
        session: populated_session(),
        ..ShowFile::new()
    };
    assert!(!file.is_dirty());

    let applied = file.apply(&Command::SetExecutorPage { page: 1 }).unwrap();
    assert!(!file.is_dirty(), "paging the fader bank is not an edit");
    assert!(
        applied
            .deltas
            .iter()
            .all(|delta| !matches!(delta, prism_domain::Delta::DirtyFlag { .. }))
    );

    let applied = file
        .apply(&Command::StoreView {
            view_id: ViewId::new(4),
            name: "Show".to_owned(),
        })
        .unwrap();
    assert!(file.is_dirty());
    assert!(applied.deltas.contains(&prism_domain::Delta::DirtyFlag {
        unsaved_changes: true
    }));

    // The LED is already on, so a show edit behind it does not light it twice.
    let applied = file.apply(&common::patch_command(9, 3, 1)).unwrap();
    assert!(
        applied
            .deltas
            .iter()
            .all(|delta| !matches!(delta, prism_domain::Delta::DirtyFlag { .. }))
    );

    assert!(file.mark_saved());
    assert!(!file.is_dirty());
    assert!(!file.mark_saved());
}

proptest! {
    /// The invariants that make the session describable at all, over arbitrary
    /// command sequences: the focused window is one that is open, no two open
    /// windows share a number, and the active view exists.
    #[test]
    fn the_session_invariants_survive_any_command_sequence(
        commands in proptest::collection::vec(prism_domain::arb::command(), 1..24)
    ) {
        let mut session = populated_session();
        for command in commands {
            let before = snapshot(&session);
            if session.apply(&command).is_err() {
                prop_assert_eq!(snapshot(&session), before);
            }

            let state = session.session();
            let ids: Vec<WindowInstanceId> = state
                .open_windows
                .iter()
                .map(|window| window.instance_id)
                .collect();
            if let Some(focused) = state.focused_window {
                prop_assert!(ids.contains(&focused));
            }
            let mut unique = ids.clone();
            unique.sort_unstable();
            unique.dedup();
            prop_assert_eq!(unique.len(), ids.len());
            prop_assert!(session.view(state.active_view_id).is_some());
            prop_assert_eq!(state.id, SessionId::new(1));
        }
    }
}
