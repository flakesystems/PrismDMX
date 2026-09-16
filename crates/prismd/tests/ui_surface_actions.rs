//! Every [`SurfaceAction`] this build has, on the wire, for the interface's
//! hand-written decoder — punch-list **B55** (GitHub #23).
//!
//! # What went wrong, and why no test saw it
//!
//! `ui/src/ipc/protocol.ts::readSurfaceAction` is written out arm by arm,
//! because what arrives off a socket is `unknown` and `CLAUDE.md` forbids a
//! cast. S43 gave the vocabulary two variants — `OpenWindowPicker` and
//! `WriteCommandLine` — and the decoder never learned them. Binding either one
//! in the control editor put it into the table in force, the next
//! `Answer::SurfaceBindings` carried it, the decoder threw, and a client that
//! cannot read a message closes the connection. **Every** later visit to the
//! Controls panel asked the same question and got the same answer, across a
//! daemon restart, because the table is `machine.json`'s.
//!
//! The generated binding said the variants existed and the compiler was
//! satisfied: a `switch` with a throwing `default` is exhaustive by
//! construction. The editor's own tests drove a fake daemon that hands the
//! panel objects rather than bytes, so the decoder was never on their path.
//! That is `docs/manual/developer.en.md` §3's *hand decoder* rule in a third
//! shape — a new **variant** rather than a new field.
//!
//! # The shape of the fix
//!
//! This target writes one `ServerMessage::Answer` whose table binds **every**
//! variant, as a base64 payload, to `ui/tests/fixtures/surface-actions.json`,
//! and `ui/src/ipc/surfaceactions.test.ts` decodes it through the real
//! `decodeServerMessage`. The list of variants is a `match` with no wildcard,
//! so a variant added in Rust does not compile until it is listed here, and
//! once it is listed the browser test goes red until the decoder reads it.
//!
//! The file is **committed and rewritten** by
//! [`the_interface_fixture_carries_every_surface_action`], S41's pattern: a
//! stale file is rewritten and the test fails, which leaves a diff to review
//! rather than a log to read.

use prism_domain::{
    Answer, BoundControl, ExecutorButtonFunction, ExecutorButtonRef, ExecutorTarget, FeatureGroup,
    GoDirection, ParamDirection, Step, SurfaceAction, SurfaceControl, ViewId, WindowType,
};
use prism_ipc::ServerMessage;

mod common;

/// The fixture's name under `ui/tests/fixtures/`.
const FIXTURE: &str = "surface-actions.json";

/// One of every [`SurfaceAction`], and one of every shape the payloads inside
/// them take.
///
/// `ExecutorButton` appears three times because [`ExecutorButtonRef`] has two
/// shapes and one of them carries either a fixed function or a written line —
/// each is a different branch of the decoder.
fn every_action() -> Vec<SurfaceAction> {
    let actions = vec![
        SurfaceAction::ExecutorMaster {
            target: ExecutorTarget::Strip,
        },
        SurfaceAction::ExecutorGo {
            target: ExecutorTarget::Selected,
            direction: GoDirection::Prev,
        },
        SurfaceAction::ExecutorOff {
            target: ExecutorTarget::Strip,
        },
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Strip,
            button: ExecutorButtonRef::Slot { index: 2 },
        },
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Selected,
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::GoForward,
            },
        },
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Selected,
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::CommandLine {
                    line: "Goto Cue 3 Sequence 7".to_owned(),
                },
            },
        },
        SurfaceAction::SelectExecutor {
            target: ExecutorTarget::Strip,
        },
        SurfaceAction::ClearProgrammer,
        SurfaceAction::ExecutorPage { delta: -1 },
        SurfaceAction::SelectView {
            view: ViewId::new(3),
        },
        SurfaceAction::StepView {
            direction: Step::Next,
        },
        SurfaceAction::ProgrammerPage { delta: 1 },
        SurfaceAction::SelectProgrammerParam {
            direction: ParamDirection::Next,
        },
        SurfaceAction::AdjustParameter,
        SurfaceAction::SetEncoderBank {
            group: FeatureGroup::Color,
        },
        SurfaceAction::OpenWindow {
            window: WindowType::FixtureSheet,
        },
        SurfaceAction::OpenWindowPicker,
        SurfaceAction::WriteCommandLine {
            line: "Store Cue".to_owned(),
            submit: false,
        },
        SurfaceAction::WriteCommandLine {
            line: "Go Executor 1".to_owned(),
            submit: true,
        },
        SurfaceAction::SaveShow,
        SurfaceAction::Oops,
        SurfaceAction::Redo,
    ];
    // No wildcard: a variant added to the enum does not compile until it has an
    // index here, and the assertion below then requires it in the list above.
    let index = |action: &SurfaceAction| -> usize {
        match action {
            SurfaceAction::ExecutorMaster { .. } => 0,
            SurfaceAction::ExecutorGo { .. } => 1,
            SurfaceAction::ExecutorOff { .. } => 2,
            SurfaceAction::ExecutorButton { .. } => 3,
            SurfaceAction::SelectExecutor { .. } => 4,
            SurfaceAction::ClearProgrammer => 5,
            SurfaceAction::ExecutorPage { .. } => 6,
            SurfaceAction::SelectView { .. } => 7,
            SurfaceAction::StepView { .. } => 8,
            SurfaceAction::ProgrammerPage { .. } => 9,
            SurfaceAction::SelectProgrammerParam { .. } => 10,
            SurfaceAction::AdjustParameter => 11,
            SurfaceAction::SetEncoderBank { .. } => 12,
            SurfaceAction::OpenWindow { .. } => 13,
            SurfaceAction::OpenWindowPicker => 14,
            SurfaceAction::WriteCommandLine { .. } => 15,
            SurfaceAction::SaveShow => 16,
            SurfaceAction::Oops => 17,
            SurfaceAction::Redo => 18,
        }
    };
    let mut seen: Vec<usize> = actions.iter().map(index).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        (0..=18).collect::<Vec<_>>(),
        "every SurfaceAction variant has to be in the fixture"
    );
    actions
}

/// The answer the Controls panel is served, with every action bound — one per
/// control, in the order the editor draws them.
fn answer() -> ServerMessage {
    let controls = BoundControl::all()
        .into_iter()
        .zip(every_action())
        .map(|(control, action)| SurfaceControl {
            control,
            name: "fixture".to_owned(),
            action: Some(action),
            permanent: false,
            reserved: false,
        })
        .collect();
    ServerMessage::Answer {
        seq: 1,
        answer: Answer::SurfaceBindings {
            controls,
            device: "Behringer X-Touch".to_owned(),
            device_key: "behringer-x-touch".to_owned(),
            profile_version: 1,
            profile: None,
            revision: 1,
            learning: false,
        },
    }
}

/// The file, as it should read.
fn expected() -> String {
    let payload = prism_ipc::frame::encode(&answer()).expect("the answer encodes");
    let document = serde_json::json!({
        "note": "Written by crates/prismd/tests/ui_surface_actions.rs — one \
                 Answer::SurfaceBindings binding every SurfaceAction. B55.",
        "actions": every_action().len(),
        "payload": common::encode_base64(&payload),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("the document serialises");
    text.push('\n');
    text
}

#[test]
fn the_interface_fixture_carries_every_surface_action() {
    let path = common::ui_fixture(FIXTURE);
    let wanted = expected();
    let found = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    if found != wanted {
        std::fs::write(&path, &wanted).expect("the fixture is writable");
        panic!(
            "{} was stale and has been rewritten — review the diff and commit it",
            path.display()
        );
    }
}

#[test]
fn the_fixture_decodes_back_to_the_answer_it_was_written_from() {
    let payload = prism_ipc::frame::encode(&answer()).expect("the answer encodes");
    let back: ServerMessage = prism_ipc::frame::decode(&payload).expect("the answer decodes");
    assert_eq!(back, answer());
}
