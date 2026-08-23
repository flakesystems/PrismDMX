//! The MIDI ports a settings window can offer — S36.
//!
//! Opening one is `prism-midi`'s business and it is platform code; **naming**
//! one is domain vocabulary, because the name is what a configuration holds and
//! what an operator reads off a list. This module is the second of those and
//! nothing else.
//!
//! # Why a port is answered rather than mirrored
//!
//! What is plugged in is not state the daemon owns. It changes when somebody
//! moves a plug, no command causes it, and a client that mirrored it would be
//! holding a copy of the operating system's opinion from whenever it last
//! connected. So it is a [`crate::Query`], like the patch conflicts and the
//! fixture library before it: asked when a settings window opens, and asked
//! again when the operator presses *rescan*.
//!
//! **The configured port is the opposite** and is not here: it is a property of
//! *this machine*, it lives in `prism_core::MachineConfig` beside the desk
//! identity and the output patch, and it travels as
//! [`crate::Command::SetSurfacePort`] and [`crate::Delta::SurfaceChanged`]. The
//! two are answered together by [`crate::Answer::MidiPorts`], because a list of
//! ports with no mark against the chosen one is a list an operator cannot act
//! on.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One MIDI port, as the operating system offers it.
///
/// The name is the whole identity, and that is a decision rather than a
/// simplification: a port index renumbers itself when a plug moves, so a
/// configuration holding one would point at a different device afterwards. The
/// matching rule that makes a name survive the platform's decoration — a
/// Windows instance prefix, an ALSA client address — is `prism_midi::selects`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct MidiPortInfo {
    /// What the operating system calls it, decoration and all.
    pub name: String,
    /// Whether this machine can receive from it.
    pub input: bool,
    /// Whether this machine can send to it.
    pub output: bool,
}

impl MidiPortInfo {
    /// A port a control surface could be attached to.
    ///
    /// Both directions are needed and neither is optional: a desk that cannot
    /// be sent to has no motor faders and no scribble strips, and a desk that
    /// cannot be received from is not a control surface at all.
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        self.input && self.output
    }
}

/// What is known about the surface at the other end — S37.
///
/// The domain's copy of `prism_surface::SurfaceHealth`, and the two are
/// deliberately separate types for `crate::LogLevel`'s reason: that one is what
/// a feedback layer switches on, this is what travels and what a settings panel
/// draws. `prismd` converts between them in one place.
///
/// The order is the state machine's: nothing there, there and quiet, heard
/// from, asked, and did not answer.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum SurfaceHealth {
    /// No surface at all — the ordinary state of a laptop.
    #[default]
    Disconnected,
    /// One is there and has said nothing yet, which is ordinary: an X-Touch
    /// speaks only when it is touched.
    Connected,
    /// It has been heard from inside the silence window.
    Live,
    /// It has been quiet long enough to have been asked, once, whether it is
    /// still there.
    Probing,
    /// It did not answer. **The port is still open and writes still land**;
    /// what has stopped is the surface's transmitter — S20's finding, and the
    /// one state whose remedy is not *reconnect*.
    Unresponsive,
}

/// What the surface layers have done and not done — S37.
///
/// `prism_surface::SurfaceCounters` on the wire, plus the two facts that belong
/// to the port under it rather than to the feedback model: how many times the
/// cable has come back, and which binding table is in force.
///
/// # Why this is an answer and not a delta
///
/// It moves continuously — `sent` climbs whenever anything on the desk changes
/// — so a delta per change would be a broadcast at feedback rates about
/// something only an open settings window is looking at. `Query::MidiPorts` is
/// asked when that window opens and again when the operator presses *rescan*,
/// which is exactly the cadence a counter panel wants.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct SurfaceStatus {
    /// What is known about the desk at the other end.
    pub health: SurfaceHealth,
    /// What to tell the operator, if anything — `SurfaceHealth::remedy`.
    ///
    /// Carried as words rather than worked out from `health` by a client,
    /// because the wording of one of them is the whole point of the state: the
    /// obvious advice for an unresponsive desk is *reconnect*, and S20
    /// established that reconnecting is the one thing that cannot recover it.
    pub remedy: Option<String>,
    /// Messages handed to the port.
    pub sent: u64,
    /// Changes overwritten by a later change before either was sent — what
    /// coalescing saved.
    pub superseded: u64,
    /// Changes to a fader that were not sent because a hand was on it.
    pub touch_suppressed: u64,
    /// Faders resynchronised after a release.
    pub resyncs: u64,
    /// Inbound events dropped because the control is reserved — SMPTE/Beats,
    /// which is never PrismDMX's (`docs/MCU_MAPPING.md` §4.3).
    pub reserved: u64,
    /// Device queries sent. **At most one per silence**, so a number climbing
    /// here would mean somebody had started polling the handshake.
    pub probes: u64,
    /// How many times a cable has been pulled out and put back.
    pub reconnects: u64,
    /// The binding profile in force, or `None` for the built-in table.
    pub profile: Option<String>,
    /// How many controls that table binds — `prism_surface::Bindings::bound`.
    pub bound_controls: u32,
}

#[cfg(test)]
mod tests {
    use super::MidiPortInfo;

    fn port(name: &str, input: bool, output: bool) -> MidiPortInfo {
        MidiPortInfo {
            name: name.to_owned(),
            input,
            output,
        }
    }

    #[test]
    fn a_surface_needs_both_directions() {
        assert!(port("X-Touch", true, true).is_usable());
        // A synthesiser: output only. Offered, because an operator looking for
        // their desk in a list needs to see why it is not there — but not
        // usable, because there would be nothing to press.
        assert!(!port("Microsoft GS Wavetable Synth", false, true).is_usable());
        // A keyboard with no lamps: input only. Same argument the other way.
        assert!(!port("nanoKEY", true, false).is_usable());
        assert!(!port("phantom", false, false).is_usable());
    }

    #[test]
    fn a_port_travels_as_camel_case() {
        let json = serde_json::to_string(&port("X-Touch", true, true)).unwrap();
        assert_eq!(json, r#"{"name":"X-Touch","input":true,"output":true}"#);
        let back: MidiPortInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, port("X-Touch", true, true));
    }
}
