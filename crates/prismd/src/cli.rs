//! The command line: how `prismd` is started when nothing is going to start it.
//!
//! `IMPLEMENTATION_PLAN.md` S17 asks for "a CLI for headless operation", and
//! that is what this is — the daemon's whole configuration, because there is no
//! settings dialogue yet and D9's shell (S29) will pass the same arguments when
//! it spawns one.
//!
//! # Why the parser is written here
//!
//! Nineteen options, all of them either a flag or one value, and every one of
//! them has to be *documented* on the same page it is parsed on or the two will
//! disagree. A parsing library would be a dependency, a derive macro and a
//! second place to put the help text; this is a match over `&str` with a test
//! per option. It is the same reasoning `prism-protocols` used for the Art-Net
//! packet: the seam is bigger than the thing.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use prism_domain::{OutputId, OutputInstance, OutputKind, UniverseId};

use crate::log::Level;

/// The default WebSocket address, and it is loopback for the reason
/// `docs/IPC_PROTOCOL.md` §2.1 gives: school networks are shared, and an
/// unauthenticated lighting console reachable from any classroom machine is not
/// acceptable.
pub const DEFAULT_WEBSOCKET: &str = "127.0.0.1:7373";

/// Universes the frame layout carries unless the operator says otherwise —
/// `UniverseId::MAX`, i.e. the desk's whole range. See
/// [`crate::engine::frame_layout`] for why it is the desk's and not the show's.
pub const DEFAULT_UNIVERSES: u32 = 64;

/// The multicast hop limit an sACN output asked for on the command line gets.
///
/// One: the local segment, which is right for a lighting network that is one
/// switch and wrong for a venue whose gateways are behind a router. A rig that
/// needs more says so in its machine configuration, where the number is a field
/// rather than a flag (`ARCHITECTURE_SPEC.md` §7.2).
pub const DEFAULT_HOP_LIMIT: u32 = 1;

/// What to do with the stage when the daemon stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Exit {
    /// Leave the last look on stage. The default, because "the desk is
    /// stopping" and "the stage should go dark" are different statements (S10).
    #[default]
    Hold,
    /// Publish a blackout frame, let the outputs send it, and then stop.
    Blackout,
}

/// An output named on the command line, before the show has said which
/// universes it patches.
///
/// **S33 replaced `OutputSpec` with the domain's own [`OutputInstance`]**: the
/// rig is data now, and a second vocabulary for the same thing would be a second
/// place to add an output kind. What the command line still cannot supply is the
/// *universes* — a flag has nowhere to put a list, and the show is not open when
/// the arguments are read — so an output built here carries none, and
/// [`fill_universes`] fills them in from the show before anything is validated.
///
/// [`OutputInstance`]: prism_domain::OutputInstance
pub type OutputSpec = OutputInstance;

/// Fills in the universes of every command-line output that named none.
///
/// `--open-dmx 3` names its universe; the rest carry the show's, because that is
/// what "put this show on this interface" means when a flag is all there is to
/// say it with. Called once, in [`crate::Daemon::start`], after the show is open
/// and before the rig is validated.
#[must_use]
pub fn fill_universes(outputs: &[OutputInstance], show: &[UniverseId]) -> Vec<OutputInstance> {
    outputs
        .iter()
        .map(|output| {
            if output.universes.is_empty() {
                OutputInstance {
                    universes: show.to_vec(),
                    ..output.clone()
                }
            } else {
                output.clone()
            }
        })
        .collect()
}

/// A mock output carrying whatever the show patches — what `--mock-output`
/// builds, as a value a test can put in [`Options`] directly.
///
/// `ARCHITECTURE_SPEC.md` §12 runs the end-to-end tests "against a daemon in
/// mock-output mode", and every daemon target in this repository starts one this
/// way. It is here rather than in a test helper because it has to stay exactly
/// what the flag produces: two spellings of the mode would be two modes.
#[must_use]
pub fn mock_output(id: u32) -> OutputSpec {
    OutputInstance::new(OutputId::new(id), "Mock output", OutputKind::Mock, [])
}

/// The next output number, so the flags are numbered 1, 2, 3 in the order they
/// were typed.
fn next_id(outputs: &[OutputInstance]) -> OutputId {
    OutputId::new(u32::try_from(outputs.len() + 1).unwrap_or(u32::MAX))
}

/// Everything the daemon was told to be.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Where the lock file and the machine configuration live. `None` asks the
    /// environment (see [`crate::paths`]).
    pub data_dir: Option<PathBuf>,
    /// The show to open. `None` is the default show in the data directory.
    pub show: Option<PathBuf>,
    /// Where the Open Fixture Library was installed, or `None` to look beside
    /// the executable and up the tree from it (S44).
    ///
    /// A flag because the library is **downloaded at install time and not
    /// committed** (`profiles/fixtures/SOURCE.md`), so an installer that put it
    /// somewhere else has to be able to say where.
    pub fixtures: Option<PathBuf>,
    /// Universes the frame layout carries.
    pub universes: u32,
    /// Outputs named on the command line.
    ///
    /// **The whole rig for this run when it is not empty** — the machine
    /// configuration's own is not read, and nothing is written back to it. See
    /// [`crate::daemon::rig_for`] for the reasoning: a flag is what a test and a
    /// bring-up use, and a daemon started with `--mock-output` must neither
    /// inherit a venue's cabling nor overwrite it.
    ///
    /// Empty is the ordinary configuration and is what a desk runs: the rig
    /// comes out of `prism_core::MachineConfig`, and S33's four commands edit
    /// it. Empty on both sides is legitimate too — a daemon with no output still
    /// holds a show and still answers clients.
    pub outputs: Vec<OutputSpec>,
    /// Open every configured output with a **mock driver** instead of the real
    /// one — S33.
    ///
    /// The daemon-wide form of `--mock-output`, and what makes S33's worked
    /// example testable: a rig of two Open DMX adapters, an sACN node and two
    /// Art-Net nodes can be configured, started and asserted frame by frame with
    /// nothing plugged in and nothing on the network. `CLAUDE.md` requires
    /// exactly that of every test, and this machine has a real SH-RS09B attached
    /// that the suite must not open.
    pub mock_devices: bool,
    /// Whether to open the named pipe or Unix domain socket.
    pub local: bool,
    /// The WebSocket address, or `None` to leave it closed.
    pub websocket: Option<SocketAddr>,
    /// The §2.1 token. Required when the WebSocket listener is not on loopback.
    pub token: Option<String>,
    /// The X-Touch binding profile to read (`docs/MCU_MAPPING.md` §4.2), or
    /// `None` for the built-in defaults.
    ///
    /// A path this daemon may fail to read: a profile that is missing or
    /// malformed produces a warning and the built-in table, never a daemon that
    /// will not start. `IMPLEMENTATION_PLAN.md` S22, and
    /// [`crate::surface::load_profile`] is where it is kept.
    pub surface_profile: Option<PathBuf>,
    /// A file to read MIDI bytes from as though a control surface were plugged
    /// in, or `None` for no surface at all.
    ///
    /// The console's counterpart to `--mock-output`, and it exists for the same
    /// reason: `CLAUDE.md` forbids a test from touching a device, and D11 is a
    /// claim about a console changing what an interface shows. With this, that
    /// claim can be *observed* from outside the process — a browser watching a
    /// real daemon while bytes from `docs/MCU_MAPPING.md` §2.1 are appended to
    /// a file. See [`crate::surface::FileSurfacePort`].
    pub mock_surface: Option<PathBuf>,
    /// What the stage does when the daemon stops.
    pub exit: Exit,
    /// How much to log.
    pub log_level: Level,
    /// Stop by itself after this long. For a smoke test and for CI, where a
    /// daemon that runs until interrupted is a job that never ends.
    pub run_for: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            data_dir: None,
            show: None,
            fixtures: None,
            universes: DEFAULT_UNIVERSES,
            outputs: Vec::new(),
            mock_devices: false,
            local: true,
            websocket: None,
            token: None,
            surface_profile: None,
            mock_surface: None,
            exit: Exit::default(),
            log_level: Level::Info,
            run_for: None,
        }
    }
}

/// What the command line asked for, when it was not to start a daemon.
#[derive(Debug, Clone, PartialEq)]
pub enum Invocation {
    /// Start.
    Run(Box<Options>),
    /// Print the help and stop.
    Help,
    /// Print the version and stop.
    Version,
}

/// Why a command line was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError(pub String);

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for CliError {}

/// The help text, which is also the list of options this module parses.
#[must_use]
pub fn usage() -> String {
    format!(
        "\
prismd {} - the PrismDMX engine daemon.

Usage: prismd [OPTIONS]

The daemon owns the show, the engine and every DMX output. It runs with no
client attached and keeps running when one goes away; that is decision D2.

Options:
  --data-dir <PATH>     where the lock file and the machine configuration live
                        (default: the platform's user data directory, or
                        {} if it is set)
  --show <PATH>         the .prism file to open, created if it is not there
  --fixtures <DIR>      the installed fixture library; found beside the
                        executable when this is not given
                        (default: default.prism in the data directory)
  --universes <N>       universes the frame layout carries, 1..={DEFAULT_UNIVERSES}
                        (default: {DEFAULT_UNIVERSES})

  --mock-output         a DMX output that accepts every frame and puts it
                        nowhere - the headless mode the tests run against
  --artnet <ADDR>       an Art-Net output, unicast to ADDR (host:port; the port
                        defaults to {})
  --sacn                an sACN output, multicast, on the show's universes
  --sacn-to <ADDR>      an sACN output unicast to ADDR, for a venue whose
                        network forbids multicast (the port defaults to {})
  --open-dmx <N>        the Open DMX USB adapter, carrying universe N
  --mock-devices        open every configured output with a mock driver
                        instead of the real one, so a rig of adapters and
                        nodes can be driven with nothing plugged in

Outputs named above are the whole rig for this run: the machine configuration
is not read and nothing is written back to it, and AddOutput and its three
companions are refused. A daemon started with none of them uses the rig in
machine.json, which is where a settings window puts it.

  --no-local            do not open the named pipe / Unix domain socket
  --websocket [ADDR]    open the WebSocket listener (default: {DEFAULT_WEBSOCKET})
  --token <TOKEN>       the access token a listener off loopback requires

  --surface-profile <PATH>
                        the X-Touch binding table to read (see
                        profiles/surface/xtouch.json). A profile that is
                        missing or malformed is reported and the built-in
                        bindings are used; it never stops the daemon
  --mock-surface <PATH> a control surface with no device behind it: MIDI bytes
                        appended to PATH are read as though the console had
                        sent them, and the feedback goes nowhere, because a
                        file has no faders. The console half of --mock-output

  --blackout-on-exit    publish a blackout before stopping the outputs
  --hold-on-exit        leave the last look on stage (default)

  --log-level <LEVEL>   debug, info, warn, error or off (default: info)
  --run-for <SECONDS>   stop by itself after this long
  -h, --help            print this and stop
  -V, --version         print the version and stop
",
        env!("CARGO_PKG_VERSION"),
        crate::paths::DATA_DIR_VARIABLE,
        prism_protocols::ART_NET_PORT,
        prism_protocols::E131_PORT,
    )
}

/// Reads a command line, without the program name.
///
/// # Errors
///
/// [`CliError`] naming the argument that was wrong and what was expected. Every
/// message is one an operator can act on: an unknown option says so, and a
/// value that will not parse says what it should have looked like.
pub fn parse<I, S>(arguments: I) -> Result<Invocation, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut options = Options::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let argument = argument.as_ref().to_owned();
        let mut value = || -> Result<String, CliError> {
            arguments
                .next()
                .map(|value| value.as_ref().to_owned())
                .ok_or_else(|| CliError(format!("{argument} needs a value")))
        };
        match argument.as_str() {
            "-h" | "--help" => return Ok(Invocation::Help),
            "-V" | "--version" => return Ok(Invocation::Version),
            "--data-dir" => options.data_dir = Some(PathBuf::from(value()?)),
            "--show" => options.show = Some(PathBuf::from(value()?)),
            "--fixtures" => options.fixtures = Some(PathBuf::from(value()?)),
            "--universes" => {
                let text = value()?;
                let count: u32 = text
                    .parse()
                    .map_err(|_| CliError(format!("--universes wants a number, not {text:?}")))?;
                if count == 0 || count > DEFAULT_UNIVERSES {
                    return Err(CliError(format!(
                        "--universes must be between 1 and {DEFAULT_UNIVERSES}, not {count}"
                    )));
                }
                options.universes = count;
            }
            "--mock-output" => {
                let id = next_id(&options.outputs);
                options.outputs.push(mock_output(id.get()));
            }
            "--mock-devices" => options.mock_devices = true,
            "--artnet" => {
                let text = value()?;
                let target = socket_address(&text, prism_protocols::ART_NET_PORT)?;
                let id = next_id(&options.outputs);
                options.outputs.push(OutputInstance::new(
                    id,
                    format!("Art-Net to {target}"),
                    OutputKind::ArtNet {
                        nodes: vec![target],
                        sync: false,
                        ports: Vec::new(),
                    },
                    [],
                ));
            }
            "--sacn" => {
                let id = next_id(&options.outputs);
                options.outputs.push(OutputInstance::new(
                    id,
                    "sACN",
                    OutputKind::Sacn {
                        receivers: Vec::new(),
                        ttl: DEFAULT_HOP_LIMIT,
                        ports: Vec::new(),
                    },
                    [],
                ));
            }
            "--sacn-to" => {
                let text = value()?;
                let target = socket_address(&text, prism_protocols::E131_PORT)?;
                let id = next_id(&options.outputs);
                options.outputs.push(OutputInstance::new(
                    id,
                    format!("sACN unicast to {target}"),
                    OutputKind::Sacn {
                        receivers: vec![target],
                        ttl: DEFAULT_HOP_LIMIT,
                        ports: Vec::new(),
                    },
                    [],
                ));
            }
            "--open-dmx" => {
                let text = value()?;
                let number: u32 = text
                    .parse()
                    .map_err(|_| CliError(format!("--open-dmx wants a universe, not {text:?}")))?;
                let universe = UniverseId::new(number);
                if !universe.is_in_range() {
                    return Err(CliError(format!(
                        "--open-dmx wants a universe between {} and {}, not {number}",
                        UniverseId::MIN,
                        UniverseId::MAX
                    )));
                }
                let id = next_id(&options.outputs);
                options.outputs.push(OutputInstance::new(
                    id,
                    format!("Open DMX USB on universe {universe}"),
                    OutputKind::OpenDmx { serial: None },
                    [universe],
                ));
            }
            "--no-local" => options.local = false,
            "--websocket" => {
                options.websocket = Some(socket_address(DEFAULT_WEBSOCKET, 0)?);
            }
            "--token" => options.token = Some(value()?),
            "--surface-profile" => options.surface_profile = Some(PathBuf::from(value()?)),
            "--mock-surface" => options.mock_surface = Some(PathBuf::from(value()?)),
            "--blackout-on-exit" => options.exit = Exit::Blackout,
            "--hold-on-exit" => options.exit = Exit::Hold,
            "--log-level" => {
                let text = value()?;
                options.log_level = Level::parse(&text)
                    .ok_or_else(|| CliError(format!("--log-level does not know {text:?}")))?;
            }
            "--run-for" => {
                let text = value()?;
                let seconds: f64 = text
                    .parse()
                    .ok()
                    .filter(|seconds: &f64| seconds.is_finite() && *seconds >= 0.0)
                    .ok_or_else(|| CliError(format!("--run-for wants seconds, not {text:?}")))?;
                options.run_for = Some(Duration::from_secs_f64(seconds));
            }
            // An address may follow --websocket, which is the one option whose
            // value is optional. Anything else beginning with a dash is a
            // mistake, and anything that does not is an argument this program
            // has no use for.
            other => {
                if let Some(websocket) = options.websocket.as_mut()
                    && !other.starts_with('-')
                {
                    *websocket = socket_address(other, 0)?;
                    continue;
                }
                return Err(CliError(format!(
                    "prismd does not know the option {other:?}; try --help"
                )));
            }
        }
    }

    // §2.1, checked before anything is opened rather than after: a listener
    // reachable from another machine requires a token, and a daemon that
    // started without one would be an unauthenticated console on a school
    // network.
    if let Some(address) = options.websocket
        && !address.ip().is_loopback()
        && options.token.is_none()
    {
        return Err(CliError(format!(
            "--websocket {address} is reachable from other machines, so --token is required \
             (docs/IPC_PROTOCOL.md section 2.1)"
        )));
    }

    Ok(Invocation::Run(Box::new(options)))
}

/// A socket address, filling in a default port when only a host was given.
fn socket_address(text: &str, default_port: u16) -> Result<SocketAddr, CliError> {
    if let Ok(address) = text.parse::<SocketAddr>() {
        return Ok(address);
    }
    if default_port > 0
        && let Ok(address) = format!("{text}:{default_port}").parse::<SocketAddr>()
    {
        return Ok(address);
    }
    Err(CliError(format!(
        "{text:?} is not an address; expected something like 192.168.1.50 or 192.168.1.50:6454"
    )))
}

#[cfg(test)]
mod tests {
    use super::{
        CliError, DEFAULT_HOP_LIMIT, DEFAULT_UNIVERSES, DEFAULT_WEBSOCKET, Exit, Invocation,
        Options, fill_universes, parse, usage,
    };
    use crate::log::Level;
    use prism_domain::{OutputKind, UniverseId};
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::time::Duration;

    fn options(arguments: &[&str]) -> Options {
        match parse(arguments) {
            Ok(Invocation::Run(options)) => *options,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn refusal(arguments: &[&str]) -> String {
        match parse(arguments) {
            Err(CliError(message)) => message,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_daemon_with_no_arguments_is_a_daemon_with_defaults() {
        let options = options(&[]);
        assert_eq!(options, Options::default());
        assert!(options.local, "the local transport is what the shell uses");
        assert_eq!(options.websocket, None, "the Web Remote is opt-in");
        assert_eq!(options.exit, Exit::Hold, "the stage keeps its look");
        assert_eq!(options.universes, DEFAULT_UNIVERSES);
        assert!(options.outputs.is_empty());
    }

    #[test]
    fn every_output_kind_can_be_asked_for() {
        let options = options(&[
            "--mock-output",
            "--sacn",
            "--artnet",
            "192.168.1.50",
            "--open-dmx",
            "3",
        ]);
        let kinds: Vec<&OutputKind> = options.outputs.iter().map(|output| &output.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &OutputKind::Mock,
                &OutputKind::Sacn {
                    receivers: Vec::new(),
                    ttl: DEFAULT_HOP_LIMIT,
                    ports: Vec::new(),
                },
                &OutputKind::ArtNet {
                    nodes: vec!["192.168.1.50:6454".parse::<SocketAddr>().unwrap()],
                    sync: false,
                    ports: Vec::new(),
                },
                &OutputKind::OpenDmx { serial: None },
            ],
            "in the order they were asked for"
        );
        // Numbered 1, 2, 3, 4 in that order, because the rig is data now and a
        // row without a number is not one an operator can name.
        let numbers: Vec<u32> = options
            .outputs
            .iter()
            .map(|output| output.id.get())
            .collect();
        assert_eq!(numbers, vec![1, 2, 3, 4]);
    }

    /// A flag has nowhere to put a list of universes, so every kind but one
    /// carries none until the show has been opened — [`fill_universes`] is what
    /// fills them, and `--open-dmx` is the exception because it *is* a universe.
    #[test]
    fn a_flag_names_no_universes_except_the_one_that_is_a_universe() {
        let options = options(&["--mock-output", "--open-dmx", "3"]);
        assert!(options.outputs[0].universes.is_empty());
        assert_eq!(options.outputs[1].universes, vec![UniverseId::new(3)]);

        let filled = fill_universes(&options.outputs, &[UniverseId::new(1), UniverseId::new(2)]);
        assert_eq!(
            filled[0].universes,
            vec![UniverseId::new(1), UniverseId::new(2)],
            "the show's universes"
        );
        assert_eq!(
            filled[1].universes,
            vec![UniverseId::new(3)],
            "and the one the flag named is left alone"
        );
    }

    /// §7.2 names unicast sACN for a venue whose network forbids multicast, and
    /// it is also the only sACN a test suite may open — S10 made "no test sends
    /// multicast" a rule rather than an omission.
    #[test]
    fn sacn_is_multicast_unless_a_receiver_is_named() {
        assert_eq!(
            options(&["--sacn"]).outputs[0].kind,
            OutputKind::Sacn {
                receivers: Vec::new(),
                ttl: DEFAULT_HOP_LIMIT,
                ports: Vec::new(),
            },
            "no receiver named is multicast"
        );
        assert_eq!(
            options(&["--sacn-to", "10.0.0.9"]).outputs[0].kind,
            OutputKind::Sacn {
                receivers: vec!["10.0.0.9:5568".parse::<SocketAddr>().unwrap()],
                ttl: DEFAULT_HOP_LIMIT,
                ports: Vec::new(),
            },
            "the port defaults to E1.31's own"
        );
        assert!(refusal(&["--sacn-to", "somewhere"]).contains("not an address"));
        assert!(refusal(&["--sacn-to"]).contains("needs a value"));
    }

    #[test]
    fn an_artnet_target_may_carry_its_own_port() {
        let options = options(&["--artnet", "10.0.0.2:6455"]);
        assert_eq!(
            options.outputs[0].kind,
            OutputKind::ArtNet {
                nodes: vec!["10.0.0.2:6455".parse::<SocketAddr>().unwrap()],
                sync: false,
                ports: Vec::new(),
            }
        );
        assert_eq!(options.outputs[0].name, "Art-Net to 10.0.0.2:6455");
        assert!(refusal(&["--artnet", "not-an-address"]).contains("192.168.1.50"));
    }

    /// S33: the flag that lets a rig of real kinds be driven with no device.
    #[test]
    fn every_output_can_be_opened_with_a_mock_driver() {
        assert!(!Options::default().mock_devices, "off unless asked for");
        let options = options(&["--mock-devices", "--open-dmx", "1"]);
        assert!(options.mock_devices);
        assert_eq!(
            options.outputs[0].kind,
            OutputKind::OpenDmx { serial: None },
            "the rig is the real configuration; only the last inch is a double"
        );
    }

    #[test]
    fn the_websocket_defaults_to_loopback_and_takes_an_address() {
        assert_eq!(
            options(&["--websocket"]).websocket,
            Some(DEFAULT_WEBSOCKET.parse::<SocketAddr>().unwrap())
        );
        assert_eq!(
            options(&["--websocket", "127.0.0.1:9000"]).websocket,
            Some("127.0.0.1:9000".parse::<SocketAddr>().unwrap())
        );
    }

    /// §2.1, and the reason it is checked here rather than at the listener:
    /// a daemon that has already bound a public port and then complains has
    /// already been reachable.
    #[test]
    fn a_listener_off_loopback_needs_a_token() {
        let message = refusal(&["--websocket", "0.0.0.0:7373"]);
        assert!(message.contains("--token"), "{message}");
        assert!(message.contains("2.1"), "{message}");

        let with_token = options(&["--websocket", "0.0.0.0:7373", "--token", "hunter2"]);
        assert_eq!(with_token.token.as_deref(), Some("hunter2"));

        // And loopback does not, which is what makes the default usable.
        assert!(options(&["--websocket"]).token.is_none());
    }

    #[test]
    fn the_paths_and_the_universe_count_are_taken_as_given() {
        let options = options(&[
            "--data-dir",
            "/srv/prism",
            "--show",
            "/srv/prism/aula.prism",
            "--universes",
            "4",
        ]);
        assert_eq!(options.data_dir, Some(PathBuf::from("/srv/prism")));
        assert_eq!(options.show, Some(PathBuf::from("/srv/prism/aula.prism")));
        assert_eq!(options.universes, 4);
    }

    #[test]
    fn a_universe_count_outside_the_desks_range_is_refused() {
        // A layout of zero universes is a daemon that publishes nothing, and one
        // above the desk's range is a number `FrameLayout` would refuse later
        // and less clearly.
        assert!(refusal(&["--universes", "0"]).contains("between 1"));
        assert!(refusal(&["--universes", "65"]).contains("between 1"));
        assert!(refusal(&["--universes", "lots"]).contains("number"));
        assert!(refusal(&["--open-dmx", "0"]).contains("universe between"));
        assert!(refusal(&["--open-dmx", "seven"]).contains("universe"));
    }

    #[test]
    fn the_exit_behaviour_is_the_last_one_asked_for() {
        assert_eq!(options(&["--blackout-on-exit"]).exit, Exit::Blackout);
        assert_eq!(options(&["--hold-on-exit"]).exit, Exit::Hold);
        assert_eq!(
            options(&["--blackout-on-exit", "--hold-on-exit"]).exit,
            Exit::Hold
        );
    }

    #[test]
    fn the_log_level_and_the_run_time_are_parsed() {
        assert_eq!(options(&["--log-level", "debug"]).log_level, Level::Debug);
        assert_eq!(options(&["--log-level", "OFF"]).log_level, Level::Off);
        assert!(refusal(&["--log-level", "chatty"]).contains("chatty"));

        assert_eq!(
            options(&["--run-for", "1.5"]).run_for,
            Some(Duration::from_millis(1500))
        );
        assert!(refusal(&["--run-for", "soon"]).contains("seconds"));
        assert!(refusal(&["--run-for", "-1"]).contains("seconds"));
        assert!(refusal(&["--run-for", "inf"]).contains("seconds"));
    }

    #[test]
    fn the_local_transport_can_be_switched_off() {
        assert!(!options(&["--no-local"]).local);
    }

    #[test]
    fn help_and_version_stop_before_anything_else_is_read() {
        for argument in ["-h", "--help"] {
            assert_eq!(parse([argument, "--nonsense"]), Ok(Invocation::Help));
        }
        for argument in ["-V", "--version"] {
            assert_eq!(parse([argument, "--nonsense"]), Ok(Invocation::Version));
        }
    }

    #[test]
    fn an_option_that_does_not_exist_says_so_rather_than_being_ignored() {
        let message = refusal(&["--tubro-mode"]);
        assert!(message.contains("tubro"), "{message}");
        assert!(message.contains("--help"), "{message}");
        // And a stray value is not silently swallowed either.
        assert!(refusal(&["aula.prism"]).contains("aula.prism"));
    }

    #[test]
    fn an_option_that_needs_a_value_and_has_none_says_which() {
        for argument in [
            "--data-dir",
            "--show",
            "--universes",
            "--artnet",
            "--open-dmx",
            "--token",
            "--log-level",
            "--run-for",
        ] {
            let message = refusal(&[argument]);
            assert!(message.contains(argument), "{message}");
            assert!(message.contains("needs a value"), "{message}");
        }
    }

    #[test]
    fn a_surface_can_be_asked_for_without_a_device() {
        // The console half of `--mock-output`: a path, not a port. The daemon
        // opens it in `Daemon::start`, and a path that will not open is a
        // warning rather than a refusal — which is why the parser has nothing
        // to say about it here.
        let options = options(&["--mock-surface", "keys.midi"]);
        assert_eq!(options.mock_surface, Some(PathBuf::from("keys.midi")));
        assert_eq!(
            Options::default().mock_surface,
            None,
            "there is no surface by default"
        );
        assert!(refusal(&["--mock-surface"]).contains("needs a value"));
    }

    /// The help is the documentation, so it has to name every option the parser
    /// answers to. A flag that works and is not written down is a flag nobody
    /// uses.
    #[test]
    fn the_help_names_every_option() {
        let usage = usage();
        for option in [
            "--data-dir",
            "--show",
            "--universes",
            "--mock-output",
            "--artnet",
            "--sacn",
            "--open-dmx",
            "--sacn-to",
            "--no-local",
            "--websocket",
            "--token",
            "--mock-devices",
            "--surface-profile",
            "--mock-surface",
            "--blackout-on-exit",
            "--hold-on-exit",
            "--log-level",
            "--run-for",
            "--help",
            "--version",
        ] {
            assert!(usage.contains(option), "the help does not mention {option}");
        }
        assert!(usage.contains(env!("CARGO_PKG_VERSION")));
        assert!(usage.contains(DEFAULT_WEBSOCKET));
        assert_eq!(CliError("no".to_owned()).to_string(), "no");
    }
}
