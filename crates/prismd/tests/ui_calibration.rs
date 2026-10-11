//! What the calibration window is drawn from, on the wire, for the interface's
//! hand-written decoder - **B70**.
//!
//! `docs/manual/developer.en.md` §3's *hand decoder* rule once more: the window's
//! own tests drive a fake daemon that hands over *objects*, so a field
//! `ui/src/ipc/protocol.ts` forgot would be a connection that drops on the first
//! sighting a desk ever takes (punch-list B55's shape, a fourth time).
//!
//! This target writes four answers as base64 to `ui/tests/fixtures/calibration.json`
//! and `ui/src/ipc/calibration.test.ts` decodes them through the real
//! `decodeServerMessage`: a pose that fits, a refusal in the daemon's words, an
//! aim, and an aim for a fixture that cannot be driven.
//!
//! The file is **committed and rewritten** by the first test, S41's pattern: a
//! stale file is rewritten and the test fails, leaving a diff to review.

use prism_domain::{AimedAt, Answer, CalibrationAnswer, FixtureId, SightingFit, Vec3};
use prism_ipc::ServerMessage;

mod common;

const FIXTURE: &str = "calibration.json";

fn answer(answer: CalibrationAnswer) -> ServerMessage {
    ServerMessage::Answer {
        seq: 1,
        answer: Answer::Calibration {
            fixture: FixtureId::new(7),
            answer,
        },
    }
}

fn solved() -> ServerMessage {
    answer(CalibrationAnswer::Solved {
        position: Vec3 {
            x: 1.25,
            y: 6.5,
            z: -2.0,
        },
        rotation: Vec3 {
            x: 4.5,
            y: -170.0,
            z: 0.0,
        },
        fits: vec![
            SightingFit {
                residual: 0.05,
                distance: 7.25,
            },
            SightingFit {
                residual: 0.5,
                distance: 4.5,
            },
            SightingFit {
                residual: 0.0,
                distance: 9.0,
            },
        ],
        rms: 0.25,
        worst: 0.5,
        position_sigma: 0.0625,
        rotation_sigma: 0.125,
        spare: 0,
        iterations: 12,
    })
}

fn refused() -> ServerMessage {
    answer(CalibrationAnswer::Refused {
        reason: "2 sightings are not enough: this needs at least 3".to_owned(),
    })
}

fn aimed() -> ServerMessage {
    answer(CalibrationAnswer::Aimed {
        aim: Some(AimedAt {
            pan: 40_000,
            tilt: 20_000,
            reached: false,
        }),
    })
}

fn nothing_to_aim() -> ServerMessage {
    answer(CalibrationAnswer::Aimed { aim: None })
}

fn payload(message: &ServerMessage) -> String {
    common::encode_base64(&prism_ipc::frame::encode(message).expect("the message encodes"))
}

fn expected() -> String {
    let document = serde_json::json!({
        "note": "Written by crates/prismd/tests/ui_calibration.rs - what the \
                 calibration window is drawn from. B70.",
        "solved": payload(&solved()),
        "refused": payload(&refused()),
        "aimed": payload(&aimed()),
        "nothingToAim": payload(&nothing_to_aim()),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("the document serialises");
    text.push('\n');
    text
}

#[test]
fn the_interface_fixture_carries_the_calibration_answers() {
    let path = common::ui_fixture(FIXTURE);
    let wanted = expected();
    let found = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    if found != wanted {
        std::fs::write(&path, &wanted).expect("the fixture is writable");
        panic!(
            "{} was stale and has been rewritten - review the diff and commit it",
            path.display()
        );
    }
}

#[test]
fn each_message_decodes_back_to_what_it_was_written_from() {
    for message in [solved(), refused(), aimed(), nothing_to_aim()] {
        let bytes = prism_ipc::frame::encode(&message).expect("the message encodes");
        let back: ServerMessage = prism_ipc::frame::decode(&bytes).expect("the message decodes");
        assert_eq!(back, message);
    }
}
