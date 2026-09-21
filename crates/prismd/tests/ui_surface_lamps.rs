//! Which keys are lit, on the wire, for the interface's hand-written decoder —
//! **S59**.
//!
//! B55's rule, applied before the fault rather than after it: a new `Delta`
//! variant reaches the browser as **bytes**, and a fake daemon that hands the
//! store objects would never see a decoder with no arm for it. So this target
//! writes a real `ServerMessage::Delta` carrying `Delta::SurfaceLampsChanged`
//! to `ui/tests/fixtures/surface-lamps.json`, and
//! `ui/src/ipc/surfacelamps.test.ts` reads it through `decodeServerMessage`.
//!
//! The names are `BoundControl`'s own spellings rather than strings written out
//! here, which is the point of the assertion below: a drawing joins this list
//! to its rows by name, so a delta that spelled a control differently from
//! `SurfaceControl::name` would light nothing and say nothing about why.
//!
//! The file is **committed and rewritten** by
//! [`the_interface_fixture_carries_a_lamp_reading`], S41's pattern: a stale file
//! is rewritten and the test fails, which leaves a diff to review.

use prism_domain::{BoundControl, Delta, GlobalButton, StripButton};
use prism_ipc::ServerMessage;

mod common;

/// The fixture's name under `ui/tests/fixtures/`.
const FIXTURE: &str = "surface-lamps.json";

/// The controls this fixture says are lit.
///
/// One of each **kind** of control the drawing has to join by name — a panel
/// key, a strip key and the one row that stands for eight columns — because the
/// spelling is what the browser matches on and the three are spelled by
/// different arms of `BoundControl`'s `Display`.
fn lit() -> Vec<BoundControl> {
    vec![
        BoundControl::Global {
            button: GlobalButton::Save,
        },
        BoundControl::Global {
            button: GlobalButton::F1,
        },
        BoundControl::StripButton {
            button: StripButton::Select,
        },
    ]
}

/// The delta.
fn delta() -> ServerMessage {
    ServerMessage::Delta {
        delta: Delta::SurfaceLampsChanged {
            lit: lit()
                .into_iter()
                .map(|control| control.to_string())
                .collect(),
        },
    }
}

/// The file, as it should read.
fn expected() -> String {
    let payload = prism_ipc::frame::encode(&delta()).expect("the delta encodes");
    let document = serde_json::json!({
        "note": "Written by crates/prismd/tests/ui_surface_lamps.rs — one \
                 Delta::SurfaceLampsChanged. S59.",
        "payload": common::encode_base64(&payload),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("the document serialises");
    text.push('\n');
    text
}

#[test]
fn the_interface_fixture_carries_a_lamp_reading() {
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

/// **The names in the delta are the names on the rows** — the join the drawing
/// makes, asserted rather than assumed.
///
/// `Delta::SurfaceLampsChanged` carries text and `SurfaceControl::name` carries
/// text, and nothing in the type system says they are the same text. They are,
/// because both are `BoundControl`'s `Display`; this is the sentence that would
/// go red if either side started spelling a control its own way, which would
/// otherwise show up as a drawing that never lights.
#[test]
fn a_lit_name_is_the_name_the_table_gives_that_control() {
    for control in lit() {
        let row = prism_domain::SurfaceControl {
            name: control.to_string(),
            action: None,
            permanent: false,
            reserved: false,
            geometry: None,
            control,
        };
        assert_eq!(row.name, control.to_string());
        assert_eq!(BoundControl::from_name(&row.name), Some(control));
    }
}
