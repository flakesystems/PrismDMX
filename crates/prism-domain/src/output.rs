//! Output health and the output patch, as reported and configured per DMX
//! interface.
//!
//! `ARCHITECTURE_SPEC.md` §7: every driver runs on its own thread and reports
//! health outwards, so an unplugged USB adapter shows as a red light in the UI
//! instead of stopping the engine. The `DmxOutput` trait itself belongs to
//! `prism-protocols`; what is domain vocabulary is the reported value and — since
//! S33 — **what an output is**.
//!
//! # Why the rig is data and not a command line *(S33)*
//!
//! Until S33 an output was a `prismd::cli::OutputSpec` built once at start-up,
//! and every network output was handed the whole show's universes. Two Art-Net
//! nodes therefore both received every universe, an sACN output could not be
//! told to carry only 3 and 4, and nothing could be added, removed or
//! re-addressed without restarting the daemon. A venue's rig — several outputs,
//! of several kinds, each carrying the universes it is actually wired for — was
//! not expressible at all.
//!
//! [`OutputInstance`] is that rig, one entry per interface. Where it *lives* is
//! the decision worth reading: `prism_core::MachineConfig`, beside the desk
//! identity, and never inside a `.prism` file. A show carried to another school
//! on a stick must not bring the first hall's cabling with it, which is the same
//! argument `prism_core::desk` makes for the sACN CID.
//!
//! # The universes are the routing, both ways
//!
//! One universe may go to several outputs — a main rig and a stage-left node
//! carrying the same 512 channels is ordinary — and one output may carry many.
//! A universe the patch uses and no output carries is a **legitimate state that
//! is reported**, `prism_core::ShowIssue::UniverseNotOutput`, because an
//! operator whose universe 7 goes nowhere has to be able to read that before the
//! show starts rather than discover it when the light does not come up.

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{OutputId, UniverseId};

/// Health of one DMX output, as shown by the status light per interface.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum OutputHealth {
    /// Sending frames at the expected rate.
    Ok,
    /// Sending, but not cleanly: late frames, retries, or a reduced rate.
    Degraded,
    /// Not sending. The driver is retrying with backoff.
    #[default]
    Disconnected,
}

impl OutputHealth {
    /// Whether frames are reaching the fixtures at all.
    #[must_use]
    pub const fn is_sending(self) -> bool {
        matches!(self, Self::Ok | Self::Degraded)
    }
}

/// Art-Net's fifteen-bit port address for one of this desk's universes.
///
/// **A four-port node is four of these**, and that is what the type is for: an
/// installer reads Net, Sub-Net and Universe off the back of the box and types
/// the three numbers in. The default is `PortAddress::for_universe` — universe N
/// becomes port address N − 1, since PrismDMX numbers universes from 1 and
/// Art-Net from 0 — and a row here overrides it for one universe, because nodes
/// disagree about which end they count from (`ARCHITECTURE_SPEC.md` §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ArtNetPort {
    /// The desk's universe this row is about.
    pub universe: UniverseId,
    /// Art-Net Net, `0..=127`.
    pub net: u8,
    /// Art-Net Sub-Net, `0..=15`.
    pub sub_net: u8,
    /// Art-Net Universe within that Sub-Net, `0..=15`.
    pub port: u8,
}

impl ArtNetPort {
    /// Highest Art-Net Net.
    pub const MAX_NET: u8 = 127;
    /// Highest Art-Net Sub-Net.
    pub const MAX_SUB_NET: u8 = 15;
    /// Highest Art-Net Universe within a Sub-Net.
    pub const MAX_PORT: u8 = 15;

    /// The row a universe has when nobody has said otherwise: port address
    /// N − 1, split into its three parts.
    #[must_use]
    pub const fn for_universe(universe: UniverseId) -> Self {
        // Masked to the field width the specification gives each part, so every
        // one of the three fits a `u8` by construction and the conversion is
        // infallible rather than merely unlikely to fail.
        let address = universe.get().saturating_sub(1);
        Self {
            universe,
            net: ((address >> 8) & 0x7f) as u8,
            sub_net: ((address >> 4) & 0x0f) as u8,
            port: (address & 0x0f) as u8,
        }
    }

    /// Whether the three numbers are inside the fields the specification gives
    /// them.
    #[must_use]
    pub const fn is_in_range(self) -> bool {
        self.net <= Self::MAX_NET
            && self.sub_net <= Self::MAX_SUB_NET
            && self.port <= Self::MAX_PORT
    }

    /// The fifteen-bit port address the three parts make.
    #[must_use]
    pub const fn address(self) -> u16 {
        ((self.net as u16) << 8) | ((self.sub_net as u16) << 4) | (self.port as u16)
    }
}

/// How one of this desk's universes goes out over sACN.
///
/// Two things, both of them data because a venue's universe 1 is not always a
/// desk's and because two sources on one universe are resolved by priority
/// (`ARCHITECTURE_SPEC.md` §7.2): which E1.31 universe it becomes, and what it
/// is worth against another source of the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct SacnPort {
    /// The desk's universe this row is about.
    pub universe: UniverseId,
    /// The E1.31 universe it is sent as, `1..=63999`.
    pub sacn_universe: u16,
    /// What it is worth against another source, `0..=200`.
    pub priority: u8,
}

impl SacnPort {
    /// Lowest E1.31 universe.
    pub const MIN_UNIVERSE: u16 = 1;
    /// Highest E1.31 universe.
    pub const MAX_UNIVERSE: u16 = 63_999;
    /// Highest priority E1.31 allows. Refused above rather than clamped: a desk
    /// that quietly lowered a number an operator typed would take over a rig it
    /// was told not to.
    pub const MAX_PRIORITY: u8 = 200;
    /// The priority a source has when nobody has chosen one.
    pub const DEFAULT_PRIORITY: u8 = 100;

    /// The row a universe has when nobody has said otherwise: sent as itself, at
    /// the default priority.
    #[must_use]
    pub const fn for_universe(universe: UniverseId) -> Self {
        Self {
            universe,
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a desk universe is 1..=64 and E1.31 reaches 63999"
            )]
            sacn_universe: universe.get() as u16,
            priority: Self::DEFAULT_PRIORITY,
        }
    }

    /// Whether both numbers are inside the ranges E1.31 gives them.
    #[must_use]
    pub const fn is_in_range(self) -> bool {
        self.sacn_universe >= Self::MIN_UNIVERSE
            && self.sacn_universe <= Self::MAX_UNIVERSE
            && self.priority <= Self::MAX_PRIORITY
    }
}

/// What kind of interface an output is, and the parameters that kind needs.
///
/// The parameters are the ones `ARCHITECTURE_SPEC.md` §7 makes configuration
/// rather than code. Everything a driver needs beyond them — the break timing,
/// the packet layout, the refresh margin — is the protocol's own and is not
/// here: a settings panel that offered them would be offering to break the
/// standard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum OutputKind {
    /// An output that accepts every frame and puts it nowhere.
    ///
    /// Not test scaffolding: `ARCHITECTURE_SPEC.md` §12 runs the end-to-end
    /// tests "against a daemon in mock-output mode", and this is that mode as a
    /// row in the patch.
    Mock,
    /// The Open DMX USB adapter — an FTDI FT232R with the host owning DMX
    /// timing (§7.1). Carries **exactly one** universe by construction.
    OpenDmx {
        /// Which cable, when there is more than one. The FTDI strings are the
        /// manufacturer's own, so the serial is the only thing that tells two
        /// SH-RS09Bs apart; `None` takes the first adapter the machine offers.
        #[serde(default)]
        serial: Option<String>,
    },
    /// An Art-Net node, unicast to the addresses named (§7.2).
    ArtNet {
        /// Where the datagrams go. Unicast, one per node, because broadcast
        /// floods a school network.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::sockets(2)")
        )]
        #[serde(with = "crate::socket")]
        #[ts(as = "Vec<String>")]
        nodes: Vec<SocketAddr>,
        /// Whether to follow each frame with an ArtSync. Off by default: a node
        /// that understands it stops displaying data until one arrives.
        #[serde(default)]
        sync: bool,
        /// Universe → Net · Sub-Net · Universe on **this** node. A universe with
        /// no row here takes [`ArtNetPort::for_universe`].
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        #[serde(default)]
        ports: Vec<ArtNetPort>,
    },
    /// An sACN (E1.31) sender (§7.2).
    Sacn {
        /// Named receivers, for a venue whose network forbids multicast. Empty
        /// is the ordinary case and means multicast, since an sACN group carries
        /// one universe and a switch that snoops IGMP delivers it only where it
        /// was asked for.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::sockets(2)")
        )]
        #[serde(default, with = "crate::socket")]
        #[ts(as = "Vec<String>")]
        receivers: Vec<SocketAddr>,
        /// How far a multicast datagram may travel. 1 keeps it on the local
        /// segment, which is right for a lighting network that is one switch.
        #[serde(default = "default_ttl")]
        ttl: u32,
        /// Universe → E1.31 universe and priority. A universe with no row here
        /// takes [`SacnPort::for_universe`].
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        #[serde(default)]
        ports: Vec<SacnPort>,
    },
}

/// The multicast hop limit an sACN output has when nobody has chosen one.
const fn default_ttl() -> u32 {
    1
}

impl OutputKind {
    /// What this kind is called in a status panel and a log line.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Mock => "Mock",
            Self::OpenDmx { .. } => "Open DMX USB",
            Self::ArtNet { .. } => "Art-Net",
            Self::Sacn { .. } => "sACN",
        }
    }

    /// Whether this kind can carry more than one universe.
    ///
    /// `false` for exactly one kind, and it is a fact about the cable rather
    /// than a policy: an Open DMX adapter is one DMX line (§7.1).
    #[must_use]
    pub const fn carries_many(&self) -> bool {
        !matches!(self, Self::OpenDmx { .. })
    }
}

/// One configured DMX output: which universes go out of which interface.
///
/// See this module's documentation for where these live and why. The number is
/// the operator's, like a fixture number or a sequence number: `Output 3` names
/// this row on the command line and in a settings panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct OutputInstance {
    /// Which output.
    pub id: OutputId,
    /// What the operator calls it — "Stage left node", "Hall dimmers".
    pub name: String,
    /// The interface and its parameters.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::boxed()")
    )]
    pub kind: OutputKind,
    /// The desk's universes this interface puts on the wire, in send order.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub universes: Vec<UniverseId>,
    /// Whether it is sending at all.
    ///
    /// A disabled output keeps its configuration and stops driving its line,
    /// which is what an operator wants of a node that is being worked on — the
    /// alternative is deleting the row and typing it back in afterwards.
    pub enabled: bool,
}

impl OutputInstance {
    /// An enabled output of `kind` carrying `universes`.
    #[must_use]
    pub fn new(
        id: OutputId,
        name: impl Into<String>,
        kind: OutputKind,
        universes: impl IntoIterator<Item = UniverseId>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            universes: universes.into_iter().collect(),
            enabled: true,
        }
    }

    /// Whether this output puts `universe` on a wire.
    #[must_use]
    pub fn carries(&self, universe: UniverseId) -> bool {
        self.universes.contains(&universe)
    }

    /// Whether replacing `self` with `other` means stopping the driver and
    /// starting a new one.
    ///
    /// **What a rename must not cost.** Re-addressing an output is a new thread
    /// on a new set of universes, and for sACN it is a stream terminated and a
    /// stream begun; giving an output a better name is neither, and an operator
    /// who typed one should not watch their rig blink. So this reads what the
    /// *driver* was built from and nothing else.
    #[must_use]
    pub fn needs_restart(&self, other: &Self) -> bool {
        self.kind != other.kind
            || self.universes != other.universes
            || self.enabled != other.enabled
            || self.id != other.id
    }
}

/// What one output's driver is **doing**, as a settings panel watches it — S37.
///
/// # Why this is an answer rather than a delta or a snapshot field
///
/// S33 carried this out of itself in as many words: *there is no channel for a
/// live frame counter between snapshots — the counter moves at 44 Hz and a delta
/// per frame is out of the question. S37 either asks (a `Query`) or accepts that
/// the number is as old as the connection.*
///
/// Accepting was the wrong half. `OutputSnapshot` carries the counter with the
/// world, so an output **added while a client is connected** would show zero
/// frames for ever — and the row an operator has just made is exactly the row
/// they are watching to see whether it works. So the panel asks, on a cadence of
/// its own and only while it is open, which is `Query::MidiPorts`' shape one
/// device along.
///
/// It is deliberately **not** `prism_ipc::OutputSnapshot`: that type carries the
/// configured row as well, and the configuration arrives by
/// [`crate::Delta::OutputsChanged`] whenever it moves. Sending it again with
/// every counter reading would be a rig on the wire once a second.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct OutputStatusInfo {
    /// Which output.
    pub id: OutputId,
    /// What its driver is doing.
    pub health: OutputHealth,
    /// Universes put on the wire, not datagrams.
    pub frames_sent: u64,
    /// The last thing that went wrong, or `None`.
    pub last_error: Option<String>,
    /// How long ago that was — an **age**, not a time, because the daemon and a
    /// browser have no shared clock (S33).
    pub last_error_ago_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use crate::{
        ArtNetPort, OutputHealth, OutputId, OutputInstance, OutputKind, SacnPort, UniverseId,
    };
    use ts_rs::{Config, TS};

    fn universe(id: u32) -> UniverseId {
        UniverseId::new(id)
    }

    #[test]
    fn health_states_are_named() {
        for (health, text) in [
            (OutputHealth::Ok, "\"Ok\""),
            (OutputHealth::Degraded, "\"Degraded\""),
            (OutputHealth::Disconnected, "\"Disconnected\""),
        ] {
            assert_eq!(serde_json::to_string(&health).unwrap(), text);
        }
    }

    #[test]
    fn an_output_that_has_not_reported_yet_counts_as_disconnected() {
        assert_eq!(OutputHealth::default(), OutputHealth::Disconnected);
    }

    #[test]
    fn only_ok_means_the_show_is_going_out() {
        assert!(OutputHealth::Ok.is_sending());
        assert!(OutputHealth::Degraded.is_sending());
        assert!(!OutputHealth::Disconnected.is_sending());
    }

    #[test]
    fn typescript_union_matches_the_specification() {
        assert_eq!(
            OutputHealth::inline(&Config::new()),
            "\"Ok\" | \"Degraded\" | \"Disconnected\""
        );
    }

    /// The default mapping is the one `ARCHITECTURE_SPEC.md` §7.2 states:
    /// universe N → port address N − 1, because this desk counts universes from
    /// 1 and Art-Net counts them from 0.
    #[test]
    fn an_art_net_row_defaults_to_the_port_address_one_below_the_universe() {
        assert_eq!(ArtNetPort::for_universe(universe(1)).address(), 0);
        assert_eq!(ArtNetPort::for_universe(universe(16)).address(), 15);
        let seventeen = ArtNetPort::for_universe(universe(17));
        assert_eq!(seventeen.address(), 16);
        assert_eq!(
            (seventeen.net, seventeen.sub_net, seventeen.port),
            (0, 1, 0)
        );
        assert!(seventeen.is_in_range());
    }

    #[test]
    fn an_art_net_row_outside_its_fields_is_out_of_range() {
        let row = ArtNetPort::for_universe(universe(1));
        assert!(!ArtNetPort { net: 128, ..row }.is_in_range());
        assert!(!ArtNetPort { sub_net: 16, ..row }.is_in_range());
        assert!(!ArtNetPort { port: 16, ..row }.is_in_range());
        assert!(
            ArtNetPort {
                net: 127,
                sub_net: 15,
                port: 15,
                ..row
            }
            .is_in_range()
        );
        assert_eq!(
            ArtNetPort {
                net: 127,
                sub_net: 15,
                port: 15,
                ..row
            }
            .address(),
            0x7fff
        );
    }

    /// Unlike Art-Net, both count from 1 — so the default is the identity, and
    /// the mapping is still data because a venue's universe 1 is not always a
    /// desk's.
    #[test]
    fn an_sacn_row_defaults_to_the_same_universe_at_the_standard_priority() {
        let row = SacnPort::for_universe(universe(3));
        assert_eq!(row.sacn_universe, 3);
        assert_eq!(row.priority, 100);
        assert!(row.is_in_range());
    }

    #[test]
    fn an_sacn_row_outside_e131s_ranges_is_out_of_range() {
        let row = SacnPort::for_universe(universe(1));
        assert!(
            !SacnPort {
                sacn_universe: 0,
                ..row
            }
            .is_in_range()
        );
        assert!(
            !SacnPort {
                sacn_universe: 64_000,
                ..row
            }
            .is_in_range()
        );
        assert!(
            !SacnPort {
                priority: 201,
                ..row
            }
            .is_in_range()
        );
        assert!(
            SacnPort {
                priority: 200,
                ..row
            }
            .is_in_range()
        );
    }

    #[test]
    fn a_kind_says_what_it_is_and_whether_it_can_carry_more_than_one_universe() {
        assert_eq!(OutputKind::Mock.label(), "Mock");
        assert_eq!(OutputKind::OpenDmx { serial: None }.label(), "Open DMX USB");
        assert!(!OutputKind::OpenDmx { serial: None }.carries_many());
        assert!(OutputKind::Mock.carries_many());
        let art_net = OutputKind::ArtNet {
            nodes: vec!["192.168.1.50:6454".parse().unwrap()],
            sync: false,
            ports: Vec::new(),
        };
        assert_eq!(art_net.label(), "Art-Net");
        assert!(art_net.carries_many());
        let sacn = OutputKind::Sacn {
            receivers: Vec::new(),
            ttl: 1,
            ports: Vec::new(),
        };
        assert_eq!(sacn.label(), "sACN");
        assert!(sacn.carries_many());
    }

    #[test]
    fn an_output_is_readable_text_on_the_wire() {
        let output = OutputInstance::new(
            OutputId::new(3),
            "Stage left node",
            OutputKind::ArtNet {
                nodes: vec!["10.0.0.9:6454".parse().unwrap()],
                sync: true,
                ports: vec![ArtNetPort {
                    universe: universe(5),
                    net: 0,
                    sub_net: 0,
                    port: 0,
                }],
            },
            [universe(5), universe(6)],
        );
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains(r#""t":"ArtNet""#), "{json}");
        assert!(json.contains(r#""10.0.0.9:6454""#), "{json}");
        assert!(json.contains(r#""subNet":0"#), "{json}");
        assert!(json.contains(r#""enabled":true"#), "{json}");
        let back: OutputInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, output);
        assert!(back.carries(universe(5)));
        assert!(!back.carries(universe(7)));
    }

    /// A configuration written before a field existed still opens, which is the
    /// rule every persisted type in this workspace follows.
    #[test]
    fn the_optional_parameters_have_defaults_so_an_older_file_still_opens() {
        let output: OutputInstance = serde_json::from_str(
            r#"{"id":1,"name":"Node","kind":{"t":"ArtNet","nodes":["10.0.0.1:6454"]},
                "universes":[1],"enabled":true}"#,
        )
        .unwrap();
        assert_eq!(
            output.kind,
            OutputKind::ArtNet {
                nodes: vec!["10.0.0.1:6454".parse().unwrap()],
                sync: false,
                ports: Vec::new(),
            }
        );

        let sacn: OutputInstance = serde_json::from_str(
            r#"{"id":2,"name":"Gateway","kind":{"t":"Sacn"},"universes":[3],"enabled":false}"#,
        )
        .unwrap();
        assert_eq!(
            sacn.kind,
            OutputKind::Sacn {
                receivers: Vec::new(),
                ttl: 1,
                ports: Vec::new(),
            },
            "a hop limit nobody chose is the one that stays on the segment"
        );
        assert!(!sacn.enabled);

        let open_dmx: OutputInstance = serde_json::from_str(
            r#"{"id":3,"name":"Cable","kind":{"t":"OpenDmx"},"universes":[1],"enabled":true}"#,
        )
        .unwrap();
        assert_eq!(open_dmx.kind, OutputKind::OpenDmx { serial: None });
    }

    /// The rule that keeps a rename free — see [`OutputInstance::needs_restart`].
    #[test]
    fn only_what_the_driver_was_built_from_costs_a_restart() {
        let output = OutputInstance::new(
            OutputId::new(1),
            "Hall",
            OutputKind::Mock,
            [universe(1), universe(2)],
        );

        let renamed = OutputInstance {
            name: "Hall dimmers".to_owned(),
            ..output.clone()
        };
        assert!(
            !output.needs_restart(&renamed),
            "a better name must not blink the rig"
        );

        for changed in [
            OutputInstance {
                universes: vec![universe(1)],
                ..output.clone()
            },
            OutputInstance {
                kind: OutputKind::OpenDmx { serial: None },
                ..output.clone()
            },
            OutputInstance {
                enabled: false,
                ..output.clone()
            },
            OutputInstance {
                id: OutputId::new(2),
                ..output.clone()
            },
        ] {
            assert!(output.needs_restart(&changed), "{changed:?}");
        }
    }
}
