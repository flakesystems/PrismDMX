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
