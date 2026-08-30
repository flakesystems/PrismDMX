//! Shared scenery for the integration targets.
//!
//! Each target compiles its own copy of this module and uses a subset of it,
//! which is what the allow below is for: a helper that only
//! `delta_round_trip.rs` needs is dead code in `show_to_engine.rs` and would
//! otherwise fail the `-D warnings` build.
#![allow(dead_code)]

use prism_core::{SessionState, Show};
use prism_domain::{
    AttributeDef, AttributeType, Command, Cue, CuePart, CueTrigger, Executor,
    ExecutorEncoderFunction, ExecutorFaderFunction, ExecutorId, FeatureGroup, Fixture, FixtureId,
    FixtureType, Group, GroupId, ObjectRef, OutputChange, OutputId, OutputInstance, OutputKind,
    OverwriteMode, ParamDirection, PlaybackTarget, Preset, PresetId, PresetValue, SelectionMode,
    Sequence, SequenceId, SequenceStoreMode, StoreMode, UniverseId, Vec3, ViewId, WindowInstanceId,
    WindowType,
};

/// An 8-bit attribute at a given offset, with everything else neutral.
pub fn attribute(attribute: AttributeType, coarse_offset: u16, home: u16) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value: home,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

/// A four-channel RGBW PAR.
pub fn par_type() -> FixtureType {
    FixtureType {
        id: "generic.rgbw.par".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "RGBW PAR".to_owned(),
        mode: "4ch".to_owned(),
        footprint: 4,
        attributes: vec![
            attribute(AttributeType::Red, 0, 0),
            attribute(AttributeType::Green, 1, 0),
            attribute(AttributeType::Blue, 2, 0),
            attribute(AttributeType::White, 3, 0),
        ],
    }
}

/// A one-channel dimmer whose home value is given, so two of them at one
/// address can be told apart on the wire.
pub fn dimmer_type(id: &str, home: u16) -> FixtureType {
    FixtureType {
        id: id.to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![attribute(AttributeType::Dimmer, 0, home)],
    }
}

/// A patched fixture at home geometry.
pub fn fixture(id: u32, type_id: &str, universe: u32, address: u16) -> Fixture {
    Fixture {
        software_dimmer: true,
        id: FixtureId::new(id),
        name: format!("Fixture {id}"),
        type_id: type_id.to_owned(),
        universe: UniverseId::new(universe),
        address,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        invert_pan: false,
        invert_tilt: false,
    }
}

/// A cue with one part and no preset link.
pub fn cue(number: &str, fixture: u32, attribute: AttributeType, value: u16) -> Cue {
    Cue {
        number: number.to_owned(),
        name: format!("Cue {number}"),
        fade_in: 3.0,
        fade_out: 3.0,
        delay: 0.0,
        trigger: CueTrigger::Go,
        trigger_time: None,
        parts: vec![CuePart {
            fixture: FixtureId::new(fixture),
            attribute,
            value,
            preset_ref: None,
            tracking: prism_domain::CueTracking::Track,
        }],
    }
}

/// A sequence carrying the cues given.
pub fn sequence(id: u32, cues: Vec<Cue>) -> Sequence {
    Sequence {
        id: SequenceId::new(id),
        name: format!("Sequence {id}"),
        color: None,
        cues,
        looping: false,
        master_level: u16::MAX,
        speed: prism_domain::SPEED_UNITY,
        is_active: false,
        current_cue_index: None,
    }
}

/// A group of the fixtures given.
pub fn group(id: u32, fixtures: &[u32]) -> Group {
    Group {
        id: GroupId::new(id),
        name: format!("Group {id}"),
        fixtures: fixtures.iter().copied().map(FixtureId::new).collect(),
    }
}

/// A preset holding one value.
pub fn preset(id: u32, fixture: u32, attribute: AttributeType, value: u16) -> Preset {
    Preset {
        id: PresetId::new(id),
        pool: attribute.feature_group().into(),
        name: format!("Preset {id}"),
        color: None,
        values: vec![PresetValue {
            fixture: FixtureId::new(fixture),
            attribute,
            value,
        }],
    }
}

/// An executor, optionally playing a sequence.
pub fn executor(id: u32, sequence_id: Option<u32>) -> Executor {
    Executor {
        id: ExecutorId::new(id),
        sequence_id: sequence_id.map(SequenceId::new),
        fader_function: ExecutorFaderFunction::Master,
        button_functions: Vec::new(),
        encoder_function: ExecutorEncoderFunction::Empty,
    }
}

/// The `PatchFixture` command for one PAR.
pub fn patch_command(id: u32, universe: u32, address: u16) -> Command {
    Command::PatchFixture {
        software_dimmer: true,
        id: FixtureId::new(id),
        name: format!("Fixture {id}"),
        type_id: "generic.rgbw.par".to_owned(),
        universe: UniverseId::new(universe),
        address,
    }
}

/// Every command of the show group (`docs/IPC_PROTOCOL.md` §5), each in a form
/// that [`populated_show`] can apply.
pub fn show_commands() -> Vec<Command> {
    vec![
        Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        },
        Command::SetAttribute {
            attribute: AttributeType::Red,
            value: 65535,
            relative: false,
        },
        Command::ApplyPreset {
            preset_id: PresetId::new(4),
        },
        // S40's `Group 3`: the show expands it, so the show is what refuses a
        // group that is not there. Group 1 exists in a populated show.
        Command::SelectGroup {
            group_id: GroupId::new(1),
            mode: SelectionMode::Set,
        },
        Command::ClearProgrammer,
        Command::StoreCue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "3".to_owned(),
            mode: StoreMode::Merge,
        },
        Command::ExecutorGo {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            direction: prism_domain::GoDirection::Next,
        },
        Command::ExecutorOff {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
        },
        // S34's. Executor 0 plays sequence 1, so this resolves to something
        // rather than being refused. The `Function` form, because
        // `common::executor` assigns its buttons nothing at all and a `Slot`
        // would be a key with nothing on it — which is a legitimate answer and a
        // poor thing for the *has a home in an applier* list to assert over.
        Command::ExecutorButton {
            executor_id: ExecutorId::new(0),
            button: prism_domain::ExecutorButtonRef::Function {
                function: prism_domain::ExecutorButtonFunction::On,
            },
            pressed: true,
        },
        Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 32768,
        },
        patch_command(9, 3, 1),
        // The three S27 added, each in a form a populated show accepts: 1 is
        // patched, 77 is free, and the desk's library carries an RGB PAR this
        // show has not embedded.
        Command::UnpatchFixture {
            id: FixtureId::new(1),
        },
        Command::RenumberFixture {
            id: FixtureId::new(1),
            to: FixtureId::new(77),
        },
        Command::EmbedFixtureType {
            type_id: "generic.rgb.par".to_owned(),
        },
        // The five S28 added. Each is in a form the populated show accepts:
        // preset 4 exists and holds a colour value, sequence 1 exists and has a
        // cue 1, sequence 9 is free, and executor 2 is an empty slot.
        Command::StorePreset {
            preset_id: PresetId::new(4),
            pool: Some(prism_domain::PresetPool::Color),
            name: "Deep blue".to_owned(),
            color: None,
            mode: StoreMode::Merge,
        },
        // The three S39 added, each in a form the populated show accepts:
        // sequence 1 exists and has cues 1 and 2 in it, so an Append has a list
        // to append to and an `EditCue` has a cue to load. `Update` is last of
        // the three on purpose — it is the one whose target is the *session*,
        // and the `EditCue` before it is what puts one there. Both orderings
        // matter to `every_show_command_is_decided_rather_than_ignored`, which
        // applies each of these to a fresh show: an `Update` on its own is a
        // refusal, so the list is applied in order where it is applied at all.
        Command::StoreSequence {
            sequence_id: SequenceId::new(1),
            name: String::new(),
            mode: SequenceStoreMode::Append,
        },
        Command::EditCue {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
        },
        Command::Update,
        Command::StoreSequence {
            sequence_id: SequenceId::new(9),
            name: "Act 2".to_owned(),
            mode: SequenceStoreMode::Append,
        },
        Command::Label {
            target: ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
            },
            name: "Blackout".to_owned(),
        },
        // `Label`'s mirror, on the one pool that has a colour. Sequence 1 exists
        // and has no colour, so this changes something — which is what every
        // command in this list has to do.
        Command::Color {
            target: ObjectRef::Sequence {
                sequence_id: SequenceId::new(1),
            },
            color: Some(prism_domain::RgbColor {
                r: 255,
                g: 140,
                b: 0,
            }),
        },
        Command::Delete {
            target: ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "2".to_owned(),
            },
        },
        Command::AssignExecutor {
            executor_id: ExecutorId::new(2),
            sequence_id: Some(SequenceId::new(1)),
        },
        // S45's. Executor 0 exists, and `common::executor` gives it a `Master`
        // fader and no buttons — so a crossfade on it is a change rather than
        // the function it already has, which is what this list asserts over.
        Command::ConfigureExecutor {
            executor_id: ExecutorId::new(0),
            change: prism_domain::ExecutorChange::Fader {
                function: ExecutorFaderFunction::XFade,
            },
        },
        // S40's, each in a form the populated show accepts. `StoreGroup` is the
        // programmer's half like every other store, so the show accepts it and
        // names who finishes; the rest reach a pool that is there — preset 4
        // exists, executor 0 plays sequence 1, and that sequence has a cue 1.
        Command::StoreGroup {
            group_id: GroupId::new(7),
            name: "Front wash".to_owned(),
            mode: OverwriteMode::Merge,
        },
        Command::SetCueProperty {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
            property: prism_domain::CueProperty::FadeIn { seconds: 7.5 },
        },
        // S48. The show group, beside `SetCueProperty`: what a cue asserts is
        // show content, and a `Block` writes values the daemon folded out of the
        // cues above it.
        Command::SetCueTracking {
            sequence_id: Some(SequenceId::new(1)),
            cue_number: "1".to_owned(),
            tracking: prism_domain::CueTrackingMode::CueOnly,
        },
        Command::Copy {
            from: ObjectRef::Preset {
                preset_id: PresetId::new(4),
            },
            to: ObjectRef::Preset {
                preset_id: PresetId::new(6),
            },
            mode: OverwriteMode::Merge,
        },
        Command::Move {
            from: ObjectRef::Preset {
                preset_id: PresetId::new(4),
            },
            to: ObjectRef::Preset {
                preset_id: PresetId::new(5),
            },
            mode: OverwriteMode::Merge,
        },
        Command::ExecutorOn {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
        },
        Command::Goto {
            target: PlaybackTarget::of_executor(ExecutorId::new(0)),
            cue_number: "1".to_owned(),
        },
        Command::Oops,
        Command::Redo,
        Command::SaveShow,
        // S37's four beside it, each with a path the show applier accepts.
        // What it checks is the extension and nothing else — whether the file
        // is there needs a disk, which is `prismd`'s (`prism_core::file`).
        Command::SaveShowAs {
            path: "aula.prism".to_owned(),
        },
        Command::OpenShow {
            path: "aula.prism".to_owned(),
        },
        Command::NewShow {
            path: "next-term.prism".to_owned(),
        },
        Command::ExportShow {
            path: "aula.json".to_owned(),
        },
        Command::ImportShow {
            path: "aula.json".to_owned(),
        },
    ]
}

/// Every command the **machine** applier accepts, each in a form a configured
/// rig takes — S33, and the surface port S36 put beside it.
///
/// The third list, and the third applier. `Delete`, `Copy`, `Move` and `Label`
/// are in two lists because their *target* decides which applier owns them;
/// these five are in one, because neither a rig nor the desk in the rack is
/// ever show content or session content. `prism_core::outputs` has the argument
/// in full, and `desk::tests::outputs_are_not_show_content` and
/// `the_surface_port_is_not_show_content` are the asserted halves.
///
/// They are applied **in order** where they are applied at all: the `AddOutput`
/// is what the three after it name.
pub fn machine_commands() -> Vec<Command> {
    vec![
        Command::AddOutput {
            output: OutputInstance::new(
                OutputId::new(1),
                "Hall dimmers",
                OutputKind::OpenDmx { serial: None },
                [UniverseId::new(1)],
            ),
        },
        Command::ConfigureOutput {
            id: OutputId::new(1),
            change: OutputChange::Name {
                name: "Hall".to_owned(),
            },
        },
        Command::SetOutputEnabled {
            id: OutputId::new(1),
            enabled: false,
        },
        Command::RemoveOutput {
            id: OutputId::new(1),
        },
        // S36's. Last but one because it has nothing to do with the four
        // before it: it is the *other* device this machine owns.
        Command::SetSurfacePort {
            port: Some("X-Touch".to_owned()),
        },
        // S37's, and the last thing about this building that was only ever a
        // command-line flag. One field at a time, like `ConfigureOutput`.
        Command::ConfigureMachine {
            change: prism_domain::MachineChange::LogLevel {
                level: prism_domain::LogLevel::Warn,
            },
        },
    ]
}

/// Every command the session applier accepts, each in a form that changes
/// something in a [`populated_session`].
///
/// `ARCHITECTURE_SPEC.md` §4.4's twelve — the eleven plus S39's `SelectSequence`
/// — plus `PlaceWindow` (S25), which is a session command without being on that
/// list because §4.4 says what a *console* issues and an X-Touch never drags a
/// window. They are here rather than in a second list because every property in
/// `session_commands.rs` is true of all of them — being refused by the show
/// applier, emitting a `SessionPatch`, asking nothing of anyone else — and a
/// list that held only some of them would silently stop covering the rest.
///
/// **S40's four generic verbs appear in both lists**, and that is the shape of
/// the decision rather than a duplicate: `Delete`, `Copy`, `Move` and `Label`
/// are session commands when they name a **view** and show commands when they
/// name anything else, because §4.1 puts the view library in the session. See
/// `prism_domain::ObjectRef`.
pub fn session_commands() -> Vec<Command> {
    vec![
        Command::SelectView {
            view_id: ViewId::new(2),
        },
        Command::StoreView {
            view_id: ViewId::new(3),
            name: "Playback".to_owned(),
        },
        Command::NewView {
            view_id: ViewId::new(5),
            name: "Blank".to_owned(),
        },
        Command::SetWindowPicker { open: true },
        Command::Label {
            target: ObjectRef::View {
                view_id: ViewId::new(2),
            },
            name: "Busking".to_owned(),
        },
        Command::Delete {
            target: ObjectRef::View {
                view_id: ViewId::new(2),
            },
        },
        Command::Move {
            from: ObjectRef::View {
                view_id: ViewId::new(2),
            },
            to: ObjectRef::View {
                view_id: ViewId::new(1),
            },
            mode: OverwriteMode::Merge,
        },
        Command::Copy {
            from: ObjectRef::View {
                view_id: ViewId::new(1),
            },
            to: ObjectRef::View {
                view_id: ViewId::new(4),
            },
            mode: OverwriteMode::Merge,
        },
        Command::PlaceWindow {
            instance_id: WindowInstanceId::new(1),
            x: 12.0,
            y: 34.0,
            w: 320.0,
            h: 240.0,
        },
        Command::OpenWindow {
            window: WindowType::SequenceSheet,
            params: None,
        },
        Command::CloseWindow {
            instance_id: WindowInstanceId::new(1),
        },
        Command::FocusWindow {
            instance_id: WindowInstanceId::new(1),
        },
        Command::SetExecutorPage { page: 1 },
        Command::SelectExecutor {
            executor_id: ExecutorId::new(3),
        },
        // S39's, and the reason it is a session command at all: a store with no
        // cue list named has to go somewhere, and the desk is what says where.
        Command::SelectSequence {
            sequence_id: SequenceId::new(1),
        },
        Command::SetEncoderBank {
            group: FeatureGroup::Color,
        },
        Command::SetProgrammerPage { page: 2 },
        Command::SelectProgrammerParam {
            direction: ParamDirection::Next,
        },
        Command::CommandLineInput {
            text: "1 thru 4 at full".to_owned(),
            run: false,
        },
    ]
}

/// A show with one of everything, saved — so a test starts from a clean dirty
/// flag and every kind of reference resolves.
pub fn populated_show() -> Show {
    let mut show = Show::new();
    show.embed_fixture_type(par_type()).unwrap();
    show.embed_fixture_type(dimmer_type("generic.dimmer", 0))
        .unwrap();
    for id in 1..=3 {
        show.patch_fixture(fixture(id, "generic.rgbw.par", 1, (id as u16 - 1) * 4 + 1))
            .unwrap();
    }
    show.patch_fixture(fixture(4, "generic.dimmer", 2, 1))
        .unwrap();
    show.store_group(group(1, &[1, 2, 3])).unwrap();
    show.store_preset(preset(4, 1, AttributeType::Red, 65535))
        .unwrap();
    show.store_sequence(sequence(
        1,
        vec![
            cue("1", 1, AttributeType::Red, 65535),
            cue("2", 2, AttributeType::Green, 32768),
        ],
    ))
    .unwrap();
    show.store_executor(executor(0, Some(1))).unwrap();
    show.store_executor(executor(1, None)).unwrap();
    show.mark_saved();
    show
}

/// A session with a canvas, a stored view that is *not* the canvas, and the
/// dirty flag cleared — so every §4.4 command has something to change and a
/// test starts from a clean Save LED.
///
/// Windows 1 and 2 are the stored view 2; window 3 was opened afterwards, which
/// is what makes `SelectView(2)` a visible change rather than a no-op.
pub fn populated_session() -> SessionState {
    let mut session = SessionState::new();
    session.open_window(WindowType::FixtureSheet, None).unwrap();
    session.open_window(WindowType::Patch, None).unwrap();
    session.store_view(ViewId::new(2), "Programming").unwrap();
    session.open_window(WindowType::Groups, None).unwrap();
    session.mark_saved();
    session
}
