//! What the tracker panel is drawn from, on the wire, for the interface's
//! hand-written decoder — **S32**.
//!
//! `docs/manual/developer.en.md` §3's *hand decoder* rule, once more: what
//! arrives off a socket is `unknown`, `ui/src/ipc/protocol.ts` reads it field by
//! field, and the panel's own tests drive a fake daemon that hands over *objects*,
//! so a field the decoder forgot would be a connection that drops the first
//! time a desk has a tracker, and nothing in the panel's suite would say so
//! (punch-list B55 was exactly that).
//!
//! This target writes three messages as base64 to
//! `ui/tests/fixtures/trackers.json`, and `ui/src/ipc/trackers.test.ts` decodes
//! them through the real `decodeServerMessage`:
//!
//! - **`Answer::Trackers`** with one tracker of each kind a row can be - being
//!   heard, gone quiet, and named but **never heard**, whose age is `u64::MAX`,
//!   the one integer a JavaScript number cannot hold;
//! - **`Delta::MachineChanged`** carrying tracker settings that are not the
//!   defaults, so a decoder that filled them in from its own constants cannot
//!   pass; and
//! - **`Delta::ShowPatch`** adding and then replacing and then removing a
//!   fixture's `follow`, the three operations `PlaceFixtures` produces.
//!
//! The file is **committed and rewritten** by the first test, S41's pattern: a
//! stale file is rewritten and the test fails, leaving a diff to review.

use prism_domain::{
    Answer, AxisSource, Delta, JsonPatchOp, MachineSettings, SeenTracker, SourceAxis,
    TrackerHealth, TrackerMapping, TrackerSettings, Vec3,
};
use prism_ipc::ServerMessage;

mod common;

const FIXTURE: &str = "trackers.json";

fn answer() -> ServerMessage {
    ServerMessage::Answer {
        seq: 1,
        answer: Answer::Trackers {
            trackers: vec![
                SeenTracker {
                    id: 3,
                    name: Some("Anna".to_owned()),
                    position: Vec3 {
                        x: 1.25,
                        y: 1.7,
                        z: -4.5,
                    },
                    age_ms: 40,
                    health: TrackerHealth::Live,
                    followers: 2,
                },
                SeenTracker {
                    id: 4,
                    name: None,
                    position: Vec3 {
                        x: -3.0,
                        y: 0.0,
                        z: 6.0,
                    },
                    age_ms: 12_345,
                    health: TrackerHealth::Quiet,
                    followers: 0,
                },
                SeenTracker {
                    id: 9,
                    name: Some("Ben".to_owned()),
                    position: Vec3::ZERO,
                    // *Never heard*, which the daemon says with the largest
                    // integer there is.
                    age_ms: u64::MAX,
                    health: TrackerHealth::Quiet,
                    followers: 1,
                },
            ],
            listening: true,
            error: None,
            rejected: 7,
        },
    }
}

fn settings() -> TrackerSettings {
    TrackerSettings {
        enabled: true,
        interface: Some("192.168.1.20".to_owned()),
        group: "236.10.10.11".to_owned(),
        port: 56_570,
        mapping: TrackerMapping {
            x: AxisSource {
                from: SourceAxis::X,
                invert: true,
            },
            y: AxisSource {
                from: SourceAxis::Z,
                invert: false,
            },
            z: AxisSource {
                from: SourceAxis::Y,
                invert: true,
            },
            scale: 0.001,
            offset: Vec3 {
                x: 1.5,
                y: 0.0,
                z: -2.0,
            },
        },
        timeout_ms: 750,
    }
}

fn machine() -> ServerMessage {
    ServerMessage::Delta {
        delta: Delta::MachineChanged {
            settings: Box::new(MachineSettings {
                desk_id: "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_owned(),
                data_dir: "C:/ProgramData/PrismDMX".to_owned(),
                universes: 8,
                jog_sensitivity: 100,
                trackers: settings(),
                ..MachineSettings::default()
            }),
        },
    }
}

fn follows() -> ServerMessage {
    let target = serde_json::json!({
        "tracker": 5,
        "offset": { "x": 0.0, "y": 0.4, "z": 0.0 },
    });
    let value = |json: serde_json::Value| -> prism_domain::JsonValue {
        serde_json::from_value(json).expect("a JSON value")
    };
    ServerMessage::Delta {
        delta: Delta::ShowPatch {
            ops: vec![
                JsonPatchOp::Add {
                    path: "/fixtures/1/follow".to_owned(),
                    value: value(target.clone()),
                },
                JsonPatchOp::Replace {
                    path: "/fixtures/1/follow".to_owned(),
                    value: value(serde_json::json!({
                        "tracker": 6,
                        "offset": { "x": 0.0, "y": 0.0, "z": 0.0 },
                    })),
                },
                JsonPatchOp::Remove {
                    path: "/fixtures/1/follow".to_owned(),
                },
            ],
        },
    }
}

fn payload(message: &ServerMessage) -> String {
    common::encode_base64(&prism_ipc::frame::encode(message).expect("the message encodes"))
}

fn expected() -> String {
    let document = serde_json::json!({
        "note": "Written by crates/prismd/tests/ui_trackers.rs — what the tracker \
                 panel and the viewer's Follows box are drawn from. S32.",
        "answer": payload(&answer()),
        "machine": payload(&machine()),
        "follows": payload(&follows()),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("the document serialises");
    text.push('\n');
    text
}

#[test]
fn the_interface_fixture_carries_the_tracker_messages() {
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
fn each_message_decodes_back_to_what_it_was_written_from() {
    for message in [answer(), machine(), follows()] {
        let bytes = prism_ipc::frame::encode(&message).expect("the message encodes");
        let back: ServerMessage = prism_ipc::frame::decode(&bytes).expect("the message decodes");
        assert_eq!(back, message);
    }
}
