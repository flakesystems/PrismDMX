//! Patched fixtures, groups, and the geometry they carry.
//!
//! Position and rotation exist here rather than in a viewer-specific structure
//! because the 3D viewer and the PSN follow calculation (`ARCHITECTURE_SPEC.md`
//! §8) must agree on where a fixture is.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{FixtureId, GroupId, UniverseId};

/// Highest DMX channel in a universe.
pub const CHANNELS_PER_UNIVERSE: u16 = 512;

/// A point or an Euler rotation in show space. Metres and degrees.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub struct Vec3 {
    /// Stage left/right.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub x: f64,
    /// Height.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub y: f64,
    /// Depth.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub z: f64,
}

impl Vec3 {
    /// The origin, and the default rotation.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Builds a vector.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// An 8-bit RGB colour, as shown on an X-Touch scribble strip.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub struct RgbColor {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

/// A patched fixture: one instance of a [`crate::FixtureType`] at an address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Fixture {
    /// User-facing fixture number.
    pub id: FixtureId,
    /// Operator-facing name.
    pub name: String,
    /// Key of the [`crate::FixtureType`] this fixture instantiates.
    pub type_id: String,
    /// Universe this fixture is patched into.
    pub universe: UniverseId,
    /// Start address, `1..=512`.
    pub address: u16,
    /// Position in show space, for the 3D viewer and PSN follow.
    pub position: Vec3,
    /// Orientation in show space.
    pub rotation: Vec3,
    /// Reverse pan for this fixture, applied after any attribute-level invert.
    pub invert_pan: bool,
    /// Reverse tilt for this fixture.
    pub invert_tilt: bool,
}

impl Fixture {
    /// The last channel this fixture occupies, given its type's `footprint`.
    ///
    /// `None` when the fixture does not fit: address 0, an empty footprint, or a
    /// range running past channel 512. `prism-engine` rejects such a patch at
    /// patch time so the tick never needs a bounds check.
    #[must_use]
    pub const fn last_address(&self, footprint: u16) -> Option<u16> {
        if self.address == 0 || footprint == 0 {
            return None;
        }
        match self.address.checked_add(footprint - 1) {
            Some(last) if last <= CHANNELS_PER_UNIVERSE => Some(last),
            _ => None,
        }
    }
}

/// A named set of fixtures, in selection order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// Group number.
    pub id: GroupId,
    /// Operator-facing name.
    pub name: String,
    /// Members, in the order they were selected when the group was stored.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(8)")
    )]
    pub fixtures: Vec<FixtureId>,
}

#[cfg(test)]
mod tests {
    use crate::{Fixture, FixtureId, Group, GroupId, RgbColor, UniverseId, Vec3};

    fn fixture() -> Fixture {
        Fixture {
            id: FixtureId::new(1),
            name: "Front left".to_owned(),
            type_id: "generic.rgbw.par".to_owned(),
            universe: UniverseId::new(1),
            address: 1,
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: true,
        }
    }

    #[test]
    fn fixture_uses_camel_case_field_names() {
        let json = serde_json::to_value(fixture()).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "address",
                "id",
                "invertPan",
                "invertTilt",
                "name",
                "position",
                "rotation",
                "typeId",
                "universe",
            ]
        );
    }

    #[test]
    fn a_fixture_occupies_a_contiguous_address_range() {
        // The footprint lives on the type, so the fixture only knows its start.
        assert_eq!(fixture().last_address(4), Some(4));
        let mut wide = fixture();
        wide.address = 510;
        assert_eq!(wide.last_address(3), Some(512));
        // ARCHITECTURE_SPEC.md: a fixture may not run past the end of a universe.
        assert_eq!(wide.last_address(4), None);
        let mut zero = fixture();
        zero.address = 0;
        assert_eq!(zero.last_address(1), None);
    }

    #[test]
    fn vectors_serialise_as_three_numbers() {
        assert_eq!(
            serde_json::to_string(&Vec3::new(1.0, -2.5, 0.0)).unwrap(),
            r#"{"x":1.0,"y":-2.5,"z":0.0}"#
        );
        assert_eq!(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn colours_are_eight_bit_rgb() {
        let color = RgbColor {
            r: 255,
            g: 128,
            b: 0,
        };
        assert_eq!(
            serde_json::to_string(&color).unwrap(),
            r#"{"r":255,"g":128,"b":0}"#
        );
    }

    #[test]
    fn a_group_is_an_ordered_list_of_fixtures() {
        let group = Group {
            id: GroupId::new(2),
            name: "Wash".to_owned(),
            fixtures: vec![FixtureId::new(3), FixtureId::new(1)],
        };
        assert_eq!(
            serde_json::to_string(&group).unwrap(),
            r#"{"id":2,"name":"Wash","fixtures":[3,1]}"#
        );
    }
}
