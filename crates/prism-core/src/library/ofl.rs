//! Reading the Open Fixture Library.
//!
//! <https://github.com/OpenLightingProject/open-fixture-library>, schema 12.5.1.
//! `profiles/fixtures/SOURCE.md` says which commit is vendored and how to
//! re-import it; `docs/fixture-format.md` upstream is the format's own
//! specification.
//!
//! # One `FixtureType` per **mode**, not per fixture
//!
//! An OFL fixture is a physical device with several DMX **modes** — a 9-channel
//! and a 14-channel personality of the same moving head are one file with two
//! entries in `modes`. A patched fixture in this desk has one footprint and one
//! channel layout, so a *mode* is what [`prism_domain::FixtureType`] means, and
//! that is also why that type has a `mode` field. The key is
//! `manufacturer/fixture/mode`, which is what the operator sees in the library
//! and what the show embeds.
//!
//! # The conversion is lossy, and this is exactly how
//!
//! This domain model has fifteen [`AttributeType`]s. OFL has some ninety
//! capability types and a wheel and matrix model besides. So:
//!
//! - **A mode whose channel list contains an object is skipped.** Those are
//!   matrix inserts (`{"insert": "matrixChannels"}`) and switching channels;
//!   both make the channel layout depend on state, which a fixed footprint
//!   cannot express. 714 of the vendored 2 798 modes.
//! - **A channel whose capabilities map to no attribute is dropped**, and still
//!   occupies its channel — the footprint is the mode's channel count, always,
//!   so everything after it stays at the right offset. Colour presets, effects,
//!   maintenance, sound sensitivity and the rest are in this group.
//! - **A channel that dims anywhere in its range is a dimmer.** A
//!   `Dimmer / Strobe` channel is `ShutterStrobe` at the bottom, `Intensity`
//!   through the middle and `ShutterStrobe` again at the top; it becomes a
//!   dimmer, because that is what an operator reaches for it to do, and because
//!   a fixture whose intensity is on the shutter bank is one nobody can find.
//!   Otherwise the first capability that maps wins.
//! - **The first channel claiming an attribute wins.** A mode with two dimmers
//!   keeps the lower one; the second is dropped rather than refused, because
//!   this model gives one physical parameter one value.
//! - **Colours this model has no attribute for are dropped**: UV, Cyan, Yellow,
//!   Magenta, Lime and Indigo. A CMY fixture therefore patches with its colour
//!   mixing missing, which is a real limitation and a real finding — it wants
//!   `AttributeType` widened, which is a domain change and a session of its own.
//!
//! None of that is silent: [`Conversion`] counts every one of them, and
//! `tests/fixture_library.rs` prints the totals for the whole vendored tree so a
//! re-import that made things worse is visible in the diff.
//!
//! # Nothing here trusts the file
//!
//! Every field is optional as far as this reader is concerned. A file that is
//! not JSON, a mode with no channels, a channel naming something that is not
//! defined — each is skipped and counted, and none of them is an error that
//! reaches a caller. A desk must start with a corrupt profile in its folder.

use prism_domain::{AttributeDef, AttributeType, FeatureGroup, FixtureType, MergeMode};
use serde_json::Value;

use super::LibraryEntry;

/// The greatest DMX footprint a mode may have and still be patchable.
///
/// A universe is 512 channels, so a mode wider than that cannot be addressed
/// anywhere. OFL has none, but it costs one comparison to be sure.
const MAX_FOOTPRINT: usize = prism_domain::CHANNELS_PER_UNIVERSE as usize;

/// What one run of the reader could and could not use.
///
/// Counted rather than logged, so a test can assert on it and a re-import can be
/// compared with the last one. Every field is a *loss* except the first two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Conversion {
    /// Files that were read and understood.
    pub fixtures: usize,
    /// Modes that became a [`FixtureType`].
    pub modes: usize,
    /// Modes skipped because their channel list is not a plain list of names.
    pub modes_with_inserts: usize,
    /// Modes that convert but control nothing this desk can address.
    pub modes_without_attributes: usize,
    /// Attributes mapped, over all modes.
    pub attributes: usize,
    /// Channels whose capabilities map to no [`AttributeType`].
    pub channels_unmapped: usize,
    /// Channels dropped because a lower channel already claimed that attribute.
    pub channels_duplicate: usize,
    /// Channel names a mode used that the fixture never defines.
    pub channels_undefined: usize,
    /// Files that could not be read or were not a fixture at all.
    pub files_rejected: usize,
    /// Files that are a **redirect** to another fixture rather than a fixture.
    pub redirects: usize,
}

impl Conversion {
    /// Adds another run's counts to this one.
    pub fn absorb(&mut self, other: Self) {
        self.fixtures += other.fixtures;
        self.modes += other.modes;
        self.modes_with_inserts += other.modes_with_inserts;
        self.modes_without_attributes += other.modes_without_attributes;
        self.attributes += other.attributes;
        self.channels_unmapped += other.channels_unmapped;
        self.channels_duplicate += other.channels_duplicate;
        self.channels_undefined += other.channels_undefined;
        self.files_rejected += other.files_rejected;
        self.redirects += other.redirects;
    }
}

/// Where a redirect file points, and what the fixture is called there.
///
/// OFL keeps a stub under the old key when a fixture is renamed, or when two
/// brands turn out to sell the same light under different names — the Lixada
/// Mini Moving Head RGBW and the Stage Right Stage Wash 7x10W are one device.
/// Seven of the 634 files are these.
///
/// Followed rather than skipped, because the operator is holding a light with
/// *Lixada* printed on it: the alias is searchable under the name on the box and
/// patches the channels of the fixture it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    /// What this file calls the fixture.
    pub name: String,
    /// The key it points at, as `manufacturer/fixture`.
    pub to: String,
}

/// The redirect a file is, if it is one rather than a fixture.
#[must_use]
pub fn read_redirect(source: &str) -> Option<Redirect> {
    let Ok(Value::Object(fixture)) = serde_json::from_str::<Value>(source) else {
        return None;
    };
    let to = string_at(&fixture, "redirectTo")?;
    Some(Redirect {
        name: string_at(&fixture, "name").unwrap_or_else(|| to.clone()),
        to,
    })
}

/// One fixture file, read into the modes this desk can patch.
///
/// `manufacturer` and `fixture` are the **keys** — the directory name and the
/// file stem — because that is what OFL identifies a fixture by and what a
/// `oflURL` is built from. The display name comes out of the file.
///
/// Answers an empty list, and a `files_rejected` of 1, for anything that is not
/// a fixture this reader understands. See the module documentation: a desk must
/// start with a corrupt profile in its folder.
#[must_use]
pub fn read_fixture(
    manufacturer_key: &str,
    manufacturer_name: &str,
    fixture_key: &str,
    source: &str,
) -> (Vec<(LibraryEntry, FixtureType)>, Conversion) {
    let mut counts = Conversion::default();
    let Ok(Value::Object(fixture)) = serde_json::from_str::<Value>(source) else {
        counts.files_rejected = 1;
        return (Vec::new(), counts);
    };
    let Some(Value::Array(modes)) = fixture.get("modes") else {
        counts.files_rejected = 1;
        return (Vec::new(), counts);
    };
    let name = string_at(&fixture, "name").unwrap_or_else(|| fixture_key.to_owned());
    counts.fixtures = 1;

    let channels = Channels::of(&fixture);
    let mut built = Vec::new();
    for mode in modes {
        let Some(Value::Array(entries)) = mode.get("channels") else {
            continue;
        };
        if entries.len() > MAX_FOOTPRINT {
            continue;
        }
        if entries
            .iter()
            .any(|entry| !entry.is_string() && !entry.is_null())
        {
            // A matrix insert or a switching channel: the layout depends on
            // state, and a footprint does not.
            counts.modes_with_inserts += 1;
            continue;
        }
        let Ok(footprint) = u16::try_from(entries.len()) else {
            continue;
        };
        if footprint == 0 {
            continue;
        }
        let mode_name = string_at_value(mode, "shortName")
            .or_else(|| string_at_value(mode, "name"))
            .unwrap_or_else(|| format!("{footprint}ch"));

        let (attributes, losses) = channels.attributes(entries);
        counts.attributes += attributes.len();
        counts.channels_unmapped += losses.unmapped;
        counts.channels_duplicate += losses.duplicate;
        counts.channels_undefined += losses.undefined;
        if attributes.is_empty() {
            counts.modes_without_attributes += 1;
        }
        counts.modes += 1;

        let id = format!("{manufacturer_key}/{fixture_key}/{mode_name}");
        built.push((
            LibraryEntry {
                id: id.clone(),
                manufacturer: manufacturer_name.to_owned(),
                name: name.clone(),
                mode: mode_name.clone(),
                footprint,
            },
            FixtureType {
                id,
                manufacturer: manufacturer_name.to_owned(),
                name: name.clone(),
                mode: mode_name,
                footprint,
                attributes,
            },
        ));
    }
    (built, counts)
}

/// What one mode's channel list cost.
#[derive(Debug, Default, Clone, Copy)]
struct Losses {
    unmapped: usize,
    duplicate: usize,
    undefined: usize,
}

/// A fixture's channel definitions, with its fine aliases resolved.
struct Channels<'a> {
    /// `availableChannels`, by name.
    available: Vec<(String, &'a Value)>,
    /// `templateChannels`, whose names carry a `$pixelKey` placeholder.
    templates: Vec<(String, &'a Value)>,
}

/// The placeholder a template channel's name carries.
const PIXEL_KEY: &str = "$pixelKey";

impl<'a> Channels<'a> {
    /// The two channel tables of a fixture.
    fn of(fixture: &'a serde_json::Map<String, Value>) -> Self {
        let table = |key: &str| -> Vec<(String, &'a Value)> {
            match fixture.get(key) {
                Some(Value::Object(members)) => members
                    .iter()
                    .map(|(name, value)| (name.clone(), value))
                    .collect(),
                _ => Vec::new(),
            }
        };
        Self {
            available: table("availableChannels"),
            templates: table("templateChannels"),
        }
    }

    /// The definition a mode entry names, if the fixture has one.
    ///
    /// Two lookups. The plain one is `availableChannels`. The second resolves a
    /// **template** channel: OFL names a matrix master channel `Red $pixelKey`
    /// and a mode refers to it as `Red Master`, so the placeholder is matched
    /// against the head and the tail of the name. Without it, one channel
    /// reference in eight of the vendored tree resolves to nothing.
    fn definition(&self, entry: &str) -> Option<&'a Value> {
        if let Some((_, value)) = self.available.iter().find(|(name, _)| name == entry) {
            return Some(value);
        }
        self.templates.iter().find_map(|(name, value)| {
            let (head, tail) = name.split_once(PIXEL_KEY)?;
            (entry.len() >= head.len() + tail.len()
                && entry.starts_with(head)
                && entry.ends_with(tail))
            .then_some(*value)
        })
    }

    /// Whether an entry is the fine channel of some coarse one, and which.
    ///
    /// The **first** alias only: OFL supports 24-bit and wider, and
    /// `AttributeDef` has one fine channel. A third byte is dropped, which costs
    /// a fixture the bottom eight bits of a parameter it is unlikely to be
    /// driven at.
    fn fine_of(&self, entry: &str) -> Option<String> {
        let matches = |name: &str, aliases: &Value, key: Option<&str>| -> Option<String> {
            let Value::Array(aliases) = aliases else {
                return None;
            };
            let first = aliases.first()?.as_str()?;
            let expected =
                key.map_or_else(|| first.to_owned(), |key| first.replace(PIXEL_KEY, key));
            (expected == entry).then(|| name.to_owned())
        };
        for (name, value) in &self.available {
            if let Some(aliases) = value.get("fineChannelAliases")
                && let Some(found) = matches(name, aliases, None)
            {
                return Some(found);
            }
        }
        // The same for a template channel, where both the coarse name and the
        // alias carry the placeholder and the mode has substituted a pixel key
        // into each.
        for (name, value) in &self.templates {
            let Some((head, tail)) = name.split_once(PIXEL_KEY) else {
                continue;
            };
            let Some(aliases) = value.get("fineChannelAliases") else {
                continue;
            };
            let Value::Array(list) = aliases else {
                continue;
            };
            let Some(first) = list.first().and_then(Value::as_str) else {
                continue;
            };
            let (alias_head, alias_tail) = match first.split_once(PIXEL_KEY) {
                Some(parts) => parts,
                None => continue,
            };
            if entry.len() >= alias_head.len() + alias_tail.len()
                && entry.starts_with(alias_head)
                && entry.ends_with(alias_tail)
            {
                let key = &entry[alias_head.len()..entry.len() - alias_tail.len()];
                return Some(format!("{head}{key}{tail}"));
            }
        }
        None
    }

    /// The attributes of one mode, in channel order.
    fn attributes(&self, entries: &[Value]) -> (Vec<AttributeDef>, Losses) {
        let mut attributes: Vec<AttributeDef> = Vec::new();
        let mut losses = Losses::default();
        // Which mode entry each attribute was claimed by, so a fine channel can
        // find the coarse one it belongs to.
        let mut claimed_by: Vec<(String, usize)> = Vec::new();

        for (offset, entry) in entries.iter().enumerate() {
            let Some(name) = entry.as_str() else {
                // `null`: a channel the mode leaves unused. It occupies its
                // place, which is the whole reason it is written down.
                continue;
            };
            let Ok(offset) = u16::try_from(offset) else {
                continue;
            };

            if let Some(coarse) = self.fine_of(name) {
                if let Some((_, index)) = claimed_by.iter().find(|(claimed, _)| *claimed == coarse)
                    && let Some(def) = attributes.get_mut(*index)
                    && def.fine_offset.is_none()
                {
                    def.fine_offset = Some(offset);
                }
                continue;
            }

            let Some(definition) = self.definition(name) else {
                losses.undefined += 1;
                continue;
            };
            let Some(attribute) = attribute_of(definition) else {
                losses.unmapped += 1;
                continue;
            };
            if attributes.iter().any(|def| def.attribute == attribute) {
                losses.duplicate += 1;
                continue;
            }
            attributes.push(definition_to_attribute(attribute, offset, definition));
            claimed_by.push((name.to_owned(), attributes.len() - 1));
        }
        (attributes, losses)
    }
}

/// One channel definition as an [`AttributeDef`].
fn definition_to_attribute(
    attribute: AttributeType,
    coarse_offset: u16,
    definition: &Value,
) -> AttributeDef {
    let (physical_from, physical_to) = physical_range(attribute, definition);
    AttributeDef {
        attribute,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        // OFL writes `defaultValue` in the channel's own resolution, which is
        // 8-bit unless it says otherwise; this model holds every value as
        // 16-bit. Scaling by 257 rather than by 256 is what makes 255 become
        // 65535 rather than 65280 — a dimmer that stopped one step short of
        // full at home would be the sort of fault nobody measures.
        default_value: default_value(attribute, definition),
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from,
        physical_to,
    }
}

/// The home value of a channel.
///
/// Pan and tilt default to the centre of their travel when the file does not
/// say, because a head that is patched and never touched should point at the
/// middle rather than at an end stop.
///
/// # A colour channel rests at full, whatever the file says — punch-list B1
///
/// **And *whatever the file says* is the correction.** The first attempt at this
/// deferred to a stated `defaultValue` on the grounds that it is the
/// manufacturer telling us where the channel rests, and the owner reported the
/// colours still starting at nought. Counted in the library this build ships:
/// **391 of 1131 colour channels state a default, and 387 of those state
/// zero.** So on most real fixtures the deferral was the whole of the fault.
///
/// The two are answering different questions. A manufacturer's `defaultValue`
/// is where the *lamp* parks when it is powered on with no DMX — dark, which is
/// the only safe answer a lamp can give. Where a **desk** parks a colour channel
/// is a convention of the desk, and on every console the owner has used it is
/// open: you mix by turning colour *down*, and the dimmer decides whether any of
/// it is seen. This model's `default_value` is the second question, so the
/// manufacturer's answer to the first one is not evidence about it.
///
/// It is a property of the *profile*, set here and in `crate::library::colour`
/// and nowhere else — a rule in the engine that overrode it would be a second
/// answer to a question that has one.
///
/// Everything else takes the file's value and, absent one, zero — which is what
/// OFL means by an absent `defaultValue`.
fn default_value(attribute: AttributeType, definition: &Value) -> u16 {
    if attribute.feature_group() == FeatureGroup::Color {
        return u16::MAX;
    }
    match stated_default(definition.get("defaultValue")) {
        Some(value) => value,
        None if matches!(attribute, AttributeType::Pan | AttributeType::Tilt) => 32768,
        None => 0,
    }
}

/// A stated `defaultValue`, in the two forms OFL writes it.
///
/// A number is in the channel's own resolution, which is 8-bit unless the file
/// says otherwise; scaling by 257 rather than by 256 is what makes 255 become
/// 65535 rather than 65280 — a dimmer that stopped one step short of full at
/// home would be the sort of fault nobody measures.
///
/// A **percentage string** is the other form, and it was being dropped: `as_u64`
/// answers `None` for `"50%"`, so a channel the file parked halfway came out at
/// nought and nothing said so. Ten of the shipped library's stated colour
/// defaults are written that way, which is how it was noticed.
fn stated_default(value: Option<&Value>) -> Option<u16> {
    let value = value?;
    if let Some(number) = value.as_u64() {
        return Some(
            u16::try_from(number.min(255))
                .unwrap_or(0)
                .saturating_mul(257),
        );
    }
    let text = value.as_str()?.trim();
    let percent: f64 = text.strip_suffix('%')?.trim().parse().ok()?;
    if !percent.is_finite() {
        return None;
    }
    // Rounded rather than truncated, so `"100%"` is full and `"50%"` is the
    // midpoint an operator would read as 50 %.
    let scaled = (percent.clamp(0.0, 100.0) / 100.0 * f64::from(u16::MAX)).round();
    Some(scaled as u16)
}

/// The physical range an attribute covers, in the units `AttributeDef` uses.
///
/// Only pan and tilt have one this model can read off a file: OFL writes
/// `angleStart` and `angleEnd` as strings with a unit (`"540deg"`). Everything
/// else is a percentage, which is what the generic profiles use too.
fn physical_range(attribute: AttributeType, definition: &Value) -> (f64, f64) {
    if !matches!(attribute, AttributeType::Pan | AttributeType::Tilt) {
        return (0.0, 100.0);
    }
    let angle = |key: &str| -> Option<f64> {
        let capability = definition.get("capability")?;
        let text = capability.get(key)?.as_str()?;
        let number = text.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
        number
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
    };
    match (angle("angleStart"), angle("angleEnd")) {
        // Centred on zero, which is how `ARCHITECTURE_SPEC.md` §6 writes a pan
        // range and how the 3D calculation (S30) will read it: OFL's 0…540°
        // is this desk's −270…+270°.
        (Some(start), Some(end)) if end > start => {
            let half = (end - start) / 2.0;
            (-half, half)
        }
        _ => (-135.0, 135.0),
    }
}

/// The attribute a channel's capabilities map to, if any.
///
/// **Intensity anywhere wins**, and otherwise the first capability that maps —
/// see the module documentation for why a `Dimmer / Strobe` channel is a dimmer
/// rather than a shutter.
fn attribute_of(definition: &Value) -> Option<AttributeType> {
    let one = definition.get("capability");
    let many = definition.get("capabilities").and_then(Value::as_array);
    let capabilities: Vec<&Value> = match (one, many) {
        (Some(single), _) => vec![single],
        (None, Some(list)) => list.iter().collect(),
        (None, None) => Vec::new(),
    };
    let mapped: Vec<AttributeType> = capabilities
        .into_iter()
        .filter_map(attribute_of_capability)
        .collect();
    mapped
        .iter()
        .copied()
        .find(|attribute| *attribute == AttributeType::Dimmer)
        .or_else(|| mapped.first().copied())
}

/// One capability as an attribute of this model, if there is one.
fn attribute_of_capability(capability: &Value) -> Option<AttributeType> {
    let kind = capability.get("type")?.as_str()?;
    match kind {
        "Pan" | "PanContinuous" => Some(AttributeType::Pan),
        "Tilt" | "TiltContinuous" => Some(AttributeType::Tilt),
        "Intensity" => Some(AttributeType::Dimmer),
        "ShutterStrobe" | "StrobeSpeed" | "StrobeDuration" => Some(AttributeType::Shutter),
        "Iris" | "IrisEffect" => Some(AttributeType::Iris),
        "Zoom" => Some(AttributeType::Zoom),
        "Focus" => Some(AttributeType::Focus),
        "Prism" | "PrismRotation" => Some(AttributeType::Prism),
        // Every wheel is a gobo wheel as far as this model is concerned. A
        // colour wheel is the case that costs something, and it costs less than
        // dropping the channel: an operator can still reach the wheel.
        "WheelSlot" | "WheelRotation" | "WheelShake" | "WheelSlotRotation" => {
            Some(AttributeType::Gobo)
        }
        "ColorIntensity" => colour_attribute(capability.get("color")?.as_str()?),
        _ => None,
    }
}

/// The attribute an additive colour maps to, if this model has one.
///
/// `None` for UV, Cyan, Yellow, Magenta, Lime and Indigo — see the module
/// documentation. Warm and cold white both become White, which is wrong in a way
/// an operator can see and work with; the alternative is a fixture whose white
/// does not respond at all.
fn colour_attribute(colour: &str) -> Option<AttributeType> {
    match colour {
        "Red" => Some(AttributeType::Red),
        "Green" => Some(AttributeType::Green),
        "Blue" => Some(AttributeType::Blue),
        "White" | "Warm White" | "Cold White" => Some(AttributeType::White),
        "Amber" => Some(AttributeType::Amber),
        _ => None,
    }
}

/// A string member of a JSON object.
fn string_at(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object.get(key)?.as_str().map(str::to_owned)
}

/// A string member of any JSON value.
fn string_at_value(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

/// `FeatureGroup` is not read from the file: OFL has no such notion, and this
/// model's grouping is the one the encoder banks walk.
const _: fn() -> FeatureGroup = || AttributeType::Pan.feature_group();

/// Nor is `MergeMode`: it follows from the attribute (`DMX_MERGE.md` §2).
const _: fn() -> MergeMode = || AttributeType::Dimmer.default_merge_mode();

#[cfg(test)]
mod tests {
    use super::{Conversion, read_fixture};
    use prism_domain::{AttributeType, FeatureGroup, MergeMode};
    use serde_json::json;

    /// The fixture the S44 request came with, cut down to the parts that matter.
    ///
    /// Written out here rather than read from `profiles/fixtures/`, so these
    /// assertions are about the *reader* and not about a file somebody may
    /// re-import. `tests/fixture_library.rs` is the other half: it reads the
    /// whole vendored tree.
    const STAGE_WASH: &str = r#"{
      "name": "Stage Wash 7x10W LED Moving Head",
      "availableChannels": {
        "Pan": {
          "fineChannelAliases": ["Pan fine"],
          "capability": { "type": "Pan", "angleStart": "0deg", "angleEnd": "540deg" }
        },
        "Tilt": {
          "fineChannelAliases": ["Tilt fine"],
          "capability": { "type": "Tilt", "angleStart": "0deg", "angleEnd": "270deg" }
        },
        "Pan/Tilt Speed": {
          "defaultValue": 0,
          "capability": { "type": "PanTiltSpeed", "speedStart": "fast", "speedEnd": "slow" }
        },
        "Dimmer / Strobe": {
          "capabilities": [
            { "dmxRange": [0, 7], "type": "ShutterStrobe", "shutterEffect": "Closed" },
            { "dmxRange": [8, 134], "type": "Intensity" },
            { "dmxRange": [135, 239], "type": "ShutterStrobe", "shutterEffect": "Strobe" }
          ]
        },
        "Red": { "defaultValue": 0, "capability": { "type": "ColorIntensity", "color": "Red" } },
        "Green": { "defaultValue": 0, "capability": { "type": "ColorIntensity", "color": "Green" } },
        "Blue": { "defaultValue": 0, "capability": { "type": "ColorIntensity", "color": "Blue" } },
        "White": { "defaultValue": 255, "capability": { "type": "ColorIntensity", "color": "White" } },
        "Reset": {
          "capabilities": [{ "dmxRange": [0, 149], "type": "NoFunction" }]
        }
      },
      "modes": [
        {
          "name": "9-channel",
          "shortName": "9ch",
          "channels": ["Pan", "Tilt", "Dimmer / Strobe", "Red", "Green", "Blue", "White",
                       "Pan/Tilt Speed", "Reset"]
        },
        {
          "name": "14-channel",
          "shortName": "14ch",
          "channels": ["Pan", "Pan fine", "Tilt", "Tilt fine", "Pan/Tilt Speed",
                       "Dimmer / Strobe", "Red", "Green", "Blue", "White",
                       null, null, null, "Reset"]
        }
      ]
    }"#;

    /// **A mode becomes a fixture type, and the channels land where the manual
    /// says.**
    ///
    /// Read off the JSON above by hand, offset by offset. A test that compared
    /// the reader with itself would pass for a reader that put every attribute
    /// on channel 0.
    #[test]
    fn a_mode_becomes_a_fixture_type_with_the_manufacturers_channel_order() {
        let (built, counts) = read_fixture("stage-right", "Stage Right", "stage-wash", STAGE_WASH);
        assert_eq!(built.len(), 2, "two modes, two profiles");
        assert_eq!(counts.fixtures, 1);
        assert_eq!(counts.modes, 2);

        let (entry, nine) = &built[0];
        assert_eq!(entry.id, "stage-right/stage-wash/9ch");
        assert_eq!(entry.manufacturer, "Stage Right");
        assert_eq!(entry.name, "Stage Wash 7x10W LED Moving Head");
        assert_eq!(entry.mode, "9ch");
        assert_eq!(entry.footprint, 9);
        assert_eq!(nine.id, entry.id);
        assert_eq!(
            nine.footprint, 9,
            "the footprint is the mode's channel count"
        );

        let at = |attribute: AttributeType| {
            nine.attributes
                .iter()
                .find(|def| def.attribute == attribute)
                .unwrap_or_else(|| panic!("{attribute:?} is missing"))
        };
        assert_eq!(at(AttributeType::Pan).coarse_offset, 0);
        assert_eq!(at(AttributeType::Tilt).coarse_offset, 1);
        // The dimmer is offset 2 — the `Dimmer / Strobe` channel, whose *first*
        // mapping capability is the strobe and whose second is the intensity.
        // This is the rule that says the intensity wins.
        assert_eq!(at(AttributeType::Dimmer).coarse_offset, 2);
        assert_eq!(at(AttributeType::Red).coarse_offset, 3);
        assert_eq!(at(AttributeType::Green).coarse_offset, 4);
        assert_eq!(at(AttributeType::Blue).coarse_offset, 5);
        assert_eq!(at(AttributeType::White).coarse_offset, 6);
        // `Pan/Tilt Speed` and `Reset` map to nothing and are dropped — and the
        // channels before them are unaffected, which is the point.
        assert_eq!(nine.attributes.len(), 7);
        assert_eq!(counts.channels_unmapped, 4, "two per mode");

        // The 9-channel mode is 8-bit throughout; nothing has a fine channel.
        assert!(nine.attributes.iter().all(|def| def.fine_offset.is_none()));
    }

    #[test]
    fn a_fine_channel_alias_becomes_the_fine_offset_of_the_channel_it_belongs_to() {
        let (built, _) = read_fixture("stage-right", "Stage Right", "stage-wash", STAGE_WASH);
        let (entry, fourteen) = &built[1];
        assert_eq!(entry.mode, "14ch");
        assert_eq!(fourteen.footprint, 14);

        let at = |attribute: AttributeType| {
            fourteen
                .attributes
                .iter()
                .find(|def| def.attribute == attribute)
                .unwrap_or_else(|| panic!("{attribute:?} is missing"))
        };
        assert_eq!(at(AttributeType::Pan).coarse_offset, 0);
        assert_eq!(at(AttributeType::Pan).fine_offset, Some(1));
        assert_eq!(at(AttributeType::Tilt).coarse_offset, 2);
        assert_eq!(at(AttributeType::Tilt).fine_offset, Some(3));
        // And everything after the fine channels is at the offset the mode's
        // list gives it, not at the one it would have without them.
        assert_eq!(at(AttributeType::Dimmer).coarse_offset, 5);
        assert_eq!(at(AttributeType::Red).coarse_offset, 6);
        assert_eq!(at(AttributeType::White).coarse_offset, 9);
    }

    /// A `null` entry occupies a channel and defines nothing, which is what
    /// keeps everything after it at the right offset.
    #[test]
    fn an_unused_channel_still_takes_up_its_place() {
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH);
        let fourteen = &built[1].1;
        assert_eq!(fourteen.footprint, 14);
        assert!(
            fourteen
                .attributes
                .iter()
                .all(|def| def.coarse_offset != 10 && def.coarse_offset != 11),
            "the three nulls are nobody's channels"
        );
    }

    /// Pan and tilt carry the angle the manufacturer states, centred on zero —
    /// which is how `ARCHITECTURE_SPEC.md` §6 writes a pan range.
    #[test]
    fn pan_and_tilt_carry_the_travel_the_file_states() {
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH);
        let nine = &built[0].1;
        let range = |attribute: AttributeType| {
            let def = nine
                .attributes
                .iter()
                .find(|def| def.attribute == attribute)
                .expect("it is there");
            (def.physical_from, def.physical_to)
        };
        assert_eq!(range(AttributeType::Pan), (-270.0, 270.0));
        assert_eq!(range(AttributeType::Tilt), (-135.0, 135.0));
        // A colour is a percentage, like every generic profile's.
        assert_eq!(range(AttributeType::Red), (0.0, 100.0));
    }

    #[test]
    fn a_default_value_is_scaled_to_the_sixteen_bit_this_model_holds() {
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH);
        let nine = &built[0].1;
        let home = |attribute: AttributeType| {
            nine.attributes
                .iter()
                .find(|def| def.attribute == attribute)
                .expect("it is there")
                .default_value
        };
        // 255 becomes 65535 and not 65280: a dimmer that stopped one step short
        // of full at home is the sort of fault nobody measures.
        assert_eq!(home(AttributeType::White), 65535);
        // **Red states `defaultValue: 0` in this file and rests at full anyway**
        // — punch-list B1, second attempt. Where a *lamp* parks with no DMX and
        // where a *desk* parks a colour channel are two questions, and 387 of the
        // shipped library's 391 stated colour defaults answer the first one with
        // nought. Believing them was the whole of the fault the owner reported.
        assert_eq!(home(AttributeType::Red), 65535);
        // Pan and tilt have no `defaultValue` here, so they centre.
        assert_eq!(home(AttributeType::Pan), 32768);
        assert_eq!(home(AttributeType::Tilt), 32768);
    }

    /// **A percentage default is read, rather than silently dropped.**
    ///
    /// OFL writes `defaultValue` as a number *or* as a percentage string, and
    /// `as_u64` answers `None` for the second — so a channel the file parked
    /// halfway came out at nought and nothing said so. Found while counting the
    /// colour defaults for B1; ten of them are written this way.
    #[test]
    fn a_default_value_written_as_a_percentage_is_read_as_one() {
        assert_eq!(super::stated_default(Some(&json!("100%"))), Some(65535));
        assert_eq!(super::stated_default(Some(&json!("0%"))), Some(0));
        assert_eq!(super::stated_default(Some(&json!(" 50 % "))), Some(32768));
        // And the forms that are not a value at all answer nothing, so the
        // attribute takes its own default rather than a number made up here.
        assert_eq!(super::stated_default(Some(&json!("halfway"))), None);
        assert_eq!(super::stated_default(Some(&json!("%"))), None);
        assert_eq!(super::stated_default(Some(&json!(null))), None);
        assert_eq!(super::stated_default(None), None);
        // A number is still a number, in the channel's own 8-bit resolution.
        assert_eq!(super::stated_default(Some(&json!(255))), Some(65535));
        assert_eq!(super::stated_default(Some(&json!(128))), Some(32896));
    }

    #[test]
    fn the_feature_group_and_the_merge_mode_are_this_model_s_own() {
        // OFL has neither. The bank an attribute is on is what the encoder bar
        // walks (S26) and the merge mode is `DMX_MERGE.md` §2's.
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH);
        for def in &built[0].1.attributes {
            assert_eq!(def.feature_group, def.attribute.feature_group());
            assert_eq!(def.merge_mode, def.attribute.default_merge_mode());
            assert!(!def.invert, "an invert is the operator's, not the file's");
        }
        let position = built[0]
            .1
            .attributes
            .iter()
            .find(|def| def.attribute == AttributeType::Pan)
            .expect("it pans");
        assert_eq!(position.feature_group, FeatureGroup::Position);
        assert_eq!(position.merge_mode, MergeMode::Ltp);
    }

    /// **A mode whose channel list is not a plain list of names is skipped.**
    ///
    /// A matrix insert and a switching channel both make the layout depend on
    /// state, and a footprint does not.
    #[test]
    fn a_mode_with_a_matrix_insert_is_skipped_and_counted() {
        let source = r#"{
          "name": "Bar",
          "availableChannels": { "Master": { "capability": { "type": "Intensity" } } },
          "modes": [
            { "shortName": "1ch", "channels": ["Master"] },
            { "shortName": "13ch", "channels": ["Master", { "insert": "matrixChannels" }] }
          ]
        }"#;
        let (built, counts) = read_fixture("m", "M", "bar", source);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].0.mode, "1ch");
        assert_eq!(counts.modes_with_inserts, 1);
        assert_eq!(counts.modes, 1);
    }

    /// A template channel referred to by its resolved name — OFL's matrix
    /// *master* convention, which one channel reference in eight of the
    /// vendored tree uses.
    #[test]
    fn a_template_channel_resolves_through_its_pixel_key() {
        let source = r#"{
          "name": "Pixel Bar",
          "templateChannels": {
            "Red $pixelKey": {
              "fineChannelAliases": ["Red $pixelKey fine"],
              "capability": { "type": "ColorIntensity", "color": "Red" }
            }
          },
          "modes": [
            { "shortName": "2ch", "channels": ["Red Master", "Red Master fine"] }
          ]
        }"#;
        let (built, counts) = read_fixture("m", "M", "bar", source);
        assert_eq!(counts.channels_undefined, 0);
        let attributes = &built[0].1.attributes;
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].attribute, AttributeType::Red);
        assert_eq!(attributes[0].coarse_offset, 0);
        assert_eq!(attributes[0].fine_offset, Some(1));
    }

    /// One physical parameter, one value: a second channel claiming an
    /// attribute a lower one already has is dropped rather than refused.
    #[test]
    fn a_second_channel_claiming_the_same_attribute_is_dropped() {
        let source = r#"{
          "name": "Two Dimmers",
          "availableChannels": {
            "Master": { "capability": { "type": "Intensity" } },
            "Dimmer": { "capability": { "type": "Intensity" } }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Master", "Dimmer"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source);
        let profile = &built[0].1;
        assert_eq!(profile.footprint, 2, "both channels are still occupied");
        assert_eq!(profile.attributes.len(), 1);
        assert_eq!(profile.attributes[0].coarse_offset, 0, "the lower one wins");
        assert_eq!(counts.channels_duplicate, 1);
    }

    /// The colours this model has no attribute for, dropped rather than guessed
    /// at — and counted, because it is a real limitation.
    #[test]
    fn a_colour_this_model_cannot_express_is_dropped() {
        let source = r#"{
          "name": "CMY",
          "availableChannels": {
            "Cyan": { "capability": { "type": "ColorIntensity", "color": "Cyan" } },
            "Magenta": { "capability": { "type": "ColorIntensity", "color": "Magenta" } },
            "Yellow": { "capability": { "type": "ColorIntensity", "color": "Yellow" } },
            "Dim": { "capability": { "type": "Intensity" } }
          },
          "modes": [{ "shortName": "4ch", "channels": ["Cyan", "Magenta", "Yellow", "Dim"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source);
        let profile = &built[0].1;
        assert_eq!(profile.footprint, 4);
        assert_eq!(profile.attributes.len(), 1);
        assert_eq!(profile.attributes[0].attribute, AttributeType::Dimmer);
        assert_eq!(profile.attributes[0].coarse_offset, 3);
        assert_eq!(counts.channels_unmapped, 3);
        assert_eq!(counts.modes_without_attributes, 0);
    }

    /// **A file that is not a fixture never stops anything.**
    ///
    /// A desk starts with a corrupt profile in its folder; it does not refuse
    /// to start and it does not pretend the file was fine.
    #[test]
    fn nothing_here_trusts_the_file() {
        for source in [
            "",
            "not json at all",
            "[]",
            "{}",
            r#"{ "name": "No modes" }"#,
            r#"{ "modes": "not a list" }"#,
            r#"{ "modes": [{ "channels": [] }] }"#,
            r#"{ "modes": [{ "shortName": "x" }] }"#,
        ] {
            let (built, counts) = read_fixture("m", "M", "f", source);
            assert!(built.is_empty(), "{source} produced a profile");
            assert_eq!(counts.attributes, 0, "{source}");
        }
        // These two are *readable* modes with nothing usable in them, which is
        // a different answer: the profile exists, has the right footprint, and
        // controls nothing. An operator can still patch it and see the channels
        // it occupies, which is more use than pretending the mode is not there.
        for source in [
            r#"{ "modes": [{ "channels": ["nothing defines this"] }] }"#,
            r#"{ "availableChannels": 7, "modes": [{ "channels": [null] }] }"#,
        ] {
            let (built, counts) = read_fixture("m", "M", "f", source);
            assert_eq!(built.len(), 1, "{source}");
            assert!(built[0].1.attributes.is_empty(), "{source}");
            assert_eq!(built[0].1.footprint, 1, "{source}");
            assert_eq!(counts.modes_without_attributes, 1, "{source}");
        }
        // The last two are *readable* files with nothing usable in them, so
        // they are not counted as rejected — the distinction matters when a
        // number is reported to a person.
        let (_, rejected) = read_fixture("m", "M", "f", "not json");
        assert_eq!(rejected.files_rejected, 1);
        let (_, thin) = read_fixture("m", "M", "f", r#"{ "modes": [{ "channels": [null] }] }"#);
        assert_eq!(thin.files_rejected, 0);
        assert_eq!(thin.modes, 1);
        assert_eq!(thin.modes_without_attributes, 1);
    }

    /// A mode wider than a universe cannot be addressed anywhere, so it is not
    /// offered. OFL has none; it costs one comparison to be sure.
    #[test]
    fn a_mode_wider_than_a_universe_is_not_offered() {
        let channels = (0..600).map(|_| "\"X\"").collect::<Vec<_>>().join(",");
        let source = format!(
            r#"{{ "availableChannels": {{ "X": {{ "capability": {{ "type": "Intensity" }} }} }},
                  "modes": [{{ "shortName": "600ch", "channels": [{channels}] }}] }}"#
        );
        let (built, counts) = read_fixture("m", "M", "f", &source);
        assert!(built.is_empty());
        assert_eq!(counts.modes, 0);
    }

    /// A mode with no name of its own is named after its width, so two modes of
    /// one fixture cannot collide on a key.
    #[test]
    fn a_mode_with_no_name_is_named_after_its_width() {
        let source = r#"{
          "availableChannels": { "D": { "capability": { "type": "Intensity" } } },
          "modes": [{ "channels": ["D"] }, { "name": "Extended", "channels": ["D", null] }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source);
        assert_eq!(built[0].0.id, "m/f/1ch");
        assert_eq!(
            built[1].0.id, "m/f/Extended",
            "a name is used when there is one"
        );
    }

    #[test]
    fn counts_add_up_across_files() {
        let mut total = Conversion::default();
        let (_, one) = read_fixture("m", "M", "f", STAGE_WASH);
        let (_, two) = read_fixture("m", "M", "g", STAGE_WASH);
        total.absorb(one);
        total.absorb(two);
        assert_eq!(total.fixtures, 2);
        assert_eq!(total.modes, one.modes * 2);
        assert_eq!(total.attributes, one.attributes * 2);
    }
}
