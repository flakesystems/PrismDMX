//! The transcription check: `ui/src/telemetry/frame.ts` against this crate.
//!
//! The same shape, and the same reason, as `interface_protocol.rs`. The
//! telemetry layout is **this crate's** — `TELEMETRY_VERSION`,
//! `TELEMETRY_HEADER_BYTES`, the magic, the section stride — and the interface
//! has to know all four before it can read a single level. `prism-ipc` exports
//! no TypeScript (it is the wire, not the vocabulary), so the numbers are
//! transcribed by hand in `ui`, and this target is what makes that safe.
//!
//! It reads the TypeScript back with `include_str!` — compiled in, so a file
//! that has been moved or deleted fails the *build* — and holds it to the
//! constants this crate ships. `crates/prismd/tests/ui_telemetry.rs` checks the
//! other half: that what the interface decodes is what this crate's decoder
//! decoded, on frames a running daemon actually sent.

use prism_ipc::{TELEMETRY_HEADER_BYTES, TELEMETRY_VERSION};

/// The interface's transcription of `crates/prism-ipc/src/telemetry.rs`.
const FRAME_TS: &str = include_str!("../../../ui/src/telemetry/frame.ts");

/// Channels in one universe, as `prism-domain` has it.
const CHANNELS: usize = prism_domain::CHANNELS_PER_UNIVERSE as usize;

#[test]
fn the_interface_reads_this_layout_version() {
    assert!(
        FRAME_TS.contains(&format!(
            "export const TELEMETRY_VERSION = {TELEMETRY_VERSION};"
        )),
        "ui/src/telemetry/frame.ts does not read layout version {TELEMETRY_VERSION}"
    );
}

/// The four numbers a decoder cannot get wrong quietly.
///
/// A header one byte out reads every level one byte out; a section stride one
/// byte out drifts a universe further wrong with each one. Both produce a
/// picture rather than an error, which is why they are asserted here as well as
/// against a recording.
#[test]
fn the_interface_knows_the_shape_of_a_frame() {
    for (name, value) in [
        ("TELEMETRY_HEADER_BYTES", TELEMETRY_HEADER_BYTES),
        ("CHANNELS_PER_UNIVERSE", CHANNELS),
    ] {
        assert!(
            FRAME_TS.contains(&format!("export const {name} = {value};")),
            "ui/src/telemetry/frame.ts has no {name} = {value}"
        );
    }
    assert!(
        FRAME_TS.contains("export const UNIVERSE_SECTION_BYTES = 2 + CHANNELS_PER_UNIVERSE;"),
        "the interface's universe section is not the number plus its levels"
    );
    assert!(
        FRAME_TS.contains(r#"export const TELEMETRY_MAGIC = "PTLM";"#),
        "the interface does not know the four bytes a frame starts with"
    );
    assert!(
        FRAME_TS.contains(&format!(
            "export const MAX_UNIVERSES = {};",
            prism_domain::UniverseId::MAX.get()
        )),
        "the interface disagrees about how many universes a desk has room for"
    );
}

/// Every way a frame can be refused, in the spelling the interface uses.
///
/// The three faults are `TelemetryError`'s three variants. The match has no
/// wildcard, so a variant added to the enum will not compile until it is listed
/// here — and once it is listed, this test requires it in the TypeScript.
#[test]
fn the_interface_knows_every_way_a_frame_can_be_refused() {
    let names = [
        prism_ipc::TelemetryError::NotTelemetry,
        prism_ipc::TelemetryError::UnknownVersion {
            found: 2,
            expected: TELEMETRY_VERSION,
        },
        prism_ipc::TelemetryError::Truncated {
            expected: 16,
            found: 0,
        },
    ]
    .map(|error| match error {
        prism_ipc::TelemetryError::NotTelemetry => "not-telemetry",
        prism_ipc::TelemetryError::UnknownVersion { .. } => "unknown-version",
        prism_ipc::TelemetryError::Truncated { .. } => "truncated",
    });
    for name in names {
        assert!(
            FRAME_TS.contains(&format!(r#""{name}""#)),
            "ui/src/telemetry/frame.ts has no {name} fault, so a frame it cannot read \
             would be reported as something else"
        );
    }
    // And the rule that makes an unknown layout harmless: it is dropped, not
    // guessed at (`docs/IPC_PROTOCOL.md` §7). The interface's decoder answers
    // with a fault instead of a frame, and nothing else in it may do so.
    assert!(
        FRAME_TS.contains("read(payload: Uint8Array): TelemetryFault | null"),
        "the interface's decoder does not answer with a fault"
    );
}
