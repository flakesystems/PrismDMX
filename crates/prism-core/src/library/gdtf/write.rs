//! Writing **GDTF** — **B65**, the MVR export.
//!
//! # Why a desk that reads GDTF has to write it
//!
//! An `.mvr` carries the rig **and** the `.gdtf` of every fixture in it, and a
//! planner that is handed an MVR with a fixture whose profile is missing has
//! nothing to draw. The Open Fixture Library's profiles — and a venue's own
//! hand-written ones — have no GDTF file anywhere, and the owner's condition for
//! the export was that **they are not lost**. So this module makes one.
//!
//! # What it writes, and what it does not
//!
//! One `.gdtf` per **fixture** (a manufacturer and a name), holding **every
//! mode of it the show uses**: the channels of each mode in their places, each
//! with its attribute, its home value, its physical range and its named ranges
//! as `ChannelSet`s, a 16-bit channel as the two offsets GDTF writes it with —
//! and a **device**: the geometry tree the profile carries, or — for a profile
//! that says nothing about one — the moving head or can this desk's own viewer
//! would draw, with pan on a yoke axis and tilt on a head axis (`tree`).
//!
//! It does **not** write what a [`FixtureType`] does not carry: 3D models, wheel
//! pictures, a manufacturer's thumbnail. That is a deliberate floor and not a
//! gap to fill — a planner draws a primitive body for it, which is what it does
//! for a fixture it has no model of anyway. Where the library holds the
//! manufacturer's own GDTF file for a profile, the **export copies that file
//! instead** (`library::mvr::write`); this is only for the rest.
//!
//! The one thing it takes care over is that **its own reader reads it back**:
//! the tests below put a profile through [`write_archive`] and
//! [`super::read_archive`] and compare the channels, which is the only check a
//! build machine has against a planner it does not own.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use prism_domain::{AttributeDef, AttributeType, FeatureGroup, FixtureType};

use super::attributes;
use crate::library::zip::Writer;

mod tree;

/// What a model with no size of its own is drawn as, in metres — the viewer's
/// own box for a profile that says nothing about the device.
const DEFAULT_BODY: f64 = 0.3;

/// What a beam states when a profile has none of its own to carry, in degrees.
const DEFAULT_BEAM_ANGLE: f64 = 25.0;

/// Writes the modes of one fixture as a `.gdtf`.
///
/// `modes` are the profiles of **one** fixture — the same manufacturer and
/// name, which is what a `.gdtf` is. Their order is the order of the modes in
/// the file. `None` for an empty list, or where the archive would not fit the
/// plain ZIP format.
#[must_use]
pub fn write_archive(modes: &[&FixtureType]) -> Option<Vec<u8>> {
    let first = modes.first()?;
    let mut writer = Writer::new();
    writer.add_deflated("description.xml", description(first, modes).as_bytes());
    writer.finish()
}

/// The name a `.gdtf` is given inside an MVR: `Manufacturer@Fixture.gdtf`, the
/// convention the format's own share uses, with nothing in it a file system
/// would object to.
#[must_use]
pub fn file_name(fixture: &FixtureType) -> String {
    let clean = |text: &str| -> String {
        let kept: String = text
            .chars()
            .map(|character| {
                if character.is_alphanumeric() || " -_.()".contains(character) {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        let kept = kept.trim().trim_matches('.').to_owned();
        if kept.is_empty() {
            "Unknown".to_owned()
        } else {
            kept
        }
    };
    format!(
        "{}@{}.gdtf",
        clean(&fixture.manufacturer),
        clean(&fixture.name)
    )
}

/// The whole `description.xml`.
fn description(first: &FixtureType, modes: &[&FixtureType]) -> String {
    // The attributes of every mode, once each: the table is per **fixture**.
    let mut table: BTreeMap<String, (&AttributeDef, String)> = BTreeMap::new();
    for mode in modes {
        for definition in &mode.attributes {
            let name = channel_attribute(definition);
            table.entry(name).or_insert_with(|| {
                (
                    definition,
                    definition
                        .label
                        .clone()
                        .unwrap_or_else(|| attributes::pretty(&channel_attribute(definition))),
                )
            });
        }
    }

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    xml.push_str("<GDTF DataVersion=\"1.2\">\n");
    let _ = writeln!(
        xml,
        "  <FixtureType Name=\"{name}\" ShortName=\"{short}\" LongName=\"{name}\" \
         Manufacturer=\"{manufacturer}\" Description=\"{description}\" \
         FixtureTypeID=\"{guid}\" RefFT=\"\" CanHaveChildren=\"No\">",
        name = escape(&first.name),
        short = escape(&short_name(&first.name)),
        manufacturer = escape(&first.manufacturer),
        description = escape("Written by PrismDMX from the profile the show patched"),
        guid = guid_of(first),
    );

    // Feature groups, only the ones used.
    xml.push_str("    <AttributeDefinitions>\n      <ActivationGroups/>\n      <FeatureGroups>\n");
    let mut used: Vec<FeatureGroup> = table
        .values()
        .map(|(definition, _)| definition.feature_group)
        .collect();
    used.sort();
    used.dedup();
    for group in &used {
        let (name, feature) = feature_of(*group);
        let _ = writeln!(
            xml,
            "        <FeatureGroup Name=\"{name}\" Pretty=\"{name}\"><Feature Name=\"{feature}\"/></FeatureGroup>"
        );
    }
    xml.push_str("      </FeatureGroups>\n      <Attributes>\n");
    for (name, (definition, pretty)) in &table {
        let (group, feature) = feature_of(definition.feature_group);
        let _ = writeln!(
            xml,
            "        <Attribute Name=\"{}\" Pretty=\"{}\" Feature=\"{group}.{feature}\"/>",
            escape(name),
            escape(pretty),
        );
    }
    xml.push_str("      </Attributes>\n    </AttributeDefinitions>\n");

    // The device: a tree of geometries the channels are put on.
    let moves = modes.iter().any(|mode| {
        mode.attributes
            .iter()
            .any(|a| matches!(a.attribute, AttributeType::Pan | AttributeType::Tilt))
    });
    let tree = tree::tree_of(first, moves);
    tree.write(&mut xml);

    // The modes.
    xml.push_str("    <DMXModes>\n");
    let mut seen: Vec<&str> = Vec::new();
    for mode in modes {
        // A file with two modes of one name is one the reader would keep the
        // first of; saying it twice would only make the second unreachable.
        if seen.contains(&mode.mode.as_str()) {
            continue;
        }
        seen.push(&mode.mode);
        let _ = writeln!(
            xml,
            "      <DMXMode Name=\"{}\" Geometry=\"{}\">\n        <DMXChannels>",
            escape(&mode.mode),
            escape(tree.root()),
        );
        let mut ordered: Vec<&AttributeDef> = mode.attributes.iter().collect();
        ordered.sort_by_key(|definition| definition.coarse_offset);
        for definition in ordered {
            channel(&mut xml, definition, tree.geometry_of(definition, first));
        }
        xml.push_str(
            "        </DMXChannels>\n        <Relations/>\n        <FTMacros/>\n      </DMXMode>\n",
        );
    }
    xml.push_str("    </DMXModes>\n  </FixtureType>\n</GDTF>\n");
    xml
}

/// One `DMXChannel`.
fn channel(xml: &mut String, definition: &AttributeDef, geometry: &str) {
    let attribute = channel_attribute(definition);
    let offset = definition.fine_offset.map_or_else(
        || (definition.coarse_offset + 1).to_string(),
        |fine| format!("{},{}", definition.coarse_offset + 1, fine + 1),
    );
    let bytes = if definition.fine_offset.is_some() {
        2
    } else {
        1
    };
    let function = format!("{attribute} 1");
    let _ = writeln!(
        xml,
        "          <DMXChannel DMXBreak=\"1\" Offset=\"{offset}\" Highlight=\"None\" Geometry=\"{geometry}\" \
         InitialFunction=\"{geometry}_{attr}.{attr}.{function}\">",
        geometry = escape(geometry),
        attr = escape(&attribute),
        function = escape(&function),
    );
    let _ = writeln!(
        xml,
        "            <LogicalChannel Attribute=\"{}\" Snap=\"No\" Master=\"None\" MibFade=\"0\" DMXChangeTimeLimit=\"0\">",
        escape(&attribute)
    );
    let _ = writeln!(
        xml,
        "              <ChannelFunction Name=\"{}\" Attribute=\"{}\" OriginalAttribute=\"\" DMXFrom=\"0/{bytes}\" \
         Default=\"{}\" PhysicalFrom=\"{}\" PhysicalTo=\"{}\" RealFade=\"0\" RealAcceleration=\"0\">",
        escape(&function),
        escape(&attribute),
        dmx_value(definition.default_value, bytes),
        number(definition.physical_from),
        number(definition.physical_to),
    );
    for range in &definition.ranges {
        let _ = writeln!(
            xml,
            "                <ChannelSet Name=\"{}\" DMXFrom=\"{}\"/>",
            escape(&range.name),
            dmx_value(range.from, bytes),
        );
    }
    xml.push_str("              </ChannelFunction>\n            </LogicalChannel>\n          </DMXChannel>\n");
}

/// The GDTF attribute a channel is written under.
fn channel_attribute(definition: &AttributeDef) -> String {
    attributes::name_of(
        definition.attribute,
        definition.occurrence,
        definition.label.as_deref(),
    )
}

/// A value of this desk's sixteen bits, as `DMXValue` writes it in `bytes`
/// bytes — the inverse of the reader's `dmx_value`, which **scales**.
///
/// An 8-bit channel's value is the nearest of its 256 steps: a value that came
/// from an 8-bit file is a multiple of 257 and comes back exactly.
fn dmx_value(value: u16, bytes: u32) -> String {
    if bytes >= 2 {
        format!("{value}/2")
    } else {
        let step = ((u32::from(value) + 128) / 257).min(255);
        format!("{step}/1")
    }
}

/// The feature group and feature GDTF files an attribute under.
const fn feature_of(group: FeatureGroup) -> (&'static str, &'static str) {
    match group {
        FeatureGroup::Dimmer => ("Dimmer", "Dimmer"),
        FeatureGroup::Position => ("Position", "PanTilt"),
        FeatureGroup::Gobo => ("Gobo", "Gobo"),
        FeatureGroup::Color => ("Color", "Color"),
        FeatureGroup::Beam => ("Beam", "Beam"),
        FeatureGroup::Focus => ("Focus", "Focus"),
        FeatureGroup::Control => ("Control", "Control"),
    }
}

/// The fixture's GUID — the one it came with where it has one, and one made
/// from its name where it has not.
///
/// Made, not random: the same fixture exported twice is the same identity, which
/// is what lets a planner recognise it as the fixture it already has.
fn guid_of(fixture: &FixtureType) -> String {
    if let Some(physical) = &fixture.physical {
        let given = physical.fixture_type_id.trim();
        if is_guid(given) {
            return given.to_ascii_uppercase();
        }
    }
    // FNV-1a over the manufacturer and the name, twice with different bases.
    let hash = |basis: u64| -> u64 {
        let mut value = basis;
        for byte in fixture
            .manufacturer
            .bytes()
            .chain(std::iter::once(b'/'))
            .chain(fixture.name.bytes())
        {
            value ^= u64::from(byte);
            value = value.wrapping_mul(0x0000_0100_0000_01b3);
        }
        value
    };
    let high = hash(0xcbf2_9ce4_8422_2325);
    let low = hash(0x8422_2325_cbf2_9ce4);
    format!(
        "{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
        (high >> 32) as u32,
        (high >> 16) as u16,
        // Version 4, variant 1: it is a name-made identity in a random one's
        // shape, which is all the format asks of a GUID.
        0x4000 | (high as u16 & 0x0fff),
        0x8000 | ((low >> 48) as u16 & 0x3fff),
        low & 0xffff_ffff_ffff,
    )
}

/// Whether text has the shape of a GUID.
fn is_guid(text: &str) -> bool {
    let groups: Vec<&str> = text.split('-').collect();
    groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(group, length)| {
            group.len() == length && group.chars().all(|character| character.is_ascii_hexdigit())
        })
}

/// A short name for the file's `ShortName`: capitals and digits of the name, or
/// its first letters.
fn short_name(name: &str) -> String {
    let capitals: String = name
        .chars()
        .filter(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        .collect();
    if capitals.len() >= 2 {
        capitals.chars().take(8).collect()
    } else {
        name.chars()
            .filter(|character| character.is_alphanumeric())
            .take(8)
            .collect()
    }
}

/// A number as an attribute value: six places, no trailing noise, no `-0`.
fn number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text {
        "" | "-0" => "0".to_owned(),
        other => other.to_owned(),
    }
}

/// Text as an XML attribute value.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // XML 1.0 has no way to say these at all.
            character if (character as u32) < 0x20 && !matches!(character, '\t' | '\n' | '\r') => {}
            character => out.push(character),
        }
    }
    out
}

#[cfg(test)]
mod tests;
