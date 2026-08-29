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

use prism_domain::{MachineOverride, OutputId, OutputInstance, OutputKind, UniverseId};

use crate::log::Level;

/// The default WebSocket address, and it is loopback for the reason
/// `docs/IPC_PROTOCOL.md` §2.1 gives: school networks are shared, and an
/// unauthenticated lighting console reachable from any classroom machine is not
/// acceptable.
///
/// **Since S37 this is where the listener is by default rather than a value a
/// flag opts in to.** `prism_core::Settings::websocket` holds it, the settings
/// window edits it, and the constant is here so the help text can print it.
pub const DEFAULT_WEBSOCKET: &str = "127.0.0.1:7373";

/// Universes the frame layout carries unless it is told otherwise —
/// `UniverseId::MAX`, i.e. the desk's whole range. See
/// [`crate::engine::frame_layout`] for why it is the desk's and not the show's.
///
/// One number, held in `prism-core` beside the setting that carries it, so that
/// the default a machine configuration is written with and the bound this parser
/// checks against cannot drift apart.
pub const DEFAULT_UNIVERSES: u32 = prism_core::DEFAULT_UNIVERSES;

/// What a command line says about the WebSocket listener — S37.
///
/// Three states rather than an `Option<SocketAddr>`, because since S37 there are
/// three things to say: *use the setting* (the default, and the setting is a
/// loopback listener), *use exactly this address* and *none at all*. Before S37
/// there was no setting, so absence meant off and two states were enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Listen {
    /// Whatever `prism_core::Settings::websocket` says. The default.
    #[default]
    Configured,
    /// `--websocket [ADDR]`: this address, and the setting is not read.
    At(SocketAddr),
    /// `--no-websocket`: nothing at all, and the setting is not read.
    Off,
}

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

/// The domain's spelling and this crate's, converted in one place — S37.
///
/// `crate::log::Level`'s reason exactly: what travels and what the shutdown path
/// switches on are the same fact and two types, because `prism-domain` is not
/// allowed to know what a blackout frame is.
impl From<prism_domain::ExitAction> for Exit {
    fn from(action: prism_domain::ExitAction) -> Self {
        match action {
            prism_domain::ExitAction::Hold => Self::Hold,
            prism_domain::ExitAction::Blackout => Self::Blackout,
        }
    }
}

impl From<Exit> for prism_domain::ExitAction {
    fn from(exit: Exit) -> Self {
        match exit {
            Exit::Hold => Self::Hold,
            Exit::Blackout => Self::Blackout,
        }
    }
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
    /// Universes the frame layout carries, or `None` for the configured count
    /// — S37.
    pub universes: Option<u32>,
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
    /// Whether to listen for Art-Net node discovery — S46.
    ///
    /// On unless it is turned off, and it opens a socket only once the rig has
    /// an Art-Net output. Two reasons an operator turns it off, and neither of
    /// them is *it went wrong*:
    ///
    /// **A second desk on one machine.** Art-Net's port is a fixed number, so
    /// two `prismd` processes on one box cannot both hold it. The second one's
    /// socket does not bind, which is a warning and a daemon that starts (S37's
    /// rule for the WebSocket listener, and the same fixed-port argument) — but
    /// an operator who *meant* to run two should be able to say so rather than
    /// read the warning every morning.
    ///
    /// **A suite must not open a socket it does not use.** The discovery socket
    /// is bound on every interface, because that is where a node's reply is
    /// addressed; the test targets that start a daemon with a real Art-Net
    /// output over loopback therefore turn it off, exactly as they turn off the
    /// WebSocket listener and for the same sentence.
    pub artnet_discovery: bool,
    /// Whether to open the named pipe or Unix domain socket, or `None` for the
    /// configured answer — S37.
    pub local: Option<bool>,
    /// What the command line said about the WebSocket listener — S37.
    pub websocket: Listen,
    /// The §2.1 token, or `None` for the configured one. Required when the
    /// WebSocket listener is not on loopback.
    pub token: Option<String>,
    /// The X-Touch binding profile to read (`docs/MCU_MAPPING.md` §4.2), or
    /// `None` for the built-in defaults.
    ///
    /// A path this daemon may fail to read: a profile that is missing or
    /// malformed produces a warning and the built-in table, never a daemon that
    /// will not start. `IMPLEMENTATION_PLAN.md` S22, and
    /// [`crate::surface::load_profile`] is where it is kept.
    pub surface_profile: Option<PathBuf>,
    /// The MIDI port the control surface is on, or `None` to use whatever the
    /// machine configuration names — S36.
    ///
    /// **The surface for this run when it is given.** The stored port is
    /// neither read nor written, and `Command::SetSurfacePort` is refused, for
    /// the reason `--mock-output` refuses the four output commands (S33): a
    /// daemon started with a port on its command line must not overwrite the
    /// one the venue configured, and a `SetSurfacePort` that appeared to work
    /// and vanished at the next restart would be worse than one that says why.
    ///
    /// A port that is not plugged in is a **warning and a daemon that starts**.
    pub surface: Option<String>,
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
    /// What the stage does when the daemon stops, or `None` for the configured
    /// answer — S37.
    pub exit: Option<Exit>,
    /// How much to log, or `None` for the configured level — S37.
    pub log_level: Option<Level>,
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
            universes: None,
            outputs: Vec::new(),
            mock_devices: false,
            artnet_discovery: true,
            local: None,
            websocket: Listen::Configured,
            token: None,
            surface_profile: None,
            surface: None,
            mock_surface: None,
            exit: None,
            log_level: None,
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
    /// Print the MIDI ports this machine has and stop — S36.
    ///
    /// How a person finds the name to write into `--surface` or into a settings
    /// window, and the plainest form of S36's exit criterion: on a machine with
    /// no MIDI device it prints an empty list and exits **successfully**, rather
    /// than reporting that it could not look.
    MidiPorts,
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
Every option below that is not about *this run* is a **setting** since S37:
the machine configuration holds it, the settings window edits it, and a flag
here is the value for this run only - the stored one is neither read nor
written, and the window says which flag is holding which row.

  --data-dir <PATH>     where the lock file and the machine configuration live
                        (default: the platform's user data directory, or
                        {} if it is set)
  --show <PATH>         the .prism file to open, created if it is not there
                        (default: the one this desk last had open, or
                        default.prism in the data directory)
  --fixtures <DIR>      the installed fixture library; found beside the
                        executable when this is not given
  --universes <N>       universes the frame layout carries, 1..={DEFAULT_UNIVERSES}
                        (default: the configured count, 64 out of the box)

  --mock-output         a DMX output that accepts every frame and puts it
                        nowhere - the headless mode the tests run against
  --artnet <ADDR>       an Art-Net output, unicast to ADDR (host:port; the port
                        defaults to {})
  --sacn                an sACN output, multicast, on the show's universes
  --sacn-to <ADDR>      an sACN output unicast to ADDR, for a venue whose
                        network forbids multicast (the port defaults to {})
  --open-dmx <N>        the Open DMX USB adapter, carrying universe N
  --mock-devices        open every configured output with a mock driver
  --no-artnet-discovery do not listen for Art-Net nodes (a second desk on this machine)
                        instead of the real one, so a rig of adapters and
                        nodes can be driven with nothing plugged in

Outputs named above are the whole rig for this run: the machine configuration
is not read and nothing is written back to it, and AddOutput and its three
companions are refused. A daemon started with none of them uses the rig in
machine.json, which is where a settings window puts it.

  --local, --no-local   open the named pipe / Unix domain socket, or do not
                        (default: the configured answer, open out of the box)
  --websocket [ADDR]    bind the WebSocket listener here
  --no-websocket        do not bind it at all
                        (default: the configured address, {DEFAULT_WEBSOCKET} out of
                        the box - a listener that cannot bind is a warning and
                        a daemon that starts)
  --token <TOKEN>       the access token a listener off loopback requires

  --surface-profile <PATH>
                        the X-Touch binding table to read (see
                        profiles/surface/xtouch.json). A profile that is
                        missing or malformed is reported and the built-in
                        bindings are used; it never stops the daemon
  --surface <PORT>      the MIDI port the control surface is on, by name. A
                        port that is not plugged in is a warning and a daemon
                        that starts, and one plugged in later is picked up
                        without a restart. This is the surface for the run:
                        the configured port is neither read nor written, and
                        SetSurfacePort is refused
  --midi-ports          print the MIDI ports this machine has and stop
  --mock-surface <PATH> a control surface with no device behind it: MIDI bytes
                        appended to PATH are read as though the console had
                        sent them, and the feedback goes nowhere, because a
                        file has no faders. The console half of --mock-output

  --blackout-on-exit    publish a blackout before stopping the outputs
  --hold-on-exit        leave the last look on stage
                        (default: the configured action, hold out of the box)

  --log-level <LEVEL>   debug, info, warn, error or off
                        (default: the configured level, info out of the box)
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

/// What `--midi-ports` prints — S36.
///
/// Here rather than in `main.rs` because it is arithmetic over a list and
/// `main.rs` has no test target: a two-line binary is the one thing in this
/// crate that cannot be asserted, so as little as possible goes in it.
///
/// A machine with nothing plugged in prints the backend and **`no MIDI ports`**,
/// and the exit code is success — *there is no MIDI device here* is an answer
/// rather than a failure to look, which is one of S36's exit criteria and is
/// also what CI runs.
#[must_use]
pub fn midi_port_report(listed: &prism_midi::PortList) -> String {
    use core::fmt::Write as _;

    let mut out = format!(
        "MIDI backend: {}
",
        prism_midi::backend_name()
    );
    if listed.is_empty() {
        out.push_str(
            "no MIDI ports
",
        );
        return out;
    }
    for name in listed.names() {
        // Both directions are marked because a surface needs both: a
        // synthesiser is output only and a keyboard with no lamps is input
        // only, and an operator looking for their desk in this list has to be
        // able to see why the wrong row is the wrong row.
        let directions = match (
            listed.inputs.contains(&name),
            listed.outputs.contains(&name),
        ) {
            (true, true) => "in+out",
            (true, false) => "in    ",
            (false, true) => "   out",
            (false, false) => "      ",
        };
        let _ = writeln!(out, "  [{directions}] {name}");
    }
    out
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
                options.universes = Some(count);
            }
            "--mock-output" => {
                let id = next_id(&options.outputs);
                options.outputs.push(mock_output(id.get()));
            }
            "--mock-devices" => options.mock_devices = true,
            "--no-artnet-discovery" => options.artnet_discovery = false,
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
            "--no-local" => options.local = Some(false),
            "--local" => options.local = Some(true),
            "--websocket" => {
                options.websocket = Listen::At(socket_address(DEFAULT_WEBSOCKET, 0)?);
            }
            "--no-websocket" => options.websocket = Listen::Off,
            "--token" => options.token = Some(value()?),
            "--surface-profile" => options.surface_profile = Some(PathBuf::from(value()?)),
            "--surface" => options.surface = Some(value()?),
            "--midi-ports" => return Ok(Invocation::MidiPorts),
            "--mock-surface" => options.mock_surface = Some(PathBuf::from(value()?)),
            "--blackout-on-exit" => options.exit = Some(Exit::Blackout),
            "--hold-on-exit" => options.exit = Some(Exit::Hold),
            "--log-level" => {
                let text = value()?;
                options.log_level = Some(
                    Level::parse(&text)
                        .ok_or_else(|| CliError(format!("--log-level does not know {text:?}")))?,
                );
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
                if matches!(options.websocket, Listen::At(_)) && !other.starts_with('-') {
                    options.websocket = Listen::At(socket_address(other, 0)?);
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
    if let Listen::At(address) = options.websocket
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

/// What the daemon actually runs with, once the command line and the machine
/// configuration have been put together — S37.
///
/// # Why this is one function and not fifteen `unwrap_or`s
///
/// The rule is S33's and S36's, generalised: **a flag is the value for that
/// run**, and the stored setting is neither read nor written. Written inline at
/// each use that rule would be fifteen places to get it wrong, and — worse — the
/// list of which settings a run is holding would have to be built a second time
/// for the settings window to grey the right rows out. So it is decided once,
/// here, and [`Resolved::overrides`] falls out of the same match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Whether to open the named pipe / Unix domain socket.
    pub local: bool,
    /// Where the WebSocket listener should bind, or `None`.
    pub websocket: Option<SocketAddr>,
    /// The §2.1 token.
    pub token: Option<String>,
    /// Universes the frame layout carries.
    pub universes: u32,
    /// How much to log.
    pub log_level: Level,
    /// What the stage does when the daemon stops.
    pub exit: Exit,
    /// Where the installed fixture library is.
    pub fixtures: Option<PathBuf>,
    /// Which X-Touch binding profile to read.
    pub surface_profile: Option<PathBuf>,
    /// Which settings this run's command line is holding — what a settings
    /// window greys out, and the flag it names when it does.
    pub overrides: Vec<MachineOverride>,
}

/// Puts a command line and a machine configuration together — S37.
///
/// `outputs_from_flags` and `surface_from_flag` are the two S33 and S36 already
/// decided; they are arguments rather than being read off `options` because the
/// rig's answer is `crate::daemon::rig_for`'s and this function must not be a
/// second opinion about it.
#[must_use]
pub fn resolve(
    options: &Options,
    settings: &prism_core::Settings,
    outputs_from_flags: bool,
    surface_from_flag: bool,
) -> Resolved {
    let mut overrides = Vec::new();
    let mut held = |what: MachineOverride| overrides.push(what);
    if outputs_from_flags {
        held(MachineOverride::Outputs);
    }
    if surface_from_flag {
        held(MachineOverride::Surface);
    }

    let local = match options.local {
        Some(local) => {
            held(MachineOverride::Local);
            local
        }
        None => settings.local,
    };
    let websocket = match options.websocket {
        Listen::At(address) => {
            held(MachineOverride::Websocket);
            Some(address)
        }
        Listen::Off => {
            held(MachineOverride::Websocket);
            None
        }
        Listen::Configured => settings.websocket,
    };
    let token = match &options.token {
        Some(token) => {
            held(MachineOverride::Token);
            Some(token.clone())
        }
        None => settings.token.clone(),
    };
    let universes = match options.universes {
        Some(universes) => {
            held(MachineOverride::Universes);
            universes
        }
        None => settings.universes,
    };
    let log_level = match options.log_level {
        Some(level) => {
            held(MachineOverride::LogLevel);
            level
        }
        None => Level::from(settings.log_level),
    };
    let exit = match options.exit {
        Some(exit) => {
            held(MachineOverride::ExitAction);
            exit
        }
        None => Exit::from(settings.exit_action),
    };
    let fixtures = match &options.fixtures {
        Some(path) => {
            held(MachineOverride::FixtureLibrary);
            Some(path.clone())
        }
        None => settings.fixture_library.as_deref().map(PathBuf::from),
    };
    let surface_profile = match &options.surface_profile {
        Some(path) => {
            held(MachineOverride::SurfaceProfile);
            Some(path.clone())
        }
        None => settings.surface_profile.as_deref().map(PathBuf::from),
    };

    Resolved {
        local,
        websocket,
        token,
        universes,
        log_level,
        exit,
        fixtures,
        surface_profile,
        overrides,
    }
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
        CliError, DEFAULT_HOP_LIMIT, DEFAULT_WEBSOCKET, Exit, Invocation, Listen, Options,
        Resolved, fill_universes, parse, resolve, usage,
    };
    use crate::log::Level;
    use prism_domain::{OutputKind, UniverseId};
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::midi_port_report;

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

    /// The default machine configuration, so a `resolve` test is about the
    /// flags rather than about the settings.
    fn settings() -> prism_core::Settings {
        prism_core::Settings::default()
    }

    /// A command line that says nothing leaves every setting to the machine
    /// configuration, and **holds nothing** — S37.
    ///
    /// The second half is what a settings window draws from: an empty
    /// `overrides` is a panel with no greyed-out rows in it.
    #[test]
    fn a_silent_command_line_takes_the_configuration_as_it_stands() {
        let settings = prism_core::Settings {
            local: false,
            websocket: Some("127.0.0.1:9001".parse().unwrap()),
            token: Some("stored".to_owned()),
            log_level: prism_domain::LogLevel::Warn,
            universes: 12,
            exit_action: prism_domain::ExitAction::Blackout,
            autostart: true,
            fixture_library: Some("D:/fixtures".to_owned()),
            surface_profile: Some("D:/xtouch.json".to_owned()),
        };
        let resolved = resolve(&Options::default(), &settings, false, false);
        assert_eq!(
            resolved,
            Resolved {
                local: false,
                websocket: Some("127.0.0.1:9001".parse().unwrap()),
                token: Some("stored".to_owned()),
                universes: 12,
                log_level: Level::Warn,
                exit: Exit::Blackout,
                fixtures: Some(PathBuf::from("D:/fixtures")),
                surface_profile: Some(PathBuf::from("D:/xtouch.json")),
                overrides: Vec::new(),
            }
        );
    }

    /// **A flag is the value for that run** — S33's rule and S36's, generalised
    /// — and every one of them says so in `overrides`.
    ///
    /// The list is what the settings window greys rows out from, and it names
    /// the flag: a box an operator can type into that does nothing is worse than
    /// a box that is not there.
    #[test]
    fn every_flag_holds_its_setting_and_says_which_flag_is_holding_it() {
        let options = Options {
            local: Some(true),
            websocket: Listen::At("0.0.0.0:7000".parse().unwrap()),
            token: Some("from-the-flag".to_owned()),
            universes: Some(4),
            log_level: Some(Level::Debug),
            exit: Some(Exit::Blackout),
            fixtures: Some(PathBuf::from("/opt/fixtures")),
            surface_profile: Some(PathBuf::from("/opt/xtouch.json")),
            ..Options::default()
        };
        let resolved = resolve(&options, &settings(), true, true);

        assert!(resolved.local);
        assert_eq!(
            resolved.websocket,
            Some("0.0.0.0:7000".parse::<SocketAddr>().unwrap())
        );
        assert_eq!(resolved.token.as_deref(), Some("from-the-flag"));
        assert_eq!(resolved.universes, 4);
        assert_eq!(resolved.log_level, Level::Debug);
        assert_eq!(resolved.exit, Exit::Blackout);
        assert_eq!(resolved.fixtures, Some(PathBuf::from("/opt/fixtures")));

        // Every override there is, including the two S33 and S36 decided.
        use prism_domain::MachineOverride as Held;
        for held in [
            Held::Outputs,
            Held::Surface,
            Held::Local,
            Held::Websocket,
            Held::Token,
            Held::LogLevel,
            Held::Universes,
            Held::ExitAction,
            Held::FixtureLibrary,
            Held::SurfaceProfile,
        ] {
            assert!(resolved.overrides.contains(&held), "{held:?}");
        }
    }

    /// `--no-websocket` is an override too, and that is the point of it.
    ///
    /// Before S37 there was no setting, so *absence* meant off. Now the setting
    /// says *loopback*, so saying nothing is not the same as saying no — and a
    /// run that says no is holding the setting exactly as one that names an
    /// address is.
    #[test]
    fn saying_no_listener_is_different_from_saying_nothing() {
        let off = resolve(
            &Options {
                websocket: Listen::Off,
                ..Options::default()
            },
            &settings(),
            false,
            false,
        );
        assert_eq!(off.websocket, None);
        assert!(
            off.overrides
                .contains(&prism_domain::MachineOverride::Websocket)
        );

        let silent = resolve(&Options::default(), &settings(), false, false);
        assert_eq!(
            silent.websocket.map(|address| address.to_string()),
            Some(DEFAULT_WEBSOCKET.to_owned()),
            "the desk's own setting, and since S37 that is a loopback listener"
        );
        assert!(silent.overrides.is_empty());
    }

    #[test]
    fn a_daemon_with_no_arguments_is_a_daemon_with_defaults() {
        let options = options(&[]);
        assert_eq!(options, Options::default());
        // **Every one of these is now *unset* rather than a value** — S37. A
        // command line that says nothing about a setting is a command line that
        // leaves it to the machine configuration, and `resolve` is where the two
        // are put together. Before S37 there was no configuration, so a default
        // here was the whole answer.
        assert_eq!(options.local, None);
        assert_eq!(options.websocket, Listen::Configured);
        assert_eq!(options.exit, None);
        assert_eq!(options.universes, None);
        assert_eq!(options.log_level, None);
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
    /// Art-Net discovery is on by default and can be turned off by name — S46.
    #[test]
    fn art_net_discovery_is_on_unless_a_second_desk_says_otherwise() {
        assert!(
            Options::default().artnet_discovery,
            "an installer should not have to ask to be told whether their nodes answer"
        );
        assert!(!options(&["--no-artnet-discovery"]).artnet_discovery);
    }

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
            Listen::At(DEFAULT_WEBSOCKET.parse::<SocketAddr>().unwrap())
        );
        assert_eq!(
            options(&["--websocket", "127.0.0.1:9000"]).websocket,
            Listen::At("127.0.0.1:9000".parse::<SocketAddr>().unwrap())
        );
        // And the flag that says *not at all*, which S37 needed because the
        // setting says *yes* — before it, absence was how you said no.
        assert_eq!(options(&["--no-websocket"]).websocket, Listen::Off);
        assert_eq!(options(&["--no-local"]).local, Some(false));
        assert_eq!(options(&["--local"]).local, Some(true));
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
        assert_eq!(options.universes, Some(4));
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
        assert_eq!(options(&["--blackout-on-exit"]).exit, Some(Exit::Blackout));
        assert_eq!(options(&["--hold-on-exit"]).exit, Some(Exit::Hold));
        assert_eq!(
            options(&["--blackout-on-exit", "--hold-on-exit"]).exit,
            Some(Exit::Hold)
        );
    }

    #[test]
    fn the_log_level_and_the_run_time_are_parsed() {
        assert_eq!(
            options(&["--log-level", "debug"]).log_level,
            Some(Level::Debug)
        );
        assert_eq!(options(&["--log-level", "OFF"]).log_level, Some(Level::Off));
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
        assert_eq!(options(&["--no-local"]).local, Some(false));
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

    /// What `--midi-ports` prints, on a machine with nothing plugged in and on
    /// one with a desk on it.
    #[test]
    fn the_port_listing_says_what_there_is_and_which_way_each_one_goes() {
        use prism_midi::PortList;

        // The exit criterion in one line: an empty list is an answer.
        let nothing = midi_port_report(&PortList::default());
        assert!(nothing.contains("no MIDI ports"), "{nothing}");
        assert!(nothing.contains("MIDI backend:"), "{nothing}");

        let listed = PortList {
            inputs: vec!["2- X-Touch".to_owned(), "nanoKEY".to_owned()],
            outputs: vec!["2- X-Touch".to_owned(), "Wavetable Synth".to_owned()],
        };
        let report = midi_port_report(&listed);
        assert!(report.contains("[in+out] 2- X-Touch"), "{report}");
        assert!(report.contains("[in    ] nanoKEY"), "{report}");
        assert!(report.contains("[   out] Wavetable Synth"), "{report}");
        assert!(!report.contains("no MIDI ports"), "{report}");
        // Inputs first, and a port that is both is offered once.
        assert_eq!(report.matches("X-Touch").count(), 1, "{report}");

        // And the real machine's, whatever it is: it answers, and nothing is
        // opened to answer it.
        let real = midi_port_report(&prism_midi::ports());
        assert!(real.starts_with("MIDI backend: "), "{real}");
    }

    /// S36's two: the port a real surface is on, and the way a person finds its
    /// name.
    #[test]
    fn a_real_surface_is_named_by_its_port_and_the_ports_can_be_listed() {
        let chosen = options(&["--surface", "2- X-Touch"]);
        assert_eq!(chosen.surface, Some("2- X-Touch".to_owned()));
        assert_eq!(
            Options::default().surface,
            None,
            "a daemon with no --surface uses whatever machine.json names"
        );
        assert!(refusal(&["--surface"]).contains("needs a value"));

        // The listing stops before anything else is read, like --help: it is a
        // question about the machine rather than a daemon to start.
        assert_eq!(
            parse(["--midi-ports", "--nonsense"]),
            Ok(Invocation::MidiPorts)
        );

        // A port name is taken as it stands. Whether it selects anything is
        // `prism_midi`'s rule and not the parser's — a name with a space, a
        // hyphen or a bracket in it is what Windows and ALSA actually produce.
        for name in [
            "X-Touch",
            "MIDIIN2 (X-Touch)",
            "X-Touch:X-Touch MIDI 1 24:0",
        ] {
            assert_eq!(options(&["--surface", name]).surface, Some(name.to_owned()));
        }
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
            "--surface",
            "--midi-ports",
            "--open-dmx",
            "--sacn-to",
            "--no-local",
            "--websocket",
            "--token",
            "--mock-devices",
            "--no-artnet-discovery",
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
