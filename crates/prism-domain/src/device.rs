//! What a GDTF device **is**, in the detail a visualiser needs — **S30b**.
//!
//! S61 carried a device's size, its body's model and its beams, flattened to
//! world positions. That was enough for boxes and cones and not for a
//! visualiser: a moving head is a base, a yoke that pans on it and a head that
//! tilts in the yoke, each drawn with its own model, and what its beam looks
//! like depends on **every function of every channel** — a shutter that is
//! closed, open or strobing at 0.3 to 20 Hz on one channel, a frost that is
//! light below 84 and medium above it, a gobo index that becomes a rotation
//! when another channel says so.
//!
//! So a GDTF profile carries three more things on
//! [`crate::FixturePhysical`], all of them **only** GDTF's and all of them
//! skipped when empty, so a show from before this reads back unchanged:
//!
//! - [`GeometryNode`]s — the geometry tree, flattened in document order with a
//!   parent index, each node's transform **relative to its parent** in show
//!   space (metres, Y up, `z` upstage — `crate::placement`);
//! - [`ChannelDetail`]s — for every patched channel, which geometry it acts on
//!   and each of its [`ChannelFunction`]s with its DMX range, its physical
//!   range, its wheel and the channel sets inside it;
//! - [`Wheel`]s — every wheel, and each slot's colour, picture and prism
//!   facets.
//!
//! None of it changes what a value **is**. The engine never reads any of this,
//! and the tick does not know it exists: it is description, and a viewer is the
//! only thing that draws with it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{RgbColor, Vec3};

/// One node of a device's geometry tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct GeometryNode {
    /// The geometry's name — *Base*, *Yoke*, *Head*, *Beam*. A channel names
    /// the geometry it acts on by this ([`ChannelDetail::geometry`]).
    pub name: String,
    /// Which node it hangs from, as an index into the same list, or `None` for
    /// a root. Always lower than this node's own index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    /// GDTF's element name: `Geometry`, `Axis`, `Beam`, `FilterBeam`,
    /// `FilterColor`, `FilterGobo`, `FilterShaper`, `Display`, … A viewer
    /// needs `Axis` (it turns) and `Beam` (light leaves it); the rest are
    /// drawn as bodies.
    pub kind: String,
    /// The model file GDTF names, without a directory or an extension — the
    /// archive ships it as `models/gltf/<file>.glb` or `models/3ds/<file>.3ds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// GDTF's `PrimitiveType` where it is not `Undefined`: `Cube`, `Cylinder`,
    /// `Sphere`, `Base`, `Yoke`, `Head`, `Scanner`, `Conventional`,
    /// `Pigtail`, … A viewer with no model file draws this shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive: Option<String>,
    /// The model's box in show axes, metres: across, height, depth. Nought
    /// when the node names no model.
    pub size: Vec3,
    /// Where the node's origin sits in its parent's frame, metres.
    pub position: Vec3,
    /// The node's X axis in its parent's frame, show axes — a column of its
    /// rotation.
    pub x_axis: Vec3,
    /// Its Y axis.
    pub y_axis: Vec3,
    /// Its Z axis.
    pub z_axis: Vec3,
    /// What the light is like, for a `Beam`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beam: Option<BeamShape>,
}

/// A `Beam` geometry's optics, as GDTF states them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct BeamShape {
    /// `Wash`, `Spot`, `None`, `Rectangle`, `PC`, `Fresnel`, `Glow`.
    pub beam_type: String,
    /// Full angle at half intensity, degrees.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub beam_angle: f64,
    /// Full angle at a tenth of the intensity, degrees — the soft edge.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub field_angle: f64,
    /// Radius of the lens the light leaves through, metres.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub beam_radius: f64,
    /// Lumens at full.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub luminous_flux: f64,
    /// Kelvin of the source.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub color_temperature: f64,
    /// Width over height of a rectangular beam; 1 for a round one.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub rectangle_ratio: f64,
}

/// One patched channel, as a visualiser reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ChannelDetail {
    /// Offset of its coarse byte in the footprint, from nought — the same
    /// number as [`crate::AttributeDef::coarse_offset`], which is how the two
    /// are matched.
    pub offset: u16,
    /// Offset of its fine byte, or `None` for an 8-bit channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine: Option<u16>,
    /// The geometry it acts on — *Yoke* for a pan, *Head* for a tilt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    /// GDTF's attribute for the channel as a whole — `Shutter1`, `Gobo1Pos`.
    pub attribute: String,
    /// Every function, in the order the file lists them.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub functions: Vec<ChannelFunction>,
}

/// One `ChannelFunction`: a DMX range of a channel that is one parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ChannelFunction {
    /// GDTF's attribute for this range — `Shutter1Strobe`, `Gobo1PosRotate`,
    /// `Frost2`.
    pub attribute: String,
    /// Where it starts, `0..=65535`.
    pub from: u16,
    /// Where it ends, `0..=65535` — the next function's start less one, or
    /// full.
    pub to: u16,
    /// The physical value at `from` — degrees, hertz, kelvin, a fraction.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub physical_from: f64,
    /// The physical value at `to`.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub physical_to: f64,
    /// The wheel this range selects slots of, by [`Wheel::name`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wheel: Option<String>,
    /// The channel sets inside it, lowest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub sets: Vec<ChannelSet>,
    /// The GDTF attribute of the channel this function **depends on**, where
    /// one decides it: `Gobo1Pos` is an index while `Gobo1` stands in one
    /// range and a rotation while it stands in another. `None` for a function
    /// that is always in force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_master: Option<String>,
    /// The master's range this function is in force for, `0..=65535`.
    #[serde(default)]
    pub mode_from: u16,
    /// Its end.
    #[serde(default)]
    pub mode_to: u16,
}

/// One `ChannelSet` inside a function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ChannelSet {
    /// Its name, as the file states it — may be empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Where it starts, `0..=65535`.
    pub from: u16,
    /// Where it ends.
    pub to: u16,
    /// The wheel slot it selects, **one-based** as GDTF counts them; `None`
    /// for a set that selects none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u16>,
}

/// One wheel of a device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Wheel {
    /// What channel functions name it by.
    pub name: String,
    /// Its slots, in order — slot 1 first.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub slots: Vec<WheelSlot>,
}

/// One slot of a wheel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct WheelSlot {
    /// Its name.
    pub name: String,
    /// The colour it filters to, from GDTF's CIE `xyY`, as the brightest sRGB
    /// that hue is — `None` for a slot that is white, which is most gobos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<RgbColor>,
    /// How much light it lets through, `0..=1` — GDTF's `Y` over 100. A deep
    /// blue passes a few per cent and a frost nearly all of it.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub transmission: f64,
    /// The picture's file name inside the archive, without the directory
    /// (`wheels/`) or an extension — a gobo's shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    /// A prism slot's facets: for each, where its beam is pushed to, as the
    /// point `(x, y)` a unit along the beam lands at in the beam's own frame.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub facets: Vec<PrismFacet>,
}

/// One facet of a prism.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct PrismFacet {
    /// Across the beam.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub x: f64,
    /// Up the beam's cross-section.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub y: f64,
}
