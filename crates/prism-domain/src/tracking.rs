//! What a tracking system is to this machine - **S32**.
//!
//! A tracker (PSN, `ARCHITECTURE_SPEC.md` §8) is a stream of positions in *the
//! tracking system's own space*: its own units, its own idea of which way is up.
//! Where each head points is the **show's** business and lives on the fixture
//! (`crate::Fixture::follow`); what the tracking system *is* - which network it
//! speaks on, how its axes line up with the stage - is the **machine's**, and is
//! here. A show carried to another hall keeps who follows whom and meets that
//! hall's trackers by number.
//!
//! # Axes are configuration because the format does not say
//!
//! PSN gives three numbers and neither the unit nor the up. MA's own consoles ask
//! the operator to map each axis and to invert any of them, and so does this
//! desk: [`TrackerMapping`] names, for each axis of **show space** (metres, Y up,
//! `z` upstage - `crate::placement`), which axis of the tracker's space it is read
//! from, whether to flip it, and then one scale and one offset for all three.
//! The default is the common one - a right-handed tracking space with `z` up and
//! `y` upstage, which is show space with `y` and `z` swapped.
//!
//! That default is a guess, and the one thing about this module nobody has
//! confirmed on a real tracker. It is correct for the space it names; whether a
//! given system uses that space is exactly what the panel's *seen* list and the
//! 3D viewer are there to show an installer within a minute of plugging it in.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Vec3;

/// The multicast group PSN systems send to by default.
pub const DEFAULT_GROUP: &str = "236.10.10.10";

/// The UDP port PSN systems send to by default.
pub const DEFAULT_PORT: u16 = 56_565;

/// How long a tracker may be quiet before the desk says so, in milliseconds.
///
/// `ARCHITECTURE_SPEC.md` §8's own number. The head holds its last position for
/// as long as the tracker is quiet - that is not what the timeout decides - and
/// this is only how soon the operator is told.
pub const DEFAULT_TIMEOUT_MS: u32 = 500;

/// The shortest and longest the timeout may be set to, in milliseconds.
///
/// A tracker sends thirty to sixty times a second, so under 100 ms a healthy
/// system would flicker between *live* and *quiet*; over a minute is not a
/// warning anybody is still standing there to read.
pub const TIMEOUT_RANGE_MS: core::ops::RangeInclusive<u32> = 100..=60_000;

/// An axis of the tracking system's own space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum SourceAxis {
    /// The first number of a position.
    X,
    /// The second.
    Y,
    /// The third.
    Z,
}

/// An axis of show space - the one a [`TrackerMapping`] row is *for*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ShowAxis {
    /// Across the stage.
    X,
    /// Up.
    Y,
    /// Upstage - away from the audience.
    Z,
}

/// Where one axis of show space is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct AxisSource {
    /// Which of the tracker's axes it is.
    pub from: SourceAxis,
    /// Whether it runs the other way.
    pub invert: bool,
}

/// How a tracking system's positions become positions on this stage.
///
/// `show = scale * (the picked axes, flipped where asked) + offset`, per axis.
/// One scale rather than one per axis: a tracking system that is out by a
/// different factor on each axis is not a unit mismatch, it is a system that is
/// miscalibrated, and a desk that hid that by correcting for it would be putting
/// a lie in the one place that has to be right.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct TrackerMapping {
    /// Where show `x` comes from.
    pub x: AxisSource,
    /// Where show `y` - up - comes from.
    pub y: AxisSource,
    /// Where show `z` - upstage - comes from.
    pub z: AxisSource,
    /// Metres per unit of the tracking system: `1` for metres, `0.001` for
    /// millimetres. Positive and finite; see [`Self::is_valid`].
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub scale: f64,
    /// Added after the scale, in metres of show space: where the tracking
    /// system's own origin is on this stage.
    pub offset: Vec3,
}

impl Default for TrackerMapping {
    /// A right-handed tracking space, `z` up and `y` upstage, in metres.
    fn default() -> Self {
        let pick = |from| AxisSource {
            from,
            invert: false,
        };
        Self {
            x: pick(SourceAxis::X),
            y: pick(SourceAxis::Z),
            z: pick(SourceAxis::Y),
            scale: 1.0,
            offset: Vec3::ZERO,
        }
    }
}

impl TrackerMapping {
    /// Whether this is a mapping the desk will use: a scale that is a positive
    /// number and an offset of a stage's size.
    ///
    /// A scale of nought would put every performer at the offset and a negative
    /// one would turn the stage inside out - which is what the invert on each
    /// axis is for. Neither is a unit.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let offset = self.offset;
        self.scale.is_finite()
            && self.scale > 0.0
            && offset.x.is_finite()
            && offset.y.is_finite()
            && offset.z.is_finite()
            && offset.x.abs() <= crate::MAX_REACH
            && offset.y.abs() <= crate::MAX_REACH
            && offset.z.abs() <= crate::MAX_REACH
    }

    /// The row for one axis of show space.
    #[must_use]
    pub const fn source(&self, axis: ShowAxis) -> AxisSource {
        match axis {
            ShowAxis::X => self.x,
            ShowAxis::Y => self.y,
            ShowAxis::Z => self.z,
        }
    }

    /// Sets the row for one axis of show space.
    pub const fn set_source(&mut self, axis: ShowAxis, source: AxisSource) {
        match axis {
            ShowAxis::X => self.x = source,
            ShowAxis::Y => self.y = source,
            ShowAxis::Z => self.z = source,
        }
    }

    /// A position as the tracking system sent it, in show space.
    ///
    /// Computed in `f64` from the `f32` the wire carries, and **not** checked:
    /// a NaN or an infinity comes out as one, and `prism_engine::TrackerTable`
    /// is what refuses it.
    #[must_use]
    pub fn apply(&self, raw: [f32; 3]) -> Vec3 {
        let axis = |source: AxisSource, offset: f64| {
            let value = f64::from(match source.from {
                SourceAxis::X => raw[0],
                SourceAxis::Y => raw[1],
                SourceAxis::Z => raw[2],
            });
            (if source.invert { -value } else { value }) * self.scale + offset
        };
        Vec3 {
            x: axis(self.x, self.offset.x),
            y: axis(self.y, self.offset.y),
            z: axis(self.z, self.offset.z),
        }
    }
}

/// How this machine listens for trackers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct TrackerSettings {
    /// Whether the desk listens at all.
    ///
    /// **Off by default.** A desk that opens a multicast socket nobody asked for
    /// is a desk that surprises a firewall and a school's network admin; and a
    /// show that follows nothing has no use for one.
    pub enabled: bool,
    /// The address of the network interface to listen on, or `None` to let the
    /// operating system choose.
    ///
    /// Multicast is joined **on an interface**, and a laptop with Wi-Fi and a
    /// lighting network is exactly the machine where the system's choice is the
    /// wrong one. Text, parsed and refused by the daemon if it is not an IPv4
    /// address - see [`Self::interface_address`].
    pub interface: Option<String>,
    /// The group to join, `236.10.10.10` unless the tracking system was told
    /// otherwise. An address that is **not** a multicast one is listened on
    /// directly instead - which is how a tracker that sends to one machine is
    /// received, and how the tests receive one without putting a multicast
    /// datagram on the network they run on.
    pub group: String,
    /// The UDP port, `56565` unless the tracking system was told otherwise.
    pub port: u16,
    /// How the system's axes line up with the stage.
    pub mapping: TrackerMapping,
    /// How long a tracker may be quiet before the desk says so, in
    /// milliseconds - see [`DEFAULT_TIMEOUT_MS`].
    pub timeout_ms: u32,
}

impl Default for TrackerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            interface: None,
            group: DEFAULT_GROUP.to_owned(),
            port: DEFAULT_PORT,
            mapping: TrackerMapping::default(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

impl TrackerSettings {
    /// The group as an address, or `None` if the text is not an IPv4 address.
    #[must_use]
    pub fn group_address(&self) -> Option<std::net::Ipv4Addr> {
        self.group.trim().parse().ok()
    }

    /// The interface as an address: `Ok(None)` for none chosen.
    ///
    /// # Errors
    ///
    /// The parse error when an interface is named and is not an IPv4 address.
    pub fn interface_address(
        &self,
    ) -> Result<Option<std::net::Ipv4Addr>, std::net::AddrParseError> {
        match self.interface.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(text) => text.parse().map(Some),
        }
    }
}

/// One setting of [`TrackerSettings`], changed on its own - what
/// `MachineChange::Tracker` carries.
///
/// One field per change: a change carrying the whole struct would make a client
/// read it, change one field and send the rest back, and two operators with the
/// panel open would each undo the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum TrackerChange {
    /// Whether the desk listens at all.
    Enabled {
        /// Listen, or do not.
        enabled: bool,
    },
    /// Which network interface the group is joined on, or `None` for the
    /// operating system's choice.
    Interface {
        /// The interface's IPv4 address as text, or `None`. Refused by the desk
        /// if it is not one.
        address: Option<String>,
    },
    /// The group to join - or, if it is not a multicast address, the address to
    /// listen on directly.
    Group {
        /// An IPv4 address as text.
        group: String,
    },
    /// The UDP port.
    Port {
        /// `1..=65535`.
        port: u16,
    },
    /// Where one axis of show space is read from.
    Axis {
        /// The axis of the stage.
        axis: ShowAxis,
        /// The tracker axis it is read from.
        from: SourceAxis,
        /// Whether it runs the other way.
        invert: bool,
    },
    /// Metres per unit of the tracking system.
    Scale {
        /// Positive and finite; refused otherwise.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::finite_f64()")
        )]
        scale: f64,
    },
    /// Where the tracking system's origin is on this stage, in metres.
    Offset {
        /// Of a stage's size; refused otherwise.
        offset: Vec3,
    },
    /// How long a tracker may be quiet before the desk says so.
    Timeout {
        /// Milliseconds, clamped into [`TIMEOUT_RANGE_MS`].
        milliseconds: u32,
    },
}

/// Whether a tracker is being heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum TrackerHealth {
    /// A position arrived within the timeout.
    Live,
    /// None has, and the heads following it are holding the last one.
    Quiet,
}

/// One tracker this desk has heard from - what `Query::Trackers` answers with.
///
/// **An age and not a time**, for `NodeHealth`'s reason: the daemon and a browser
/// share no clock, and an age is true on arrival where a timestamp would have to
/// be trusted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct SeenTracker {
    /// The tracker's number.
    pub id: u16,
    /// What the system calls it, if its info packet has been heard.
    pub name: Option<String>,
    /// Where it last was, in show space - after the mapping, which is what makes
    /// this the line an installer reads to see whether the mapping is right.
    pub position: Vec3,
    /// Milliseconds since the last position.
    pub age_ms: u64,
    /// Whether it is being heard.
    pub health: TrackerHealth,
    /// How many heads follow it.
    pub followers: u32,
}

#[cfg(test)]
mod tests {
    use super::{AxisSource, ShowAxis, SourceAxis, TrackerMapping, TrackerSettings};
    use crate::Vec3;

    #[test]
    fn the_default_swaps_y_and_z() {
        // A right-handed `z`-up system's `(x, y, z)` is show space's `(x, z, y)`.
        let at = TrackerMapping::default().apply([1.0, 2.0, 3.0]);
        assert_eq!((at.x, at.y, at.z), (1.0, 3.0, 2.0));
    }

    #[test]
    fn a_scale_an_invert_and_an_offset_apply_in_that_order() {
        let mut mapping = TrackerMapping {
            scale: 0.001, // millimetres
            offset: Vec3 {
                x: 10.0,
                y: 0.0,
                z: -1.0,
            },
            ..TrackerMapping::default()
        };
        mapping.x.invert = true;
        let at = mapping.apply([2000.0, 1000.0, 500.0]);
        assert!((at.x - 8.0).abs() < 1e-9, "{at:?}");
        assert!((at.y - 0.5).abs() < 1e-9, "{at:?}");
        assert!((at.z - 0.0).abs() < 1e-9, "{at:?}");
    }

    #[test]
    fn an_axis_can_be_read_from_anywhere() {
        let mut mapping = TrackerMapping::default();
        mapping.set_source(
            ShowAxis::Y,
            AxisSource {
                from: SourceAxis::Y,
                invert: false,
            },
        );
        assert_eq!(mapping.source(ShowAxis::Y).from, SourceAxis::Y);
        assert_eq!(mapping.apply([0.0, 7.0, 9.0]).y, 7.0);
    }

    #[test]
    fn a_mapping_is_valid_with_a_positive_scale_and_a_stage_sized_offset() {
        let mut mapping = TrackerMapping::default();
        assert!(mapping.is_valid());
        mapping.scale = 0.0;
        assert!(!mapping.is_valid());
        mapping.scale = -1.0;
        assert!(!mapping.is_valid());
        mapping.scale = f64::NAN;
        assert!(!mapping.is_valid());
        mapping.scale = 1.0;
        mapping.offset.y = 5_000.0;
        assert!(!mapping.is_valid());
    }

    #[test]
    fn the_defaults_are_the_published_ones_and_the_listener_is_off() {
        let settings = TrackerSettings::default();
        assert!(!settings.enabled);
        assert_eq!(settings.group, "236.10.10.10");
        assert_eq!(settings.port, 56_565);
        assert_eq!(settings.timeout_ms, 500);
        assert_eq!(
            settings.group_address(),
            Some(std::net::Ipv4Addr::new(236, 10, 10, 10))
        );
    }

    #[test]
    fn an_interface_is_none_blank_or_an_address() {
        let mut settings = TrackerSettings::default();
        assert_eq!(settings.interface_address(), Ok(None));
        settings.interface = Some("  ".to_owned());
        assert_eq!(settings.interface_address(), Ok(None));
        settings.interface = Some("192.168.1.20".to_owned());
        assert_eq!(
            settings.interface_address(),
            Ok(Some(std::net::Ipv4Addr::new(192, 168, 1, 20)))
        );
        settings.interface = Some("wifi".to_owned());
        assert!(settings.interface_address().is_err());
    }
}
