//! The output patch: which universes leave this building by which cable — S33.
//!
//! # The third applier, and why the rig is not show content
//!
//! [`crate::Show`] answers show commands and [`crate::SessionState`] answers
//! session commands. S33 adds a fourth model with a third applier, and the
//! argument for it is `crate::desk`'s, one step further along:
//!
//! **A show carried to another hall on a stick must not bring the first hall's
//! cabling with it.** The sACN CID is in [`MachineConfig`] because nothing in a
//! `.prism` file can tell *this is the same desk* from *this is a copy*; the
//! output patch is there for the same reason and one more besides — the rig is a
//! property of the building. The school's template for next term names an
//! Art-Net node at `192.168.1.50` that the hall it is opened in has never heard
//! of, and a show that carried it would silently point a universe at nothing.
//!
//! The session is not a home for it either, because the session is *persisted
//! with the show* (`ARCHITECTURE_SPEC.md` §4.1) and would carry the cabling by
//! the same route.
//!
//! So `prism_domain::Command::is_machine_command` is the predicate the daemon
//! routes on, [`MachineConfig::apply`] is where the four commands land, and both
//! of the other appliers refuse them by name. `outputs_are_not_show_content` is
//! the asserted half, written after `desk_id_is_not_show_content`.
//!
//! # What is validated here and what is not
//!
//! Everything that can be decided from the row alone: an Open DMX adapter
//! carries exactly one universe because that is what the cable is
//! (`ARCHITECTURE_SPEC.md` §7.1); an Art-Net output with no node addresses would
//! unicast to nobody; an sACN priority above 200 is refused rather than clamped,
//! because a desk that quietly lowered a number an operator typed would take
//! over a rig it was told not to.
//!
//! What is **not** decided here is whether the device is there. A cable that is
//! not plugged in is an ordinary state of a correct configuration — the driver
//! retries with backoff and the status light is red — and a daemon that refused
//! the row would make an operator unable to configure a rig before the get-in.

use core::fmt;

use prism_domain::{
    ArtNetPort, Command, Delta, MachineChange, OutputChange, OutputId, OutputInstance, OutputKind,
    SacnPort, UniverseId,
};

use crate::command::{Applied, Effect};
use crate::desk::MachineConfig;

/// Why a command against the output patch was refused.
///
/// **Not merged into [`crate::ShowError`]**: these are refusals about the
/// building rather than about the show, and a caller that could not tell them
/// apart would report *the show refused it* for a mistyped hop limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineError {
    /// A show or session command reached the machine applier.
    /// `Command::is_machine_command` is the predicate a daemon routes on.
    NotAMachineCommand,
    /// This daemon's rig was named on its command line, so it is not the
    /// machine configuration's to change.
    ///
    /// Refused rather than applied to a rig nothing will write down: a daemon
    /// started with `--mock-output` — every test in this repository — must
    /// neither inherit a venue's cabling nor overwrite it, and an `AddOutput`
    /// that appeared to work and vanished at the next restart would be worse
    /// than one that says why.
    ConfiguredOnTheCommandLine,
    /// This daemon's control surface was named on its command line, so which
    /// port it is on is not the machine configuration's to change — S36.
    ///
    /// The same rule as [`Self::ConfiguredOnTheCommandLine`] one cable along,
    /// and separate from it because the message has to name the right flag: a
    /// daemon may perfectly well have its rig from a file and its surface from
    /// `--surface`, and an operator told the wrong one would go looking in the
    /// wrong place.
    SurfaceOnTheCommandLine,
    /// No output carries that number.
    UnknownOutput(OutputId),
    /// That number is already an output. An add that replaced a running node
    /// would take a universe off stage without saying so.
    OutputExists(OutputId),
    /// An output that carries nothing sends nothing, which looks exactly like a
    /// broken cable to whoever is setting the rig up.
    NoUniverses,
    /// A universe outside the desk's `1..=64`.
    UniverseOutOfRange(UniverseId),
    /// The same universe twice on one output — two frame positions for one
    /// wire, which is a mistake rather than a doubling.
    DuplicateUniverse(UniverseId),
    /// An Open DMX adapter was given more than one universe. It is one DMX
    /// line: `ARCHITECTURE_SPEC.md` §7.1.
    OneUniversePerAdapter {
        /// How many it was given.
        universes: usize,
    },
    /// An Art-Net output with nowhere to send. Unicast is the default and
    /// broadcast is opt-in by name (§7.2), so an empty node list is an output
    /// that transmits into the void.
    NoNodes,
    /// A port row names a universe this output does not carry.
    PortNotCarried(UniverseId),
    /// A port row names the same universe twice.
    DuplicatePort(UniverseId),
    /// An Art-Net row outside the fields the specification gives it.
    BadPortAddress(ArtNetPort),
    /// An sACN row outside the ranges E1.31 gives it.
    BadSacnPort(SacnPort),
    /// A multicast hop limit of zero, which is a datagram that leaves no
    /// machine at all.
    ZeroHopLimit,
    /// A universe count outside the desk's `1..=64` — S37.
    UniverseCountOutOfRange(u32),
    /// A WebSocket listener asked for an address that is **not** loopback,
    /// while this desk has no §2.1 token — S37.
    ///
    /// The whole of the network-exposure rule, refused here rather than in an
    /// interface: a second client could otherwise put an unauthenticated
    /// lighting console on a school's network, which is the thing
    /// `docs/IPC_PROTOCOL.md` §2.1 exists to prevent. The remedy is one command
    /// — `MachineChange::NewToken` — and the message says so.
    NoTokenForNetwork(std::net::SocketAddr),
}

impl fmt::Display for MachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAMachineCommand => {
                write!(f, "this is not a command about this machine's outputs")
            }
            Self::ConfiguredOnTheCommandLine => write!(
                f,
                "this daemon's outputs were named on its command line; \
                 start it without them to configure the rig from an interface"
            ),
            Self::SurfaceOnTheCommandLine => write!(
                f,
                "this daemon's control surface was named on its command line;                  start it without --surface to configure the port from an interface"
            ),
            Self::UnknownOutput(id) => write!(f, "there is no output {id}"),
            Self::OutputExists(id) => write!(f, "output {id} already exists"),
            Self::NoUniverses => write!(f, "an output has to carry at least one universe"),
            Self::UniverseOutOfRange(universe) => write!(
                f,
                "universe {universe} is outside {}..={}",
                UniverseId::MIN,
                UniverseId::MAX
            ),
            Self::DuplicateUniverse(universe) => {
                write!(f, "universe {universe} is on this output twice")
            }
            Self::OneUniversePerAdapter { universes } => write!(
                f,
                "an Open DMX adapter carries exactly one universe, not {universes}"
            ),
            Self::NoNodes => write!(f, "an Art-Net output needs at least one node address"),
            Self::PortNotCarried(universe) => write!(
                f,
                "there is a port for universe {universe}, which this output does not carry"
            ),
            Self::DuplicatePort(universe) => {
                write!(f, "universe {universe} has two ports on this output")
            }
            Self::BadPortAddress(port) => write!(
                f,
                "net {}, sub-net {}, universe {} is not an Art-Net port address \
                 (0..={}, 0..={}, 0..={})",
                port.net,
                port.sub_net,
                port.port,
                ArtNetPort::MAX_NET,
                ArtNetPort::MAX_SUB_NET,
                ArtNetPort::MAX_PORT
            ),
            Self::BadSacnPort(port) => write!(
                f,
                "E1.31 universe {} at priority {} is out of range ({}..={}, 0..={})",
                port.sacn_universe,
                port.priority,
                SacnPort::MIN_UNIVERSE,
                SacnPort::MAX_UNIVERSE,
                SacnPort::MAX_PRIORITY
            ),
            Self::ZeroHopLimit => write!(f, "a hop limit of 0 leaves the machine it was sent from"),
            Self::UniverseCountOutOfRange(universes) => write!(
                f,
                "a desk carries between 1 and {} universes, not {universes}",
                UniverseId::MAX
            ),
            Self::NoTokenForNetwork(address) => write!(
                f,
                "{address} is reachable from other machines, so it needs an access token:                  make one first"
            ),
        }
    }
}

impl core::error::Error for MachineError {}

/// Checks everything about one output that can be decided from the row alone.
///
/// # Errors
///
/// [`MachineError`] naming the field that is wrong, in words an operator can
/// act on.
pub fn validate(output: &OutputInstance) -> Result<(), MachineError> {
    if output.universes.is_empty() {
        return Err(MachineError::NoUniverses);
    }
    let mut seen = Vec::with_capacity(output.universes.len());
    for universe in &output.universes {
        if !universe.is_in_range() {
            return Err(MachineError::UniverseOutOfRange(*universe));
        }
        if seen.contains(universe) {
            return Err(MachineError::DuplicateUniverse(*universe));
        }
        seen.push(*universe);
    }
    if !output.kind.carries_many() && output.universes.len() > 1 {
        return Err(MachineError::OneUniversePerAdapter {
            universes: output.universes.len(),
        });
    }
    match &output.kind {
        OutputKind::Mock | OutputKind::OpenDmx { .. } => Ok(()),
        OutputKind::ArtNet { nodes, ports, .. } => {
            if nodes.is_empty() {
                return Err(MachineError::NoNodes);
            }
            let mut mapped = Vec::with_capacity(ports.len());
            for port in ports {
                if !output.carries(port.universe) {
                    return Err(MachineError::PortNotCarried(port.universe));
                }
                if mapped.contains(&port.universe) {
                    return Err(MachineError::DuplicatePort(port.universe));
                }
                if !port.is_in_range() {
                    return Err(MachineError::BadPortAddress(*port));
                }
                mapped.push(port.universe);
            }
            Ok(())
        }
        OutputKind::Sacn { ttl, ports, .. } => {
            if *ttl == 0 {
                return Err(MachineError::ZeroHopLimit);
            }
            let mut mapped = Vec::with_capacity(ports.len());
            for port in ports {
                if !output.carries(port.universe) {
                    return Err(MachineError::PortNotCarried(port.universe));
                }
                if mapped.contains(&port.universe) {
                    return Err(MachineError::DuplicatePort(port.universe));
                }
                if !port.is_in_range() {
                    return Err(MachineError::BadSacnPort(*port));
                }
                mapped.push(port.universe);
            }
            Ok(())
        }
    }
}

/// Applies one of S33's four commands to the output patch.
///
/// # Errors
///
/// [`MachineError`]. **The configuration is unchanged after any error** —
/// everything is validated before anything is written, which is the rule
/// `Show::apply` follows and which `tests/outputs.rs` asserts on the serialised
/// bytes rather than on the claim.
pub(crate) fn apply(
    config: &mut MachineConfig,
    command: &Command,
) -> Result<Applied, MachineError> {
    match command {
        Command::AddOutput { output } => {
            if config.output(output.id).is_some() {
                return Err(MachineError::OutputExists(output.id));
            }
            validate(output)?;
            config.insert_output(output.clone());
            Ok(changed(config))
        }
        Command::ConfigureOutput { id, change } => {
            let existing = config
                .output(*id)
                .ok_or(MachineError::UnknownOutput(*id))?
                .clone();
            let mut next = existing;
            match change {
                OutputChange::Name { name } => next.name.clone_from(name),
                OutputChange::Kind { kind } => next.kind = kind.clone(),
                OutputChange::Universes { universes } => next.universes.clone_from(universes),
            }
            validate(&next)?;
            config.insert_output(next);
            Ok(changed(config))
        }
        Command::RemoveOutput { id } => {
            if config.remove_output(*id).is_none() {
                return Err(MachineError::UnknownOutput(*id));
            }
            Ok(changed(config))
        }
        Command::SetOutputEnabled { id, enabled } => {
            let mut next = config
                .output(*id)
                .ok_or(MachineError::UnknownOutput(*id))?
                .clone();
            next.enabled = *enabled;
            config.insert_output(next);
            Ok(changed(config))
        }
        // **A port name is not validated against a cable being plugged in**,
        // for the reason the module documentation gives about outputs: a desk
        // that is switched off, or a show being prepared a week before the
        // get-in, is an ordinary state of a correct configuration. Whether the
        // port is there is the daemon's business, and its answer is a warning.
        Command::SetSurfacePort { port } => {
            config.set_surface_port(port.as_deref());
            Ok(Applied {
                deltas: vec![Delta::SurfaceChanged {
                    port: config.surface_port().map(str::to_owned),
                }],
                effects: vec![Effect::Surface],
            })
        }
        // S37. The delta is **not** built here, unlike every other machine
        // command's: half of what `Delta::MachineChanged` carries is the
        // daemon's knowledge rather than the configuration's — where its data
        // directory is, what is actually listening, and which settings this
        // run's command line is holding. So this answers with the effect alone
        // and `prismd` says what changed.
        Command::ConfigureMachine { change } => {
            config.configure(change)?;
            let mut effects = Vec::new();
            match change {
                MachineChange::NewIdentity => effects.push(Effect::NewDeskIdentity),
                MachineChange::NewToken => effects.push(Effect::NewToken),
                _ => {}
            }
            effects.push(Effect::Machine);
            Ok(Applied {
                deltas: Vec::new(),
                effects,
            })
        }
        _ => Err(MachineError::NotAMachineCommand),
    }
}

/// The one answer every machine command gives: the rig as it now stands, and a
/// note to whoever owns the driver threads.
///
/// Sent whole rather than as a patch — see `prism_domain::Delta::OutputsChanged`
/// — and the effect is separate from the delta because the two go to different
/// places: the delta to every client, the effect to the one thread that owns the
/// cables.
fn changed(config: &MachineConfig) -> Applied {
    Applied {
        deltas: vec![Delta::OutputsChanged {
            outputs: config.outputs().to_vec(),
        }],
        effects: vec![Effect::Outputs],
    }
}

#[cfg(test)]
mod tests {
    use super::{MachineError, apply, validate};
    use crate::desk::MachineConfig;
    use prism_domain::{
        ArtNetPort, Command, Delta, OutputChange, OutputId, OutputInstance, OutputKind, SacnPort,
        UniverseId,
    };

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    fn mock(id: u32, universes: &[u32]) -> OutputInstance {
        OutputInstance::new(
            OutputId::new(id),
            format!("Output {id}"),
            OutputKind::Mock,
            universes.iter().copied().map(UniverseId::new),
        )
    }

    fn art_net(id: u32, universes: &[u32], ports: Vec<ArtNetPort>) -> OutputInstance {
        OutputInstance::new(
            OutputId::new(id),
            "Node",
            OutputKind::ArtNet {
                nodes: vec!["10.0.0.9:6454".parse().unwrap()],
                sync: false,
                ports,
            },
            universes.iter().copied().map(UniverseId::new),
        )
    }

    fn sacn(id: u32, universes: &[u32], ttl: u32, ports: Vec<SacnPort>) -> OutputInstance {
        OutputInstance::new(
            OutputId::new(id),
            "Gateway",
            OutputKind::Sacn {
                receivers: Vec::new(),
                ttl,
                ports,
            },
            universes.iter().copied().map(UniverseId::new),
        )
    }

    fn add(config: &mut MachineConfig, output: OutputInstance) -> Result<(), MachineError> {
        apply(config, &Command::AddOutput { output }).map(drop)
    }

    #[test]
    fn an_output_is_added_under_the_number_it_was_given() {
        let mut config = MachineConfig::default();
        assert!(config.outputs().is_empty());

        let applied = apply(
            &mut config,
            &Command::AddOutput {
                output: mock(3, &[1, 2]),
            },
        )
        .unwrap();
        assert_eq!(
            applied.deltas,
            vec![Delta::OutputsChanged {
                outputs: vec![mock(3, &[1, 2])]
            }]
        );
        assert_eq!(applied.effects, vec![crate::Effect::Outputs]);
        assert_eq!(config.output(OutputId::new(3)), Some(&mock(3, &[1, 2])));
    }

    /// The patch is answered in number order however it was built, because a
    /// settings panel draws it in that order and a delta that reordered rows
    /// would move them under an operator's hand.
    #[test]
    fn the_rig_is_answered_in_output_number_order() {
        let mut config = MachineConfig::default();
        for id in [5, 1, 3] {
            add(&mut config, mock(id, &[1])).unwrap();
        }
        let numbers: Vec<u32> = config
            .outputs()
            .iter()
            .map(|output| output.id.get())
            .collect();
        assert_eq!(numbers, vec![1, 3, 5]);
    }

    #[test]
    fn a_number_that_is_taken_is_refused_rather_than_overwritten() {
        let mut config = MachineConfig::default();
        add(&mut config, mock(1, &[1])).unwrap();
        assert_eq!(
            add(&mut config, mock(1, &[7])),
            Err(MachineError::OutputExists(OutputId::new(1)))
        );
        assert_eq!(
            config.output(OutputId::new(1)).unwrap().universes,
            vec![universe(1)],
            "the running output kept its universe"
        );
    }

    #[test]
    fn one_field_changes_at_a_time_and_the_rest_stands() {
        let mut config = MachineConfig::default();
        add(&mut config, mock(1, &[1])).unwrap();

        apply(
            &mut config,
            &Command::ConfigureOutput {
                id: OutputId::new(1),
                change: OutputChange::Name {
                    name: "Hall dimmers".to_owned(),
                },
            },
        )
        .unwrap();
        apply(
            &mut config,
            &Command::ConfigureOutput {
                id: OutputId::new(1),
                change: OutputChange::Universes {
                    universes: vec![universe(4), universe(5)],
                },
            },
        )
        .unwrap();

        let output = config.output(OutputId::new(1)).unwrap();
        assert_eq!(output.name, "Hall dimmers");
        assert_eq!(output.universes, vec![universe(4), universe(5)]);
        assert_eq!(output.kind, OutputKind::Mock, "and the kind is untouched");
    }

    #[test]
    fn an_output_can_be_stopped_and_started_without_losing_its_configuration() {
        let mut config = MachineConfig::default();
        add(&mut config, art_net(1, &[5, 6], Vec::new())).unwrap();
        let before = config.output(OutputId::new(1)).unwrap().clone();

        apply(
            &mut config,
            &Command::SetOutputEnabled {
                id: OutputId::new(1),
                enabled: false,
            },
        )
        .unwrap();
        let disabled = config.output(OutputId::new(1)).unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.kind, before.kind);
        assert_eq!(disabled.universes, before.universes);

        apply(
            &mut config,
            &Command::SetOutputEnabled {
                id: OutputId::new(1),
                enabled: true,
            },
        )
        .unwrap();
        assert_eq!(config.output(OutputId::new(1)), Some(&before));
    }

    #[test]
    fn an_output_can_be_taken_out_and_a_number_that_is_not_there_is_refused() {
        let mut config = MachineConfig::default();
        add(&mut config, mock(1, &[1])).unwrap();
        let applied = apply(
            &mut config,
            &Command::RemoveOutput {
                id: OutputId::new(1),
            },
        )
        .unwrap();
        assert_eq!(
            applied.deltas,
            vec![Delta::OutputsChanged { outputs: vec![] }]
        );
        assert!(config.outputs().is_empty());

        for command in [
            Command::RemoveOutput {
                id: OutputId::new(1),
            },
            Command::SetOutputEnabled {
                id: OutputId::new(1),
                enabled: false,
            },
            Command::ConfigureOutput {
                id: OutputId::new(1),
                change: OutputChange::Name {
                    name: String::new(),
                },
            },
        ] {
            assert_eq!(
                apply(&mut config, &command),
                Err(MachineError::UnknownOutput(OutputId::new(1))),
                "{command:?}"
            );
        }
    }

    /// `ARCHITECTURE_SPEC.md` §7.1: the cable is one DMX line. This is the rule
    /// stated where an operator meets it rather than discovered when the second
    /// universe never appears.
    #[test]
    fn an_open_dmx_adapter_carries_exactly_one_universe() {
        let mut config = MachineConfig::default();
        let two = OutputInstance::new(
            OutputId::new(1),
            "Cable",
            OutputKind::OpenDmx { serial: None },
            [universe(1), universe(2)],
        );
        assert_eq!(
            add(&mut config, two),
            Err(MachineError::OneUniversePerAdapter { universes: 2 })
        );
        assert!(config.outputs().is_empty());

        let one = OutputInstance::new(
            OutputId::new(1),
            "Cable",
            OutputKind::OpenDmx {
                serial: Some("B0037HIY".to_owned()),
            },
            [universe(1)],
        );
        assert!(add(&mut config, one).is_ok());
    }

    #[test]
    fn an_output_that_carries_nothing_is_refused() {
        let mut config = MachineConfig::default();
        assert_eq!(
            add(&mut config, mock(1, &[])),
            Err(MachineError::NoUniverses)
        );
        assert_eq!(
            add(&mut config, mock(1, &[65])),
            Err(MachineError::UniverseOutOfRange(universe(65)))
        );
        assert_eq!(
            add(&mut config, mock(1, &[0])),
            Err(MachineError::UniverseOutOfRange(universe(0)))
        );
        assert_eq!(
            add(&mut config, mock(1, &[3, 3])),
            Err(MachineError::DuplicateUniverse(universe(3)))
        );
    }

    #[test]
    fn an_art_net_output_needs_somewhere_to_send() {
        let mut config = MachineConfig::default();
        let nowhere = OutputInstance::new(
            OutputId::new(1),
            "Node",
            OutputKind::ArtNet {
                nodes: Vec::new(),
                sync: false,
                ports: Vec::new(),
            },
            [universe(1)],
        );
        assert_eq!(add(&mut config, nowhere), Err(MachineError::NoNodes));
    }

    /// The four rows an installer reads off the back of a node — and the three
    /// ways of getting one wrong.
    #[test]
    fn an_art_net_port_row_names_a_universe_this_output_carries() {
        let mut config = MachineConfig::default();
        let row = |universe: u32, net: u8, sub_net: u8, port: u8| ArtNetPort {
            universe: UniverseId::new(universe),
            net,
            sub_net,
            port,
        };

        assert_eq!(
            add(&mut config, art_net(1, &[5, 6], vec![row(7, 0, 0, 0)])),
            Err(MachineError::PortNotCarried(universe(7)))
        );
        assert_eq!(
            add(
                &mut config,
                art_net(1, &[5, 6], vec![row(5, 0, 0, 0), row(5, 0, 0, 1)])
            ),
            Err(MachineError::DuplicatePort(universe(5)))
        );
        assert_eq!(
            add(&mut config, art_net(1, &[5], vec![row(5, 0, 16, 0)])),
            Err(MachineError::BadPortAddress(row(5, 0, 16, 0)))
        );

        // A four-port node, written out the way the box is labelled.
        let node = art_net(
            1,
            &[5, 6, 7, 8],
            vec![
                row(5, 0, 0, 0),
                row(6, 0, 0, 1),
                row(7, 0, 0, 2),
                row(8, 0, 0, 3),
            ],
        );
        assert!(add(&mut config, node).is_ok());
    }

    #[test]
    fn an_sacn_row_is_held_to_e131s_ranges_and_the_hop_limit_to_one_machine() {
        let mut config = MachineConfig::default();
        let row = SacnPort {
            universe: universe(3),
            sacn_universe: 3,
            priority: 201,
        };
        // The same universe twice on one gateway, which is two E1.31 rows for
        // one wire. The Art-Net branch has this rule and so does this one; the
        // twin is written out because a rule that only one of the two branches
        // enforces is the shape of an sACN output that quietly sends the wrong
        // priority.
        let twice = SacnPort {
            universe: universe(3),
            sacn_universe: 3,
            priority: 100,
        };
        assert_eq!(
            add(&mut config, sacn(1, &[3, 4], 1, vec![twice, twice])),
            Err(MachineError::DuplicatePort(universe(3)))
        );
        assert_eq!(
            add(&mut config, sacn(1, &[3, 4], 1, vec![row])),
            Err(MachineError::BadSacnPort(row)),
            "a priority above 200 is refused rather than clamped"
        );
        assert_eq!(
            add(&mut config, sacn(1, &[3], 0, Vec::new())),
            Err(MachineError::ZeroHopLimit)
        );
        assert_eq!(
            add(
                &mut config,
                sacn(
                    1,
                    &[3],
                    1,
                    vec![SacnPort {
                        universe: universe(9),
                        ..row
                    }]
                )
            ),
            Err(MachineError::PortNotCarried(universe(9)))
        );
        assert!(
            add(
                &mut config,
                sacn(
                    1,
                    &[3, 4],
                    8,
                    vec![
                        SacnPort {
                            universe: universe(3),
                            sacn_universe: 101,
                            priority: 200,
                        },
                        SacnPort::for_universe(universe(4)),
                    ]
                )
            )
            .is_ok()
        );
    }

    /// Everything a refusal says has to be something an operator can act on, so
    /// every variant is asserted to name the thing that is wrong.
    #[test]
    fn every_refusal_says_what_is_wrong_in_words() {
        let row = ArtNetPort::for_universe(universe(1));
        for (error, fragment) in [
            (MachineError::NotAMachineCommand, "outputs"),
            (
                MachineError::ConfiguredOnTheCommandLine,
                "named on its command line",
            ),
            (MachineError::SurfaceOnTheCommandLine, "--surface"),
            (MachineError::UnknownOutput(OutputId::new(4)), "output 4"),
            (MachineError::OutputExists(OutputId::new(4)), "already"),
            (MachineError::NoUniverses, "at least one universe"),
            (MachineError::UniverseOutOfRange(universe(65)), "1..=64"),
            (MachineError::DuplicateUniverse(universe(2)), "twice"),
            (
                MachineError::OneUniversePerAdapter { universes: 3 },
                "exactly one universe",
            ),
            (MachineError::NoNodes, "node address"),
            (MachineError::PortNotCarried(universe(7)), "does not carry"),
            (MachineError::DuplicatePort(universe(7)), "two ports"),
            (
                MachineError::BadPortAddress(ArtNetPort { sub_net: 16, ..row }),
                "sub-net 16",
            ),
            (
                MachineError::BadSacnPort(SacnPort {
                    universe: universe(1),
                    sacn_universe: 0,
                    priority: 100,
                }),
                "1..=63999",
            ),
            (MachineError::ZeroHopLimit, "hop limit"),
        ] {
            let text = error.to_string();
            assert!(
                text.contains(fragment),
                "{text:?} does not mention {fragment:?}"
            );
            let as_error: &dyn core::error::Error = &error;
            assert_eq!(as_error.to_string(), text);
        }
    }

    #[test]
    fn a_command_that_is_not_about_this_machine_is_refused_here() {
        let mut config = MachineConfig::default();
        assert_eq!(
            apply(&mut config, &Command::ClearProgrammer),
            Err(MachineError::NotAMachineCommand)
        );
        assert_eq!(
            apply(&mut config, &Command::SaveShow),
            Err(MachineError::NotAMachineCommand)
        );
    }

    /// A refusal writes nothing at all — asserted on the bytes, which is the
    /// rule `tests/command_application.rs` holds the show applier to.
    #[test]
    fn a_refusal_leaves_the_configuration_byte_identical() {
        let mut config = MachineConfig::default();
        add(&mut config, mock(1, &[1, 2])).unwrap();
        add(&mut config, art_net(2, &[5], Vec::new())).unwrap();
        let before = serde_json::to_vec(&config).unwrap();

        for command in [
            Command::AddOutput {
                output: mock(1, &[9]),
            },
            Command::AddOutput {
                output: mock(9, &[]),
            },
            Command::RemoveOutput {
                id: OutputId::new(7),
            },
            Command::ConfigureOutput {
                id: OutputId::new(2),
                change: OutputChange::Universes {
                    universes: Vec::new(),
                },
            },
            Command::ConfigureOutput {
                id: OutputId::new(1),
                change: OutputChange::Kind {
                    kind: OutputKind::OpenDmx { serial: None },
                },
            },
            Command::ClearProgrammer,
        ] {
            assert!(apply(&mut config, &command).is_err(), "{command:?}");
            assert_eq!(
                serde_json::to_vec(&config).unwrap(),
                before,
                "{command:?} changed the configuration on its way to being refused"
            );
        }
    }

    /// A configuration change is validated as a **whole row**: making an output
    /// an Open DMX adapter while it carries two universes is refused, because
    /// the pair is what is wrong rather than either half.
    #[test]
    fn a_change_is_validated_against_the_row_it_would_make() {
        let mut config = MachineConfig::default();
        add(&mut config, mock(1, &[1, 2])).unwrap();
        assert_eq!(
            apply(
                &mut config,
                &Command::ConfigureOutput {
                    id: OutputId::new(1),
                    change: OutputChange::Kind {
                        kind: OutputKind::OpenDmx { serial: None },
                    },
                }
            ),
            Err(MachineError::OneUniversePerAdapter { universes: 2 })
        );

        // One universe first, then the kind: the same two edits in the order
        // that never passes through an impossible row.
        apply(
            &mut config,
            &Command::ConfigureOutput {
                id: OutputId::new(1),
                change: OutputChange::Universes {
                    universes: vec![universe(1)],
                },
            },
        )
        .unwrap();
        assert!(
            apply(
                &mut config,
                &Command::ConfigureOutput {
                    id: OutputId::new(1),
                    change: OutputChange::Kind {
                        kind: OutputKind::OpenDmx { serial: None },
                    },
                }
            )
            .is_ok()
        );
    }

    /// S36's fifth machine command, and the two things it has to do: name a
    /// port, and answer with something a second client can act on.
    #[test]
    fn the_surface_port_is_named_and_the_answer_says_so() {
        let mut config = MachineConfig::default();
        assert_eq!(config.surface_port(), None, "a fresh desk has no surface");

        let applied = apply(
            &mut config,
            &Command::SetSurfacePort {
                port: Some("2- X-Touch".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(config.surface_port(), Some("2- X-Touch"));
        assert_eq!(
            applied.deltas,
            vec![Delta::SurfaceChanged {
                port: Some("2- X-Touch".to_owned())
            }],
            "a second client has to learn which desk this one chose"
        );
        assert_eq!(
            applied.effects,
            vec![crate::Effect::Surface],
            "and somebody has to open it"
        );

        // A port that is not plugged in is **accepted**, exactly as an output
        // row naming a node that is switched off is: a show is prepared before
        // the get-in, and a desk that refused the name could not be configured
        // until the van arrived.
        apply(
            &mut config,
            &Command::SetSurfacePort {
                port: Some("A desk nobody owns".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(config.surface_port(), Some("A desk nobody owns"));

        // And taking it away is a change like any other, with a delta of its own.
        let applied = apply(&mut config, &Command::SetSurfacePort { port: None }).unwrap();
        assert_eq!(config.surface_port(), None);
        assert_eq!(applied.deltas, vec![Delta::SurfaceChanged { port: None }]);
    }

    #[test]
    fn validation_is_reachable_on_its_own_for_a_row_nobody_has_added_yet() {
        // A settings panel checks a row before it sends the command, so the
        // rules are not locked inside the applier.
        assert!(validate(&mock(1, &[1])).is_ok());
        assert_eq!(validate(&mock(1, &[])), Err(MachineError::NoUniverses));
    }
}
