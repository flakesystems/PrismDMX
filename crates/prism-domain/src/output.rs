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

/// Whether an Art-Net node this desk is addressed to is **answering** - S46.
///
/// # Why this is a second word and not three more [`OutputHealth`] variants
///
/// [`OutputHealth`] is the *socket's* opinion, and for a network output that
/// opinion is worth very little: UDP accepts every datagram it is handed, so an
/// Art-Net output with no node on the far end reports `Ok` for ever. That is
/// punch-list **B6**, and it cannot be repaired by renaming a variant, because
/// the fact the operator is missing is one the socket does not have.
///
/// It is also not three more variants of `OutputHealth`, and that is the
/// decision worth reading: an Open DMX cable and an sACN sender have no
/// answer-back at all, so `NeverAnswered` would be a state they could enter and
/// never leave. What answers is a **node**, so the word belongs to a node — and
/// what an output reports goes on being `OutputHealth`, folded down from these
/// by the daemon (`prismd::outputs::reported_health`).
///
/// # The third state carries a time, and the time is the daemon's
///
/// *Answering* and *never answered* are complete on their own; *stopped* is only
/// useful with a moment attached, because an installer wants to know whether it
/// stopped while they were walking to the rack. The moment travels as an **age**
/// beside this value ([`NodeReach::last_reply_ago_ms`]) rather than as a
/// timestamp inside it, which is S33's rule for `OutputStatusInfo::last_error`:
/// the daemon and a browser share no clock, so the daemon says *how long ago*
/// and the client renders that against its own.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum NodeHealth {
    /// A reply arrived inside the timeout. The only value that means *there is
    /// something out there listening*.
    Answering,
    /// Polled, and has never replied since this daemon started.
    ///
    /// The default, and deliberately so: the safe reading of silence is silence.
    #[default]
    NeverAnswered,
    /// Answered once and has not answered since - see
    /// [`NodeReach::last_reply_ago_ms`] for when.
    Stopped,
}

impl NodeHealth {
    /// Whether something is known to be listening at the far end.
    #[must_use]
    pub const fn is_answering(self) -> bool {
        matches!(self, Self::Answering)
    }
}

/// One address an output sends to, and whether anything there answers - S46.
///
/// One row per node of an `OutputKind::ArtNet`, in the order the rig names them.
/// **Empty for every other kind**, because nothing else this desk drives has an
/// answer-back: an Open DMX cable and an sACN stream are told, never asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct NodeReach {
    /// The configured address, as the rig spells it.
    pub address: String,
    /// Whether it answers.
    pub health: NodeHealth,
    /// The node's short name, once it has said one. `None` for a node that has
    /// never answered - there is nothing to call it.
    pub name: Option<String>,
    /// How long ago the last reply was, in milliseconds, or `None` for a node
    /// that has never answered.
    ///
    /// An **age** and not a time, for [`NodeHealth`]'s reason.
    pub last_reply_ago_ms: Option<u64>,
}

/// What this desk's Art-Net discovery has done and been sent - S46.
///
/// # Why a panel is shown counters at all
///
/// Because the first thing this session got wrong in a hall could not be seen
/// from the outside. A node was connected, reachable and answering, and the desk
/// read `Degraded`; the panel showed an empty node list and had no way to say
/// whether that meant *nobody is out there*, *nobody was asked*, or *something
/// came back and could not be read*. Those are three different faults with three
/// different remedies, and the daemon knew which one it was the whole time.
///
/// So the numbers travel. `polls_sent` climbing with `replies` at zero is a node
/// that is not answering **or** a reply that is not arriving; `malformed`
/// climbing beside it is a reply that is arriving and being dropped, which is a
/// bug in this desk rather than a fault in the hall. An operator does not have
/// to read them — the row above says the useful thing — but somebody diagnosing
/// a rig at eleven at night does.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ArtNetCounters {
    /// `ArtPoll` datagrams that have gone out.
    pub polls_sent: u64,
    /// Polls that could not be sent — a node whose route has gone.
    pub polls_failed: u64,
    /// `ArtPollReply` datagrams that were read.
    pub replies: u64,
    /// Datagrams that were not a reply this desk could read, dropped rather
    /// than believed.
    pub malformed: u64,
    /// Replies from a node the table had no room for.
    pub dropped: u64,
    /// Reads the socket refused.
    pub read_errors: u64,
}

/// A node that answered an `ArtPoll` - S46.
///
/// # A discovered node is not a configured one
///
/// The two lists answer different questions and an installer needs both: *what
/// is out there* and *what this desk is addressed to*. `configured` is where
/// they meet, and [`unaddressed_ports`](Self::unaddressed_ports) and
/// [`missing_ports`](Self::missing_ports) are where they disagree - a node
/// outputting a universe this desk sends nothing on is as much a fault as a
/// configured node that never answers, and neither is visible from one list
/// alone.
///
/// Both disagreements are **the daemon's arithmetic**, for `Query`'s own rule:
/// a client that intersected the rig with the discovery table would be a second
/// opinion about something the daemon already holds both halves of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ArtNetNodeInfo {
    /// Where the reply came from - the address a new output would be given.
    pub address: String,
    /// The address the node says it has, which differs from `address` on a node
    /// behind a translating router and is worth showing when it does.
    pub ip: String,
    /// `ShortName`, 18 bytes in the packet: what a node's front panel shows.
    pub short_name: String,
    /// `LongName`, 64 bytes.
    pub long_name: String,
    /// `MAC`, as six hexadecimal pairs. The one identifier that survives a node
    /// being re-addressed.
    pub mac: String,
    /// `VersInfoH`/`VersInfoL` - the node's own firmware revision.
    pub firmware: u16,
    /// `Style`: what kind of device it says it is (`StNode`, `StController`, ...).
    pub style: u8,
    /// `Status1` and `Status2`, carried as numbers rather than read: they are a
    /// bit field of the node's own diagnostics, and a desk that interpreted them
    /// would be interpreting one vendor's spelling of them.
    pub status1: u8,
    /// See [`status1`](Self::status1).
    pub status2: u8,
    /// The node's **output** port addresses, fifteen bits each.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub ports: Vec<u16>,
    /// The node's **input** port addresses - a node sending DMX *into* the
    /// network, which this desk does not consume and which is worth showing so
    /// an installer can see why a universe is being driven twice.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub inputs: Vec<u16>,
    /// Whether the rig already addresses this node.
    pub configured: bool,
    /// Port addresses the node outputs that this desk sends it nothing on.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub unaddressed_ports: Vec<u16>,
    /// Port addresses this desk sends to the node that the node does not list.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub missing_ports: Vec<u16>,
    /// The universes an output for this node would carry if nobody said
    /// otherwise — the deliverable's *one click*.
    ///
    /// The **inverse of the default mapping** ([`ArtNetPort::for_universe`]:
    /// universe N goes to port address N − 1), applied to the port addresses the
    /// node says it outputs. It is answered rather than worked out by a client
    /// for `Query`'s rule: that mapping is stated in `ARCHITECTURE_SPEC.md` §7.0
    /// and implemented twice in Rust already, and a third spelling of it in
    /// TypeScript would drift the first time it moved.
    ///
    /// Empty for a node that lists no output ports, which is an ordinary state
    /// for a node that has not been addressed yet: an output made from it would
    /// carry nothing, and the operator types the universes instead.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub suggested_universes: Vec<UniverseId>,
    /// How many replies have arrived since the daemon started.
    pub replies: u64,
    /// How long ago the last one was, in milliseconds - an age, not a time.
    pub last_reply_ago_ms: u64,
}

impl ArtNetNodeInfo {
    /// The desk universe a node's port address maps to by default.
    ///
    /// The inverse of [`ArtNetPort::for_universe`], and the one place it is
    /// written: universe N goes to port address N − 1, so port address P is
    /// universe P + 1. `None` outside [`UniverseId`]'s own range, which a
    /// fifteen-bit port address reaches and this desk does not.
    #[must_use]
    pub fn universe_for_port(port: u16) -> Option<UniverseId> {
        let universe = UniverseId::new(u32::from(port) + 1);
        universe.is_in_range().then_some(universe)
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
    /// Whether the nodes this output sends to are answering — S46.
    ///
    /// One row per configured node of an Art-Net output, and **empty** for every
    /// other kind, for [`NodeReach`]'s reason: nothing else this desk drives can
    /// be asked. Also empty while the discovery socket is not bound, which is a
    /// different fact from *nothing answers* — see `Answer::ArtNetNodes`.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(2)")
    )]
    #[serde(default)]
    pub nodes: Vec<NodeReach>,
}

#[cfg(test)]
mod tests {
    use crate::{
        ArtNetNodeInfo, ArtNetPort, NodeHealth, NodeReach, OutputHealth, OutputId, OutputInstance,
        OutputKind, OutputStatusInfo, SacnPort, UniverseId,
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

    /// S38's rule kept: a type's own crate tests it, or the figure that says so
    /// is somebody else's suite.
    #[test]
    fn a_node_is_answering_never_answered_or_stopped_and_nothing_else() {
        for (health, text) in [
            (NodeHealth::Answering, "\"Answering\""),
            (NodeHealth::NeverAnswered, "\"NeverAnswered\""),
            (NodeHealth::Stopped, "\"Stopped\""),
        ] {
            assert_eq!(serde_json::to_string(&health).unwrap(), text);
        }
        assert_eq!(
            NodeHealth::inline(&Config::new()),
            "\"Answering\" | \"NeverAnswered\" | \"Stopped\""
        );
    }

    /// The safe reading of silence is silence — and only one value means *there
    /// is something out there*, which is the whole of punch-list B6.
    #[test]
    fn a_node_nobody_has_heard_from_has_not_answered() {
        assert_eq!(NodeHealth::default(), NodeHealth::NeverAnswered);
        assert!(NodeHealth::Answering.is_answering());
        assert!(!NodeHealth::NeverAnswered.is_answering());
        assert!(!NodeHealth::Stopped.is_answering());
    }

    /// The age travels **beside** the value, never inside it — S33's rule for
    /// `last_error_ago_ms`, one field along.
    #[test]
    fn a_node_row_carries_an_age_and_not_a_time() {
        let reach = NodeReach {
            address: "10.0.0.9:6454".to_owned(),
            health: NodeHealth::Stopped,
            name: Some("Stage left".to_owned()),
            last_reply_ago_ms: Some(90_000),
        };
        let json = serde_json::to_string(&reach).unwrap();
        assert!(json.contains(r#""lastReplyAgoMs":90000"#), "{json}");
        assert!(json.contains(r#""health":"Stopped""#), "{json}");
        assert_eq!(serde_json::from_str::<NodeReach>(&json).unwrap(), reach);

        // A node that has never answered has nothing to be called and no age.
        let silent = NodeReach {
            address: "10.0.0.9:6454".to_owned(),
            health: NodeHealth::NeverAnswered,
            name: None,
            last_reply_ago_ms: None,
        };
        let json = serde_json::to_string(&silent).unwrap();
        assert!(json.contains(r#""name":null"#), "{json}");
        assert_eq!(serde_json::from_str::<NodeReach>(&json).unwrap(), silent);
    }

    /// A status row from a daemon that never heard of nodes still decodes, and
    /// the field it is missing means *nothing to say* rather than *nothing
    /// answers*.
    #[test]
    fn an_output_status_row_without_nodes_still_opens() {
        let row: OutputStatusInfo = serde_json::from_str(
            r#"{"id":1,"health":"Ok","framesSent":42,"lastError":null,"lastErrorAgoMs":null}"#,
        )
        .unwrap();
        assert!(row.nodes.is_empty());
        assert_eq!(row.frames_sent, 42);
    }

    /// The default mapping run backwards, and the one place it is written.
    #[test]
    fn a_port_address_maps_back_to_the_universe_one_above_it() {
        assert_eq!(
            ArtNetNodeInfo::universe_for_port(0),
            Some(UniverseId::new(1)),
            "Art-Net counts from 0 and this desk counts from 1"
        );
        assert_eq!(
            ArtNetNodeInfo::universe_for_port(63),
            Some(UniverseId::new(64))
        );
        assert_eq!(
            ArtNetNodeInfo::universe_for_port(64),
            None,
            "a fifteen-bit port address reaches further than this desk does"
        );
        assert_eq!(ArtNetNodeInfo::universe_for_port(0x7fff), None);
        // …and it is the exact inverse of the mapping the rig uses.
        for universe in [1u32, 2, 16, 17, 64] {
            let row = ArtNetPort::for_universe(UniverseId::new(universe));
            assert_eq!(
                ArtNetNodeInfo::universe_for_port(row.address()),
                Some(UniverseId::new(universe))
            );
        }
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
