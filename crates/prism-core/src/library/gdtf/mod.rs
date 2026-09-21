//! Reading **GDTF**, the fixture definition format this desk patches from —
//! **S60**.
//!
//! [General Device Type Format](https://gdtf.eu), DIN SPEC 15800. A `.gdtf`
//! file is a ZIP archive ([`super::zip`]) holding a `description.xml`, the
//! device's 3D models, and the picture of every gobo on every wheel.
//!
//! # Why the library moved here from the Open Fixture Library
//!
//! The Open Fixture Library describes **channels**. It says a fixture has a
//! gobo wheel with seven slots and what each slot is called; it does not say
//! what the gobos look like, how big the fixture is, where its beam comes out
//! of the body, or which way the body is pointing when the yoke is at home.
//!
//! The 3D viewer (S30) needs every one of those. GDTF states all of them, in
//! the file the manufacturer publishes, and it is what the rest of this
//! industry has standardised on — a venue's MVR file, the format a rig is
//! exchanged in, refers to fixtures by their GDTF identity and by nothing else.
//!
//! So GDTF is what the installed library is, and [`super::ofl`] stays for the
//! venue's own profiles: a light nobody has published a GDTF for is a light
//! somebody has to describe by hand, and a channel list in JSON is a far kinder
//! thing to write by hand than a ZIP archive of XML. Both are read, the venue's
//! own wins, and `profiles/fixtures/SOURCE.md` says which is which.
//!
//! # One [`FixtureType`] per **DMX mode**
//!
//! Exactly as with OFL, and for the same reason: a patched fixture has one
//! footprint and one channel layout. The key is
//! `manufacturer/fixture/mode`, slugged out of what the **file** says it is
//! rather than out of the file name. That is a change
//! from OFL, where the key is the directory and the file stem, and it is the
//! better identity: two copies of one manufacturer's GDTF under two file names
//! are one fixture, and a venue that puts its own corrected copy of a Robe T1
//! in its own folder overrides the installed one without having to guess what
//! the installer called it.
//!
//! # What is read, and what is left
//!
//! Read: every DMX mode of break 1, every channel's attribute, its default, its
//! physical range, its named value ranges and — the point of the exercise —
//! **the wheel slot pictures, the device's size, its 3D model and every beam it
//! has, with where that beam sits and which way it points**.
//!
//! Left, and counted in [`Conversion`] so that a re-import which made things
//! worse is visible in a diff:
//!
//! - **A channel on a second DMX break.** A fixture with two DMX starts is two
//!   addresses, and a patched fixture in this desk has one. Counted as
//!   [`Conversion::channels_other_break`].
//! - **A virtual channel** — one with no `Offset`, which GDTF uses for a
//!   parameter that exists in the fixture's own logic and not on the wire.
//! - **A `ChannelFunction` that is not the channel's first.** GDTF lets one
//!   channel be several parameters over its range, the way OFL's switching
//!   channels do; the ranges of all of them are kept as **names**, which is
//!   what an operator reads, and the attribute is the first function's.
//!
//! Nothing is dropped silently, and an attribute this desk has no word for
//! becomes an [`AttributeType::Raw`] knob carrying GDTF's own name rather than
//! a hole in the footprint — the same answer S54 gave an unused OFL channel.
//!
//! # Nothing here trusts the file
//!
//! Every element and every attribute is optional as far as this reader is
//! concerned, every number is checked, and the whole of it answers an empty
//! list rather than an error. A desk must start with a corrupt profile in its
//! folder.

pub mod xml;

mod attributes;
mod geometry;

use std::collections::{BTreeMap, BTreeSet};

use prism_domain::{
    AttributeDef, AttributeKey, AttributeRange, AttributeType, FixturePhysical, FixtureType,
    LibraryEntry,
};

use self::geometry::Geometries;
use self::xml::Node;

/// The most channels one mode may have, which is a universe.
const MAX_FOOTPRINT: usize = prism_domain::CHANNELS_PER_UNIVERSE as usize;

/// The DMX break a patched fixture's address is.
///
/// GDTF lets a device take several DMX starts — a moving head whose lamp is on
/// a second address, an LED bar whose pixels are addressed separately. A
/// fixture in this desk has one address (`prism_domain::Fixture::address`), so
/// break 1 is what is patched and anything else is counted and left.
const PATCHED_BREAK: u32 = 1;

/// What one run of the reader could and could not use.
///
/// Counted rather than logged, so a test can assert on it and an installed
/// library can be compared with the last one. Every field is a *loss* except
/// the first two and the last three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Conversion {
    /// Files that were read and understood.
    pub fixtures: usize,
    /// DMX modes that became a [`FixtureType`].
    pub modes: usize,
    /// Modes that convert but control nothing this desk can address.
    pub modes_without_attributes: usize,
    /// Attributes mapped, over all modes.
    pub attributes: usize,
    /// Channels this desk has no word for, which became a raw knob.
    ///
    /// Not a hole: the channel keeps its place in the footprint and its name,
    /// and an operator can still put a value on it. Counted because a large
    /// number here is a table that has fallen behind the format.
    pub channels_raw: usize,
    /// Channels on a DMX break this fixture is not patched at.
    pub channels_other_break: usize,
    /// Channels with no `Offset` — a parameter of the fixture's own logic that
    /// is not on the wire.
    pub channels_virtual: usize,
    /// Channels whose attribute was already claimed, and were renumbered.
    ///
    /// `Shutter1` and `Shutter1Strobe` are both this desk's `Shutter`, and two
    /// attributes of one fixture may not share a key. The second becomes
    /// *Shutter 2* — see [`AttributeKey`].
    pub channels_renumbered: usize,
    /// Files that are not a GDTF archive, or whose XML would not parse.
    pub files_rejected: usize,
    /// Wheel slot pictures carried onto a named range.
    pub wheel_media: usize,
    /// Beams found in the geometry tree, over all modes.
    pub beams: usize,
    /// Modes whose geometry names a 3D model for the body.
    pub models: usize,
}

impl Conversion {
    /// Adds another run's counts to this one.
    pub fn absorb(&mut self, other: Self) {
        self.fixtures += other.fixtures;
        self.modes += other.modes;
        self.modes_without_attributes += other.modes_without_attributes;
        self.attributes += other.attributes;
        self.channels_raw += other.channels_raw;
        self.channels_other_break += other.channels_other_break;
        self.channels_virtual += other.channels_virtual;
        self.channels_renumbered += other.channels_renumbered;
        self.files_rejected += other.files_rejected;
        self.wheel_media += other.wheel_media;
        self.beams += other.beams;
        self.models += other.models;
    }
}

/// Reads a `.gdtf` archive into the modes this desk can patch.
///
/// `own` says whether this is the venue's own file rather than one the
/// installer wrote — punch-list **B43**, and the flag the picker marks a row
/// with.
///
/// Answers an empty list and a `files_rejected` of 1 for anything that is not a
/// GDTF archive this reader understands: a truncated download, a ZIP with no
/// `description.xml`, XML that will not parse. See the module documentation: a
/// desk must start with a corrupt profile in its folder.
#[must_use]
pub fn read_archive(bytes: &[u8], own: bool) -> (Vec<(LibraryEntry, FixtureType)>, Conversion) {
    let rejected = (
        Vec::new(),
        Conversion {
            files_rejected: 1,
            ..Conversion::default()
        },
    );
    let Some(archive) = super::zip::Archive::read(bytes) else {
        return rejected;
    };
    let Some(name) = archive.find("description.xml") else {
        return rejected;
    };
    let Some(source) = archive.file(name) else {
        return rejected;
    };
    read_description(&source, own)
}

/// Reads an unpacked `description.xml`.
///
/// The same thing [`read_archive`] does once it has the bytes out of the ZIP,
/// and public because that is how a GDTF is kept on disk once an installer has
/// unpacked it: a directory with `description.xml` in it, beside the models and
/// the wheel pictures the viewer will want. Reading the description alone does
/// not unpack the models.
#[must_use]
pub fn read_description(
    source: &[u8],
    own: bool,
) -> (Vec<(LibraryEntry, FixtureType)>, Conversion) {
    let mut counts = Conversion::default();
    let Some(root) = xml::parse(source) else {
        counts.files_rejected = 1;
        return (Vec::new(), counts);
    };
    // The root is `GDTF`; a file that opens straight onto `FixtureType` is not
    // one this format produces, but it costs nothing to read.
    let Some(fixture) = root.child("FixtureType").or({
        if root.name == "FixtureType" {
            Some(&root)
        } else {
            None
        }
    }) else {
        counts.files_rejected = 1;
        return (Vec::new(), counts);
    };

    let name = non_empty(fixture.get("Name")).unwrap_or("Fixture");
    let manufacturer = non_empty(fixture.get("Manufacturer")).unwrap_or(UNKNOWN_MANUFACTURER);
    let key = format!("{}/{}", slug(manufacturer), slug(name));
    counts.fixtures = 1;

    let wheels = Wheels::of(fixture);
    let pretty = PrettyNames::of(fixture);
    let geometries = Geometries::of(fixture);

    let mut built = Vec::new();
    let Some(modes) = fixture.child("DMXModes") else {
        return (built, counts);
    };
    for mode in modes.children_named("DMXMode") {
        let mode_name = non_empty(mode.get("Name")).unwrap_or("Mode").to_owned();
        let Some((attributes, losses, footprint)) = read_mode(mode, &wheels, &pretty) else {
            continue;
        };
        counts.attributes += attributes.len();
        counts.channels_raw += losses.raw;
        counts.channels_other_break += losses.other_break;
        counts.channels_virtual += losses.virtual_channels;
        counts.channels_renumbered += losses.renumbered;
        counts.wheel_media += losses.media;
        if attributes.is_empty() {
            counts.modes_without_attributes += 1;
        }
        counts.modes += 1;

        let physical = geometries.physical(mode.get("Geometry"), fixture.get("FixtureTypeID"));
        counts.beams += physical.beams.len();
        if physical.model.is_some() {
            counts.models += 1;
        }

        let id = format!("{key}/{}", slug(&mode_name));
        built.push((
            LibraryEntry {
                id: id.clone(),
                manufacturer: manufacturer.to_owned(),
                name: name.to_owned(),
                mode: mode_name.clone(),
                footprint,
                own,
                gdtf: true,
            },
            FixtureType {
                id,
                manufacturer: manufacturer.to_owned(),
                name: name.to_owned(),
                mode: mode_name,
                footprint,
                attributes,
                physical: Some(physical),
            },
        ));
    }
    (built, counts)
}

/// The manufacturer a file that names none is filed under.
const UNKNOWN_MANUFACTURER: &str = "Unknown";

/// What one mode's channel list cost.
#[derive(Debug, Default, Clone, Copy)]
struct Losses {
    raw: usize,
    other_break: usize,
    virtual_channels: usize,
    renumbered: usize,
    media: usize,
}

/// One `DMXMode`, as the attributes it patches and the width it occupies.
///
/// `None` when the mode has no channel on the patched break at all, or when it
/// is wider than a universe — both are modes a fixture cannot be patched in.
fn read_mode(
    mode: &Node,
    wheels: &Wheels,
    pretty: &PrettyNames,
) -> Option<(Vec<AttributeDef>, Losses, u16)> {
    let channels = mode.child("DMXChannels")?;
    let mut losses = Losses::default();
    let mut attributes: Vec<AttributeDef> = Vec::new();
    let mut taken: BTreeSet<AttributeKey> = BTreeSet::new();
    let mut footprint: usize = 0;

    for channel in channels.children_named("DMXChannel") {
        let break_of = channel.get("DMXBreak");
        // `Overwrite` is what a geometry reference writes when it takes over
        // the parent's break, which is the patched one.
        let patched = break_of.is_empty()
            || break_of.eq_ignore_ascii_case("Overwrite")
            || break_of.parse::<u32>() == Ok(PATCHED_BREAK);
        if !patched {
            losses.other_break += 1;
            continue;
        }
        let Some(offsets) = offsets_of(channel.get("Offset")) else {
            losses.virtual_channels += 1;
            continue;
        };
        let coarse = offsets.0;
        let last = offsets.1.unwrap_or(coarse);
        footprint = footprint.max(usize::from(coarse).max(usize::from(last)));
        if footprint > MAX_FOOTPRINT {
            return None;
        }

        let Some(definition) = channel_definition(channel, wheels, pretty) else {
            continue;
        };
        let (attribute, stated) = definition.attribute;
        if attribute == AttributeType::Raw {
            losses.raw += 1;
        }
        // Two channels of one fixture may not carry one key — the merge refuses
        // a rig that does (`MergeError::DuplicateAttribute`). GDTF's own
        // numbering is used where it states one; where two of its names mean
        // one of this desk's, the second takes the next free number.
        let mut occurrence = stated;
        while !taken.insert(AttributeKey::new(attribute, occurrence)) {
            occurrence = occurrence.saturating_add(1);
            losses.renumbered += 1;
            if occurrence == u8::MAX {
                break;
            }
        }
        losses.media += definition
            .ranges
            .iter()
            .filter(|range| range.media.is_some())
            .count();

        attributes.push(AttributeDef {
            switched: None,
            attribute,
            label: definition.label,
            occurrence,
            feature_group: attribute.feature_group(),
            coarse_offset: coarse - 1,
            fine_offset: offsets.1.map(|fine| fine - 1),
            default_value: definition.default,
            merge_mode: attribute.default_merge_mode(),
            invert: false,
            physical_from: definition.physical_from,
            physical_to: definition.physical_to,
            ranges: definition.ranges,
        });
    }
    let footprint = u16::try_from(footprint).ok()?;
    if footprint == 0 {
        return None;
    }
    Some((attributes, losses, footprint))
}

/// One channel's coarse and fine offsets, **one-based**, as `Offset` states
/// them.
///
/// GDTF writes them most significant first: `"1,2"` is a 16-bit parameter whose
/// coarse byte is channel 1. A third byte is dropped — [`AttributeDef`] carries
/// one fine channel, and the third byte of a pan is a quarter of a step.
///
/// `None` for a virtual channel: `Offset` absent, empty, or the format's own
/// word `None`.
fn offsets_of(offset: &str) -> Option<(u16, Option<u16>)> {
    if offset.is_empty() || offset.eq_ignore_ascii_case("None") {
        return None;
    }
    let mut numbers = offset
        .split(',')
        .filter_map(|part| part.trim().parse::<u16>().ok())
        .filter(|number| *number >= 1 && *number <= MAX_FOOTPRINT as u16);
    let coarse = numbers.next()?;
    Some((coarse, numbers.next()))
}

/// What one channel turns into, before it is given a number.
struct Definition {
    /// The attribute and the occurrence GDTF's own name states, nought-based.
    attribute: (AttributeType, u8),
    /// The manufacturer's word for this channel — S53's `label`.
    label: Option<String>,
    /// Where it rests.
    default: u16,
    /// The physical range the primary function states.
    physical_from: f64,
    physical_to: f64,
    /// Its named ranges, in value order.
    ranges: Vec<AttributeRange>,
}

/// Reads one `DMXChannel`.
///
/// The channel's attribute is its **first** logical channel's, and its physical
/// range the first channel function's: GDTF allows a channel to be several
/// parameters over its range, and which one it *is* has to be one answer. Every
/// function's channel sets become named ranges, so the rest of it is still
/// readable on the encoder.
fn channel_definition(channel: &Node, wheels: &Wheels, pretty: &PrettyNames) -> Option<Definition> {
    let logical = channel.child("LogicalChannel")?;
    let name = logical.get("Attribute");
    let functions: Vec<&Node> = logical.children_named("ChannelFunction").collect();
    let first = functions.first().copied();
    // GDTF 1.0 put the attribute on the logical channel and 1.1 repeated it on
    // the function. Either is the same answer; a file that states only the
    // second is read from the second.
    let name = if name.is_empty() {
        first.map_or("", |function| function.get("Attribute"))
    } else {
        name
    };
    let attribute = attributes::of(name);

    let default = first
        .and_then(|function| dmx_value(function.get("Default")))
        .or_else(|| dmx_value(channel.get("Default")))
        .unwrap_or_else(|| attributes::home(attribute.0));
    let (physical_from, physical_to) = first.map_or((0.0, 100.0), |function| {
        (
            number(function.get("PhysicalFrom")).unwrap_or(0.0),
            number(function.get("PhysicalTo")).unwrap_or(1.0),
        )
    });

    let mut ranges = Vec::new();
    for function in &functions {
        wheels.ranges(function, &mut ranges);
    }
    close_ranges(&mut ranges);

    Some(Definition {
        attribute,
        // The pretty name GDTF gives the attribute is this desk's `label` —
        // the manufacturer's word, shown on the encoder in place of ours. The
        // channel function's own name is not used: it is *Pan 1* on every
        // moving head published and says nothing the attribute does not.
        label: non_empty(name).map(|name| pretty.of_name(name)),
        default,
        physical_from,
        physical_to,
        ranges,
    })
}

/// Sorts named ranges and closes the gaps between them.
///
/// A `ChannelSet` states where it starts and the next one's start is where it
/// ends, exactly as an OFL capability's does (S51). Written here rather than
/// read off the file because GDTF states only `DMXFrom`: the last range runs to
/// the top of the channel, and two ranges that both start at one value leave
/// only the first.
fn close_ranges(ranges: &mut Vec<AttributeRange>) {
    ranges.sort_by_key(|range| range.from);
    ranges.dedup_by_key(|range| range.from);
    for index in 0..ranges.len() {
        let end = ranges
            .get(index + 1)
            .map_or(u16::MAX, |next| next.from.saturating_sub(1));
        if let Some(range) = ranges.get_mut(index) {
            range.to = end.max(range.from);
        }
    }
}

/// What a fixture calls each of its attributes, in words — **S60**.
///
/// GDTF's `AttributeDefinitions` table states a `Pretty` beside every
/// attribute name: `Dim` for `Dimmer`, `Gobo1 <> ` for a gobo wheel's index.
/// That is the manufacturer's own word, which is exactly what
/// [`AttributeDef::label`] is for (S53) — so it is used where the file gives
/// one, and [`attributes::pretty`] spaces out the attribute's own name where
/// it does not.
#[derive(Debug, Default)]
struct PrettyNames(BTreeMap<String, String>);

impl PrettyNames {
    /// A fixture's `AttributeDefinitions/Attributes` table.
    fn of(fixture: &Node) -> Self {
        let mut names = BTreeMap::new();
        let Some(table) = fixture.path(&["AttributeDefinitions", "Attributes"]) else {
            return Self(names);
        };
        for attribute in table.children_named("Attribute") {
            let (Some(name), Some(pretty)) = (
                non_empty(attribute.get("Name")),
                non_empty(attribute.get("Pretty")),
            ) else {
                continue;
            };
            names.insert(name.to_owned(), pretty.to_owned());
        }
        Self(names)
    }

    /// What to show on the encoder for one attribute name.
    fn of_name(&self, name: &str) -> String {
        self.0
            .get(name)
            .cloned()
            .unwrap_or_else(|| attributes::pretty(name))
    }
}

/// The wheels a fixture declares, by name.
///
/// Held for one reason: a `ChannelFunction` says *this range of this channel
/// selects a slot of that wheel*, and the slot is where the gobo's **picture**
/// is named. Without this the desk would carry the ranges and not the images,
/// which is the half of the job OFL already did.
#[derive(Debug, Default)]
struct Wheels(BTreeMap<String, Vec<Slot>>);

/// One slot of a wheel: what it is called and what it looks like.
#[derive(Debug, Clone)]
struct Slot {
    name: String,
    media: Option<String>,
}

impl Wheels {
    /// Every `Wheel` of a fixture, with its slots in the order it lists them.
    fn of(fixture: &Node) -> Self {
        let mut wheels = BTreeMap::new();
        let Some(node) = fixture.child("Wheels") else {
            return Self(wheels);
        };
        for wheel in node.children_named("Wheel") {
            let name = wheel.get("Name").to_owned();
            if name.is_empty() {
                continue;
            }
            let slots = wheel
                .children_named("Slot")
                .enumerate()
                .map(|(index, slot)| Slot {
                    name: non_empty(slot.get("Name"))
                        .map_or_else(|| format!("Slot {}", index + 1), str::to_owned),
                    media: non_empty(slot.get("MediaFileName")).map(str::to_owned),
                })
                .collect();
            wheels.insert(name, slots);
        }
        Self(wheels)
    }

    /// Appends one channel function's named ranges.
    ///
    /// A `ChannelSet` names itself; where it does not, and where it points at a
    /// wheel slot, the slot's name is used — which is how *Gobo 3* comes out as
    /// *Triangles* with the picture of the triangles beside it.
    fn ranges(&self, function: &Node, into: &mut Vec<AttributeRange>) {
        let wheel = self.0.get(function.get("Wheel"));
        for set in function.children_named("ChannelSet") {
            let Some(from) = dmx_value(set.get("DMXFrom")) else {
                continue;
            };
            // One-based, and nought is the format's own word for *this set
            // selects no slot*.
            let slot = set
                .get("WheelSlotIndex")
                .parse::<usize>()
                .ok()
                .filter(|index| *index >= 1)
                .and_then(|index| wheel.and_then(|slots| slots.get(index - 1)));
            let name = non_empty(set.get("Name"))
                .map(str::to_owned)
                .or_else(|| slot.map(|slot| slot.name.clone()));
            let Some(name) = name else {
                continue;
            };
            into.push(AttributeRange {
                name,
                from,
                // Closed by `close_ranges` once every function has been read.
                to: from,
                media: slot.and_then(|slot| slot.media.clone()),
            });
        }
    }
}

/// A GDTF `DMXValue` — `"128/1"`, `"32768/2"` — scaled to this desk's 16 bits.
///
/// The number after the slash is how many **bytes** the value is written in, so
/// `"255/1"` and `"65535/2"` are both *full*. A trailing `s` means the file
/// wants the value shifted rather than scaled when it is widened; the two agree
/// at nought and at full and differ by less than a step in between, and this
/// desk scales — which is what [`super::ofl`] does with an OFL capability.
///
/// `None` when there is no value there at all.
fn dmx_value(value: &str) -> Option<u16> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (number, bytes) = match value.split_once('/') {
        Some((number, bytes)) => (number, bytes.trim_end_matches(['s', 'S'])),
        None => (value, "1"),
    };
    let number: u64 = number.trim().parse().ok()?;
    let bytes: u32 = bytes.trim().parse().ok()?;
    if bytes == 0 || bytes > 8 {
        return None;
    }
    let maximum = 256_u64.checked_pow(bytes)?.saturating_sub(1);
    if maximum == 0 {
        return None;
    }
    let scaled = number.min(maximum) * u64::from(u16::MAX) / maximum;
    u16::try_from(scaled).ok()
}

/// A floating-point attribute, when it is one and is finite.
fn number(value: &str) -> Option<f64> {
    let parsed: f64 = value.trim().parse().ok()?;
    parsed.is_finite().then_some(parsed)
}

/// The string, unless it is empty or only spaces.
fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// A name as a key: lower case, and anything that is not a letter or a digit is
/// a dash.
///
/// The Open Fixture Library's own directory and file names are already in this
/// shape, which is the point — a GDTF key and an OFL key are the same kind of
/// string, so [`super::fixture_key`] splits both and a venue's OFL correction
/// of a GDTF fixture lands on the same key if it is named for it.
fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut dash = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
            dash = false;
        } else if character.is_alphanumeric() {
            // A manufacturer with a non-ASCII name keeps its letters rather
            // than becoming a row of dashes.
            out.extend(character.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "fixture".to_owned()
    } else {
        out
    }
}

/// What a mode with no geometry to speak of carries.
///
/// Not `None`: a GDTF fixture always states *something* physical, and a viewer
/// that is told a device has no beams draws a body with none, which is right
/// for a dimmer pack and is a different claim from *this profile predates the
/// format*.
pub(crate) fn empty_physical(fixture_type_id: &str) -> FixturePhysical {
    FixturePhysical {
        fixture_type_id: fixture_type_id.trim().to_owned(),
        size: prism_domain::Vec3::ZERO,
        model: None,
        beams: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
