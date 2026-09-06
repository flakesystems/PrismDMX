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
    /// Whether the desk supplies an intensity for this fixture when its profile
    /// has none — S43, and the owner's answer to what B1 turned up.
    ///
    /// # Why a fixture needs one
    ///
    /// B1 gave colour channels a home value of **full**, because on every desk
    /// the owner has used a colour starts open and you subtract; the dimmer is
    /// what decides whether any of it is seen. That is right for anything with
    /// an intensity channel and wrong for a fixture without one: an RGBW PAR has
    /// four colour channels and nothing else, so *colour open* and *lamp at
    /// full* are the same eight bits, and a rig of them came up white the moment
    /// the daemon started. A console that lights the stage with nothing
    /// programmed is not a console anybody can run a show on.
    ///
    /// So the desk supplies the missing channel. The fixture gets a `Dimmer`
    /// attribute that exists in the merge and on the encoders but occupies no
    /// DMX channel, resting at **nought**; what it scales on the way out is the
    /// fixture's colour, which for a fixture with no intensity of its own *is*
    /// its intensity. The rig is dark at home, the colour is open underneath it,
    /// and both halves of B1 hold.
    ///
    /// # Why it can be switched off
    ///
    /// The owner's own condition. A PAR that is on a dimmer pack, or one whose
    /// colour channels a house rig drives directly, wants its channels written
    /// through untouched — and the desk cannot know which. So this is a patch
    /// field per fixture and not a rule.
    ///
    /// It says nothing at all about a fixture whose profile **has** an
    /// intensity: there the profile's own channel is the dimmer and this is
    /// ignored. [`Self::has_software_dimmer`] is the question worth asking, and
    /// this field on its own never is.
    #[serde(default = "supplied")]
    pub software_dimmer: bool,
}

/// The default for [`Fixture::software_dimmer`]: a fixture that needs one gets
/// one, so a rig read from a show file written before S43 comes up dark.
pub(crate) const fn supplied() -> bool {
    true
}

impl Fixture {
    /// Whether the desk supplies this fixture's intensity.
    ///
    /// True when the operator has left [`Self::software_dimmer`] on **and** the
    /// type has no intensity channel of its own. The two halves are asked
    /// together everywhere, so they are asked together here: a caller that
    /// looked only at the field would give a moving head two dimmers, and one
    /// that looked only at the type would take the operator's switch away.
    #[must_use]
    pub fn has_software_dimmer(&self, fixture_type: &crate::FixtureType) -> bool {
        self.software_dimmer && !fixture_type.has_dimmer()
    }

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
            software_dimmer: true,
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

    /// **The desk supplies an intensity only where the profile has none** — S43.
    ///
    /// Both halves, because either on its own is a wrong answer: read the field
    /// alone and a moving head gets a second dimmer nothing can reach; read the
    /// type alone and the operator's switch does nothing.
    #[test]
    fn the_desk_supplies_an_intensity_only_where_the_profile_has_none() {
        let colour_only = crate::FixtureType {
            id: "test.par".to_owned(),
            manufacturer: "Test".to_owned(),
            name: "PAR".to_owned(),
            mode: "1ch".to_owned(),
            footprint: 1,
            attributes: vec![crate::AttributeDef {
                attribute: crate::AttributeType::Red,
                label: None,
                occurrence: 0,
                feature_group: crate::FeatureGroup::Color,
                coarse_offset: 0,
                fine_offset: None,
                default_value: u16::MAX,
                merge_mode: crate::MergeMode::Ltp,
                invert: false,
                physical_from: 0.0,
                physical_to: 100.0,
                ranges: Vec::new(),
            }],
        };
        let mut with_intensity = colour_only.clone();
        with_intensity.attributes.push(crate::AttributeDef {
            attribute: crate::AttributeType::Dimmer,
            label: None,
            occurrence: 0,
            feature_group: crate::FeatureGroup::Dimmer,
            coarse_offset: 1,
            fine_offset: None,
            default_value: 0,
            merge_mode: crate::MergeMode::Htp,
            invert: false,
            physical_from: 0.0,
            physical_to: 100.0,
            ranges: Vec::new(),
        });

        let mut patched = fixture();
        patched.software_dimmer = true;
        assert!(
            patched.has_software_dimmer(&colour_only),
            "a fixture with no intensity of its own gets one"
        );
        assert!(
            !patched.has_software_dimmer(&with_intensity),
            "a fixture with a dimmer channel already has one"
        );

        patched.software_dimmer = false;
        assert!(
            !patched.has_software_dimmer(&colour_only),
            "and the operator can switch it off"
        );
    }

    /// A show file written before S43 carries no such field, and the fixture it
    /// describes must come up **dark** rather than lit — which is the whole
    /// reason the default is *supplied* and not `false`.
    #[test]
    fn a_fixture_read_without_the_field_gets_the_supplied_intensity() {
        let mut json = serde_json::to_value(fixture()).unwrap();
        json.as_object_mut().unwrap().remove("softwareDimmer");
        let read: Fixture = serde_json::from_value(json).unwrap();
        assert!(read.software_dimmer);
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
                // **S43.** The desk supplies an intensity for a fixture whose
                // profile has none, and this is the operator's switch for it.
                "softwareDimmer",
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
