//! The platform split itself — the only `#[cfg(target_os = …)]` S36 adds.
//!
//! `ARCHITECTURE_SPEC.md` §10.1 gained a fourth exception for this crate, and
//! this module is the whole of it: everything above [`MidiBackend`] is one code
//! path on every target, exactly as `prism-ipc`'s framing is one code path over
//! a named pipe and a Unix domain socket.
//!
//! Two backends:
//!
//! - [`midir`], on any build where the dependency is present — Windows and
//!   macOS always, Linux with `--features alsa`.
//! - **None**, otherwise: an empty enumeration and a [`PortError::NoBackend`].
//!   That is not a degraded mode to apologise for, it is the same answer a
//!   machine with nothing plugged in gives, and it is what the Linux CI jobs
//!   run against.
//!
//! The `cfg` is written once, here, as a single predicate with a name, so that
//! adding a platform is an edit to one line rather than a search.

use crate::PortList;
use crate::port::{MidiBackend, OpenMidiPort, PortError};

/// The backend this build was compiled with, for a log line and a status panel.
///
/// A person looking at *no MIDI ports* needs to know whether that is a machine
/// with nothing plugged in or a build that could not have found one.
#[must_use]
pub const fn backend_name() -> &'static str {
    #[cfg(target_os = "windows")]
    const NAME: &str = "midir (WinMM)";
    #[cfg(target_os = "macos")]
    const NAME: &str = "midir (CoreMIDI)";
    #[cfg(all(target_os = "linux", feature = "alsa"))]
    const NAME: &str = "midir (ALSA)";
    #[cfg(all(target_os = "linux", not(feature = "alsa")))]
    const NAME: &str = "none";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    const NAME: &str = "midir";
    NAME
}

/// This machine's MIDI service.
#[must_use]
pub fn system_backend() -> Box<dyn MidiBackend> {
    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    {
        Box::new(midir_backend::Midir)
    }
    #[cfg(all(target_os = "linux", not(feature = "alsa")))]
    {
        Box::new(NoBackend)
    }
}

/// A build with no MIDI service compiled into it.
///
/// It answers rather than failing, because the two facts a caller acts on —
/// *nothing to offer* and *nothing to open* — are the same two facts a machine
/// with an empty USB bus produces.
#[derive(Debug, Clone, Copy)]
pub struct NoBackend;

impl MidiBackend for NoBackend {
    fn ports(&self) -> PortList {
        PortList::default()
    }

    fn open(&self, _configured: &str) -> Result<Box<dyn OpenMidiPort>, PortError> {
        Err(PortError::NoBackend)
    }
}

#[cfg(any(not(target_os = "linux"), feature = "alsa"))]
mod midir_backend {
    //! `midir` — WinMM, CoreMIDI or ALSA, and none of it reaches the codec.

    use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

    use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

    use crate::PortList;
    use crate::port::{Inbox, MidiBackend, OpenMidiPort, PortError};

    /// What this application calls itself to the operating system's MIDI
    /// service. It is what shows up in `aconnect` and in Audio MIDI Setup.
    const CLIENT: &str = "PrismDMX";

    /// The name of the connection, which some platforms show beside the client.
    const CONNECTION: &str = "surface";

    /// The system MIDI service.
    #[derive(Debug, Clone, Copy)]
    pub struct Midir;

    impl MidiBackend for Midir {
        fn ports(&self) -> PortList {
            PortList {
                inputs: input_names(),
                outputs: output_names(),
            }
        }

        fn open(&self, configured: &str) -> Result<Box<dyn OpenMidiPort>, PortError> {
            let mut input = MidiInput::new(CLIENT).map_err(refused)?;
            // SysEx has to arrive: the device query of `docs/MCU_MAPPING.md`
            // §2.3 is answered with one, and it is how a surface that has gone
            // quiet is told from one that is merely idle. Timing clock and
            // active sensing are dropped by the service rather than counted by
            // the codec — they are a stream of bytes an X-Touch never sends
            // anything meaningful in, and every one of them would otherwise
            // wake the surface poll.
            input.ignore(Ignore::TimeAndActiveSense);
            let in_ports = input.ports();
            let in_names: Vec<String> = in_ports
                .iter()
                .map(|port| input.port_name(port).unwrap_or_default())
                .collect();
            let (in_index, name) =
                crate::choose(configured, &in_names).ok_or_else(|| PortError::NotFound {
                    configured: configured.to_owned(),
                })?;
            let name = name.to_owned();
            let in_port = in_ports.get(in_index).ok_or_else(|| {
                PortError::Refused("the port list changed while it was being read".to_owned())
            })?;

            let output = MidiOutput::new(CLIENT).map_err(refused)?;
            let out_ports = output.ports();
            let out_names: Vec<String> = out_ports
                .iter()
                .map(|port| output.port_name(port).unwrap_or_default())
                .collect();
            // Matched against the **input port's** name rather than against the
            // configuration: the two halves of one device are the same device,
            // and a configuration exact enough to name the input is exact
            // enough that the output beside it should follow it rather than be
            // matched again from scratch.
            let (out_index, _) =
                crate::choose(&name, &out_names).ok_or_else(|| PortError::NotFound {
                    configured: name.clone(),
                })?;
            let out_port = out_ports.get(out_index).ok_or_else(|| {
                PortError::Refused("the port list changed while it was being read".to_owned())
            })?;

            let (sink, inbox) = channel();
            let connection = input
                .connect(
                    in_port,
                    CONNECTION,
                    |_stamp, message, sink: &mut Sender<Vec<u8>>| {
                        // The callback runs on the service's own thread and does
                        // one thing: hand the bytes over. Everything that
                        // interprets them — the codec, the shadow model, the
                        // binding table — runs on the surface thread, where a
                        // clock is an argument and nothing is shared.
                        let _ = sink.send(message.to_vec());
                    },
                    sink,
                )
                .map_err(|error| PortError::Refused(error.to_string()))?;
            let output = output
                .connect(out_port, CONNECTION)
                .map_err(|error| PortError::Refused(error.to_string()))?;

            Ok(Box::new(MidirPort {
                name,
                _input: connection,
                output,
                messages: inbox,
                waiting: Inbox::new(),
            }))
        }
    }

    /// One open device: an input connection, an output connection and the queue
    /// between the service's thread and ours.
    ///
    /// Everything here is glue. The framing — a message longer than the
    /// caller's buffer delivered in pieces — is `crate::Inbox`'s, on purpose:
    /// it is the one part of a real port that is arithmetic rather than
    /// platform, and it is asserted there, where no device is needed for it.
    struct MidirPort {
        name: String,
        /// Dropping this closes the input port, so it is held even though
        /// nothing reads it: the callback is the reader.
        _input: MidiInputConnection<Sender<Vec<u8>>>,
        output: MidiOutputConnection,
        messages: Receiver<Vec<u8>>,
        waiting: Inbox,
    }

    impl OpenMidiPort for MidirPort {
        fn name(&self) -> &str {
            &self.name
        }

        fn read(&mut self, buffer: &mut [u8]) -> Option<usize> {
            if self.waiting.is_empty() {
                match self.messages.try_recv() {
                    Ok(message) => self.waiting.push(&message),
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => return None,
                }
            }
            self.waiting.read(buffer)
        }

        fn write(&mut self, bytes: &[u8]) -> bool {
            self.output.send(bytes).is_ok()
        }
    }

    fn input_names() -> Vec<String> {
        let Ok(mut input) = MidiInput::new(CLIENT) else {
            return Vec::new();
        };
        input.ignore(Ignore::None);
        input
            .ports()
            .iter()
            .filter_map(|port| input.port_name(port).ok())
            .filter(|name| !name.is_empty())
            .collect()
    }

    fn output_names() -> Vec<String> {
        let Ok(output) = MidiOutput::new(CLIENT) else {
            return Vec::new();
        };
        output
            .ports()
            .iter()
            .filter_map(|port| output.port_name(port).ok())
            .filter(|name| !name.is_empty())
            .collect()
    }

    fn refused(error: impl core::fmt::Display) -> PortError {
        PortError::Refused(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{NoBackend, backend_name, system_backend};
    use crate::port::{MidiBackend, PortError};

    #[test]
    fn a_build_with_no_backend_answers_rather_than_failing() {
        let backend = NoBackend;
        assert!(backend.ports().is_empty());
        assert!(matches!(backend.open("X-Touch"), Err(PortError::NoBackend)));
    }

    /// The backend this build has, whatever it is, answers the two questions a
    /// caller asks it — and **opens nothing**.
    #[test]
    fn the_system_backend_enumerates_without_opening_anything() {
        let backend = system_backend();
        let listed = backend.ports();
        assert_eq!(listed.is_empty(), listed.names().is_empty());
        // A name nothing can be plugged in as. The answer is a refusal rather
        // than a panic, and on a machine with a real X-Touch attached — which is
        // the machine this was written on — nothing is opened either way.
        assert!(backend.open("no such port \u{1F50C}").is_err());
        assert!(!backend_name().is_empty());
    }
}
