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
//! This domain model has forty [`AttributeType`]s. The format defines **43
//! capability types**, and — the part that took until S53 to see — it states
//! **discriminators** beside them: properties that split one type into distinct
//! physical parameters. `ColorIntensity` carries a `color`; `WheelSlot` carries
//! a `wheel`, whose slots have types of their own; `BladeInsertion` and
//! `BladeRotation` carry a `blade`; `Fog` carries a `fogType`. Reading the type
//! alone is lossy in a way **no counter shows** — nothing is *unmapped*, it just
//! arrives under the wrong knob. So:
//!
//! - **A matrix insert is written out** — S52. `{"insert": "matrixChannels"}`
//!   means *repeat these template channels once per pixel*, and until S52 a
//!   mode containing one was skipped whole: 90 of the 634 installed profiles
//!   could not be patched at all. `super::matrix` resolves them, and it is this
//!   session that could, because an expanded matrix is a fixture with eight
//!   reds — which is what an attribute's **occurrence** is for.
//! - **A switching channel is still skipped**, and the difference is worth
//!   naming: a matrix insert depends on the fixture's own geometry, which the
//!   file states, and a switching channel depends on another channel's *value*,
//!   which is a footprint that changes while the show runs.
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
//! - **A second channel of a kind is numbered, not dropped** — S52. A head with
//!   two colour wheels keeps both, as `ColorWheel` and `ColorWheel 2`, in the
//!   manufacturer's own order, which is channel order. Where the file states
//!   *which* one this is — a framing shutter's `blade` — that is the number
//!   instead, so `Blade 3` is physically blade three (S53). Before S52 the first
//!   channel claiming an attribute won and the rest were counted as duplicates:
//!   **2 679 channels** of the installed library.
//! - **Every colour the format names has an attribute** — S51 for CMY, UV, lime
//!   and indigo, S52 for warm and cold white. A CMY head patches with its colour
//!   mixing, and a lamp with a dedicated warm *and* cold white keeps both
//!   channels rather than folding them onto `White` and losing one.
//! - **A wheel goes to the bank its slots say it belongs on** — S53, and to the
//!   bank **most** of them say: a wheel whose slots are colours is a colour
//!   wheel whatever its manufacturer called it, and one that mixes an iris into
//!   a gobo wheel is still a gobo wheel. The wheel's *name* is never consulted;
//!   it is free text.
//! - **The channel's own name travels with it** — S53, on
//!   [`prism_domain::AttributeDef::label`], so the encoder can read *Rotating
//!   Gobo* rather than this desk's word. It is a **label and never a key**.
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

use prism_domain::{
    AttributeDef, AttributeRange, AttributeType, FeatureGroup, FixtureType, MergeMode,
};
use std::collections::BTreeMap;

use serde_json::Value;

use super::LibraryEntry;
use super::matrix::{Matrix, PIXEL_KEY};

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
    ///
    /// **Nought over the whole installed library since S51** (punch-list B38),
    /// and `crates/prism-core/tests/fixture_library.rs` asserts it rather than
    /// printing it: every capability type the Open Fixture Library has now maps
    /// to an attribute and a bank, so a channel that reaches this counter is a
    /// capability type that did not exist when the table was written.
    pub channels_unmapped: usize,
    /// Channels whose every capability is OFL's `NoFunction` — **S51**.
    ///
    /// Not a loss and not a mapping gap: the file is saying the channel does
    /// nothing, in as many words. It occupies its place in the footprint like
    /// any other. Counted separately so that [`Self::channels_unmapped`] can be
    /// held to nought and still mean something.
    pub channels_without_function: usize,
    /// Channels dropped because a lower channel already claimed that attribute.
    pub channels_duplicate: usize,
    /// Mode entries that are `null` — **S54**.
    ///
    /// The mode's own word for *this slot is unused*. It becomes an
    /// [`AttributeType::Raw`] knob named `Ch 7` rather than a hole, because
    /// *unused* is the profile author's judgement and an operator may still
    /// need the channel.
    pub channels_unused: usize,
    /// Fine bytes with no coarse channel here to belong to — **S54**.
    ///
    /// Either the coarse channel was itself raw, or this is a third byte and
    /// [`AttributeDef`] carries one fine channel. Both are still slots, so both
    /// are still knobs.
    pub channels_orphan_fine: usize,
    /// **Switching aliases resolved to what their positions agree on** — S54.
    ///
    /// Not a loss: the opposite. A mode entry naming a channel the fixture
    /// never defines, whose every switched resolution is the same parameter, is
    /// that parameter. Counted so the number can be watched — **111 of the
    /// installed corpus**, out of 377 switched slots.
    pub channels_switched: usize,
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
        self.channels_without_function += other.channels_without_function;
        self.channels_duplicate += other.channels_duplicate;
        self.channels_unused += other.channels_unused;
        self.channels_orphan_fine += other.channels_orphan_fine;
        self.channels_switched += other.channels_switched;
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
    own: bool,
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
    let matrix = Matrix::of(&fixture, source);
    let mut built = Vec::new();
    for mode in modes {
        let Some(Value::Array(written)) = mode.get("channels") else {
            continue;
        };
        // **S52 — a matrix insert is written out.** Until S52 any mode whose
        // channel list held an object was skipped whole, because the layout
        // depended on state and a footprint does not. A matrix insert's state
        // is the fixture's own geometry, which the file states, so it can be
        // resolved: 90 of the 634 installed profiles were unpatchable for it.
        // What still cannot be is a **switching channel**, whose layout depends
        // on another channel's value while the show runs — `expand` answers
        // `None` for one, and that is what this counter now counts.
        let expanded;
        let entries: &[Value] = if written
            .iter()
            .any(|entry| !entry.is_string() && !entry.is_null())
        {
            match matrix.expand(written) {
                Some(list) => {
                    expanded = list;
                    &expanded
                }
                None => {
                    counts.modes_with_inserts += 1;
                    continue;
                }
            }
        } else {
            written
        };
        if entries.len() > MAX_FOOTPRINT {
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
        counts.channels_without_function += losses.without_function;
        counts.channels_unused += losses.unused;
        counts.channels_orphan_fine += losses.orphan_fine;
        counts.channels_switched += losses.switched;
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
                own,
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
    without_function: usize,
    /// Mode entries that are `null` — the mode's own word for *unused*.
    unused: usize,
    /// Fine bytes with no coarse channel here to belong to.
    orphan_fine: usize,
    /// Switching aliases resolved to the parameter their positions agree on.
    switched: usize,
}

/// A fixture's channel definitions, with its fine aliases resolved.
struct Channels<'a> {
    /// `availableChannels`, by name.
    available: Vec<(String, &'a Value)>,
    /// `templateChannels`, whose names carry a `$pixelKey` placeholder.
    templates: Vec<(String, &'a Value)>,
    /// The fixture's wheels, so a slot can be called what it is called — S52.
    wheels: Wheels,
    /// **Every switching channel the fixture declares** — S54, `B49`.
    ///
    /// A capability may carry `switchChannels`, a map from an **alias** the
    /// mode lists to the channel that occupies that slot while this capability's
    /// range is live. The alias is never in `availableChannels`, so before S54
    /// the mode named a channel the reader had never heard of and the slot was
    /// dropped: **377 of the installed library's channels**, 34 of the 35 of a
    /// `glp/knv-cube`.
    ///
    /// The footprint does **not** move — an alias is one slot in every position
    /// — so the only open question is what the slot *is*, and the file answers
    /// it as a **set**: every channel the alias can be. Where every one of them
    /// is the same parameter this desk knows, the alias is that parameter (68
    /// aliases of the corpus, 111 mode channels). Where they disagree — 178
    /// aliases — the slot is [`AttributeType::Raw`] under its own name, because
    /// a knob labelled *Colour Wheel* that is a gobo in half the positions is
    /// worse than one labelled *Channel 2*.
    switches: BTreeMap<String, Vec<String>>,
}

impl<'a> Channels<'a> {
    /// The two channel tables of a fixture, and its wheels.
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
        let available = table("availableChannels");
        let templates = table("templateChannels");
        let mut switches: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (_, definition) in available.iter().chain(templates.iter()) {
            for capability in capabilities_of(definition) {
                let Some(Value::Object(map)) = capability.get("switchChannels") else {
                    continue;
                };
                for (alias, target) in map {
                    let Some(target) = target.as_str() else {
                        continue;
                    };
                    let targets = switches.entry(alias.clone()).or_default();
                    if !targets.iter().any(|seen| seen == target) {
                        targets.push(target.to_owned());
                    }
                }
            }
        }
        Self {
            available,
            templates,
            wheels: Wheels::of(fixture),
            switches,
        }
    }

    /// What a **switching alias** is, where every channel it can be agrees —
    /// S54.
    ///
    /// `None` for a name that is not an alias, and for one whose resolutions
    /// disagree about the parameter or name a channel the fixture never
    /// defines. The occurrence is deliberately dropped: which colour wheel a
    /// switched slot is depends on the position it is in, and channel order
    /// will number it among the ones actually present.
    fn switched(&self, entry: &str) -> Option<AttributeType> {
        let targets = self.switches.get(entry)?;
        let mut agreed: Option<AttributeType> = None;
        for target in targets {
            let definition = self.definition(target)?;
            if does_nothing(definition) {
                // A position in which the slot does nothing says nothing about
                // what the slot is in the others.
                continue;
            }
            let found = attribute_of(definition, &self.wheels, target)?.attribute;
            match agreed {
                None => agreed = Some(found),
                Some(seen) if seen == found => {}
                Some(_) => return None,
            }
        }
        agreed
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

        // **S54 — the floor.** Every branch below that used to `continue` left
        // the slot with no knob on it: the fixture kept its footprint, the desk
        // drove that channel to nought for ever, and **no counter noticed**,
        // because `channels_unmapped` only ever asked whether a channel reached
        // *an* attribute. 707 slots of the installed library were in that state,
        // 34 of the 35 of a `glp/knv-cube`. Each branch now falls through to a
        // raw knob instead, and the counters stay as **diagnostics** — they say
        // what the reader did not understand, which is a different question from
        // what an operator can reach.
        let raw = |attributes: &mut Vec<AttributeDef>,
                   offset: u16,
                   definition: Option<&Value>,
                   channel: Option<&str>| {
            let occurrence = attributes
                .iter()
                .filter(|def| def.attribute == AttributeType::Raw)
                .count();
            if let Ok(occurrence) = u8::try_from(occurrence) {
                attributes.push(raw_attribute(occurrence, offset, definition, channel));
            }
        };

        for (offset, entry) in entries.iter().enumerate() {
            let Ok(offset) = u16::try_from(offset) else {
                continue;
            };
            let Some(name) = entry.as_str() else {
                // `null`: a channel the mode leaves unused. It occupies its
                // place, which is the whole reason it is written down — and
                // since S54 it gets a knob, because *unused* is the profile
                // author's word and an operator may still need the slot.
                losses.unused += 1;
                raw(&mut attributes, offset, None, None);
                continue;
            };

            if let Some(coarse) = self.fine_of(name) {
                if let Some((_, index)) = claimed_by.iter().find(|(claimed, _)| *claimed == coarse)
                    && let Some(def) = attributes.get_mut(*index)
                    && def.fine_offset.is_none()
                {
                    def.fine_offset = Some(offset);
                } else {
                    // A fine byte whose coarse channel is not here — either it
                    // was dropped, or it is a third byte and `AttributeDef` has
                    // one fine channel. It is still a slot, so it is still a
                    // knob — S54.
                    losses.orphan_fine += 1;
                    raw(&mut attributes, offset, self.definition(name), Some(name));
                }
                continue;
            }

            // **A switching alias** — S54, `B49`. The mode names a channel the
            // fixture never defines because *which* channel it is depends on
            // another channel's value. Where every position the file describes
            // agrees about the parameter, that is what it is.
            if let Some(attribute) = self.switched(name) {
                losses.switched += 1;
                let occurrence = u8::try_from(
                    attributes
                        .iter()
                        .filter(|def| def.attribute == attribute)
                        .count(),
                );
                if let Ok(occurrence) = occurrence {
                    let mut def = raw_attribute(occurrence, offset, None, Some(name));
                    def.attribute = attribute;
                    def.feature_group = attribute.feature_group();
                    def.merge_mode = attribute.default_merge_mode();
                    // **Where it rests is the desk's question here, not the
                    // file's.** The alias carries no `defaultValue` of its own
                    // — which channel is live decides that — so the answer is
                    // the one `default_value` gives when a file states nothing:
                    // an additive emitter rests **open** (B1), pan and tilt rest
                    // centred, everything else rests shut. Passing nought here
                    // instead put a switched red at nought and
                    // `no_profile_in_the_installed_library_rests_a_colour_shut`
                    // caught it on `gruft/pixel-tube`.
                    def.default_value = default_value(attribute, &Value::Null);
                    // The named steps **are** left out on purpose: which ranges
                    // the slot has belongs to whichever channel is live, and the
                    // positions disagree about them even where they agree about
                    // the parameter. Nothing is invented.
                    claimed_by.push((name.to_owned(), attributes.len()));
                    attributes.push(def);
                    continue;
                }
            }

            let Some(definition) = self.definition(name) else {
                losses.undefined += 1;
                raw(&mut attributes, offset, None, Some(name));
                continue;
            };
            // A channel whose every capability is `NoFunction` **does nothing**,
            // by the file's own statement. It holds its place in the footprint
            // — and since S54 it holds a knob too, under its own name:
            // *Reserved for future use* is a channel a firmware update may give
            // a meaning to, and an operator who needs it then should not need a
            // new build of this desk.
            if does_nothing(definition) {
                losses.without_function += 1;
                raw(&mut attributes, offset, Some(definition), Some(name));
                continue;
            }
            let Some(parameter) = attribute_of(definition, &self.wheels, name) else {
                losses.unmapped += 1;
                raw(&mut attributes, offset, Some(definition), Some(name));
                continue;
            };
            let attribute = parameter.attribute;
            let taken = |at: u8| {
                attributes
                    .iter()
                    .any(|def: &AttributeDef| def.attribute == attribute && def.occurrence == at)
            };
            // **S53 — the file's own number first.** Where a capability carries
            // a discriminator that says *which* one of its kind this is — a
            // framing blade's `blade` — that is the occurrence, so `Blade 2` is
            // physically blade two. Where it does not, S52's rule stands.
            //
            // **S52 — a fixture may have two of a parameter.** Until S52 a
            // second channel of a kind the fixture already had was counted as a
            // duplicate and dropped: 2 679 channels of the installed library, a
            // head's upper colour wheel and every pixel of a tube but one. They
            // are numbered in the **manufacturer's own order**, which is channel
            // order — so the lower-addressed wheel is the first.
            let occurrence = match parameter.occurrence {
                // A stated number another channel has already taken is a
                // profile contradicting itself; it falls back to counting
                // rather than landing on top of a channel that is already there.
                Some(stated) if !taken(stated) => Some(stated),
                _ => u8::try_from(
                    attributes
                        .iter()
                        .filter(|def| def.attribute == attribute)
                        .count(),
                )
                .ok()
                .filter(|counted| !taken(*counted)),
            };
            let Some(occurrence) = occurrence else {
                // More than 256 channels of one kind in one mode, or a stated
                // number that collides with a counted one. The key is one byte
                // wide and nothing in the library comes within an order of
                // magnitude of it; a profile that did would be counted, not
                // silently truncated onto an occurrence somebody else owns.
                losses.duplicate += 1;
                continue;
            };
            attributes.push(definition_to_attribute(
                attribute,
                occurrence,
                offset,
                definition,
                &self.wheels,
                name,
            ));
            claimed_by.push((name.to_owned(), attributes.len() - 1));
        }
        (attributes, losses)
    }
}

/// One channel definition as an [`AttributeDef`].
/// **A slot this desk has no word for, as a knob** — S54.
///
/// The floor under every other rule in this file: whatever a mode entry turns
/// out to be, the slot it stands on gets exactly one [`AttributeDef`], so the
/// sentence *no slot of a patched fixture is out of reach* has no exceptions in
/// it. `definition` is the channel's own, where there is one — a `NoFunction`
/// channel has a `defaultValue` worth honouring, and a `null` mode entry has
/// nothing at all.
///
/// The label is the manufacturer's word where the file gives one and **`Ch 7`**
/// where it does not, which is the channel's own place in the fixture counted
/// from one — the number on the patch sheet, and the only thing that can be
/// said about a slot nobody named.
fn raw_attribute(
    occurrence: u8,
    coarse_offset: u16,
    definition: Option<&Value>,
    channel: Option<&str>,
) -> AttributeDef {
    AttributeDef {
        attribute: AttributeType::Raw,
        label: channel
            .map(str::to_owned)
            .filter(|name| !name.trim().is_empty())
            .or_else(|| Some(format!("Ch {}", coarse_offset.saturating_add(1)))),
        occurrence,
        feature_group: AttributeType::Raw.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value: definition.map_or(0, |definition| {
            default_value(AttributeType::Raw, definition)
        }),
        merge_mode: AttributeType::Raw.default_merge_mode(),
        invert: false,
        // A slot with no known function has no physical quantity behind it
        // either, so the encoder reads percent — which is what these two mean
        // everywhere else in this reader.
        physical_from: 0.0,
        physical_to: 100.0,
        // **Deliberately empty.** A raw channel's ranges would have to be
        // invented, and the owner's rule for S52 was that nothing is.
        ranges: Vec::new(),
    }
}

fn definition_to_attribute(
    attribute: AttributeType,
    occurrence: u8,
    coarse_offset: u16,
    definition: &Value,
    wheels: &Wheels,
    channel: &str,
) -> AttributeDef {
    let (physical_from, physical_to) = physical_range(attribute, definition);
    AttributeDef {
        attribute,
        // **S53.** What the manufacturer calls this channel, shown on the
        // encoder in place of this desk's own word for the attribute. A label
        // and not a key: see `prism_domain::AttributeDef::label`.
        label: Some(channel.to_owned()).filter(|name| !name.trim().is_empty()),
        // **S52.** Which channel of this kind it is, counted from nought in the
        // order the manufacturer wrote the channels down.
        occurrence,
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
        // **S51, B38.** What the channel's ranges are called, so a gobo wheel
        // stops being a number an operator has to know by heart.
        ranges: ranges_of(definition, wheels, channel),
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
    // **The bank was the wrong question, and S51 is where it showed** (B38).
    // Until then *a colour rests open* was read as `FeatureGroup::Color`, which
    // was right while the only colours this model had were additive emitters.
    // It now has cyan, magenta and yellow — **filters**, where open is nought
    // and full is opaque — and a colour *wheel*, whose value is a slot number
    // with no *open* to rest at. Resting those at full would have blacked out
    // every CMY rig on the first frame after this build.
    if attribute.is_additive_emitter() {
        return u16::MAX;
    }
    // A **filter** falls through to the file, and that is the same argument
    // read the other way. B1's rule is a convention of the *desk*: an emitter
    // has an obvious open end and every console the owner has used parks it
    // there. A cyan flag has no such convention — which end is open is how that
    // head is wired — so the file is the only evidence there is, and absent one
    // nought is the answer, which is where all but three of the installed
    // library's hundred-odd flags sit.
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

/// Every capability of a channel definition, in the two shapes OFL writes them.
///
/// A single `capability` object is a channel that does one thing over its whole
/// travel; a `capabilities` array is a channel split into named ranges.
fn capabilities_of(definition: &Value) -> Vec<&Value> {
    match (
        definition.get("capability"),
        definition.get("capabilities").and_then(Value::as_array),
    ) {
        (Some(single), _) => vec![single],
        (None, Some(list)) => list.iter().collect(),
        (None, None) => Vec::new(),
    }
}

/// Whether every capability of this channel is OFL's `NoFunction`.
///
/// The file saying, in as many words, that the channel controls nothing. It
/// still occupies its place in the footprint — that is what it is written down
/// for — and it is not a gap in this desk's table.
fn does_nothing(definition: &Value) -> bool {
    let capabilities = capabilities_of(definition);
    !capabilities.is_empty()
        && capabilities
            .iter()
            .all(|capability| capability.get("type").and_then(Value::as_str) == Some("NoFunction"))
}

/// What one channel is: an attribute, and the occurrence the **file** states
/// when it states one — S53.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Parameter {
    attribute: AttributeType,
    /// Which one of its kind, when the capability carries a discriminator that
    /// says so — a framing blade's number. `None` leaves it to channel order.
    occurrence: Option<u8>,
}

/// The attribute a channel's capabilities map to, if any.
///
/// **Intensity anywhere wins**, and otherwise the first capability that maps —
/// see the module documentation for why a `Dimmer / Strobe` channel is a dimmer
/// rather than a shutter.
fn attribute_of(definition: &Value, wheels: &Wheels, channel: &str) -> Option<Parameter> {
    let mapped: Vec<Parameter> = capabilities_of(definition)
        .into_iter()
        .filter_map(|capability| attribute_of_capability(capability, wheels, channel))
        .collect();
    mapped
        .iter()
        .copied()
        .find(|parameter| parameter.attribute == AttributeType::Dimmer)
        .or_else(|| mapped.first().copied())
}

/// An attribute with no occurrence of its own — channel order decides.
const fn just(attribute: AttributeType) -> Option<Parameter> {
    Some(Parameter {
        attribute,
        occurrence: None,
    })
}

/// One capability as an attribute of this model, if there is one.
///
/// # Every capability type OFL has, and where S51 put it — B38
///
/// The table was eleven rows and everything else fell through it: 5 037 of the
/// installed library's 15 150 channels reached no attribute at all and were
/// dropped, which is punch-list entry **B38**. It is now one row per capability
/// type the format defines, and the corpus test asserts that nothing falls
/// through.
///
/// Two rows are the ones worth arguing about, and both are decisions rather
/// than deductions:
///
/// - **`ColorPreset` is a colour wheel.** A channel of named colour presets and
///   a physical wheel of glass are the same gesture to an operator — *pick a
///   colour by slot* — and this model has one attribute for it.
/// - **`Maintenance`, `Generic` and anything else a machine does is `Control`.**
///   That bank is documented as *the row you touch once a show and never during
///   one*, which is exactly what a reset, a lamp strike and a fan are.
///
/// `NoFunction` deliberately maps to nothing and is answered before this is
/// reached ([`does_nothing`]): a capability that says *this range does nothing*
/// is not evidence about what the channel is for, and a channel that is only
/// that is not a parameter at all.
///
/// # The type alone is not the parameter — S53
///
/// S51's table read a capability's **type** and nothing else, and that is
/// lossy in a way no counter could show: `channels_unmapped` stayed at nought
/// while channels arrived under the wrong knob. The format gives several types
/// a **discriminator**, a property that splits one type into distinct physical
/// parameters, and this function now reads it:
///
/// | Discriminator | What it decides | Corpus |
/// |---|---|---|
/// | `Wheel*.wheel` | which wheel, and so which bank | 121 wheel channels were on the **gobo** bank — 115 colour wheels, two prism wheels, four colour-wheel rotations |
/// | `Blade*.blade` | which framing blade | 53 channels with no relation to the blade they drive |
/// | `Fog*.fogType` | fog or haze | 12 haze channels called fog |
///
/// **What a wheel *is* comes from its slots, not from its name.** The file
/// states each slot's `type`, so a wheel carrying `Color` slots is a colour
/// wheel whatever the manufacturer called it — see [`WheelKind`]. A name would
/// have been a guess, and this reader does not guess.
fn attribute_of_capability(
    capability: &Value,
    wheels: &Wheels,
    channel: &str,
) -> Option<Parameter> {
    let kind = capability.get("type")?.as_str()?;
    match kind {
        "Pan" | "PanContinuous" => just(AttributeType::Pan),
        "Tilt" | "TiltContinuous" => just(AttributeType::Tilt),
        "PanTiltSpeed" => just(AttributeType::PositionSpeed),
        "Intensity" => just(AttributeType::Dimmer),
        "ShutterStrobe" | "StrobeSpeed" | "StrobeDuration" => just(AttributeType::Shutter),
        "Iris" | "IrisEffect" => just(AttributeType::Iris),
        // A beam angle *is* a zoom: both say how wide the beam is, and a desk
        // with two knobs for it would be a desk with a knob that does nothing on
        // every fixture that names the other one.
        "Zoom" | "BeamAngle" => just(AttributeType::Zoom),
        "Focus" => just(AttributeType::Focus),
        "Frost" | "FrostEffect" => just(AttributeType::Frost),
        "Prism" => just(AttributeType::Prism),
        "PrismRotation" => just(AttributeType::PrismRotation),
        // **Which wheel, and so which knob** — S53. A wheel's *rotation* is a
        // parameter of its own, which is what makes a head with a rotating gobo
        // two knobs rather than one knob and a dropped channel; and *which*
        // wheel decides the bank, because a colour wheel belongs beside the
        // colours and not beside the gobos.
        "WheelSlot" | "WheelShake" => just(wheels.wheel_of(capability, channel).selects()),
        "WheelRotation" | "WheelSlotRotation" => {
            just(wheels.wheel_of(capability, channel).rotates())
        }
        "ColorPreset" => just(AttributeType::ColorWheel),
        "ColorTemperature" => just(AttributeType::ColorTemperature),
        "ColorIntensity" => just(colour_attribute(capability.get("color")?.as_str()?)?),
        "Effect" | "EffectParameter" => just(AttributeType::Effect),
        "EffectSpeed" | "EffectDuration" => just(AttributeType::EffectSpeed),
        // **Which blade, and inserting is not rotating** — S53. The blade is
        // the *occurrence*, so `Blade 2` is physically blade two rather than
        // the second blade channel this reader happened to meet.
        "BladeInsertion" => Some(Parameter {
            attribute: AttributeType::Blade,
            occurrence: blade_of(capability),
        }),
        "BladeRotation" => Some(Parameter {
            attribute: AttributeType::BladeRotation,
            occurrence: blade_of(capability),
        }),
        // The whole frame turning, which carries no `blade` at all — so it has
        // no occurrence to be told apart by, and folding it in with the row
        // above would have put it on top of blade one.
        "BladeSystemRotation" => just(AttributeType::BladeSystem),
        "BeamPosition" => just(AttributeType::BeamPosition),
        // **Fog or haze** — S53, and the file says which. A hazer and a fogger
        // are not the same machine to anybody standing in front of them.
        "Fog" | "FogType" => just(match capability.get("fogType").and_then(Value::as_str) {
            Some("Haze") => AttributeType::Haze,
            // `Fog`, and an unstated type: a fog machine is what a fog
            // capability is until the file says otherwise.
            _ => AttributeType::Fog,
        }),
        "FogOutput" => just(AttributeType::Fog),
        "Rotation" | "Speed" | "Time" => just(AttributeType::Speed),
        "SoundSensitivity" => just(AttributeType::Sound),
        "Maintenance" | "Generic" => just(AttributeType::Control),
        // `NoFunction` is answered by `does_nothing` before this is reached, and
        // anything else is a capability type added upstream since this table was
        // written. It is counted, and the corpus test is what says so.
        _ => None,
    }
}

/// The attribute an emitter colour maps to.
///
/// **Every colour OFL names has one of its own since S52.** S51 gave all
/// thirteen an attribute but folded `Warm White` and `Cold White` into
/// [`AttributeType::White`], and wrote in this paragraph that it was *wrong in
/// a way an operator can see and work with*. It was worse than that: on a lamp
/// with **both** — six in the installed corpus, `generic/cw-ww-fader` among
/// them — the second one collided with the first and was dropped, so half the
/// fixture did not respond at all. They are their own attributes now.
///
/// `Cyan`, `Magenta` and `Yellow` are **subtractive** and this model says so —
/// see `AttributeType::is_additive_emitter`, which is what stops a CMY head
/// resting at full on all three and therefore black.
fn colour_attribute(colour: &str) -> Option<AttributeType> {
    match colour {
        "Red" => Some(AttributeType::Red),
        "Green" => Some(AttributeType::Green),
        "Blue" => Some(AttributeType::Blue),
        "White" => Some(AttributeType::White),
        "Warm White" => Some(AttributeType::WarmWhite),
        "Cold White" => Some(AttributeType::ColdWhite),
        "Amber" => Some(AttributeType::Amber),
        "UV" => Some(AttributeType::Uv),
        "Lime" => Some(AttributeType::Lime),
        "Indigo" => Some(AttributeType::Indigo),
        "Cyan" => Some(AttributeType::Cyan),
        "Magenta" => Some(AttributeType::Magenta),
        "Yellow" => Some(AttributeType::Yellow),
        // A colour the format grows later. Counted rather than guessed at.
        _ => None,
    }
}

/// The channel's named ranges, in this model's 16-bit values — **S51, B38**.
///
/// Only for a channel written as a `capabilities` **array**, which is OFL's own
/// way of saying *this channel is split into ranges*. A channel with a single
/// `capability` covers its whole travel with one thing, and naming that one
/// thing would put a label under every continuous encoder on the desk saying
/// what the encoder is already called.
///
/// A range with no `dmxRange` is skipped rather than guessed at: OFL allows one
/// only on a single-capability channel, where this does not run.
fn ranges_of(definition: &Value, wheels: &Wheels, channel: &str) -> Vec<AttributeRange> {
    let Some(Value::Array(list)) = definition.get("capabilities") else {
        return Vec::new();
    };
    let full = channel_maximum(definition);
    let mut ranges = Vec::new();
    for capability in list {
        let Some(Value::Array(bounds)) = capability.get("dmxRange") else {
            continue;
        };
        let (Some(low), Some(high)) = (
            bounds.first().and_then(Value::as_u64),
            bounds.get(1).and_then(Value::as_u64),
        ) else {
            continue;
        };
        let (low, high) = if low <= high {
            (low, high)
        } else {
            (high, low)
        };
        ranges.push(AttributeRange {
            name: capability_name(capability, wheels, channel),
            from: scale_to_full(low, full),
            // The **top** of the step the range ends on, so two neighbouring
            // ranges meet with nothing between them: at eight bits, `[0, 7]`
            // and `[8, 134]` would otherwise leave 1 800..2 055 belonging to
            // neither, and an encoder standing there would name nothing.
            //
            // A range that ends at the channel's own maximum ends at ours, and
            // it is written out rather than computed: the step above the last
            // one does not exist, so scaling it and taking one back off lands a
            // step short and leaves the top of every channel unnamed.
            to: if high >= full {
                u16::MAX
            } else {
                scale_to_full(high.saturating_add(1), full).saturating_sub(1)
            },
        });
    }
    ranges
}

/* -------------------------------------------------------------------------- */
/* Wheels — what a slot is actually called                                    */
/* -------------------------------------------------------------------------- */

/// A fixture's wheels, and what each of their slots is called — **S52**.
///
/// # 3 440 slots that read *Slot 3*
///
/// S51 read a channel's capabilities and gave the encoder the names it found
/// (B38) — but it looked for them **in the capability**: `comment`,
/// `effectName`, `shutterEffect`. A wheel capability rarely has one. What it
/// has is `wheel` and `slotNumber`, and the name lives in the fixture's
/// top-level `wheels` block, which this reader had never opened. Counted over
/// the installed library: **4 497 wheel capabilities, 293 with a comment, 3 440
/// with nothing but a slot number.** So a gobo picker would have offered *Slot
/// 1, Slot 2, Slot 3* for three of every four wheel positions.
///
/// # Nothing here is invented
///
/// A slot is called what the file calls it: its `name` where it has one, else
/// the last part of the `resource` key the library files a gobo under, else the
/// slot's own `type` — which is how *Open* and *Closed* get their words, since
/// the format gives those two nothing else. Where the file says none of those,
/// this answers `None` and S51's fallbacks stand.
struct Wheels(BTreeMap<String, Wheel>);

/// One wheel: what is on it, and what each slot is called.
struct Wheel {
    kind: WheelKind,
    slots: Vec<String>,
}

/// **What a wheel is, read off the slots it carries** — S53.
///
/// The format states each slot's `type`, so this is a fact the file gives
/// rather than a reading of the wheel's name: a wheel holding `Color` slots is
/// a colour wheel whether its manufacturer called it *Color Wheel*, *Farbrad*
/// or *Wheel 1*. Counted over the installed corpus: 109 colour wheels, 131 gobo
/// wheels, and one each of prism, iris and frost.
///
/// `Open` and `Closed` are on nearly every wheel and say nothing about what
/// kind it is, so they are not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WheelKind {
    Color,
    Gobo,
    Prism,
    Iris,
    Frost,
    /// A wheel whose slots say nothing this desk knows. It behaves as a gobo
    /// wheel, which is what every wheel did before S53.
    Unknown,
}

impl WheelKind {
    /// The attribute a channel that **picks a slot** of this wheel controls.
    const fn selects(self) -> AttributeType {
        match self {
            Self::Color => AttributeType::ColorWheel,
            Self::Prism => AttributeType::Prism,
            Self::Iris => AttributeType::Iris,
            Self::Frost => AttributeType::Frost,
            Self::Gobo | Self::Unknown => AttributeType::Gobo,
        }
    }

    /// The attribute a channel that **turns** this wheel controls.
    ///
    /// A colour wheel's scroll is a colour gesture and belongs beside the
    /// colours rather than beside the gobos.
    ///
    /// **No channel of the installed corpus reaches
    /// [`AttributeType::ColorWheelRotation`]**, and that is the reader being
    /// right rather than a row nothing uses: every colour wheel in the corpus
    /// puts its scroll as a *range at the top of the select channel* — slot one
    /// … slot eight, then rotate CW — and such a channel is **one knob**, which
    /// the reader gets by taking the channel's first mapped capability. Only a
    /// fixture that gives the scroll a channel of its own reaches this row, and
    /// the one profile that does (`futurelight/dmh-75-i-led-moving-head`)
    /// declares that channel without putting it in any mode.
    const fn rotates(self) -> AttributeType {
        match self {
            Self::Color => AttributeType::ColorWheelRotation,
            Self::Prism => AttributeType::PrismRotation,
            // An iris or a frost wheel that turns is turning glass in a gate,
            // which is what `GoboRotation` already means.
            Self::Gobo | Self::Iris | Self::Frost | Self::Unknown => AttributeType::GoboRotation,
        }
    }
}

impl Wheels {
    /// The wheels a fixture declares, each with its kind and its slots' names.
    fn of(fixture: &serde_json::Map<String, Value>) -> Self {
        let Some(Value::Object(wheels)) = fixture.get("wheels") else {
            return Self(BTreeMap::new());
        };
        Self(
            wheels
                .iter()
                .filter_map(|(name, wheel)| {
                    let slots = wheel.get("slots")?.as_array()?;
                    Some((
                        name.clone(),
                        Wheel {
                            kind: kind_of(slots),
                            slots: slots.iter().map(slot_label).collect(),
                        },
                    ))
                })
                .collect(),
        )
    }

    /// What kind of wheel a capability names, defaulting to the channel's own
    /// name — which is what the format says an absent `wheel` means.
    ///
    /// A capability naming a wheel the fixture does not declare, or naming
    /// several at once, answers [`WheelKind::Unknown`]: it behaves as it did
    /// before S53 rather than picking one of the wheels to be right about.
    fn wheel_of(&self, capability: &Value, channel: &str) -> WheelKind {
        let named = match capability.get("wheel") {
            None => channel,
            Some(Value::String(name)) => name.as_str(),
            Some(Value::Array(list)) if list.len() == 1 => {
                list.first().and_then(Value::as_str).unwrap_or(channel)
            }
            Some(_) => return WheelKind::Unknown,
        };
        self.0
            .get(named)
            .map_or(WheelKind::Unknown, |wheel| wheel.kind)
    }

    /// What the slot a capability points at is called, if the file says.
    ///
    /// The wheel is the capability's `wheel`, and **where it has none the
    /// channel's own name is the wheel's name** — the format says so, and it is
    /// how most single-wheel fixtures are written. `wheel` may also be a list,
    /// on a capability that moves two wheels at once; the first is taken,
    /// because a range has one name.
    fn slot_name(&self, capability: &Value, channel: &str) -> Option<String> {
        let wheel = match capability.get("wheel") {
            None => channel,
            Some(Value::String(name)) => name.as_str(),
            Some(Value::Array(list)) => list.first()?.as_str()?,
            Some(_) => return None,
        };
        let slots = &self.0.get(wheel)?.slots;
        let number = |key: &str| capability.get(key).and_then(Value::as_f64);
        if let Some(at) = number("slotNumber") {
            return between(slots, at);
        }
        // A **proportional** capability sweeps from one slot to another over
        // its range. Both ends are named, because that is what the file says
        // and either one alone would be half a truth.
        let (from, to) = (number("slotNumberStart")?, number("slotNumberEnd")?);
        match (between(slots, from), between(slots, to)) {
            (Some(from), Some(to)) if from == to => Some(from),
            (Some(from), Some(to)) => Some(format!("{from} to {to}")),
            _ => None,
        }
    }
}

/// The slot a one-based, possibly fractional slot number names.
///
/// A whole number is that slot. A fraction sits **between two**, which is what
/// the format uses it for — a wheel stopped half way between two gobos — and
/// both are named, because naming one of them would be saying the wheel is
/// somewhere it is not. The count wraps, so slot 0.5 is between the last and
/// the first, exactly as a wheel does.
fn between(slots: &[String], at: f64) -> Option<String> {
    if slots.is_empty() || !at.is_finite() {
        return None;
    }
    let count = slots.len();
    let wrapped = |number: f64| -> Option<&String> {
        let index = number.round() as i64 - 1;
        let count = i64::try_from(count).ok()?;
        slots.get(usize::try_from(index.rem_euclid(count)).ok()?)
    };
    if (at - at.round()).abs() < f64::EPSILON {
        return wrapped(at).cloned();
    }
    let low = wrapped(at.floor())?;
    let high = wrapped(at.ceil())?;
    Some(if low == high {
        low.clone()
    } else {
        format!("{low} / {high}")
    })
}

/// What kind of wheel these slots make — S53, and the file's own answer.
///
/// **What most of the wheel is**, and the *most* is not pedantry: ten wheels of
/// the installed corpus mix kinds, and on six of them the first slot that says
/// anything is not what the wheel is. A gobo wheel with an iris position at the
/// bottom of it — `beamz/panther-7r`, `elation/proteus-hybrid` and four others,
/// all of them called *Gobo Wheel* by their manufacturer — would go to the iris
/// knob on a *first one wins* rule. It is a gobo wheel with an iris on it.
///
/// A tie breaks toward the slot the file lists first, which is the only order
/// there is to appeal to.
fn kind_of(slots: &[Value]) -> WheelKind {
    let mut tally: Vec<(WheelKind, usize)> = Vec::new();
    for slot in slots {
        let found = match slot.get("type").and_then(Value::as_str) {
            Some("Color") => WheelKind::Color,
            Some("Gobo" | "AnimationGoboStart" | "AnimationGoboEnd") => WheelKind::Gobo,
            Some("Prism") => WheelKind::Prism,
            Some("Iris") => WheelKind::Iris,
            Some("Frost") => WheelKind::Frost,
            // `Open` and `Closed` are on nearly every wheel and say nothing
            // about what it holds.
            _ => continue,
        };
        match tally.iter_mut().find(|(kind, _)| *kind == found) {
            Some((_, count)) => *count += 1,
            None => tally.push((found, 1)),
        }
    }
    // `max_by_key` keeps the **last** maximum, and the tie is meant to go to
    // the first, so the tally is walked in reverse.
    tally
        .iter()
        .rev()
        .max_by_key(|(_, count)| *count)
        .map_or(WheelKind::Unknown, |(kind, _)| *kind)
}

/// Which framing blade a capability names, as an occurrence — S53.
///
/// The format allows `Top`, `Right`, `Bottom`, `Left` or a number, and this is
/// the one place the four words become an order: the order the format itself
/// lists them in. A number is one-based, as every other count in the format is.
fn blade_of(capability: &Value) -> Option<u8> {
    match capability.get("blade")? {
        Value::String(word) => match word.as_str() {
            "Top" => Some(0),
            "Right" => Some(1),
            "Bottom" => Some(2),
            "Left" => Some(3),
            _ => None,
        },
        Value::Number(number) => u8::try_from(number.as_u64()?.checked_sub(1)?).ok(),
        _ => None,
    }
}

/// What one wheel slot is called, in the words the file uses.
fn slot_label(slot: &Value) -> String {
    if let Some(name) = slot.get("name").and_then(Value::as_str)
        && !name.trim().is_empty()
    {
        return name.trim().to_owned();
    }
    // A gobo with no name of its own is filed under a library **resource** key
    // such as `gobos/textured/biohazard`; the last part of it is the word a
    // manufacturer's manual prints beside that slot.
    if let Some(resource) = slot.get("resource").and_then(Value::as_str)
        && let Some(last) = resource.rsplit('/').next()
        && !last.trim().is_empty()
    {
        return last.replace(['-', '_'], " ");
    }
    slot.get("type")
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

/// The largest DMX value one channel of this definition can hold.
///
/// OFL writes `dmxValueResolution` when a channel's capabilities are given in
/// something other than eight bits. Absent means eight, which is the format's
/// own default and what all but a handful of the installed library use.
fn channel_maximum(definition: &Value) -> u64 {
    match definition
        .get("dmxValueResolution")
        .and_then(Value::as_str)
        .unwrap_or("8bit")
    {
        "16bit" => 65_535,
        "24bit" => 16_777_215,
        _ => 255,
    }
}

/// A DMX value in a channel's own resolution, as one of this model's `0..=65535`.
fn scale_to_full(value: u64, maximum: u64) -> u16 {
    if maximum == 0 {
        return 0;
    }
    let scaled = value
        .saturating_mul(u64::from(u16::MAX))
        .div_euclid(maximum)
        .min(u64::from(u16::MAX));
    u16::try_from(scaled).unwrap_or(u16::MAX)
}

/// What to call one capability, in the words a manufacturer used.
///
/// OFL has no single *name* field: the human words live in `comment`, and the
/// rest is type-specific. So the comment is preferred, then the two fields that
/// carry a name of their own, and the capability's **type** is the last resort —
/// which is still better than a blank row, because *ShutterStrobe* between
/// *Open* and *Closed* tells an operator what the middle of the channel does.
fn capability_name(capability: &Value, wheels: &Wheels, channel: &str) -> String {
    for key in ["comment", "effectName", "shutterEffect", "colorTemperature"] {
        if let Some(text) = capability.get(key).and_then(Value::as_str)
            && !text.trim().is_empty()
        {
            return text.trim().to_owned();
        }
    }
    // **S52.** A wheel capability rarely has a comment; what it has is a wheel
    // and a slot number, and the name is in the fixture's own `wheels` block.
    // 3 440 of the installed library's 4 497 wheel capabilities read *Slot 3*
    // before this line — three of every four wheel positions.
    if let Some(name) = wheels.slot_name(capability, channel) {
        return name;
    }
    if let Some(slot) = capability.get("slotNumber").and_then(Value::as_u64) {
        return format!("Slot {slot}");
    }
    capability
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
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
        let (built, counts) = read_fixture(
            "stage-right",
            "Stage Right",
            "stage-wash",
            STAGE_WASH,
            false,
        );
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
        // **`Pan/Tilt Speed` arrives now** — S51, B38. It was one of the five
        // thousand channels that mapped to nothing and were dropped; OFL calls
        // it `PanTiltSpeed` and this model has an attribute for it.
        assert_eq!(at(AttributeType::PositionSpeed).coarse_offset, 7);
        // `Reset` is a channel whose only capability is `NoFunction` — the file
        // saying it does nothing. It holds its place in the footprint and is
        // **not** a mapping gap, which is why it has a counter of its own.
        // **Since S54 it is a knob as well**: nine attributes for nine
        // channels, the last of them raw and called what the file calls it.
        assert_eq!(nine.attributes.len(), 9);
        let reset = nine.attributes.last().expect("the Reset channel");
        assert_eq!(reset.attribute, AttributeType::Raw);
        assert_eq!(reset.label.as_deref(), Some("Reset"));
        assert_eq!(
            counts.channels_unmapped, 0,
            "every capability type this file uses maps to an attribute"
        );
        assert_eq!(counts.channels_without_function, 2, "one Reset per mode");

        // The 9-channel mode is 8-bit throughout; nothing has a fine channel.
        assert!(nine.attributes.iter().all(|def| def.fine_offset.is_none()));
    }

    #[test]
    fn a_fine_channel_alias_becomes_the_fine_offset_of_the_channel_it_belongs_to() {
        let (built, _) = read_fixture(
            "stage-right",
            "Stage Right",
            "stage-wash",
            STAGE_WASH,
            false,
        );
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

    /// A `null` entry occupies a channel, which is what keeps everything after
    /// it at the right offset — and **since S54 it is reachable**.
    ///
    /// Until S54 it defined nothing, and *nothing* is a slot the desk drove to
    /// nought for ever with no knob to change it. `null` is the mode author's
    /// word for *unused*, which is a judgement rather than a fact about the
    /// fixture, so the slot is a raw knob named after where it is.
    #[test]
    fn an_unused_channel_still_takes_up_its_place() {
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH, false);
        let fourteen = &built[1].1;
        assert_eq!(fourteen.footprint, 14);
        let unused: Vec<_> = fourteen
            .attributes
            .iter()
            .filter(|def| def.coarse_offset == 10 || def.coarse_offset == 11)
            .map(|def| (def.attribute, def.label.as_deref()))
            .collect();
        assert_eq!(
            unused,
            vec![
                (AttributeType::Raw, Some("Ch 11")),
                (AttributeType::Raw, Some("Ch 12")),
            ],
            "the nulls are nobody's channels, so they are the operator's"
        );
    }

    /// Pan and tilt carry the angle the manufacturer states, centred on zero —
    /// which is how `ARCHITECTURE_SPEC.md` §6 writes a pan range.
    #[test]
    fn pan_and_tilt_carry_the_travel_the_file_states() {
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH, false);
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
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH, false);
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
        let (built, _) = read_fixture("m", "M", "f", STAGE_WASH, false);
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

    /// **A matrix insert is written out; a switching channel is still not** —
    /// S52.
    ///
    /// Both used to be skipped, and this test said so: a footprint may not
    /// depend on state. The difference S52 draws is *whose* state. A matrix
    /// insert depends on the fixture's own geometry, which the file states, so
    /// it can be resolved once at read time — 90 of the 634 installed profiles
    /// were unpatchable for one. A **switching channel** depends on another
    /// channel's value while the show runs, and that is a footprint that
    /// changes under an operator's hands. It is still counted and skipped, and
    /// `docs/ISSUES.md` carries it as its own entry.
    #[test]
    fn a_matrix_insert_is_written_out_and_a_switching_channel_is_not() {
        let source = r#"{
          "name": "Bar",
          "matrix": { "pixelCount": [3, 1, 1] },
          "availableChannels": { "Master": { "capability": { "type": "Intensity" } } },
          "templateChannels": {
            "Red $pixelKey": {
              "capability": { "type": "ColorIntensity", "color": "Red" }
            }
          },
          "modes": [
            { "shortName": "1ch", "channels": ["Master"] },
            {
              "shortName": "4ch",
              "channels": [
                "Master",
                {
                  "insert": "matrixChannels",
                  "repeatFor": "eachPixelABC",
                  "channelOrder": "perPixel",
                  "templateChannels": ["Red $pixelKey"]
                }
              ]
            },
            { "shortName": "switch", "channels": ["Master", { "switch": "Programs" }] }
          ]
        }"#;
        let (built, counts) = read_fixture("m", "M", "bar", source, false);
        assert_eq!(built.len(), 2);
        assert_eq!(built[1].0.mode, "4ch");
        // Four channels: the master and a red per pixel, **numbered** — which is
        // the whole reason this could be resolved in S52 and not before.
        assert_eq!(built[1].1.footprint, 4);
        assert_eq!(
            built[1]
                .1
                .attributes
                .iter()
                .map(|def| (def.attribute, def.occurrence, def.coarse_offset))
                .collect::<Vec<_>>(),
            vec![
                (AttributeType::Dimmer, 0, 0),
                (AttributeType::Red, 0, 1),
                (AttributeType::Red, 1, 2),
                (AttributeType::Red, 2, 3),
            ]
        );
        assert_eq!(counts.channels_duplicate, 0);
        // The switching mode is the one left, and it is counted rather than
        // guessed at.
        assert_eq!(counts.modes_with_inserts, 1);
        assert_eq!(counts.modes, 2);
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
        let (built, counts) = read_fixture("m", "M", "bar", source, false);
        assert_eq!(counts.channels_undefined, 0);
        let attributes = &built[0].1.attributes;
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].attribute, AttributeType::Red);
        assert_eq!(attributes[0].coarse_offset, 0);
        assert_eq!(attributes[0].fine_offset, Some(1));
    }

    /// **S52 turned this test round.** A second channel of a kind the fixture
    /// already has is **numbered**, not dropped.
    ///
    /// It used to say that one physical parameter meant one value and that the
    /// lower channel won — which is what dropped 2 679 channels of the
    /// installed library, a head's upper colour wheel and every pixel of a tube
    /// but one. The occurrence is nought-based and follows the manufacturer's
    /// own order, which is channel order: the lower-addressed one is the first,
    /// so a show written before S52 finds its values where it left them.
    #[test]
    fn a_second_channel_of_a_kind_is_numbered_rather_than_dropped() {
        let source = r#"{
          "name": "Two Dimmers",
          "availableChannels": {
            "Master": { "capability": { "type": "Intensity" } },
            "Dimmer": { "capability": { "type": "Intensity" } }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Master", "Dimmer"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        let profile = &built[0].1;
        assert_eq!(profile.footprint, 2, "both channels are still occupied");
        assert_eq!(profile.attributes.len(), 2, "and both are reachable now");
        assert_eq!(
            profile
                .attributes
                .iter()
                .map(|def| (def.coarse_offset, def.occurrence))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 1)],
            "the manufacturer's order is the numbering"
        );
        assert_eq!(counts.channels_duplicate, 0);
    }

    /// **A warm white and a cold white are two lamps** — S52, and the owner's
    /// own fault report.
    ///
    /// The Open Fixture Library names thirteen emitter colours and this model
    /// had eleven: `Warm White` and `Cold White` were both read as `White`. On
    /// a lamp with one of them that was a wrong label; on a lamp with **both**
    /// the second collided with the first and was dropped, so half the fixture
    /// did not answer. Two attributes, two channels, two knobs.
    #[test]
    fn a_warm_white_and_a_cold_white_are_two_attributes() {
        let source = r#"{
          "name": "CW/WW Fader",
          "availableChannels": {
            "Cold": { "capability": { "type": "ColorIntensity", "color": "Cold White" } },
            "Warm": { "capability": { "type": "ColorIntensity", "color": "Warm White" } }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Cold", "Warm"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        let profile = &built[0].1;
        assert_eq!(counts.channels_duplicate, 0);
        assert_eq!(
            profile
                .attributes
                .iter()
                .map(|def| (def.attribute, def.occurrence))
                .collect::<Vec<_>>(),
            vec![(AttributeType::ColdWhite, 0), (AttributeType::WarmWhite, 0)]
        );
        // Both are additive emitters, so both rest **open** — B1's rule, and
        // the reason it asks `is_additive_emitter` rather than the bank.
        assert!(
            profile
                .attributes
                .iter()
                .all(|def| def.default_value == u16::MAX)
        );
    }

    /// **A wheel slot is called what the file calls it** — S52.
    ///
    /// S51 read a range's name out of the capability (`comment`, `effectName`),
    /// and a wheel capability rarely has one: it has `wheel` and `slotNumber`,
    /// and the name is in the fixture's own `wheels` block. 3 440 of the
    /// installed corpus's 4 497 wheel capabilities read *Slot 3* before this.
    ///
    /// Where a slot has no name of its own, the library **resource** key it is
    /// filed under is the manufacturer's word for it; where it has neither, the
    /// slot's `type` is — which is how *Open* gets its word, since the format
    /// gives that one nothing else. Nothing here is invented.
    #[test]
    fn a_wheel_slot_is_named_out_of_the_wheels_block() {
        let source = r#"{
          "name": "A Head",
          "wheels": {
            "Gobo": {
              "slots": [
                { "type": "Open" },
                { "type": "Gobo", "name": "Biohazard" },
                { "type": "Gobo", "resource": "gobos/textured/wood-grain" }
              ]
            }
          },
          "availableChannels": {
            "Gobo": {
              "capabilities": [
                { "dmxRange": [0, 9], "type": "WheelSlot", "slotNumber": 1 },
                { "dmxRange": [10, 19], "type": "WheelSlot", "slotNumber": 2 },
                { "dmxRange": [20, 29], "type": "WheelSlot", "slotNumber": 3 },
                { "dmxRange": [30, 255], "type": "WheelSlot", "slotNumber": 2.5 }
              ]
            }
          },
          "modes": [{ "shortName": "1ch", "channels": ["Gobo"] }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        let names: Vec<&str> = built[0].1.attributes[0]
            .ranges
            .iter()
            .map(|range| range.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "Open",
                "Biohazard",
                "wood grain",
                // A fractional slot number sits **between** two, which is what
                // the format uses it for, and both are named: saying one of
                // them would put the wheel somewhere it is not.
                "Biohazard / wood grain",
            ]
        );
    }

    /// **A colour wheel is a colour, and a gobo wheel is a gobo** — S53.
    ///
    /// Both are `WheelSlot`, so the capability's *type* cannot tell them apart.
    /// What can is the `wheel` it names and the `type` of that wheel's slots,
    /// which the file states — and getting it wrong put **115 wheel channels of
    /// the installed corpus on the gobo bank**, 110 of them colour wheels.
    ///
    /// The wheel's *name* is deliberately not consulted: it is free text, and
    /// this one is called `Wheel A` on purpose.
    #[test]
    fn a_wheel_goes_to_the_bank_its_slots_say_it_belongs_on() {
        let source = r#"{
          "name": "A Head",
          "wheels": {
            "Wheel A": {
              "slots": [{ "type": "Open" }, { "type": "Color", "name": "Deep blue" }]
            },
            "Wheel B": {
              "slots": [{ "type": "Open" }, { "type": "Gobo", "name": "Breakup" }]
            }
          },
          "availableChannels": {
            "Wheel A": { "capability": { "type": "WheelSlot", "slotNumber": 1 } },
            "Wheel A Rotation": {
              "capability": { "type": "WheelRotation", "wheel": "Wheel A", "speed": "fast CW" }
            },
            "Wheel B": { "capability": { "type": "WheelSlot", "slotNumber": 1 } },
            "Wheel B Rotation": {
              "capability": { "type": "WheelRotation", "wheel": "Wheel B", "speed": "fast CW" }
            }
          },
          "modes": [{
            "shortName": "4ch",
            "channels": ["Wheel A", "Wheel A Rotation", "Wheel B", "Wheel B Rotation"]
          }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        assert_eq!(
            built[0]
                .1
                .attributes
                .iter()
                .map(|def| (def.attribute, def.feature_group))
                .collect::<Vec<_>>(),
            vec![
                (AttributeType::ColorWheel, FeatureGroup::Color),
                (AttributeType::ColorWheelRotation, FeatureGroup::Color),
                (AttributeType::Gobo, FeatureGroup::Gobo),
                (AttributeType::GoboRotation, FeatureGroup::Gobo),
            ],
            "the slots decide the wheel, and the wheel decides the bank"
        );
    }

    /// **A wheel is what most of it is** — S53.
    ///
    /// Ten wheels of the installed corpus mix kinds, and on six of them the
    /// first slot that says anything is not what the wheel is: `beamz/panther-7r`
    /// and `elation/proteus-hybrid` both call a wheel *Gobo Wheel* and put an
    /// iris at the top of it. A *first one wins* rule sent all six to the iris
    /// knob. Counting the body slots sends them where their manufacturer's own
    /// name for them says they go, and moved nine profiles back to the gobo bank.
    #[test]
    fn a_wheel_that_mixes_kinds_is_the_kind_most_of_its_slots_are() {
        let source = r#"{
          "name": "A Head",
          "wheels": {
            "Gobo Wheel": {
              "slots": [
                { "type": "Open" },
                { "type": "Iris", "openPercent": "50%" },
                { "type": "Gobo", "name": "Breakup" },
                { "type": "Gobo", "name": "Dots" },
                { "type": "Closed" }
              ]
            }
          },
          "availableChannels": {
            "Gobo Wheel": { "capability": { "type": "WheelSlot", "slotNumber": 1 } }
          },
          "modes": [{ "shortName": "1ch", "channels": ["Gobo Wheel"] }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        assert_eq!(
            built[0].1.attributes[0].attribute,
            AttributeType::Gobo,
            "the iris is a position on a gobo wheel, not an iris wheel"
        );
    }

    /// A tie goes to the slot the file lists first, which is the only order
    /// there is to appeal to — S53.
    #[test]
    fn a_wheel_split_evenly_between_two_kinds_takes_the_one_listed_first() {
        let source = r#"{
          "name": "A Head",
          "wheels": {
            "Wheel": {
              "slots": [
                { "type": "Open" },
                { "type": "Color", "name": "Red" },
                { "type": "Gobo", "name": "Breakup" }
              ]
            }
          },
          "availableChannels": {
            "Wheel": { "capability": { "type": "WheelSlot", "slotNumber": 1 } }
          },
          "modes": [{ "shortName": "1ch", "channels": ["Wheel"] }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        assert_eq!(
            built[0].1.attributes[0].attribute,
            AttributeType::ColorWheel
        );
    }

    /// **A switching alias is the parameter its positions agree on** — S54.
    ///
    /// The mode names `Layer Red`, which the fixture never defines: *which*
    /// channel occupies that slot depends on the mode channel's value, and the
    /// file states the whole set. Every one of them here is a red, so the slot
    /// is a red — and the encoder carries the alias's own name.
    #[test]
    fn a_switching_alias_is_the_parameter_its_positions_agree_on() {
        let source = r#"{
          "name": "A Bar",
          "availableChannels": {
            "Mode": {
              "capabilities": [
                {
                  "dmxRange": [0, 127], "type": "Maintenance", "comment": "Layer 1",
                  "switchChannels": { "Layer Red": "Red 1" }
                },
                {
                  "dmxRange": [128, 255], "type": "Maintenance", "comment": "Layer 2",
                  "switchChannels": { "Layer Red": "Red 2" }
                }
              ]
            },
            "Red 1": { "capability": { "type": "ColorIntensity", "color": "Red" } },
            "Red 2": { "capability": { "type": "ColorIntensity", "color": "Red" } }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Mode", "Layer Red"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        assert_eq!(counts.channels_switched, 1);
        assert_eq!(counts.channels_undefined, 0);
        assert_eq!(
            built[0]
                .1
                .attributes
                .iter()
                .map(|def| (def.attribute, def.label.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                (AttributeType::Control, Some("Mode")),
                // The alias's own name, and a **red** rather than a raw knob.
                (AttributeType::Red, Some("Layer Red")),
            ]
        );
    }

    /// **A switching alias whose positions disagree is a raw knob** — S54.
    ///
    /// 178 of the corpus's 246 aliases are this shape. Naming the knob after
    /// one of the positions would be wrong in the others, so it is named after
    /// itself and reaches the desk as [`AttributeType::Raw`] — which is still
    /// better than the nothing it was before S54.
    #[test]
    fn a_switching_alias_whose_positions_disagree_is_raw_under_its_own_name() {
        let source = r#"{
          "name": "A Laser",
          "availableChannels": {
            "Mode": {
              "capabilities": [
                {
                  "dmxRange": [0, 127], "type": "Maintenance", "comment": "Manual",
                  "switchChannels": { "Channel 2": "Red" }
                },
                {
                  "dmxRange": [128, 255], "type": "Maintenance", "comment": "Auto",
                  "switchChannels": { "Channel 2": "Speed" }
                }
              ]
            },
            "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } },
            "Speed": { "capability": { "type": "Speed", "speedStart": "fast CW", "speedEnd": "slow CW" } }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Mode", "Channel 2"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        assert_eq!(counts.channels_switched, 0);
        assert_eq!(
            counts.channels_undefined, 1,
            "still counted as not understood"
        );
        let def = &built[0].1.attributes[1];
        assert_eq!(def.attribute, AttributeType::Raw);
        assert_eq!(def.label.as_deref(), Some("Channel 2"));
        assert_eq!(def.feature_group, FeatureGroup::Control);
        // **No invented steps.** Which named ranges the slot has depends on
        // which channel is live, so it has none — the owner's rule from S52.
        assert!(def.ranges.is_empty());
    }

    /// **A slot the mode leaves unused is still a knob** — S54.
    ///
    /// `null` is the mode's own word for *nothing here*, and it is a judgement
    /// rather than a fact about the fixture. The slot is reachable, named by
    /// the only thing that can be said about it: where it is.
    #[test]
    fn a_slot_the_mode_leaves_unused_is_still_a_knob() {
        let source = r#"{
          "name": "A Tube",
          "availableChannels": {
            "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } }
          },
          "modes": [{ "shortName": "3ch", "channels": ["Red", null, null] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        assert_eq!(counts.channels_unused, 2);
        assert_eq!(built[0].1.footprint, 3);
        assert_eq!(
            built[0]
                .1
                .attributes
                .iter()
                .map(|def| (
                    def.attribute,
                    def.occurrence,
                    def.coarse_offset,
                    def.label.as_deref()
                ))
                .collect::<Vec<_>>(),
            vec![
                (AttributeType::Red, 0, 0, Some("Red")),
                // One-based, because that is how an operator counts channels
                // off a patch sheet — and the *key* stays nought-based, which
                // is the pair `AttributeKey::Display` already reconciles.
                (AttributeType::Raw, 0, 1, Some("Ch 2")),
                (AttributeType::Raw, 1, 2, Some("Ch 3")),
            ]
        );
    }

    /// **A channel the file says does nothing is still a knob** — S54.
    ///
    /// *Reserved for future use* is a channel a firmware update may give a
    /// meaning to, and an operator who needs it then should not need a new
    /// build of this desk. It keeps the manufacturer's own word and the file's
    /// own resting value.
    #[test]
    fn a_channel_the_file_says_does_nothing_is_still_a_knob() {
        let source = r#"{
          "name": "A Head",
          "availableChannels": {
            "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } },
            "Reserved 1": {
              "defaultValue": 255,
              "capability": { "type": "NoFunction" }
            }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Red", "Reserved 1"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        assert_eq!(counts.channels_without_function, 1);
        let def = &built[0].1.attributes[1];
        assert_eq!(def.attribute, AttributeType::Raw);
        assert_eq!(def.label.as_deref(), Some("Reserved 1"));
        // The file said where it rests, so that is where it rests.
        assert_eq!(def.default_value, u16::MAX);
    }

    /// **Every slot of a mode has exactly one knob** — S54, the floor itself.
    ///
    /// The corpus asserts this over 40 953 slots
    /// (`no_slot_of_any_profile_is_out_of_reach`); this is the same statement
    /// small enough to read, over a mode carrying one of every way a slot used
    /// to go missing.
    #[test]
    fn every_slot_of_a_mode_has_exactly_one_knob() {
        let source = r#"{
          "name": "Everything At Once",
          "availableChannels": {
            "Mode": {
              "capabilities": [
                {
                  "dmxRange": [0, 127], "type": "Maintenance", "comment": "One",
                  "switchChannels": { "Switched": "Red" }
                },
                {
                  "dmxRange": [128, 255], "type": "Maintenance", "comment": "Two",
                  "switchChannels": { "Switched": "Zoom" }
                }
              ]
            },
            "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } },
            "Zoom": { "capability": { "type": "Zoom", "angleStart": "10deg", "angleEnd": "30deg" } },
            "Reserved": { "capability": { "type": "NoFunction" } },
            "Dimmer": {
              "fineChannelAliases": ["Dimmer fine", "Dimmer finest"],
              "capability": { "type": "Intensity" }
            }
          },
          "modes": [{
            "shortName": "7ch",
            "channels": [
              "Mode", "Switched", "Reserved", null,
              "Dimmer", "Dimmer fine", "Dimmer finest"
            ]
          }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        let profile = &built[0].1;
        let footprint = usize::from(profile.footprint);
        let mut covered = vec![0_usize; footprint];
        for def in &profile.attributes {
            for offset in [Some(def.coarse_offset), def.fine_offset]
                .into_iter()
                .flatten()
            {
                covered[usize::from(offset)] += 1;
            }
        }
        assert_eq!(footprint, 7);
        assert_eq!(
            covered,
            vec![1; 7],
            "every slot is reached exactly once: {:?}",
            profile
                .attributes
                .iter()
                .map(|def| (def.attribute, def.coarse_offset, def.fine_offset))
                .collect::<Vec<_>>()
        );
        // And the third byte of a 24-bit dimmer is a knob of its own rather
        // than the silent drop it was — `AttributeDef` carries one fine channel.
        let deepest = profile.attributes.last().expect("a last attribute");
        assert_eq!(deepest.attribute, AttributeType::Raw);
        assert_eq!(deepest.label.as_deref(), Some("Dimmer finest"));
    }

    /// **Which blade, out of the file** — S53.
    ///
    /// A framing shutter system is four blades that each insert and rotate.
    /// Before S53 all eight channels were one attribute numbered by the order
    /// this reader met them, so *Blade 3* meant *the third blade channel* and
    /// not *blade three*. The `blade` property is the occurrence now, and
    /// inserting is not rotating.
    #[test]
    fn a_framing_blade_is_numbered_by_the_blade_and_not_by_the_channel() {
        let source = r#"{
          "name": "A Profile Spot",
          "availableChannels": {
            "Blade 3 Insertion": {
              "capability": { "type": "BladeInsertion", "blade": 3, "insertion": "0%" }
            },
            "Blade 1 Insertion": {
              "capability": { "type": "BladeInsertion", "blade": 1, "insertion": "0%" }
            },
            "Blade 1 Rotation": {
              "capability": { "type": "BladeRotation", "blade": 1, "angle": "0deg" }
            },
            "Frame Rotation": {
              "capability": { "type": "BladeSystemRotation", "angle": "0deg" }
            }
          },
          "modes": [{
            "shortName": "4ch",
            "channels": [
              "Blade 3 Insertion", "Blade 1 Insertion", "Blade 1 Rotation", "Frame Rotation"
            ]
          }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        assert_eq!(counts.channels_duplicate, 0);
        assert_eq!(
            built[0]
                .1
                .attributes
                .iter()
                .map(|def| (def.attribute, def.occurrence))
                .collect::<Vec<_>>(),
            vec![
                // The *third* blade, though it is the first channel.
                (AttributeType::Blade, 2),
                (AttributeType::Blade, 0),
                (AttributeType::BladeRotation, 0),
                // The whole frame, which carries no `blade` and so would have
                // landed on top of blade one had it shared the attribute.
                (AttributeType::BladeSystem, 0),
            ]
        );
    }

    /// **A hazer is not a fogger** — S53, and `fogType` is what says so.
    #[test]
    fn fog_and_haze_are_told_apart_by_the_type_the_file_states() {
        let source = r#"{
          "name": "A Machine",
          "availableChannels": {
            "Haze": { "capability": { "type": "Fog", "fogType": "Haze" } },
            "Fog": { "capability": { "type": "Fog", "fogType": "Fog" } },
            "Output": { "capability": { "type": "FogOutput", "fogOutput": "0%" } }
          },
          "modes": [{ "shortName": "3ch", "channels": ["Haze", "Fog", "Output"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        assert_eq!(counts.channels_duplicate, 0);
        assert_eq!(
            built[0]
                .1
                .attributes
                .iter()
                .map(|def| def.attribute)
                .collect::<Vec<_>>(),
            vec![
                AttributeType::Haze,
                AttributeType::Fog,
                // An output with no type stated is a fog machine's until the
                // file says otherwise.
                AttributeType::Fog
            ]
        );
    }

    /// **The encoder reads the manufacturer's word** — S53.
    ///
    /// The channel's own name travels on `AttributeDef::label` and is what the
    /// band shows in place of this desk's word for the attribute. It is a
    /// **label and not a key**: both of these are `Gobo`, so a preset still
    /// means the same thing on this head as on any other.
    #[test]
    fn a_channel_carries_the_name_its_manufacturer_gave_it() {
        let source = r#"{
          "name": "A Head",
          "availableChannels": {
            "Static Gobo": { "capability": { "type": "WheelSlot", "slotNumber": 1 } },
            "Rotating Gobo": { "capability": { "type": "WheelSlot", "slotNumber": 1 } }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Static Gobo", "Rotating Gobo"] }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        assert_eq!(
            built[0]
                .1
                .attributes
                .iter()
                .map(|def| (def.attribute, def.occurrence, def.label.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                (AttributeType::Gobo, 0, Some("Static Gobo")),
                (AttributeType::Gobo, 1, Some("Rotating Gobo")),
            ]
        );
    }

    /// The channel's own name is the wheel's name when the capability omits it.
    ///
    /// The format says so, and it is how most single-wheel fixtures are
    /// written — a reader that insisted on `wheel` would find no slot on any of
    /// them and fall back to *Slot 2*.
    #[test]
    fn a_capability_with_no_wheel_names_its_own_channel() {
        let source = r#"{
          "name": "A Head",
          "wheels": {
            "Colour": {
              "slots": [{ "type": "Open" }, { "type": "Color", "name": "Deep blue" }]
            }
          },
          "availableChannels": {
            "Colour": {
              "capabilities": [
                { "dmxRange": [0, 127], "type": "WheelSlot", "slotNumber": 1 },
                { "dmxRange": [128, 255], "type": "WheelSlot", "slotNumber": 2 }
              ]
            }
          },
          "modes": [{ "shortName": "1ch", "channels": ["Colour"] }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        assert_eq!(
            built[0].1.attributes[0]
                .ranges
                .iter()
                .map(|range| range.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Open", "Deep blue"]
        );
    }

    /// **S51 turned this test round, and that is B38.**
    ///
    /// It used to say that cyan, magenta and yellow were colours this model
    /// had no attribute for, and that they were dropped and counted. They are
    /// three attributes now, so a CMY head patches whole — and the second
    /// assertion is the one that matters more than the first: **a subtractive
    /// flag rests at nought**. B1's rule is *a colour rests open*, and open for
    /// a filter is out of the beam. Giving these three B1's resting value would
    /// have made every CMY rig black at home, which is B34 in reverse and the
    /// sort of fault a corpus does not catch because it is about a number
    /// rather than a shape.
    #[test]
    fn a_subtractive_colour_arrives_and_rests_out_of_the_beam() {
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
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        let profile = &built[0].1;
        assert_eq!(profile.footprint, 4);
        assert_eq!(profile.attributes.len(), 4, "nothing is dropped now");
        assert_eq!(counts.channels_unmapped, 0);
        assert_eq!(counts.modes_without_attributes, 0);

        let at = |attribute: AttributeType| {
            profile
                .attributes
                .iter()
                .find(|def| def.attribute == attribute)
                .unwrap_or_else(|| panic!("{attribute:?} is missing"))
        };
        for subtractive in [
            AttributeType::Cyan,
            AttributeType::Magenta,
            AttributeType::Yellow,
        ] {
            let def = at(subtractive);
            assert_eq!(
                def.feature_group,
                FeatureGroup::Color,
                "{subtractive:?} belongs on the colour bank"
            );
            assert_eq!(
                def.default_value, 0,
                "{subtractive:?} is a filter: open is nought, and full is black"
            );
        }
        assert_eq!(at(AttributeType::Dimmer).coarse_offset, 3);
    }

    /// A colour the format grows after this table was written is still counted
    /// — **and since S54 it is still reachable**.
    ///
    /// Two guards in one. `channels_unmapped == 0` over the corpus stays
    /// meaningful because the counter still counts: it is nought because the
    /// table is complete, not because it stopped looking. And the channel is
    /// **not lost** for being uncounted-for — a capability type nobody has
    /// taught this reader yet arrives as [`AttributeType::Raw`] under the
    /// manufacturer's own name, which is the whole point of a floor rather than
    /// a list of fixes.
    #[test]
    fn a_colour_this_model_has_never_heard_of_is_still_counted() {
        let source = r#"{
          "name": "Future",
          "availableChannels": {
            "Octarine": { "capability": { "type": "ColorIntensity", "color": "Octarine" } },
            "Dim": { "capability": { "type": "Intensity" } }
          },
          "modes": [{ "shortName": "2ch", "channels": ["Octarine", "Dim"] }]
        }"#;
        let (built, counts) = read_fixture("m", "M", "f", source, false);
        assert_eq!(counts.channels_unmapped, 1);
        assert_eq!(
            built[0]
                .1
                .attributes
                .iter()
                .map(|def| (def.attribute, def.label.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                (AttributeType::Raw, Some("Octarine")),
                (AttributeType::Dimmer, Some("Dim")),
            ],
            "a colour from the future is a knob, not a hole"
        );
    }

    /// **A channel's named ranges are read** — S51, B38, and the half of the
    /// entry that is about capabilities rather than about channels.
    ///
    /// The `Dimmer / Strobe` channel of the fixture the S44 request came with:
    /// three ranges, in eight-bit DMX, which this model holds as `0..=65535`.
    /// The ends are the ones that matter — two neighbouring ranges have to meet
    /// with nothing between them, or an encoder standing in the gap names
    /// nothing at all.
    #[test]
    fn a_channel_split_into_ranges_carries_their_names_and_their_ends() {
        let (built, _) = read_fixture(
            "stage-right",
            "Stage Right",
            "stage-wash",
            STAGE_WASH,
            false,
        );
        let dimmer = built[0]
            .1
            .attributes
            .iter()
            .find(|def| def.attribute == AttributeType::Dimmer)
            .expect("the Dimmer / Strobe channel is the dimmer");

        let names: Vec<&str> = dimmer
            .ranges
            .iter()
            .map(|range| range.name.as_str())
            .collect();
        assert_eq!(names, vec!["Closed", "Intensity", "Strobe"]);

        // 8-bit `[0, 7]`, `[8, 134]`, `[135, 239]` scaled to this model's range.
        assert_eq!(dimmer.ranges[0].from, 0);
        assert_eq!(
            dimmer.ranges[0].to + 1,
            dimmer.ranges[1].from,
            "the first two ranges must meet"
        );
        assert_eq!(
            dimmer.ranges[1].to + 1,
            dimmer.ranges[2].from,
            "the second two ranges must meet"
        );

        // And the reading an encoder makes: which range is it standing in.
        assert_eq!(
            dimmer.range_at(0).map(|range| range.name.as_str()),
            Some("Closed")
        );
        assert_eq!(
            dimmer
                .range_at(dimmer.ranges[1].middle())
                .map(|r| r.name.as_str()),
            Some("Intensity")
        );
        // Above the last range the file describes there is nothing named, and
        // the encoder says so rather than naming the nearest.
        assert_eq!(dimmer.range_at(u16::MAX), None);
    }

    /// A channel that does **one** thing over its whole travel has no ranges.
    ///
    /// Naming that one thing would put a label under every continuous encoder
    /// on the desk saying what the encoder is already called.
    #[test]
    fn a_channel_with_one_capability_has_no_named_ranges() {
        let (built, _) = read_fixture(
            "stage-right",
            "Stage Right",
            "stage-wash",
            STAGE_WASH,
            false,
        );
        let pan = built[0]
            .1
            .attributes
            .iter()
            .find(|def| def.attribute == AttributeType::Pan)
            .expect("pan is there");
        assert!(pan.ranges.is_empty());
    }

    /// Ranges given in sixteen bits are read in sixteen bits.
    ///
    /// OFL says so with `dmxValueResolution`, and a channel read at the wrong
    /// resolution would put every range name in the bottom 0.4 % of the travel.
    #[test]
    fn a_sixteen_bit_channel_states_its_resolution_and_is_read_in_it() {
        let source = r#"{
          "name": "Wheel",
          "availableChannels": {
            "Gobo": {
              "dmxValueResolution": "16bit",
              "capabilities": [
                { "dmxRange": [0, 32767], "type": "WheelSlot", "comment": "Open" },
                { "dmxRange": [32768, 65535], "type": "WheelSlot", "comment": "Gobo 1" }
              ]
            }
          },
          "modes": [{ "shortName": "1ch", "channels": ["Gobo"] }]
        }"#;
        let (built, _) = read_fixture("m", "M", "f", source, false);
        let gobo = &built[0].1.attributes[0];
        assert_eq!(gobo.attribute, AttributeType::Gobo);
        assert_eq!(gobo.ranges.len(), 2);
        assert_eq!(gobo.ranges[0].from, 0);
        assert_eq!(gobo.ranges[1].to, u16::MAX);
        assert_eq!(
            gobo.range_at(60_000).map(|range| range.name.as_str()),
            Some("Gobo 1")
        );
    }

    /// Every capability type the format defines maps to an attribute.
    ///
    /// The unit-test half of B38's corpus claim: the corpus can only show that
    /// the *library* has nothing this table misses, and this shows that the
    /// table itself is the one that was written. A type added upstream later
    /// falls through and is counted, which is what
    /// `a_colour_this_model_has_never_heard_of_is_still_counted` holds.
    #[test]
    fn every_capability_type_the_format_defines_has_an_attribute() {
        // `docs/capability-types.md` upstream, minus `NoFunction`, which is
        // answered before the table is reached.
        let types = [
            "ShutterStrobe",
            "StrobeSpeed",
            "StrobeDuration",
            "Intensity",
            "ColorTemperature",
            "Pan",
            "PanContinuous",
            "Tilt",
            "TiltContinuous",
            "PanTiltSpeed",
            "WheelSlot",
            "WheelShake",
            "WheelSlotRotation",
            "WheelRotation",
            "Effect",
            "EffectSpeed",
            "EffectDuration",
            "EffectParameter",
            "SoundSensitivity",
            "BeamAngle",
            "BeamPosition",
            "Focus",
            "Zoom",
            "Iris",
            "IrisEffect",
            "Frost",
            "FrostEffect",
            "Prism",
            "PrismRotation",
            "BladeInsertion",
            "BladeRotation",
            "BladeSystemRotation",
            "Fog",
            "FogOutput",
            "FogType",
            "Rotation",
            "Speed",
            "Time",
            "Maintenance",
            "Generic",
            "ColorPreset",
        ];
        // **S53.** The table asks the wheels and the channel name as well as
        // the type, because several types carry a discriminator. An empty
        // wheel table and a bare channel name are the case where the file
        // states no discriminator at all, which is what this test is about:
        // every type reaches an attribute even then.
        let nowhere = super::Wheels(std::collections::BTreeMap::new());
        for kind in types {
            let capability = json!({ "type": kind });
            assert!(
                super::attribute_of_capability(&capability, &nowhere, "Channel").is_some(),
                "{kind} maps to no attribute, so a channel of them would be dropped"
            );
        }
        // And the colour a `ColorIntensity` names, for every colour OFL has.
        for colour in [
            "Red",
            "Green",
            "Blue",
            "White",
            "Warm White",
            "Cold White",
            "Amber",
            "UV",
            "Lime",
            "Indigo",
            "Cyan",
            "Magenta",
            "Yellow",
        ] {
            let capability = json!({ "type": "ColorIntensity", "color": colour });
            assert!(
                super::attribute_of_capability(&capability, &nowhere, "Channel").is_some(),
                "{colour} maps to no attribute"
            );
        }
        assert_eq!(
            super::attribute_of_capability(&json!({ "type": "NoFunction" }), &nowhere, "Channel"),
            None,
            "NoFunction is not an attribute, and `does_nothing` is what reads it"
        );
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
            let (built, counts) = read_fixture("m", "M", "f", source, false);
            assert!(built.is_empty(), "{source} produced a profile");
            assert_eq!(counts.attributes, 0, "{source}");
        }
        // These two are *readable* modes made of nothing this reader
        // understands — a mode naming a channel that does not exist, and one
        // whose channel table is not even an object. **Since S54 the answer is
        // a knob rather than an empty profile**: the slot is there, so it is
        // reachable, named after the file's own word for it or after where it
        // is. A profile that patches and controls nothing was the old answer
        // and it was the wrong one.
        for source in [
            r#"{ "modes": [{ "channels": ["nothing defines this"] }] }"#,
            r#"{ "availableChannels": 7, "modes": [{ "channels": [null] }] }"#,
        ] {
            let (built, counts) = read_fixture("m", "M", "f", source, false);
            assert_eq!(built.len(), 1, "{source}");
            assert_eq!(built[0].1.footprint, 1, "{source}");
            assert_eq!(
                built[0]
                    .1
                    .attributes
                    .iter()
                    .map(|def| def.attribute)
                    .collect::<Vec<_>>(),
                vec![AttributeType::Raw],
                "{source}"
            );
            assert_eq!(counts.modes_without_attributes, 0, "{source}");
        }
        // The last two are *readable* files with nothing usable in them, so
        // they are not counted as rejected — the distinction matters when a
        // number is reported to a person.
        let (_, rejected) = read_fixture("m", "M", "f", "not json", false);
        assert_eq!(rejected.files_rejected, 1);
        let (_, thin) = read_fixture(
            "m",
            "M",
            "f",
            r#"{ "modes": [{ "channels": [null] }] }"#,
            false,
        );
        assert_eq!(thin.files_rejected, 0);
        assert_eq!(thin.modes, 1);
        // **Nought since S54**, and it is now nought by construction rather
        // than by luck: a mode with entries in it always has a knob per entry,
        // and a mode with no entries is refused before it gets here. The
        // counter stays, held to nought the way `channels_unmapped` is.
        assert_eq!(thin.modes_without_attributes, 0);
        assert_eq!(thin.channels_unused, 1);
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
        let (built, counts) = read_fixture("m", "M", "f", &source, false);
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
        let (built, _) = read_fixture("m", "M", "f", source, false);
        assert_eq!(built[0].0.id, "m/f/1ch");
        assert_eq!(
            built[1].0.id, "m/f/Extended",
            "a name is used when there is one"
        );
    }

    #[test]
    fn counts_add_up_across_files() {
        let mut total = Conversion::default();
        let (_, one) = read_fixture("m", "M", "f", STAGE_WASH, false);
        let (_, two) = read_fixture("m", "M", "g", STAGE_WASH, false);
        total.absorb(one);
        total.absorb(two);
        assert_eq!(total.fixtures, 2);
        assert_eq!(total.modes, one.modes * 2);
        assert_eq!(total.attributes, one.attributes * 2);
    }
}
