//! The profiles this desk knows, before a show has been written.
//!
//! # Why a library exists at all, and why it is here
//!
//! A show **embeds** the fixture types it uses (S11, and the reasoning is on
//! [`Show::embed_fixture_type`](crate::Show::embed_fixture_type)): a show that
//! referenced an external library would silently change meaning when that
//! library was updated underneath it. That is right, and it leaves one question
//! open — where does the *first* profile come from? A brand-new show carries no
//! profiles at all, so until S27 there was no way to patch anything into one
//! except by writing the file from Rust.
//!
//! So the desk carries a handful of generic profiles and
//! `Command::EmbedFixtureType` **copies one into the show**. The copy is the
//! point: after it, the show owns that profile and a desk with a different
//! library opens the show unchanged.
//!
//! # Why the client does not send the profile
//!
//! `Command::EmbedFixtureType` carries a key and nothing else, for the same
//! reason `Command::PatchFixture` carries no channels (**D3**): a client that
//! sent a whole [`FixtureType`] would be authoring show content, and the daemon
//! would be reduced to validating whatever arrived. The library is served in the
//! snapshot so a client can *offer* the list; what a client sends is which one.
//!
//! # What this is not
//!
//! It is not a fixture library in the sense a venue means. There is no import,
//! no GDTF, no per-manufacturer modes, and no way for an operator to define a
//! profile of their own — `ARCHITECTURE_SPEC.md` §9 reserves `profiles/fixtures/`
//! for that and no session has built it yet. What is here is the smallest set
//! that lets a person open PrismDMX on a fresh show and patch a real rig:
//! a dimmer, two PARs and a moving head.

use prism_domain::{AttributeDef, AttributeType, FeatureGroup, FixtureType, MergeMode};

/// An 8-bit attribute at an offset, filed under its own feature group.
fn eight_bit(attribute: AttributeType, coarse_offset: u16, default_value: u16) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
    }
}

/// A 16-bit attribute over two channels, centred at home.
fn sixteen_bit(
    attribute: AttributeType,
    coarse_offset: u16,
    physical_from: f64,
    physical_to: f64,
) -> AttributeDef {
    AttributeDef {
        attribute,
        feature_group: FeatureGroup::Position,
        coarse_offset,
        fine_offset: Some(coarse_offset + 1),
        // Centre, so a head that is patched and never touched points at the
        // middle of its travel rather than at one end stop.
        default_value: 32768,
        merge_mode: MergeMode::Ltp,
        invert: false,
        physical_from,
        physical_to,
    }
}

/// A one-channel dimmer: the smallest profile that controls anything.
fn dimmer() -> FixtureType {
    FixtureType {
        id: "generic.dimmer".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Dimmer".to_owned(),
        mode: "1ch".to_owned(),
        footprint: 1,
        attributes: vec![eight_bit(AttributeType::Dimmer, 0, 0)],
    }
}

/// A three-channel RGB PAR.
fn rgb_par() -> FixtureType {
    FixtureType {
        id: "generic.rgb.par".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "RGB PAR".to_owned(),
        mode: "3ch".to_owned(),
        footprint: 3,
        attributes: vec![
            eight_bit(AttributeType::Red, 0, 0),
            eight_bit(AttributeType::Green, 1, 0),
            eight_bit(AttributeType::Blue, 2, 0),
        ],
    }
}

/// A four-channel RGBW PAR.
fn rgbw_par() -> FixtureType {
    FixtureType {
        id: "generic.rgbw.par".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "RGBW PAR".to_owned(),
        mode: "4ch".to_owned(),
        footprint: 4,
        attributes: vec![
            eight_bit(AttributeType::Red, 0, 0),
            eight_bit(AttributeType::Green, 1, 0),
            eight_bit(AttributeType::Blue, 2, 0),
            eight_bit(AttributeType::White, 3, 0),
        ],
    }
}

/// An eleven-channel moving head with 16-bit pan and tilt.
///
/// The profile that makes the *rest* of the desk reachable from a fresh show:
/// it has attributes on all five encoder banks, so an operator who patches one
/// has something to turn on every one of them.
fn moving_head() -> FixtureType {
    FixtureType {
        id: "generic.movinghead".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Moving Head".to_owned(),
        mode: "11ch".to_owned(),
        footprint: 11,
        attributes: vec![
            sixteen_bit(AttributeType::Pan, 0, -270.0, 270.0),
            sixteen_bit(AttributeType::Tilt, 2, -135.0, 135.0),
            eight_bit(AttributeType::Dimmer, 4, 0),
            eight_bit(AttributeType::Shutter, 5, 0),
            eight_bit(AttributeType::Red, 6, 0),
            eight_bit(AttributeType::Green, 7, 0),
            eight_bit(AttributeType::Blue, 8, 0),
            eight_bit(AttributeType::Zoom, 9, 0),
            eight_bit(AttributeType::Focus, 10, 0),
        ],
    }
}

/// Every profile this desk can embed, in the order a list should show them.
///
/// Built rather than held as a constant: a [`FixtureType`] owns three `String`s
/// and a `Vec`, none of which is constructible in a `const`.
#[must_use]
pub fn fixture_library() -> Vec<FixtureType> {
    vec![dimmer(), rgb_par(), rgbw_par(), moving_head()]
}

/// One profile by key, or `None` when this desk does not carry it.
#[must_use]
pub fn library_type(type_id: &str) -> Option<FixtureType> {
    fixture_library()
        .into_iter()
        .find(|fixture_type| fixture_type.id == type_id)
}

#[cfg(test)]
mod tests {
    use super::{fixture_library, library_type};
    use crate::Show;
    use prism_domain::{AttributeType, FeatureGroup};
    use std::collections::BTreeSet;

    /// The library is only worth having if the show model accepts all of it.
    ///
    /// `embed_fixture_type` refuses an empty footprint, a duplicated attribute
    /// and an offset outside the footprint, so this is the whole of the
    /// validation a profile has to pass — run against every profile rather than
    /// against the one that was being edited.
    #[test]
    fn every_profile_in_the_library_is_one_a_show_accepts() {
        let mut show = Show::new();
        for fixture_type in fixture_library() {
            show.embed_fixture_type(fixture_type.clone())
                .unwrap_or_else(|error| panic!("{}: {error}", fixture_type.id));
        }
        assert_eq!(show.fixture_types().count(), fixture_library().len());
    }

    #[test]
    fn the_keys_are_unique_and_the_library_is_found_by_key() {
        let keys: BTreeSet<String> = fixture_library()
            .into_iter()
            .map(|fixture_type| fixture_type.id)
            .collect();
        assert_eq!(keys.len(), fixture_library().len(), "a key is used twice");
        for key in &keys {
            assert_eq!(library_type(key).map(|found| found.id).as_ref(), Some(key));
        }
        assert_eq!(library_type("nothing.at.all"), None);
    }

    /// Every channel of every profile is inside its footprint and used once —
    /// which `embed_fixture_type` checks for the coarse offsets and, as of S27,
    /// this checks for the *fine* ones as well.
    #[test]
    fn no_two_attributes_of_a_profile_share_a_channel() {
        for fixture_type in fixture_library() {
            let mut used = BTreeSet::new();
            for def in &fixture_type.attributes {
                for offset in [Some(def.coarse_offset), def.fine_offset]
                    .into_iter()
                    .flatten()
                {
                    assert!(
                        offset < fixture_type.footprint,
                        "{} {:?} is at {offset}, outside {}",
                        fixture_type.id,
                        def.attribute,
                        fixture_type.footprint
                    );
                    assert!(
                        used.insert(offset),
                        "{} uses channel {offset} twice",
                        fixture_type.id
                    );
                }
            }
            assert_eq!(
                used.len(),
                usize::from(fixture_type.footprint),
                "{} leaves a channel of its footprint unused",
                fixture_type.id
            );
        }
    }

    /// The moving head reaches every encoder bank, which is what makes a fresh
    /// show worth patching one into.
    #[test]
    fn the_moving_head_has_something_on_every_bank() {
        let head = library_type("generic.movinghead").expect("the library carries a moving head");
        let banks: BTreeSet<FeatureGroup> = head
            .attributes
            .iter()
            .map(|def| def.feature_group)
            .collect();
        assert_eq!(banks, FeatureGroup::ALL.into_iter().collect());
        // And pan and tilt are 16-bit, because a head that steps in 256 places
        // is one an operator can see stepping.
        for attribute in [AttributeType::Pan, AttributeType::Tilt] {
            let def = head
                .attributes
                .iter()
                .find(|def| def.attribute == attribute)
                .expect("a moving head pans and tilts");
            assert!(def.fine_offset.is_some(), "{attribute:?} is 8-bit");
            assert_eq!(def.default_value, 32768, "{attribute:?} is not centred");
        }
    }
}
