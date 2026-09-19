//! A switched slot's table and the daemon's reading of it, on the wire, for the
//! interface's hand-written decoder — punch-list **B52**.
//!
//! B55's rule, applied before the fault rather than after it: a new `Delta`
//! variant reaches the browser as **bytes**, and a fake daemon that hands the
//! store objects would never see a decoder with no arm for it. So this target
//! writes a real `ServerMessage::Delta` carrying `Delta::SwitchPositions` — one
//! slot in a position, one in none — to
//! `ui/tests/fixtures/switch-positions.json`, and
//! `ui/src/ipc/switchpositions.test.ts` reads it through `decodeServerMessage`.
//!
//! The file is **committed and rewritten** by
//! [`the_interface_fixture_carries_a_switch_reading`], S41's pattern: a stale
//! file is rewritten and the test fails, which leaves a diff to review.

use prism_domain::{Delta, FixtureId, SwitchState};
use prism_ipc::ServerMessage;

mod common;

/// The fixture's name under `ui/tests/fixtures/`.
const FIXTURE: &str = "switch-positions.json";

/// The delta: fixture 9's slot at offset 1 in its second position, fixture 9's
/// slot at offset 3 standing where the file names none.
fn delta() -> ServerMessage {
    ServerMessage::Delta {
        delta: Delta::SwitchPositions {
            positions: vec![
                SwitchState {
                    fixture: FixtureId::new(9),
                    offset: 1,
                    position: Some(1),
                },
                SwitchState {
                    fixture: FixtureId::new(9),
                    offset: 3,
                    position: None,
                },
            ],
        },
    }
}

/// The file, as it should read.
fn expected() -> String {
    let payload = prism_ipc::frame::encode(&delta()).expect("the delta encodes");
    let document = serde_json::json!({
        "note": "Written by crates/prismd/tests/ui_switch_positions.rs — one \
                 Delta::SwitchPositions. B52.",
        "payload": common::encode_base64(&payload),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("the document serialises");
    text.push('\n');
    text
}

#[test]
fn the_interface_fixture_carries_a_switch_reading() {
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
fn the_fixture_decodes_back_to_the_delta_it_was_written_from() {
    let payload = prism_ipc::frame::encode(&delta()).expect("the delta encodes");
    let back: ServerMessage = prism_ipc::frame::decode(&payload).expect("the delta decodes");
    assert_eq!(back, delta());
}
