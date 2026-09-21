//! How far an update of the fixture library has got, on the wire, for the
//! interface's hand-written decoder — **S62**.
//!
//! B55's rule, applied before the fault rather than after it: a new `Delta`
//! variant reaches the browser as **bytes**, and a fake daemon that hands the
//! store objects would never catch a decoder with no arm for it. So this target
//! writes real `ServerMessage::Delta`s carrying `Delta::LibraryUpdate` to
//! `ui/tests/fixtures/library-update.json`, and
//! `ui/src/ipc/libraryupdate.test.ts` reads them through `decodeServerMessage`.
//!
//! **Three of them, because the variant has three states** and a progress row
//! draws each differently: before the service has answered there is no total to
//! count towards, during the download the count is the answer, and at the end
//! the daemon's own sentence is. A fixture carrying only the middle one would
//! leave the two ends of the run untested, which are the two an operator looks
//! at.
//!
//! The file is **committed and rewritten** by
//! [`the_interface_fixture_carries_a_library_update`], S41's pattern: a stale
//! file is rewritten and the test fails, which leaves a diff to review.

use prism_domain::Delta;
use prism_ipc::ServerMessage;

mod common;

/// The fixture's name under `ui/tests/fixtures/`.
const FIXTURE: &str = "library-update.json";

/// Signed in, and the list has not come back yet: nought of nought, running.
fn starting() -> ServerMessage {
    ServerMessage::Delta {
        delta: Delta::LibraryUpdate {
            done: 0,
            total: 0,
            finished: false,
            message: String::new(),
        },
    }
}

/// Part way through a download of three thousand profiles.
fn running() -> ServerMessage {
    ServerMessage::Delta {
        delta: Delta::LibraryUpdate {
            done: 412,
            total: 3000,
            finished: false,
            message: String::new(),
        },
    }
}

/// Stopped, with the sentence the operator is left with.
fn finished() -> ServerMessage {
    ServerMessage::Delta {
        delta: Delta::LibraryUpdate {
            done: 3000,
            total: 3000,
            finished: true,
            message: "Library updated: 2998 fixtures from 3000 published, 2 skipped".to_owned(),
        },
    }
}

/// The file, as it should read.
fn expected() -> String {
    let payload = |message: &ServerMessage| {
        common::encode_base64(&prism_ipc::frame::encode(message).expect("the delta encodes"))
    };
    let document = serde_json::json!({
        "note": "Written by crates/prismd/tests/ui_library_update.rs — three \
                 Delta::LibraryUpdate, one per state. S62.",
        "starting": payload(&starting()),
        "running": payload(&running()),
        "finished": payload(&finished()),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("the document serialises");
    text.push('\n');
    text
}

#[test]
fn the_interface_fixture_carries_a_library_update() {
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
fn every_state_decodes_back_to_the_delta_it_was_written_from() {
    for message in [starting(), running(), finished()] {
        let payload = prism_ipc::frame::encode(&message).expect("the delta encodes");
        let back: ServerMessage = prism_ipc::frame::decode(&payload).expect("the delta decodes");
        assert_eq!(back, message);
    }
}
