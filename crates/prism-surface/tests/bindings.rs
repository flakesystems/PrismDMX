//! Layer 3 against the document that specifies it.
//!
//! `docs/MCU_MAPPING.md` §4.1 is a table in prose, and this target is that table
//! **transcribed by hand**. The distinction is the whole point and it is S19's
//! finding for the third time: a test that built its expectations out of
//! `Bindings::defaults()` would pass for any defaults at all, including ones
//! where Play and Stop had been swapped. So the left-hand column below is copied
//! from the document by a person, and nothing here reads the table it is
//! checking.
//!
//! Three claims, in order:
//!
//! 1. every row of §4.1 is in the built-in defaults;
//! 2. the shipped `profiles/surface/xtouch.json` **is** those defaults, so the
//!    file a user edits and the table a daemon falls back to are one thing;
//! 3. a profile that will not parse never blocks a startup — asserted as a
//!    property over deliberately broken texts rather than on one example.

use prism_domain::{
    Command, ExecutorButtonFunction, ExecutorButtonRef, ExecutorId, FeatureGroup, ParamDirection,
    ViewId, WindowType,
};
use prism_surface::{
    Bindings, BoundControl, ButtonId, ExecutorTarget, Fader, GlobalButton, ProfileError, Step,
    StripButton, SurfaceAction, SurfaceContext, SurfaceEvent, X_TOUCH,
};

/// The profile as it ships. Compiled in, so a file that stopped parsing fails
/// the build rather than one desk on one evening.
const SHIPPED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../profiles/surface/xtouch.json"
));

/// A session in the middle of a show: page 2, executor 19 selected, views 1 and
/// 7 either side of the active one, programmer on page 3.
///
/// Nothing at its default, for S14's reason — a context of zeroes cannot tell
/// *carried through* from *never read*.
fn context() -> SurfaceContext {
    SurfaceContext {
        executor_page: 2,
        selected_executor: Some(ExecutorId::new(19)),
        previous_view: Some(ViewId::new(1)),
        next_view: Some(ViewId::new(7)),
        programmer_page: 3,
        parameter: Some(prism_domain::AttributeType::Pan),
    }
}

/// The executor under strip *n* on page 2 — `page * 8 + n`, worked out here so
/// the expectations below are numbers rather than a call to the thing under
/// test.
fn on_strip(strip: u32) -> ExecutorId {
    ExecutorId::new(16 + strip)
}

fn press(button: GlobalButton) -> SurfaceEvent {
    SurfaceEvent::Button {
        button: ButtonId::Global(button),
        pressed: true,
    }
}

fn strip_press(strip: u8, button: StripButton) -> SurfaceEvent {
    SurfaceEvent::Button {
        button: ButtonId::Strip { strip, button },
        pressed: true,
    }
}

// ---------------------------------------------------------------- §4.1, by hand

#[test]
fn strip_faders_are_the_master_of_the_executor_on_that_strip() {
    // §4.1 row 1: "Strip fader 1-8 | Master of the executor on that strip |
    // Engine". D7 puts that executor at executorPage * 8 + index.
    let table = Bindings::defaults();
    let context = context();
    for strip in 0..8u8 {
        assert_eq!(
            table.command(
                SurfaceEvent::Moved {
                    fader: Fader::Strip(strip),
                    level: 30_000,
                },
                &context
            ),
            Some(Command::SetExecutorMaster {
                executor_id: on_strip(u32::from(strip)),
                level: 30_000,
            }),
            "strip {strip}"
        );
    }
}

#[test]
fn the_four_strip_buttons_press_that_strips_executors_own_buttons() {
    // §4.1 row 2: "Strip Rec / Solo / Mute / Select | Go+ | Engine |
    // **yes — Empty / Go+ / Go- / LearnSpeed / Off / On / Flash / Toggle**".
    //
    // That list is `ExecutorButtonFunction`, which is *show* data on the
    // executor. Until S34 the protocol had no command that could carry a press
    // without also deciding what it meant, so the row was bound to `Go+` and the
    // deviation recorded. It now sends the **position**, and the executor's own
    // `buttonFunctions` decide — which is what the row says, and the reason the
    // configurable column can finally be honoured for all eight functions rather
    // than the three that happened to have commands of their own.
    let table = Bindings::defaults();
    let context = context();
    for (index, button) in [
        StripButton::Rec,
        StripButton::Solo,
        StripButton::Mute,
        StripButton::Select,
    ]
    .into_iter()
    .enumerate()
    {
        for strip in 0..8u8 {
            assert_eq!(
                table.command(strip_press(strip, button), &context),
                Some(Command::ExecutorButton {
                    executor_id: on_strip(u32::from(strip)),
                    button: ExecutorButtonRef::Slot {
                        index: u8::try_from(index).unwrap(),
                    },
                    pressed: true,
                }),
                "strip {strip} {button}"
            );
        }
    }
}

/// The release of a strip button reaches the daemon too, and nothing else's
/// does.
///
/// A `Flash` is momentary and its release is the half that puts the master
/// back. Layer 3 cannot know whether the executor has a `Flash` on that key, so
/// it forwards both edges; `prism_core::Show::apply` drops the release of every
/// function that is not momentary.
#[test]
fn a_strip_button_release_is_forwarded_and_a_panel_instruction_release_is_not() {
    let table = Bindings::defaults();
    let context = context();
    assert_eq!(
        table.command(
            SurfaceEvent::Button {
                button: ButtonId::Strip {
                    strip: 0,
                    button: StripButton::Rec,
                },
                pressed: false,
            },
            &context
        ),
        Some(Command::ExecutorButton {
            executor_id: on_strip(0),
            button: ExecutorButtonRef::Slot { index: 0 },
            pressed: false,
        })
    );
    assert_eq!(
        table.command(
            SurfaceEvent::Button {
                button: ButtonId::Global(GlobalButton::Save),
                pressed: false,
            },
            &context
        ),
        None
    );
}

#[test]
fn the_strip_encoder_is_empty() {
    // §4.1 row 3: "Strip encoder | Empty". Empty is a binding decision, not an
    // omission: the row exists and says nothing is on it.
    let table = Bindings::defaults();
    assert_eq!(table.action(BoundControl::StripEncoder), None);
    assert_eq!(
        table.command(SurfaceEvent::Encoder { strip: 4, steps: 2 }, &context()),
        None
    );
}

#[test]
fn the_main_fader_and_the_flip_button_act_on_the_selected_executor() {
    // §4.1 rows 5 and 6: "Main fader | XFade of the selected executor" and
    // "Flip button | Go+ of the selected executor".
    //
    // XFade is an `ExecutorFaderFunction` rather than a command — the protocol
    // has one fader command for all four functions and what the fader does is
    // the executor's own setting (`ARCHITECTURE_SPEC.md` §6). Since S34 the
    // daemon routes `SetExecutorMaster` through that setting, so a fader whose
    // executor says `XFade` crossfades and one that says `Master` moves a
    // master — the row means what it says, and this asserts the binding half of
    // it: *the selected executor's fader*.
    let table = Bindings::defaults();
    let context = context();
    assert_eq!(
        table.command(
            SurfaceEvent::Moved {
                fader: Fader::Main,
                level: 65_535,
            },
            &context
        ),
        Some(Command::SetExecutorMaster {
            executor_id: ExecutorId::new(19),
            level: 65_535,
        })
    );
    assert_eq!(
        table.command(press(GlobalButton::Flip), &context),
        Some(Command::ExecutorButton {
            executor_id: ExecutorId::new(19),
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::GoForward,
            },
            pressed: true,
        })
    );
}

#[test]
fn the_transport_section_is_on_off_forward_and_back_on_the_selected_executor() {
    // §4.1 row 7: "Play / Stop / Forward / Backward | On / Off / Go+ / Go- on
    // the selected executor | Engine".
    //
    // The four functions are named outright, which is what a *profile* is
    // allowed to do: the desk's own configuration, written by a person,
    // choosing which function a key sends. What it may not do — and does not —
    // is decide what that function comes out as. `Toggle` bound here would
    // still be resolved against `is_active` by the daemon.
    //
    // Until S34 there was no command that could carry `On`, so Play resolved to
    // a Go and the deviation was recorded. It no longer does.
    let table = Bindings::defaults();
    let context = context();
    let selected = ExecutorId::new(19);
    for (button, function) in [
        (GlobalButton::Play, ExecutorButtonFunction::On),
        (GlobalButton::Stop, ExecutorButtonFunction::Off),
        (GlobalButton::FastForward, ExecutorButtonFunction::GoForward),
        (GlobalButton::Rewind, ExecutorButtonFunction::GoBack),
    ] {
        assert_eq!(
            table.command(press(button), &context),
            Some(Command::ExecutorButton {
                executor_id: selected,
                button: ExecutorButtonRef::Function { function },
                pressed: true,
            }),
            "{button}"
        );
    }
}

#[test]
fn record_clears_the_programmer() {
    // §4.1 row 8: "Record | Clear (three-stage) | Programmer". The three stages
    // are the programmer's; the surface presses the button.
    assert_eq!(
        Bindings::defaults().command(press(GlobalButton::Record), &context()),
        Some(Command::ClearProgrammer)
    );
}

#[test]
fn the_fader_bank_pages_the_executors_and_the_channel_keys_change_the_view() {
    // §4.1 rows 9 and 10: "Faderbank ◀▶ | executor page down / up - 8 per page
    // (D7) | Session" and "Channel ◀▶ | switch UI view - SelectView (D8) |
    // Session".
    let table = Bindings::defaults();
    let context = context();
    assert_eq!(
        table.command(press(GlobalButton::BankLeft), &context),
        Some(Command::SetExecutorPage { page: 1 })
    );
    assert_eq!(
        table.command(press(GlobalButton::BankRight), &context),
        Some(Command::SetExecutorPage { page: 3 })
    );
    assert_eq!(
        table.command(press(GlobalButton::ChannelLeft), &context),
        Some(Command::SelectView {
            view_id: ViewId::new(1)
        })
    );
    assert_eq!(
        table.command(press(GlobalButton::ChannelRight), &context),
        Some(Command::SelectView {
            view_id: ViewId::new(7)
        })
    );
}

#[test]
fn the_cursor_cluster_pages_the_programmer_and_walks_its_parameters() {
    // §4.1 rows 11 and 12: "Zoom ▲▼ | programmer page up / down | Session" and
    // "Zoom ◀▶ | select previous / next programmer parameter | Session".
    // `XTouch.txt`'s Zoom cluster is this panel's cursor keys (§2.1, notes
    // 96-99); the Zoom button beside them (note 100) is what a DAW uses to put
    // those keys into a second mode, and PrismDMX leaves it unbound.
    let table = Bindings::defaults();
    let context = context();
    assert_eq!(
        table.command(press(GlobalButton::CursorUp), &context),
        Some(Command::SetProgrammerPage { page: 2 })
    );
    assert_eq!(
        table.command(press(GlobalButton::CursorDown), &context),
        Some(Command::SetProgrammerPage { page: 4 })
    );
    assert_eq!(
        table.command(press(GlobalButton::CursorLeft), &context),
        Some(Command::SelectProgrammerParam {
            direction: ParamDirection::Prev,
        })
    );
    assert_eq!(
        table.command(press(GlobalButton::CursorRight), &context),
        Some(Command::SelectProgrammerParam {
            direction: ParamDirection::Next,
        })
    );
    assert_eq!(table.action(BoundControl::Global(GlobalButton::Zoom)), None);
}

#[test]
fn the_jog_wheel_changes_the_selected_programmer_parameter() {
    // §4.1 row 13: "Jog wheel | change the value of the selected programmer
    // parameter | Programmer". The steps arrive through the jog curve already
    // (§2.7: the wheel raises its rate, never its magnitude), so the value here
    // is the one layer 2 worked out and this layer does no arithmetic.
    let table = Bindings::defaults();
    assert_eq!(
        table.command(SurfaceEvent::Jog { steps: -5 }, &context()),
        Some(Command::SetAttribute {
            attribute: prism_domain::AttributeType::Pan,
            value: -5,
            relative: true,
        })
    );
}

#[test]
fn the_encoder_assign_section_switches_the_encoder_bank() {
    // §4.1 row 14: "Encoder Assign section | switch encoder bank - Dimmer /
    // Position / Color / Beam / Focus | Session". Five groups, six buttons; the
    // sixth is left alone.
    let table = Bindings::defaults();
    let context = context();
    for (button, group) in [
        (GlobalButton::AssignTrack, FeatureGroup::Dimmer),
        (GlobalButton::AssignSend, FeatureGroup::Position),
        (GlobalButton::AssignPan, FeatureGroup::Color),
        (GlobalButton::AssignPlugin, FeatureGroup::Beam),
        (GlobalButton::AssignEq, FeatureGroup::Focus),
    ] {
        assert_eq!(
            table.command(press(button), &context),
            Some(Command::SetEncoderBank { group }),
            "{button}"
        );
    }
    assert_eq!(
        table.action(BoundControl::Global(GlobalButton::AssignInstrument)),
        None
    );
}

#[test]
fn the_eight_function_keys_are_free_and_four_of_them_open_a_window() {
    // §4.1 row 15: "F1-F8 (XKeys) | free: open window, jump to view, macro,
    // executor | Session or Engine | yes". *Free* is the requirement, so what is
    // asserted is that they are configurable and that the defaults are windows
    // rather than which windows: the file is where that is decided.
    let table = Bindings::defaults();
    let context = context();
    for key in [
        GlobalButton::F1,
        GlobalButton::F2,
        GlobalButton::F3,
        GlobalButton::F4,
    ] {
        assert!(
            matches!(
                table.command(press(key), &context),
                Some(Command::OpenWindow { .. })
            ),
            "{key} should open a window"
        );
    }
    for key in [
        GlobalButton::F5,
        GlobalButton::F6,
        GlobalButton::F7,
        GlobalButton::F8,
    ] {
        assert_eq!(table.action(BoundControl::Global(key)), None, "{key}");
    }
    // And "free" is a claim about the table rather than about the defaults: any
    // of the eight takes any action.
    let mut edited = Bindings::defaults();
    edited.set(
        BoundControl::Global(GlobalButton::F8),
        Some(SurfaceAction::SelectView {
            view: ViewId::new(2),
        }),
    );
    assert_eq!(
        edited.command(press(GlobalButton::F8), &context),
        Some(Command::SelectView {
            view_id: ViewId::new(2)
        })
    );
}

#[test]
fn save_writes_the_show_and_undo_is_oops() {
    // §4.1 rows 16 and 17: "Save | save show file; LED lit while unsaved changes
    // exist | Core" and "Undo | Oops | Core". The LED is the daemon's to drive
    // from `Delta::DirtyFlag`; the binding is the press.
    let table = Bindings::defaults();
    let context = context();
    assert_eq!(
        table.command(press(GlobalButton::Save), &context),
        Some(Command::SaveShow)
    );
    assert_eq!(
        table.command(press(GlobalButton::Undo), &context),
        Some(Command::Oops)
    );
}

#[test]
fn nothing_else_on_the_panel_is_bound_by_default() {
    // The complement of §4.1, which the table has to get right as well: a
    // default that quietly bound the Automation row would be a desk that did
    // something nobody asked for. Transcribed by hand from the rows above.
    let table = Bindings::defaults();
    let bound = [
        GlobalButton::AssignTrack,
        GlobalButton::AssignSend,
        GlobalButton::AssignPan,
        GlobalButton::AssignPlugin,
        GlobalButton::AssignEq,
        GlobalButton::BankLeft,
        GlobalButton::BankRight,
        GlobalButton::ChannelLeft,
        GlobalButton::ChannelRight,
        GlobalButton::Flip,
        GlobalButton::F1,
        GlobalButton::F2,
        GlobalButton::F3,
        GlobalButton::F4,
        GlobalButton::Save,
        GlobalButton::Undo,
        GlobalButton::Enter,
        GlobalButton::Rewind,
        GlobalButton::FastForward,
        GlobalButton::Stop,
        GlobalButton::Play,
        GlobalButton::Record,
        GlobalButton::CursorUp,
        GlobalButton::CursorDown,
        GlobalButton::CursorLeft,
        GlobalButton::CursorRight,
    ];
    for button in GlobalButton::ALL {
        let action = table.action(BoundControl::Global(button));
        assert_eq!(
            action.is_some(),
            bound.contains(&button),
            "Global.{button} is {}bound and should not be",
            if action.is_some() { "" } else { "un" }
        );
    }
    // 26 panel buttons, the two strip rows, the two faders and the wheel.
    assert_eq!(table.bound(), bound.len() + 5 + 1 + 1 + 1);
}

#[test]
fn the_reserved_button_is_unbound_in_the_defaults_as_well() {
    // §4.3: unbound in *every* mode, so that one profile is safe on a desk whose
    // mode nobody has checked. Layer 2 would drop its presses anyway, which is
    // exactly why this is worth asserting: two independent guards, and the test
    // says which one it is checking.
    let table = Bindings::defaults();
    assert_eq!(
        table.action(BoundControl::Global(GlobalButton::SmpteBeats)),
        None
    );
    assert!(X_TOUCH.is_reserved(GlobalButton::SmpteBeats));
    // Name/Value is not reserved — it merely has no lamp — and the defaults
    // still leave it alone, because a control with no feedback is a poor place
    // for an operating function (§2.7).
    assert_eq!(
        table.action(BoundControl::Global(GlobalButton::NameValue)),
        None
    );
    assert!(!X_TOUCH.is_reserved(GlobalButton::NameValue));
}

// ------------------------------------------------- the file that ships with it

#[test]
fn the_shipped_profile_is_the_built_in_default_table() {
    // Two things that must not drift: the file a user edits and the table a
    // daemon falls back to when that file is broken. If they differed, a
    // malformed profile would silently change what the desk does — which is the
    // opposite of what falling back is for.
    let table = Bindings::parse(SHIPPED, &X_TOUCH).expect("the profile this repository ships");
    assert_eq!(table, Bindings::defaults());
}

#[test]
fn the_shipped_profile_names_the_device_and_the_format_version() {
    let wrong_device = SHIPPED.replace("behringer-x-touch", "some-other-desk");
    assert!(matches!(
        Bindings::parse(&wrong_device, &X_TOUCH),
        Err(ProfileError::WrongDevice { .. })
    ));
    let wrong_version = SHIPPED.replace("\"profileVersion\": 1", "\"profileVersion\": 2");
    assert!(matches!(
        Bindings::parse(&wrong_version, &X_TOUCH),
        Err(ProfileError::UnsupportedVersion { found: 2, .. })
    ));
}

// ------------------------------------------------------ the fallback guarantee

#[test]
fn no_profile_whatever_it_says_can_stop_a_desk_starting() {
    // The exit criterion, asserted as a property rather than on one example:
    // every one of these is a way a profile can be wrong, and every one produces
    // a usable table. `load` has no error path at all, so this is a check that
    // the *table* is the default one and that the reason is reported.
    let broken = [
        "",
        "{",
        "[]",
        "null",
        "{\"profileVersion\":1}",
        r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":"none"}"#,
        r#"{"profileVersion":0,"device":"behringer-x-touch","bindings":[]}"#,
        r#"{"profileVersion":1,"device":"x-touch-extender","bindings":[]}"#,
        r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[{"control":"Strip[9].Fader","action":null}]}"#,
        r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[{"control":"Global.F1","action":{"t":"Nonesuch"}}]}"#,
        r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[{"control":"Global.F1","action":{"t":"OpenWindow","window":"Wibble"}}]}"#,
        r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[{"control":"Global.SmpteBeats","action":{"t":"Oops"}}]}"#,
    ];
    for text in broken {
        let (table, error) = Bindings::load(text, &X_TOUCH);
        assert_eq!(table, Bindings::defaults(), "{text}");
        let error = error.unwrap_or_else(|| panic!("{text} should have been refused"));
        assert!(!error.to_string().is_empty(), "{text}");
        // And the desk still works: the fallback table is a working table, not
        // an empty one.
        assert_eq!(
            table.command(press(GlobalButton::Record), &context()),
            Some(Command::ClearProgrammer),
            "{text}"
        );
    }
}

#[test]
fn an_empty_binding_list_is_a_surface_that_does_nothing_rather_than_a_default_one() {
    // The difference between *this profile binds nothing* and *this profile is
    // broken*. A fallback that could not tell them apart would give a desk the
    // defaults back whenever somebody deliberately cleared it.
    let text = r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[]}"#;
    let (table, error) = Bindings::load(text, &X_TOUCH);
    assert_eq!(error, None);
    assert_eq!(table.bound(), 0);
    assert_eq!(table.command(press(GlobalButton::Record), &context()), None);
}

#[test]
fn a_profile_that_binds_the_reserved_button_is_refused_by_name() {
    // Belt and braces: layer 2 drops SMPTE/Beats presses and counts them, so a
    // binding on it could never fire. The loader refuses it anyway, because a
    // profile that names it was written by somebody who believes they have bound
    // it — and the message is what tells them why they have not.
    let text = r#"{"profileVersion":1,"device":"behringer-x-touch",
        "bindings":[{"control":"Global.SmpteBeats","action":{"t":"SaveShow"}}]}"#;
    let error = Bindings::parse(text, &X_TOUCH).expect_err("a bound reservation");
    assert_eq!(
        error,
        ProfileError::ReservedControl(GlobalButton::SmpteBeats)
    );
    let message = error.to_string();
    assert!(message.contains("Global.SmpteBeats"), "{message}");
    assert!(message.contains("Xctl+MC"), "{message}");
}

#[test]
fn a_user_written_profile_takes_effect_where_it_differs_from_the_defaults() {
    // The other half of "user-editable": a valid profile is used, not merged
    // with the defaults. Here the transport section is spent on live-show work
    // the way §4.3 asks for, and Play stops being Go+.
    let text = r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[
        {"control":"Global.Play","action":{"t":"SelectView","view":9}},
        {"control":"Global.Record","action":{"t":"OpenWindow","window":"Viewer3D"}}
    ]}"#;
    let table = Bindings::parse(text, &X_TOUCH).expect("a profile of one's own");
    let context = context();
    assert_eq!(
        table.command(press(GlobalButton::Play), &context),
        Some(Command::SelectView {
            view_id: ViewId::new(9)
        })
    );
    assert_eq!(
        table.command(press(GlobalButton::Record), &context),
        Some(Command::OpenWindow {
            window: WindowType::Viewer3D,
            params: None,
        })
    );
    assert_eq!(
        table.command(press(GlobalButton::Stop), &context),
        None,
        "what the profile does not name is not bound"
    );
}

#[test]
fn the_step_and_target_vocabularies_read_the_way_the_document_spells_them() {
    // The two enums a profile's author types by hand, checked against the
    // spellings the shipped file uses.
    let text = r#"{"profileVersion":1,"device":"behringer-x-touch","bindings":[
        {"control":"Global.F5","action":{"t":"StepView","direction":"Prev"}},
        {"control":"Global.F6","action":{"t":"ExecutorMaster","target":"Selected"}}
    ]}"#;
    let table = Bindings::parse(text, &X_TOUCH).expect("the documented spellings");
    assert_eq!(
        table.action(BoundControl::Global(GlobalButton::F5)),
        Some(SurfaceAction::StepView {
            direction: Step::Prev
        })
    );
    assert_eq!(
        table.action(BoundControl::Global(GlobalButton::F6)),
        Some(SurfaceAction::ExecutorMaster {
            target: ExecutorTarget::Selected
        })
    );
}
