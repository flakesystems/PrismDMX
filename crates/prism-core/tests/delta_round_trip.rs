//! S11 and S12 exit criterion: applying the deltas to a copy reproduces the
//! source state exactly.
//!
//! The copy is a [`ShowMirror`] or a [`SessionMirror`] — the same appliers a
//! Rust client would use — started from a snapshot of an empty show or a fresh
//! session and never given anything but the operations the model emitted. If
//! the two agree at the end of a hundred random edits, then
//! `docs/IPC_PROTOCOL.md` §6's promise ("a client that has applied every delta
//! since its snapshot holds state identical to the daemon's") holds — for the
//! show half and, since S12, for the session half, which is the half **D11**
//! depends on: a view switched from the X-Touch reaches every screen as a
//! `SessionPatch` and nothing else.
//!
//! Both paths are checked, because they are different code: the direct edits
//! S13, S15 and S27 will call, and the appliers, which wrap them in deltas.

mod common;

use common::{
    cue, dimmer_type, executor, fixture, group, par_type, patch_command, populated_session,
    populated_show, preset, sequence,
};
use prism_core::{
    SessionMirror, SessionState, Show, ShowFile, ShowMirror, session_patch_ops, show_patch_ops,
};
use prism_domain::{
    AttributeType, Command, Cue, CuePart, Delta, Executor, ExecutorId, Fixture, FixtureId,
    FixtureType, Group, GroupId, JsonPatchOp, ParamDirection, PlaybackId, Preset, PresetId,
    PresetValue, Sequence, SequenceId, StoreMode, UniverseId, ViewId, WindowInstanceId, WindowType,
};
use proptest::prelude::*;

/// A mirror of a show, and the show it mirrors.
struct Pair {
    show: Show,
    mirror: ShowMirror,
}

impl Pair {
    fn new() -> Self {
        let show = Show::new();
        let mirror = ShowMirror::new(show.to_json().unwrap());
        Self { show, mirror }
    }

    /// Feeds the mirror whatever an edit produced, and asserts the two still
    /// describe the same show.
    fn feed(&mut self, ops: &[JsonPatchOp]) {
        self.mirror.apply_all(ops).unwrap();
        self.agree();
    }

    fn agree(&self) {
        assert_eq!(
            self.mirror.value(),
            &self.show.to_json().unwrap(),
            "the mirror and the show have diverged"
        );
    }
}

#[test]
fn a_scripted_show_is_reproduced_operation_by_operation() {
    let mut pair = Pair::new();
    pair.agree();

    let ops = pair.show.embed_fixture_type(par_type()).unwrap();
    pair.feed(&ops);
    let ops = pair
        .show
        .embed_fixture_type(dimmer_type("generic.dimmer", 0))
        .unwrap();
    pair.feed(&ops);

    for id in 1..=4 {
        let ops = pair
            .show
            .patch_fixture(fixture(id, "generic.rgbw.par", 1, (id as u16 - 1) * 4 + 1))
            .unwrap();
        pair.feed(&ops);
    }

    let ops = pair.show.store_group(group(1, &[1, 2, 3])).unwrap();
    pair.feed(&ops);
    let ops = pair
        .show
        .store_preset(preset(4, 1, AttributeType::Red, 65535))
        .unwrap();
    pair.feed(&ops);
    let ops = pair
        .show
        .store_sequence(sequence(1, vec![cue("2", 1, AttributeType::Red, 65535)]))
        .unwrap();
    pair.feed(&ops);
    // Inserting between two cues renumbers the list, which is why the delta
    // replaces it.
    let ops = pair
        .show
        .store_cue(SequenceId::new(1), cue("1.5", 2, AttributeType::Green, 1))
        .unwrap();
    pair.feed(&ops);
    let ops = pair.show.store_executor(executor(0, Some(1))).unwrap();
    pair.feed(&ops);
    let ops = pair
        .show
        .set_executor_master(ExecutorId::new(0), 12345)
        .unwrap();
    pair.feed(&ops);

    // The engine reporting back travels as its own delta, and the mirror has to
    // follow that too or it drifts on exactly those two fields.
    assert!(
        pair.show
            .record_playback_state(ExecutorId::new(0).into(), true, Some(1))
            .unwrap()
    );
    pair.mirror
        .apply_delta(&Delta::PlaybackState {
            playback: PlaybackId::of_executor(ExecutorId::new(0)),
            is_active: true,
            cue_index: Some(1),
        })
        .unwrap();
    pair.agree();

    // And the removals.
    let ops = pair.show.unpatch_fixture(FixtureId::new(4)).unwrap();
    pair.feed(&ops);
    let ops = pair.show.remove_group(GroupId::new(1)).unwrap();
    pair.feed(&ops);
    let ops = pair.show.remove_preset(PresetId::new(4)).unwrap();
    pair.feed(&ops);
    let ops = pair.show.remove_sequence(SequenceId::new(1)).unwrap();
    pair.feed(&ops);
    let ops = pair.show.remove_fixture_type("generic.dimmer").unwrap();
    pair.feed(&ops);

    // A show that reached this point through deltas alone is the show itself.
    let rebuilt: Show =
        serde_json::from_value(serde_json::to_value(pair.mirror.value()).unwrap()).unwrap();
    assert_eq!(
        rmp_serde::to_vec_named(&rebuilt).unwrap(),
        rmp_serde::to_vec_named(&pair.show).unwrap()
    );
}

#[test]
fn the_command_path_produces_deltas_that_reproduce_the_show() {
    let mut pair = Pair::new();
    let ops = pair.show.embed_fixture_type(par_type()).unwrap();
    pair.feed(&ops);

    for command in [
        patch_command(1, 1, 1),
        patch_command(2, 1, 5),
        // An overlap: applied, reported, and mirrored like any other patch.
        patch_command(3, 1, 3),
        // A repatch of one already there.
        patch_command(1, 2, 100),
    ] {
        let applied = pair.show.apply(&command).unwrap();
        pair.feed(&show_patch_ops(&applied.deltas));
    }

    let ops = pair.show.store_executor(executor(0, None)).unwrap();
    pair.feed(&ops);
    let applied = pair
        .show
        .apply(&Command::SetExecutorMaster {
            executor_id: ExecutorId::new(0),
            level: 700,
        })
        .unwrap();
    pair.feed(&show_patch_ops(&applied.deltas));
}

/// One edit a property test can make.
#[derive(Debug, Clone)]
enum Edit {
    EmbedType(FixtureType),
    RemoveType(String),
    Patch(Fixture),
    Unpatch(FixtureId),
    StoreGroup(Group),
    RemoveGroup(GroupId),
    StorePreset(Preset),
    RemovePreset(PresetId),
    StoreSequence(Sequence),
    RemoveSequence(SequenceId),
    StoreCue(SequenceId, Cue),
    StoreExecutor(Executor),
    SetMaster(ExecutorId, u16),
    ExecutorState(ExecutorId, bool, Option<u32>),
}

/// Three profile keys, so an edit has a real chance of naming one that exists.
const TYPE_IDS: [&str; 3] = ["type/0", "type~1", "type 2"];

/// An arbitrary profile that describes an addressable fixture.
///
/// Built from `prism_domain::arb`'s own `FixtureType` rather than from a
/// hand-written generator, then made self-consistent: attributes deduplicated,
/// offsets packed from zero, footprint sized to fit them. Everything else —
/// the names, the physical ranges, the merge modes — stays arbitrary.
///
/// The keys deliberately contain `/` and `~`, the two characters a JSON Pointer
/// escapes, because the profile key is the only part of the show's shape that
/// is operator text.
fn arb_type() -> impl Strategy<Value = FixtureType> {
    (any::<FixtureType>(), 0usize..TYPE_IDS.len()).prop_map(|(mut fixture_type, index)| {
        fixture_type.id = TYPE_IDS[index].to_owned();
        let mut seen = Vec::new();
        fixture_type.attributes.retain(|definition| {
            let fresh = !seen.contains(&definition.attribute);
            seen.push(definition.attribute);
            fresh
        });
        for (offset, definition) in fixture_type.attributes.iter_mut().enumerate() {
            definition.coarse_offset = u16::try_from(offset).unwrap();
            definition.fine_offset = None;
        }
        fixture_type.footprint = u16::try_from(fixture_type.attributes.len().max(1)).unwrap();
        fixture_type
    })
}

/// An arbitrary fixture in a small address space, so overlaps happen often.
fn arb_fixture() -> impl Strategy<Value = Fixture> {
    (
        any::<Fixture>(),
        1u32..5,
        1u32..3,
        1u16..20,
        0usize..TYPE_IDS.len(),
    )
        .prop_map(|(mut fixture, id, universe, address, index)| {
            fixture.id = FixtureId::new(id);
            fixture.universe = UniverseId::new(universe);
            fixture.address = address;
            fixture.type_id = TYPE_IDS[index].to_owned();
            fixture
        })
}

fn arb_part() -> impl Strategy<Value = CuePart> {
    (any::<CuePart>(), 1u32..5, proptest::option::of(1u32..3)).prop_map(
        |(mut part, fixture, preset)| {
            part.fixture = FixtureId::new(fixture);
            part.preset_ref = preset.map(PresetId::new);
            part
        },
    )
}

fn arb_cue() -> impl Strategy<Value = Cue> {
    (
        any::<Cue>(),
        proptest::sample::select(vec!["1", "1.5", "2", "10"]),
        proptest::collection::vec(arb_part(), 0..3),
    )
        .prop_map(|(mut cue, number, parts)| {
            cue.number = number.to_owned();
            cue.parts = parts;
            cue
        })
}

fn arb_edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        arb_type().prop_map(Edit::EmbedType),
        proptest::sample::select(TYPE_IDS.to_vec()).prop_map(|id| Edit::RemoveType(id.to_owned())),
        arb_fixture().prop_map(Edit::Patch),
        (1u32..5).prop_map(|id| Edit::Unpatch(FixtureId::new(id))),
        (
            any::<Group>(),
            1u32..3,
            proptest::collection::vec(1u32..5, 0..4)
        )
            .prop_map(|(mut group, id, members)| {
                group.id = GroupId::new(id);
                group.fixtures = members.into_iter().map(FixtureId::new).collect();
                Edit::StoreGroup(group)
            }),
        (1u32..3).prop_map(|id| Edit::RemoveGroup(GroupId::new(id))),
        (
            any::<Preset>(),
            1u32..3,
            proptest::collection::vec((any::<PresetValue>(), 1u32..5), 0..3)
        )
            .prop_map(|(mut preset, id, values)| {
                preset.id = PresetId::new(id);
                preset.values = values
                    .into_iter()
                    .map(|(mut value, fixture)| {
                        value.fixture = FixtureId::new(fixture);
                        value
                    })
                    .collect();
                Edit::StorePreset(preset)
            }),
        (1u32..3).prop_map(|id| Edit::RemovePreset(PresetId::new(id))),
        (
            any::<Sequence>(),
            1u32..3,
            proptest::collection::vec(arb_cue(), 0..3)
        )
            .prop_map(|(mut sequence, id, cues)| {
                sequence.id = SequenceId::new(id);
                sequence.cues = cues;
                Edit::StoreSequence(sequence)
            }),
        (1u32..3).prop_map(|id| Edit::RemoveSequence(SequenceId::new(id))),
        (1u32..3, arb_cue()).prop_map(|(id, cue)| Edit::StoreCue(SequenceId::new(id), cue)),
        (any::<Executor>(), 0u32..3, proptest::option::of(1u32..3)).prop_map(
            |(mut executor, id, sequence)| {
                executor.id = ExecutorId::new(id);
                executor.sequence_id = sequence.map(SequenceId::new);
                Edit::StoreExecutor(executor)
            }
        ),
        (0u32..3, any::<u16>()).prop_map(|(id, level)| Edit::SetMaster(ExecutorId::new(id), level)),
        (0u32..3, any::<bool>(), proptest::option::of(0u32..4)).prop_map(|(id, active, index)| {
            Edit::ExecutorState(ExecutorId::new(id), active, index)
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// The exit criterion, over arbitrary edits: whatever the show accepted, the
    /// deltas it emitted put a mirror in exactly the same place.
    #[test]
    fn deltas_reproduce_the_show_they_came_from(edits in proptest::collection::vec(arb_edit(), 1..24)) {
        let mut show = Show::new();
        let mut mirror = ShowMirror::new(show.to_json().unwrap());
        let mut applied = 0usize;

        // Two edits that cannot be refused, so the property is never checked on
        // a show that never changed.
        for ops in [
            show.embed_fixture_type(par_type()).unwrap(),
            show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 100)).unwrap(),
        ] {
            applied += 1;
            mirror.apply_all(&ops).unwrap();
        }

        for edit in edits {
            let outcome = match edit {
                Edit::EmbedType(fixture_type) => show.embed_fixture_type(fixture_type),
                Edit::RemoveType(id) => show.remove_fixture_type(&id),
                Edit::Patch(fixture) => show.patch_fixture(fixture),
                Edit::Unpatch(id) => show.unpatch_fixture(id),
                Edit::StoreGroup(group) => show.store_group(group),
                Edit::RemoveGroup(id) => show.remove_group(id),
                Edit::StorePreset(preset) => show.store_preset(preset),
                Edit::RemovePreset(id) => show.remove_preset(id),
                Edit::StoreSequence(sequence) => show.store_sequence(sequence),
                Edit::RemoveSequence(id) => show.remove_sequence(id),
                Edit::StoreCue(sequence, cue) => show.store_cue(sequence, cue),
                Edit::StoreExecutor(executor) => show.store_executor(executor),
                Edit::SetMaster(id, level) => show.set_executor_master(id, level),
                Edit::ExecutorState(id, active, index) => {
                    // Not a JSON Patch: this one has its own delta.
                    if show.record_playback_state(id.into(), active, index).is_ok() {
                        applied += 1;
                        mirror
                            .apply_delta(&Delta::PlaybackState {
                                playback: PlaybackId::of_executor(id),
                                                                is_active: active,
                                cue_index: index,
                            })
                            .unwrap();
                    }
                    continue;
                }
            };
            if let Ok(ops) = outcome {
                applied += 1;
                mirror.apply_all(&ops).unwrap();
            }
        }

        prop_assert_eq!(mirror.value(), &show.to_json().unwrap());
        // Never a test of an empty show that nothing happened to.
        prop_assert!(applied >= 2);
    }
}

/// A mirror of a session, and the session it mirrors.
struct SessionPair {
    session: SessionState,
    mirror: SessionMirror,
}

impl SessionPair {
    fn new() -> Self {
        let session = populated_session();
        let mirror = SessionMirror::new(session.to_json().unwrap());
        Self { session, mirror }
    }

    /// Applies a command, feeds the mirror the delta it produced, and asserts
    /// the two still describe the same session.
    fn apply(&mut self, command: &Command) {
        let applied = self
            .session
            .apply(command)
            .unwrap_or_else(|error| panic!("{command:?} was refused: {error}"));
        for delta in &applied.deltas {
            self.mirror.apply_delta(delta).unwrap();
        }
        assert!(
            !session_patch_ops(&applied.deltas).is_empty(),
            "{command:?} changed nothing"
        );
        self.agree();
    }

    fn agree(&self) {
        assert_eq!(
            self.mirror.value(),
            &self.session.to_json().unwrap(),
            "the mirror and the session have diverged"
        );
    }
}

#[test]
fn a_scripted_session_is_reproduced_command_by_command() {
    let mut pair = SessionPair::new();
    pair.agree();

    // Every one of §4.4's eleven, in an order in which each changes something.
    pair.apply(&Command::OpenWindow {
        window: WindowType::SequenceSheet,
        params: None,
    });
    pair.apply(&Command::OpenWindow {
        window: WindowType::PresetPool,
        params: Some(std::collections::BTreeMap::from([(
            // A key with the two characters a JSON Pointer escapes, because a
            // window parameter is the one piece of operator text in here.
            "pool/name~1".to_owned(),
            prism_domain::JsonValue::String("Colour".to_owned()),
        )])),
    });
    pair.apply(&Command::FocusWindow {
        instance_id: WindowInstanceId::new(1),
    });
    pair.apply(&Command::CloseWindow {
        instance_id: WindowInstanceId::new(2),
    });
    pair.apply(&Command::StoreView {
        view_id: ViewId::new(3),
        name: "Playback".to_owned(),
    });
    pair.apply(&Command::SelectView {
        view_id: ViewId::new(2),
    });
    pair.apply(&Command::SetExecutorPage { page: 4 });
    pair.apply(&Command::SelectExecutor {
        executor_id: ExecutorId::new(35),
    });
    pair.apply(&Command::SetEncoderBank {
        group: prism_domain::FeatureGroup::Position,
    });
    pair.apply(&Command::SetProgrammerPage { page: 2 });
    pair.apply(&Command::SelectProgrammerParam {
        direction: ParamDirection::Next,
    });
    pair.apply(&Command::CommandLineInput {
        text: "1 thru 4 at full".to_owned(),
    });

    // Storing over a view, which is a replace rather than an add.
    pair.apply(&Command::StoreView {
        view_id: ViewId::new(3),
        name: "Playback 2".to_owned(),
    });

    // A session that reached this point through deltas alone is the session
    // itself, right down to the bytes S15 will write.
    let rebuilt: SessionState =
        serde_json::from_value(serde_json::to_value(pair.mirror.value()).unwrap()).unwrap();
    assert_eq!(
        rmp_serde::to_vec_named(&rebuilt).unwrap(),
        rmp_serde::to_vec_named(&pair.session).unwrap()
    );
}

#[test]
fn the_two_mirrors_ignore_each_others_deltas() {
    // One connection carries both patches, so each mirror has to leave the
    // other's alone — a `SessionPatch` applied to the show document would fail
    // on `/session`, and a mirror that guessed would corrupt itself.
    let session = populated_session();
    let mut show_mirror = ShowMirror::new(Show::new().to_json().unwrap());
    let mut session_mirror = SessionMirror::new(session.to_json().unwrap());

    let mut show = Show::new();
    let show_ops = show.embed_fixture_type(par_type()).unwrap();
    let mut moved = session;
    let session_ops = moved.set_executor_page(3).unwrap();

    let deltas = [
        Delta::ShowPatch { ops: show_ops },
        Delta::SessionPatch { ops: session_ops },
        Delta::DirtyFlag {
            unsaved_changes: true,
        },
        Delta::Notice {
            level: prism_domain::NoticeLevel::Warn,
            message: "overlap".to_owned(),
        },
        Delta::ProgrammerChanged {
            state: prism_domain::ProgrammerState::default(),
        },
        Delta::OutputHealth {
            output_id: prism_domain::OutputId::new(0),
            health: prism_domain::OutputHealth::Ok,
        },
    ];
    for delta in &deltas {
        show_mirror.apply_delta(delta).unwrap();
        session_mirror.apply_delta(delta).unwrap();
    }
    assert_eq!(show_mirror.value(), &show.to_json().unwrap());
    assert_eq!(session_mirror.value(), &moved.to_json().unwrap());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// The exit criterion over arbitrary commands: whatever the session
    /// accepted, the deltas it emitted put a mirror in exactly the same place.
    ///
    /// Show commands are in the stream on purpose — they are what the daemon's
    /// router hands the wrong way round if it ever gets the predicate wrong,
    /// and they must produce no session delta at all.
    #[test]
    fn session_deltas_reproduce_the_session_they_came_from(
        commands in proptest::collection::vec(any::<Command>(), 1..24)
    ) {
        let mut session = populated_session();
        let mut mirror = SessionMirror::new(session.to_json().unwrap());
        let mut applied = 0usize;

        for command in commands {
            if let Ok(outcome) = session.apply(&command) {
                if !outcome.deltas.is_empty() {
                    applied += 1;
                }
                for delta in &outcome.deltas {
                    mirror.apply_delta(delta).unwrap();
                }
                prop_assert!(!command.is_session_command() || outcome.effects.is_empty());
            }
            prop_assert_eq!(mirror.value(), &session.to_json().unwrap());
        }

        // A run in which nothing was ever accepted would assert nothing, so the
        // property is closed with an edit that cannot be refused.
        for delta in &session.apply(&Command::CommandLineInput { text: "go".to_owned() }).unwrap().deltas {
            mirror.apply_delta(delta).unwrap();
        }
        applied += 1;
        prop_assert_eq!(mirror.value(), &session.to_json().unwrap());
        prop_assert!(applied >= 1);
    }
}

/// A client's copy of the programmer.
///
/// There is deliberately no `ProgrammerMirror` in the crate, because there is
/// nothing for one to do: `Delta::ProgrammerChanged` carries the state whole —
/// `docs/IPC_PROTOCOL.md` §6, "sent whole: it is small and sparse" — so
/// applying it *is* the assignment below, and a JSON Patch applier would be a
/// document nobody points into. What does have to be checked is the other half
/// of the promise, and it is the half a whole-state delta makes easy to get
/// wrong: a client told nothing must have missed nothing.
#[derive(Debug, Default, PartialEq)]
struct ProgrammerMirror {
    state: prism_domain::ProgrammerState,
}

impl ProgrammerMirror {
    fn apply_delta(&mut self, delta: &Delta) {
        if let Delta::ProgrammerChanged { state } = delta {
            self.state = state.clone();
        }
    }
}

/// A show file and the three mirrors a client keeps of it.
struct FilePair {
    file: ShowFile,
    show: ShowMirror,
    session: SessionMirror,
    programmer: ProgrammerMirror,
}

impl FilePair {
    fn new() -> Self {
        let file = ShowFile {
            show: populated_show(),
            session: populated_session(),
            ..ShowFile::new()
        };
        Self {
            show: ShowMirror::new(file.show.to_json().unwrap()),
            session: SessionMirror::new(file.session.to_json().unwrap()),
            programmer: ProgrammerMirror::default(),
            file,
        }
    }

    /// Applies a command and feeds every mirror every delta, exactly as a
    /// connection does — each mirror takes what is its own and ignores the rest.
    fn apply(&mut self, command: &Command) {
        let applied = self
            .file
            .apply(command)
            .unwrap_or_else(|error| panic!("{command:?} was refused: {error}"));
        for delta in &applied.deltas {
            self.show.apply_delta(delta).unwrap();
            self.session.apply_delta(delta).unwrap();
            self.programmer.apply_delta(delta);
        }
        self.agree();
    }

    fn agree(&self) {
        assert_eq!(self.show.value(), &self.file.show.to_json().unwrap());
        assert_eq!(self.session.value(), &self.file.session.to_json().unwrap());
        assert_eq!(
            rmp_serde::to_vec_named(&self.programmer.state).unwrap(),
            rmp_serde::to_vec_named(self.file.programmer.state()).unwrap(),
            "the mirror and the programmer have diverged"
        );
    }
}

#[test]
fn a_scripted_programmer_is_reproduced_command_by_command() {
    let mut pair = FilePair::new();
    pair.agree();

    pair.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(1), FixtureId::new(2)],
        mode: prism_domain::SelectionMode::Set,
    });
    pair.apply(&Command::SetAttribute {
        attribute: AttributeType::Red,
        value: 65535,
        relative: false,
    });
    pair.apply(&Command::SetAttribute {
        attribute: AttributeType::Green,
        value: -4096,
        relative: true,
    });
    pair.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(3)],
        mode: prism_domain::SelectionMode::Add,
    });
    pair.apply(&Command::ApplyPreset {
        preset_id: PresetId::new(4),
    });
    // A store writes the show, so this one command moves two documents at once.
    pair.apply(&Command::StoreCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "3".to_owned(),
        mode: StoreMode::Merge,
    });
    // And the three stages of the Clear, the last of which moves the session.
    for _ in 0..3 {
        pair.apply(&Command::ClearProgrammer);
    }

    assert!(pair.file.programmer.state().is_empty());
    assert_eq!(pair.programmer.state, *pair.file.programmer.state());
}

/// An undo changes state, so it has to say so — in deltas, like everything
/// else. A client that mirrored the command and not its Oops would hold a show
/// that no longer exists and would go on drawing it until it reconnected.
#[test]
fn an_undo_and_a_redo_reach_all_three_mirrors() {
    let mut pair = FilePair::new();

    pair.apply(&Command::SelectFixtures {
        ids: vec![FixtureId::new(1), FixtureId::new(2)],
        mode: prism_domain::SelectionMode::Set,
    });
    pair.apply(&Command::SetAttribute {
        attribute: AttributeType::Red,
        value: 65535,
        relative: false,
    });
    // A patch, which is the show document, and a store, which is the show and
    // the programmer at once.
    pair.apply(&patch_command(9, 3, 1));
    pair.apply(&Command::StoreCue {
        sequence_id: Some(SequenceId::new(1)),
        cue_number: "3".to_owned(),
        mode: StoreMode::Merge,
    });
    // And the third Clear, which is the session as well — so walking back from
    // here crosses all three documents.
    for _ in 0..3 {
        pair.apply(&Command::ClearProgrammer);
    }
    let programmed = pair.file.show.to_json().unwrap();

    // `FilePair::apply` compares all three mirrors after every command, so the
    // walk itself is the assertion.
    for _ in 0..pair.file.journal.len() {
        pair.apply(&Command::Oops);
    }
    assert!(pair.file.show.fixture(FixtureId::new(9)).is_none());
    assert_eq!(
        pair.file
            .show
            .sequence(SequenceId::new(1))
            .unwrap()
            .cues
            .len(),
        2
    );

    for _ in 0..pair.file.journal.redo_len() {
        pair.apply(&Command::Redo);
    }
    assert_eq!(pair.show.value(), &programmed);
    assert_eq!(pair.programmer.state, *pair.file.programmer.state());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// The whole protocol against one file: whatever was accepted, a client
    /// that applied every delta holds all three documents exactly.
    ///
    /// The programmer is the one that a "changes nothing, says nothing" rule
    /// can break silently — a state that moved without a delta leaves every
    /// client wrong until the next one arrives — so it is compared after
    /// **every** command, accepted or refused.
    #[test]
    fn one_delta_stream_reproduces_all_three_documents(
        commands in proptest::collection::vec(any::<Command>(), 1..24)
    ) {
        let mut pair = FilePair::new();
        for command in commands {
            if let Ok(applied) = pair.file.apply(&command) {
                for delta in &applied.deltas {
                    pair.show.apply_delta(delta).unwrap();
                    pair.session.apply_delta(delta).unwrap();
                    pair.programmer.apply_delta(delta);
                }
                // The effect naming the programmer is carried out here, never
                // handed on: no caller of `ShowFile::apply` has to know that
                // two models decided one command.
                prop_assert!(!applied.effects.contains(&prism_core::Effect::Programmer));
            }
            prop_assert_eq!(pair.show.value(), &pair.file.show.to_json().unwrap());
            prop_assert_eq!(pair.session.value(), &pair.file.session.to_json().unwrap());
            prop_assert_eq!(&pair.programmer.state, pair.file.programmer.state());
        }
    }
}
