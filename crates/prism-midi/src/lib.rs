//! The MIDI port — the one crate in the workspace that opens a device. **S36**.
//!
//! # Why this crate exists at all
//!
//! `ARCHITECTURE_SPEC.md` §10.1 confines `#[cfg(target_os = …)]` to a named
//! list, and until S36 that list held `prism-protocols`, `prism-app` and
//! `prism-ipc`'s `transport/local.rs`. Neither `prism-surface` nor `prismd` is
//! on it, and opening a MIDI port is nothing *but* platform code: WinMM on
//! Windows, ALSA on Linux, CoreMIDI on macOS. So S20's probe — the only code in
//! the repository that had ever opened one — lived outside the workspace
//! (`tools/xtouch-probe/`), and the daemon's [`SurfacePort`] seam had two
//! implementations, neither of which touched a device.
//!
//! **A backend needs a home before it can have an implementation.** This is the
//! home: a crate whose whole purpose is to hold the split, exactly as
//! `thread-priority` holds the one for the tick thread's priority (S17). The
//! fourth exception in §10.1 is this crate, and the ARM64 cross-compile check is
//! what says the split did not leak — `prism-surface` gains no dependency at
//! all, and on `aarch64-unknown-linux-gnu` nothing here compiles `midir`.
//!
//! # The shape
//!
//! ```text
//!   [`ports`]  ─────────────▶ [`PortList`]     what is plugged in, by name
//!
//!   [`MidiSurfacePort`] ──┬─▶ [`MidiBackend`] ──▶ midir ──▶ the operating system
//!    the configured name  │    the seam a test
//!    and the cable's      │    stands in for
//!    coming and going     │
//!                         └─▶ read / write / connected / refresh
//! ```
//!
//! Nothing here knows what a Mackie Control message *is*: bytes go in, bytes
//! come out. The three layers that give them meaning are `prism-surface`'s and
//! stay platform-neutral.
//!
//! # A name is the configuration, and it has to survive a cable
//!
//! A settings window writes down *which* port, and the only handle an operating
//! system offers that means anything to a person is the name. It is also the
//! only one that survives being unplugged and plugged back in: `midir`'s port
//! identifiers are positions in a list that renumbers itself, so a configuration
//! holding one would point at a different device after somebody moved a plug.
//!
//! What a name does **not** survive unchanged is the decoration each platform
//! puts round it, so [`selects`] compares them after taking the decoration off.
//! See its documentation for the three rules and why they are in that order.
//!
//! # What this crate deliberately does not do
//!
//! - **It does not pace anything.** The minimum gap between outbound messages,
//!   the 30 Hz coalescing and the once-only handshake are `prism-surface`'s
//!   (`docs/MCU_MAPPING.md` §5, built in S21), enforced against a clock the
//!   caller supplies. A second pacer here would be a second opinion about the
//!   fault of §2.7.
//! - **It does not reopen a port because the desk has gone quiet.** That is the
//!   whole of S20's unwelcome finding: the X-Touch can stop transmitting while
//!   it goes on receiving perfectly, the port stays open, writes still land on
//!   the display, and **only a power cycle recovers it**. Reopening is the one
//!   thing that does not work, so this crate reopens only when the port has
//!   actually gone — from the enumeration, or from a write that failed.
//!   `a_desk_that_has_gone_quiet_is_not_reopened` is that sentence as a test.

mod naming;
mod port;
mod system;

pub use naming::{choose, normalise, selects};
pub use port::{Inbox, MidiBackend, MidiSurfacePort, OpenMidiPort, PortError, RECONNECT_CEILING};
pub use system::{NoBackend, backend_name, system_backend};

/// What is plugged in, in both directions.
///
/// Two lists rather than one, because they are two: a surface is an input port
/// *and* an output port, and on every platform the two enumerations are
/// separate. A device that offers only one of them is a device
/// [`MidiSurfacePort`] cannot use, and saying so needs both lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortList {
    /// Ports this machine can receive from, in the order the system lists them.
    pub inputs: Vec<String>,
    /// Ports this machine can send to, in the same order.
    pub outputs: Vec<String>,
}

impl PortList {
    /// Every name in either direction, once, inputs first.
    ///
    /// What a settings window offers: an operator picks *the desk*, not one of
    /// its two halves.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut names = self.inputs.clone();
        for name in &self.outputs {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        names
    }

    /// Whether anything at all is plugged in.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty() && self.outputs.is_empty()
    }
}

/// What is plugged into this machine, right now.
///
/// **Never fails.** A machine with no MIDI device answers with an empty list,
/// and so does a build with no backend — which is the same answer, because it
/// is the same fact from the caller's side: there is nothing to offer. That is
/// an exit criterion of S36 and it is what CI runs, on two platforms, neither of
/// which has ever had an X-Touch attached.
///
/// Enumeration is a system call and is not free; it is not something to do once
/// per surface poll. [`MidiSurfacePort`] rations its own use of it — see
/// [`MidiSurfacePort::refresh`].
#[must_use]
pub fn ports() -> PortList {
    system_backend().ports()
}

#[cfg(test)]
mod tests {
    use super::{PortList, ports};

    /// The exit criterion, asked of the real backend on whatever machine is
    /// running the suite.
    ///
    /// It cannot assert *what* is there — this machine has an X-Touch attached
    /// and a build server has nothing — so it asserts the two things that are
    /// true either way: it answers at all, and the answer is a list rather than
    /// an error. **Nothing is opened**, which is what lets a test that names a
    /// device run beside a real one.
    #[test]
    fn enumeration_answers_on_a_machine_with_nothing_plugged_in() {
        let list = ports();
        // Named ports, whatever they are: an empty string is a name nothing can
        // be configured as.
        for name in list.names() {
            assert!(!name.is_empty(), "{list:?}");
        }
        assert_eq!(list.is_empty(), list.names().is_empty());
    }

    #[test]
    fn both_directions_make_one_list_of_names() {
        let list = PortList {
            inputs: vec!["X-Touch".to_owned(), "nanoKONTROL".to_owned()],
            outputs: vec!["X-Touch".to_owned(), "Wavetable Synth".to_owned()],
        };
        assert_eq!(
            list.names(),
            vec![
                "X-Touch".to_owned(),
                "nanoKONTROL".to_owned(),
                "Wavetable Synth".to_owned()
            ],
            "a port that is both is offered once, and inputs come first"
        );
        assert!(!list.is_empty());
        assert!(PortList::default().is_empty());
        assert!(PortList::default().names().is_empty());
    }
}
