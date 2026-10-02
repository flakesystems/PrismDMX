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
//! and a body with the beams the profile states, or the same box with one beam
//! that the viewer draws for a profile with none.
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

use prism_domain::{AttributeDef, FeatureGroup, FixtureBeam, FixtureType, Vec3};

use super::attributes;
use crate::library::zip::Writer;

/// What a model with no size of its own is drawn as, in metres — the viewer's
/// own box for a profile that says nothing about the device.
const DEFAULT_BODY: f64 = 0.3;

/// What a beam states when a profile has none of its own to carry, in degrees.
const DEFAULT_BEAM_ANGLE: f64 = 25.0;

/// The geometry every mode names.
const BODY: &str = "Body";

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

    // The body, and its beams.
    let physical = first.physical.as_ref();
    let size = physical
        .map(|physical| physical.size)
        .filter(|size| size.x > 0.0 && size.y > 0.0 && size.z > 0.0)
        .unwrap_or(Vec3 {
            x: DEFAULT_BODY,
            y: DEFAULT_BODY,
            z: DEFAULT_BODY,
        });
    // GDTF's Length is X, Width Y and Height Z, and its Z is this desk's height.
    xml.push_str("    <Models>\n");
    let _ = writeln!(
        xml,
        "      <Model Name=\"{BODY}\" Length=\"{}\" Width=\"{}\" Height=\"{}\" PrimitiveType=\"Cube\"/>",
        number(size.x),
        number(size.z),
        number(size.y),
    );
    xml.push_str(
        "      <Model Name=\"Beam\" Length=\"0.05\" Width=\"0.05\" Height=\"0.01\" PrimitiveType=\"Cylinder\"/>\n",
    );
    xml.push_str("    </Models>\n    <Geometries>\n");
    let _ = writeln!(
        xml,
        "      <Geometry Name=\"{BODY}\" Model=\"{BODY}\" Position=\"{IDENTITY}\">"
    );
    let default_beam = [FixtureBeam {
        name: "Beam".to_owned(),
        position: Vec3 {
            x: 0.0,
            y: -size.y / 2.0,
            z: 0.0,
        },
        direction: Vec3 {
            x: 0.0,
            y: -1.0,
            z: 0.0,
        },
        beam_angle: DEFAULT_BEAM_ANGLE,
        luminous_flux: 0.0,
        color_temperature: 0.0,
    }];
    let beams: &[FixtureBeam] = match physical {
        Some(physical) if !physical.beams.is_empty() => &physical.beams,
        _ => &default_beam,
    };
    let mut names: Vec<String> = Vec::new();
    for (index, beam) in beams.iter().enumerate() {
        // Geometry names are unique in a file, and two pixels may share one.
        let mut name = if beam.name.trim().is_empty() {
            format!("Beam {}", index + 1)
        } else {
            beam.name.trim().to_owned()
        };
        while names.contains(&name) || name == BODY {
            name = format!("{name} {}", index + 1);
        }
        names.push(name.clone());
        let _ = writeln!(
            xml,
            "        <Beam Name=\"{}\" Model=\"Beam\" Position=\"{}\" LampType=\"LED\" \
             PowerConsumption=\"0\" LuminousFlux=\"{}\" ColorTemperature=\"{}\" BeamAngle=\"{}\" \
             FieldAngle=\"{}\" BeamRadius=\"0.025\" BeamType=\"Wash\" ColorRenderingIndex=\"100\"/>",
            escape(&name),
            beam_matrix(beam),
            number(beam.luminous_flux.max(0.0)),
            number(if beam.color_temperature > 0.0 {
                beam.color_temperature
            } else {
                6000.0
            }),
            number(if beam.beam_angle > 0.0 {
                beam.beam_angle
            } else {
                DEFAULT_BEAM_ANGLE
            }),
            number(if beam.beam_angle > 0.0 {
                beam.beam_angle
            } else {
                DEFAULT_BEAM_ANGLE
            }),
        );
    }
    xml.push_str("      </Geometry>\n    </Geometries>\n");

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
            "      <DMXMode Name=\"{}\" Geometry=\"{BODY}\">\n        <DMXChannels>",
            escape(&mode.mode)
        );
        let mut ordered: Vec<&AttributeDef> = mode.attributes.iter().collect();
        ordered.sort_by_key(|definition| definition.coarse_offset);
        for definition in ordered {
            channel(&mut xml, definition);
        }
        xml.push_str(
            "        </DMXChannels>\n        <Relations/>\n        <FTMacros/>\n      </DMXMode>\n",
        );
    }
    xml.push_str("    </DMXModes>\n  </FixtureType>\n</GDTF>\n");
    xml
}

/// One `DMXChannel`.
fn channel(xml: &mut String, definition: &AttributeDef) {
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
        "          <DMXChannel DMXBreak=\"1\" Offset=\"{offset}\" Highlight=\"None\" Geometry=\"{BODY}\" \
         InitialFunction=\"{BODY}_{attr}.{attr}.{function}\">",
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

/// GDTF's identity matrix, rows of four.
const IDENTITY: &str = "{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}";

/// A beam's `Position`: where it sits, and turned so that it leaves along
/// `direction`.
///
/// GDTF's beam leaves along its geometry's **−Z** and its Z is this desk's
/// height; the layout is the specification's, as `geometry::Matrix` reads it —
/// four rows of four, the rotation as columns, the translation in the fourth
/// entry of the first three.
fn beam_matrix(beam: &FixtureBeam) -> String {
    // Desk axes (x, height, depth) to GDTF's (x, depth, height).
    let wanted = [beam.direction.x, beam.direction.z, beam.direction.y];
    let length = wanted.iter().map(|part| part * part).sum::<f64>().sqrt();
    let target = if length > 1e-9 && length.is_finite() {
        [wanted[0] / length, wanted[1] / length, wanted[2] / length]
    } else {
        [0.0, 0.0, -1.0]
    };
    // A rotation taking −Z to `target`: Rodrigues, with the two parallel cases
    // spelled out because the general formula divides by their sum.
    let from = [0.0, 0.0, -1.0];
    let cosine = from[0] * target[0] + from[1] * target[1] + from[2] * target[2];
    let rotation: [[f64; 3]; 3] = if cosine > 1.0 - 1e-12 {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    } else if cosine < -1.0 + 1e-12 {
        // Straight up from straight down: half a turn about X.
        [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]
    } else {
        let axis = [
            from[1] * target[2] - from[2] * target[1],
            from[2] * target[0] - from[0] * target[2],
            from[0] * target[1] - from[1] * target[0],
        ];
        let factor = 1.0 / (1.0 + cosine);
        let [x, y, z] = axis;
        [
            [
                1.0 - factor * (y * y + z * z),
                -z + factor * x * y,
                y + factor * x * z,
            ],
            [
                z + factor * x * y,
                1.0 - factor * (x * x + z * z),
                -x + factor * y * z,
            ],
            [
                -y + factor * x * z,
                x + factor * y * z,
                1.0 - factor * (x * x + y * y),
            ],
        ]
    };
    let place = [beam.position.x, beam.position.z, beam.position.y];
    let row = |index: usize| {
        format!(
            "{{{},{},{},{}}}",
            number(rotation[index][0]),
            number(rotation[index][1]),
            number(rotation[index][2]),
            number(place[index]),
        )
    };
    format!("{}{}{}{{0,0,0,1}}", row(0), row(1), row(2))
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
