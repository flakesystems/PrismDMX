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
//! So the desk carries a library and `Command::EmbedFixtureType` **copies one
//! profile into the show**. The copy is the point: after it, the show owns that
//! profile, and a desk with a different library opens the show unchanged.
//!
//! # What is in it (S44)
//!
//! Three sources, in the order a key is resolved:
//!
//! 1. **The operator's own**, in `fixtures/` inside the daemon's data
//!    directory, in the Open Fixture Library's own JSON format. Read at
//!    start-up, and a key here **wins**, so a venue can correct a profile
//!    without editing vendored data and without losing the correction on the
//!    next import.
//! 2. **The Open Fixture Library**, vendored in `profiles/fixtures/` —
//!    634 fixtures across 132 manufacturers at schema 12.5.1. See
//!    [`ofl`] for how a fixture becomes profiles, and
//!    `profiles/fixtures/SOURCE.md` for which commit and how to re-import it.
//! 3. **Four generic profiles** built in Rust: a dimmer, two PARs and a moving
//!    head. They stay because a rig is often patched before anybody knows what
//!    is actually hanging in it, and because a one-channel dimmer is not a
//!    thing OFL has a sensible entry for.
//!
//! # Why the client does not send the profile, and no longer holds the list
//!
//! `Command::EmbedFixtureType` carries a key and nothing else, for the same
//! reason `Command::PatchFixture` carries no channels (**D3**): a client that
//! sent a whole [`FixtureType`] would be authoring show content, and the daemon
//! would be reduced to validating whatever arrived.
//!
//! S27 served the whole list in the `Snapshot`, which was right for four
//! profiles and is impossible for two thousand: `docs/IPC_PROTOCOL.md` §3 caps a
//! frame at 1 MiB, and a menu of two thousand entries is not a menu. So a client
//! **searches** — `Query::SearchLibrary` — and gets back small entries rather
//! than profiles. [`FixtureLibrary::search`] is that search.
//!
//! # What this is still not
//!
//! There is no GDTF import, no way to author a profile in the interface, and no
//! matrix support — a mode whose channel list depends on state is skipped, with
//! the count reported. See [`ofl`] for the whole of what conversion costs.

pub mod ofl;

use std::collections::BTreeMap;
use std::path::Path;

use prism_domain::{
    AttributeDef, AttributeType, FeatureGroup, FixtureType, LibraryEntry, MergeMode,
};

/// An 8-bit attribute at an offset, filed under its own feature group.
/// A colour channel at **full** — punch-list B1.
///
/// The owner's complaint was that a fixture selected in the programmer showed
/// its colours at 0 %, so mixing a colour started by turning three knobs *up*
/// before turning any down. On every desk they have used, colour starts open and
/// you subtract; the dimmer is what decides whether any of it is seen.
///
/// This is the **home value**, so it is what the bottom of the merge holds and
/// what an encoder reads when the programmer is empty — a rig at home now sits
/// at white with the dimmer down rather than at black twice over. It is a
/// property of the *profile*, so it is set here and in the OFL converter
/// (`crate::library::ofl`) and nowhere else: `AttributeDef::default_value` is
/// the one answer, and a rule in the engine that overrode it would be a second.
fn colour(attribute: AttributeType, coarse_offset: u16) -> AttributeDef {
    eight_bit(attribute, coarse_offset, u16::MAX)
}

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
            colour(AttributeType::Red, 0),
            colour(AttributeType::Green, 1),
            colour(AttributeType::Blue, 2),
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
            colour(AttributeType::Red, 0),
            colour(AttributeType::Green, 1),
            colour(AttributeType::Blue, 2),
            colour(AttributeType::White, 3),
        ],
    }
}

/// A thirteen-channel moving head with 16-bit pan and tilt.
///
/// The profile that makes the *rest* of the desk reachable from a fresh show:
/// it has attributes on **all seven** encoder banks, so an operator who patches
/// one has something to turn on every one of them.
///
/// **Eleven channels until S43**, when the banks became seven: a head with no
/// gobo wheel and no lamp-control channel could no longer reach all of them, and
/// the honest fix is the one a real moving head takes — it *has* a gobo wheel.
/// `the_moving_head_has_something_on_every_bank` is what holds this, and it is
/// the reason a bank added later cannot quietly become unreachable from a fresh
/// show.
fn moving_head() -> FixtureType {
    FixtureType {
        id: "generic.movinghead".to_owned(),
        manufacturer: "Generic".to_owned(),
        name: "Moving Head".to_owned(),
        mode: "13ch".to_owned(),
        footprint: 13,
        attributes: vec![
            sixteen_bit(AttributeType::Pan, 0, -270.0, 270.0),
            sixteen_bit(AttributeType::Tilt, 2, -135.0, 135.0),
            eight_bit(AttributeType::Dimmer, 4, 0),
            eight_bit(AttributeType::Shutter, 5, 0),
            colour(AttributeType::Red, 6),
            colour(AttributeType::Green, 7),
            colour(AttributeType::Blue, 8),
            eight_bit(AttributeType::Zoom, 9, 0),
            eight_bit(AttributeType::Focus, 10, 0),
            eight_bit(AttributeType::Gobo, 11, 0),
            eight_bit(AttributeType::Control, 12, 0),
        ],
    }
}

/// The four generic profiles, in the order a list should show them.
///
/// Built rather than held as a constant: a [`FixtureType`] owns three `String`s
/// and a `Vec`, none of which is constructible in a `const`.
#[must_use]
pub fn generic_profiles() -> Vec<FixtureType> {
    vec![dimmer(), rgb_par(), rgbw_par(), moving_head()]
}

/// Everything this desk can embed into a show.
///
/// Built once at start-up and never changed while the daemon runs, which is what
/// makes it safe to answer a `Query::SearchLibrary` from any thread without
/// asking the show for anything.
///
/// `PartialEq` because [`crate::ShowFile`] derives it and holds one. Two
/// libraries are equal when they hold the same profiles, which is a comparison
/// no hot path makes and which the file's own `PartialEq` needs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FixtureLibrary {
    /// Keyed by [`FixtureType::id`], so a key is resolved without a scan and so
    /// the operator's own folder can override a vendored profile by using its
    /// key.
    profiles: BTreeMap<String, FixtureType>,
    /// The searchable form of each, in the same order as `profiles`.
    entries: Vec<LibraryEntry>,
    /// What reading the directories cost and what it could not use.
    conversion: ofl::Conversion,
    /// Redirects met while walking, resolved once the whole tree is read.
    ///
    /// Held rather than followed on the spot, because the fixture a redirect
    /// points at is usually under another manufacturer and may not have been
    /// read yet.
    pending: Vec<PendingRedirect>,
}

/// A redirect waiting for the tree to finish being read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingRedirect {
    /// The key the alias is filed under, as `manufacturer/fixture`.
    from: String,
    /// The manufacturer's display name, for the alias's entry.
    manufacturer: String,
    /// What this file calls the fixture — the name on the box the operator is
    /// holding, which is the whole reason a redirect is followed.
    name: String,
    /// The key it points at.
    to: String,
}

/// How many matches a search answers with when the caller does not say.
///
/// A frame is capped at 1 MiB (`docs/IPC_PROTOCOL.md` §3) and an entry is a
/// few dozen bytes, so this is nowhere near it — it is a *human* limit. A list
/// of two hundred fixtures is not something an operator reads; they type another
/// word instead.
pub const DEFAULT_SEARCH_LIMIT: usize = 50;

/// The most a search will ever answer with, whatever it is asked for.
///
/// The guard that keeps the answer inside the frame limit no matter what a
/// client sends: 500 entries is about 60 kB.
pub const MAX_SEARCH_LIMIT: usize = 500;

impl FixtureLibrary {
    /// A library holding the four generic profiles and nothing else.
    ///
    /// What a test and a daemon with no profile directory get. It is not empty
    /// on purpose: a desk that cannot find its library is still a desk somebody
    /// can patch a dimmer into.
    #[must_use]
    pub fn generic() -> Self {
        let mut library = Self::default();
        for profile in generic_profiles() {
            library.insert_profile(profile);
        }
        library
    }

    /// Reads an Open Fixture Library tree into this library.
    ///
    /// The layout is OFL's own: one directory per manufacturer, one JSON file
    /// per fixture, and a `manufacturers.json` beside them giving each
    /// manufacturer key a display name. A directory that is not there is not an
    /// error — a daemon started from a build tree without the vendored profiles
    /// keeps the profiles it has.
    ///
    /// **A key already in this library is kept.** So the caller reads the
    /// operator's own folder *first* and the vendored tree second, and a
    /// correction in the data directory wins without anything having to know
    /// which of the two it came from.
    ///
    /// Nothing here fails. A file that will not parse is counted in
    /// [`Self::conversion`] and skipped, because a desk must start with a
    /// corrupt profile in its folder.
    pub fn read_ofl_tree(&mut self, root: &Path) {
        let names = manufacturer_names(root);
        let Ok(directory) = std::fs::read_dir(root) else {
            return;
        };
        let mut manufacturers: Vec<std::path::PathBuf> = directory
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        // Sorted, so a library built twice on two machines holds the same
        // profiles in the same order and a recording of it is stable.
        manufacturers.sort();
        for path in manufacturers {
            let Some(key) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let display = names.get(key).cloned().unwrap_or_else(|| key.to_owned());
            self.read_manufacturer(&path, key, &display);
        }
        self.resolve_redirects();
    }

    /// Files the walk found a redirect in, turned into aliases.
    ///
    /// One alias per mode of the fixture pointed at, keyed under the
    /// *redirecting* fixture and carrying its name — so a Lixada Mini Moving
    /// Head is found by searching for *Lixada*, and patches the channels the
    /// Stage Right file gives it.
    ///
    /// A redirect whose target is not there resolves to nothing and is dropped.
    /// Only a hand-edited library produces one, and a dangling alias would be a
    /// menu entry that could not be embedded.
    fn resolve_redirects(&mut self) {
        for redirect in std::mem::take(&mut self.pending) {
            let prefix = format!("{}/", redirect.to);
            let targets: Vec<(String, FixtureType)> = self
                .profiles
                .range(prefix.clone()..)
                .take_while(|(id, _)| id.starts_with(&prefix))
                .map(|(id, profile)| (id.clone(), profile.clone()))
                .collect();
            for (id, profile) in targets {
                let mode = id[prefix.len()..].to_owned();
                let alias = format!("{}/{mode}", redirect.from);
                self.insert(
                    LibraryEntry {
                        id: alias.clone(),
                        manufacturer: redirect.manufacturer.clone(),
                        name: redirect.name.clone(),
                        mode: mode.clone(),
                        footprint: profile.footprint,
                    },
                    FixtureType {
                        id: alias,
                        manufacturer: redirect.manufacturer.clone(),
                        name: redirect.name.clone(),
                        mode,
                        ..profile
                    },
                );
            }
        }
    }

    /// Reads one directory of fixture files, all under one manufacturer.
    ///
    /// The operator's own folder is read through this with a manufacturer of
    /// `"Custom"`: a file dropped there needs no directory of its own, which is
    /// the whole convenience of *drop it in and restart*.
    pub fn read_fixture_dir(&mut self, path: &Path, key: &str, display: &str) {
        self.read_manufacturer(path, key, display);
        self.resolve_redirects();
    }

    fn read_manufacturer(&mut self, path: &Path, key: &str, display: &str) {
        let Ok(directory) = std::fs::read_dir(path) else {
            return;
        };
        let mut files: Vec<std::path::PathBuf> = directory
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        files.sort();
        for file in files {
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Ok(source) = std::fs::read_to_string(&file) else {
                self.conversion.files_rejected += 1;
                continue;
            };
            if let Some(redirect) = ofl::read_redirect(&source) {
                self.conversion.redirects += 1;
                self.pending.push(PendingRedirect {
                    from: format!("{key}/{stem}"),
                    manufacturer: display.to_owned(),
                    name: redirect.name,
                    to: redirect.to,
                });
                continue;
            }
            let (built, counts) = ofl::read_fixture(key, display, stem, &source);
            self.conversion.absorb(counts);
            for (entry, profile) in built {
                self.insert(entry, profile);
            }
        }
    }

    /// Adds one profile unless its key is taken.
    ///
    /// What the daemon adds the built-in generics with, **last**, so a library
    /// profile keyed `generic.dimmer` would win over the built-in one rather
    /// than the other way round.
    pub fn insert_profile(&mut self, profile: FixtureType) {
        self.insert(
            LibraryEntry {
                id: profile.id.clone(),
                manufacturer: profile.manufacturer.clone(),
                name: profile.name.clone(),
                mode: profile.mode.clone(),
                footprint: profile.footprint,
            },
            profile,
        );
    }

    /// Adds a profile unless its key is taken. See [`Self::read_ofl_tree`].
    fn insert(&mut self, entry: LibraryEntry, profile: FixtureType) {
        if self.profiles.contains_key(&profile.id) {
            return;
        }
        self.profiles.insert(profile.id.clone(), profile);
        self.entries.push(entry);
    }

    /// How many profiles there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Whether there are none, which only a hand-built library can be.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// What reading the directories cost.
    #[must_use]
    pub const fn conversion(&self) -> ofl::Conversion {
        self.conversion
    }

    /// One profile by key, ready to embed into a show.
    #[must_use]
    pub fn profile(&self, id: &str) -> Option<&FixtureType> {
        self.profiles.get(id)
    }

    /// Every entry, in key order. What a test walks; not what a client is sent.
    #[must_use]
    pub fn entries(&self) -> &[LibraryEntry] {
        &self.entries
    }

    /// The entries matching `text`, best first, at most `limit` of them.
    ///
    /// Words rather than a substring: *robe 600* finds a Robe MMX Spot 600 with
    /// the words in either order and with anything between them, which is how a
    /// person types the name of a light they are holding. Every word must appear
    /// somewhere in the manufacturer, the name, the mode or the key.
    ///
    /// Ranked so the answer is useful when it is cut off at `limit`: a match on
    /// the fixture's **name** beats one on its manufacturer or its key, an
    /// earlier match beats a later one, and a shorter footprint breaks the tie
    /// so a fixture's modes come out smallest first.
    ///
    /// An empty query answers with the first `limit` entries rather than with
    /// nothing, so a client that has not typed anything yet has something to
    /// show.
    #[must_use]
    pub fn search(&self, text: &str, limit: usize) -> Vec<LibraryEntry> {
        let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
        let words: Vec<String> = text
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|word| !word.is_empty())
            .collect();

        let mut scored: Vec<(u32, usize, &LibraryEntry)> = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            match score(entry, &words) {
                Some(rank) => scored.push((rank, index, entry)),
                None => continue,
            }
        }
        // By rank, then by the order the library was built in, which is stable
        // across machines because the directory walk sorts.
        scored.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, _, entry)| entry.clone())
            .collect()
    }
}

/// How well one entry matches every word, or `None` when it does not match all
/// of them. **Lower is better.**
fn score(entry: &LibraryEntry, words: &[String]) -> Option<u32> {
    if words.is_empty() {
        return Some(u32::from(entry.footprint));
    }
    let name = entry.name.to_lowercase();
    let manufacturer = entry.manufacturer.to_lowercase();
    let mode = entry.mode.to_lowercase();
    let id = entry.id.to_lowercase();

    let mut total: u32 = 0;
    for word in words {
        // A hit in the name is worth most, then the manufacturer, then the
        // mode, then the key — which is the order an operator would search in.
        let hit = [&name, &manufacturer, &mode, &id]
            .into_iter()
            .enumerate()
            .filter_map(|(field, haystack)| {
                haystack
                    .find(word.as_str())
                    .map(|at| u32::try_from(field * 1000 + at.min(999)).unwrap_or(u32::MAX))
            })
            .min()?;
        total = total.saturating_add(hit);
    }
    // The footprint breaks a tie, so one fixture's modes come out smallest
    // first rather than in whatever order the file listed them.
    Some(
        total
            .saturating_mul(64)
            .saturating_add(u32::from(entry.footprint)),
    )
}

/// The display name of every manufacturer key, out of OFL's `manufacturers.json`.
fn manufacturer_names(root: &Path) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let Ok(source) = std::fs::read_to_string(root.join("manufacturers.json")) else {
        return names;
    };
    let Ok(serde_json::Value::Object(members)) = serde_json::from_str(&source) else {
        return names;
    };
    for (key, value) in members {
        if let Some(name) = value.get("name").and_then(serde_json::Value::as_str) {
            names.insert(key, name.to_owned());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_SEARCH_LIMIT, FixtureLibrary, MAX_SEARCH_LIMIT, generic_profiles};
    use crate::Show;
    use prism_domain::{AttributeType, FeatureGroup, LibraryEntry};
    use std::collections::BTreeSet;

    /// One OFL fixture with two modes, written out rather than downloaded: this
    /// crate's tests must pass on a machine that has never run the installer.
    const HEAD: &str = r#"{
      "name": "Wash 7Q5",
      "availableChannels": {
        "Pan": { "capability": { "type": "Pan", "angleStart": "0deg", "angleEnd": "540deg" } },
        "Tilt": { "capability": { "type": "Tilt", "angleStart": "0deg", "angleEnd": "180deg" } },
        "Dimmer": { "capability": { "type": "Intensity" } },
        "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } }
      },
      "modes": [
        { "shortName": "4ch", "channels": ["Pan", "Tilt", "Dimmer", "Red"] },
        { "shortName": "2ch", "channels": ["Dimmer", "Red"] }
      ]
    }"#;

    /// A library with one manufacturer's one fixture in it, on disk.
    fn on_disk(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(
            dir.path().join("manufacturers.json"),
            r#"{ "robe": { "name": "Robe" }, "chauvet": { "name": "Chauvet" } }"#,
        )
        .expect("it writes");
        for (path, source) in files {
            let full = dir.path().join(path);
            std::fs::create_dir_all(full.parent().expect("a parent")).expect("it creates");
            std::fs::write(full, source).expect("it writes");
        }
        dir
    }

    /* -- the built-in four -------------------------------------------------- */

    /// The library is only worth having if the show model accepts all of it.
    ///
    /// `embed_fixture_type` refuses an empty footprint, a duplicated attribute
    /// and an offset outside the footprint, so this is the whole of the
    /// validation a profile has to pass — run against every profile rather than
    /// against the one that was being edited.
    #[test]
    fn every_generic_profile_is_one_a_show_accepts() {
        let mut show = Show::new();
        for fixture_type in generic_profiles() {
            show.embed_fixture_type(fixture_type.clone())
                .unwrap_or_else(|error| panic!("{}: {error}", fixture_type.id));
        }
        assert_eq!(show.fixture_types().count(), generic_profiles().len());
    }

    #[test]
    fn the_keys_are_unique_and_a_profile_is_found_by_key() {
        let library = FixtureLibrary::generic();
        let keys: BTreeSet<String> = generic_profiles()
            .into_iter()
            .map(|fixture_type| fixture_type.id)
            .collect();
        assert_eq!(keys.len(), generic_profiles().len(), "a key is used twice");
        assert_eq!(library.len(), keys.len());
        assert!(!library.is_empty());
        for key in &keys {
            assert_eq!(library.profile(key).map(|found| &found.id), Some(key));
        }
        assert_eq!(library.profile("nothing.at.all"), None);
    }

    /// Every channel of every profile is inside its footprint and used once —
    /// which `embed_fixture_type` checks for the coarse offsets and, as of S27,
    /// this checks for the *fine* ones as well.
    #[test]
    fn no_two_attributes_of_a_profile_share_a_channel() {
        for fixture_type in generic_profiles() {
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

    /// **Colour rests open** — punch-list B1.
    ///
    /// The owner's complaint was that a fixture selected in the programmer
    /// showed its colours at 0 %, so mixing started by turning three knobs *up*.
    /// Colour starts open on every desk they have used and the dimmer decides
    /// whether any of it is seen, so the home value is full — which is what the
    /// bottom of the merge holds and what an encoder reads when the programmer
    /// is empty.
    ///
    /// Asserted over **every** generic profile rather than one, so a profile
    /// added later cannot quietly go back to black; the OFL converter takes the
    /// same rule and `ofl::default_value` is where it is written for a file.
    #[test]
    fn a_colour_channel_rests_open_and_nothing_else_does() {
        for profile in generic_profiles() {
            for def in &profile.attributes {
                match def.attribute.feature_group() {
                    FeatureGroup::Color => assert_eq!(
                        def.default_value,
                        u16::MAX,
                        "{} {:?} does not rest open",
                        profile.id,
                        def.attribute
                    ),
                    // Pan and tilt rest centred, for their own reason: a head
                    // that is patched and never touched should point at the
                    // middle of its travel rather than at an end stop.
                    FeatureGroup::Position => {
                        assert_eq!(def.default_value, 32768, "{} is not centred", profile.id);
                    }
                    // Everything else rests at zero — most importantly the
                    // dimmer, which is what makes a rig at home dark rather
                    // than white.
                    _ => assert_eq!(
                        def.default_value, 0,
                        "{} {:?} does not rest at zero",
                        profile.id, def.attribute
                    ),
                }
            }
        }
    }

    /// The moving head reaches every encoder bank, which is what makes a fresh
    /// show worth patching one into.
    #[test]
    fn the_moving_head_has_something_on_every_bank() {
        let library = FixtureLibrary::generic();
        let head = library
            .profile("generic.movinghead")
            .expect("the library carries a moving head");
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

    /* -- reading a tree off disk -------------------------------------------- */

    /// **The layout is OFL's own**: a directory per manufacturer, a file per
    /// fixture, and `manufacturers.json` giving each key a display name.
    #[test]
    fn an_ofl_tree_becomes_one_profile_per_mode() {
        let dir = on_disk(&[("robe/wash-7q5.json", HEAD)]);
        let mut library = FixtureLibrary::generic();
        library.read_ofl_tree(dir.path());

        assert_eq!(library.len(), generic_profiles().len() + 2, "two modes");
        let four = library
            .profile("robe/wash-7q5/4ch")
            .expect("the key is manufacturer/fixture/mode");
        assert_eq!(four.manufacturer, "Robe", "out of manufacturers.json");
        assert_eq!(four.name, "Wash 7Q5");
        assert_eq!(four.footprint, 4);
        assert!(library.profile("robe/wash-7q5/2ch").is_some());
        // And the counts say what it cost, which is what a re-import is read by.
        assert_eq!(library.conversion().fixtures, 1);
        assert_eq!(library.conversion().modes, 2);
    }

    /// A manufacturer with no entry in `manufacturers.json` is named after its
    /// directory rather than left blank — a profile nobody can find by name is
    /// no better than one that is not there.
    #[test]
    fn a_manufacturer_with_no_display_name_falls_back_to_its_key() {
        let dir = on_disk(&[("nameless-co/thing.json", HEAD)]);
        let mut library = FixtureLibrary::default();
        library.read_ofl_tree(dir.path());
        assert_eq!(
            library
                .profile("nameless-co/thing/4ch")
                .map(|profile| profile.manufacturer.as_str()),
            Some("nameless-co")
        );
    }

    /// **The operator's folder wins**, which is the whole point of having one:
    /// a venue corrects a profile without editing what the installer will
    /// replace.
    #[test]
    fn a_key_already_in_the_library_is_kept() {
        let vendored = on_disk(&[("robe/wash-7q5.json", HEAD)]);
        let corrected = on_disk(&[(
            "robe/wash-7q5.json",
            &HEAD.replace("Wash 7Q5", "Wash 7Q5 (corrected)"),
        )]);

        let mut library = FixtureLibrary::default();
        // The operator's first, the installer's second — which is the order the
        // daemon reads them in and the only thing that makes this work.
        library.read_ofl_tree(corrected.path());
        library.read_ofl_tree(vendored.path());
        assert_eq!(
            library
                .profile("robe/wash-7q5/4ch")
                .map(|profile| profile.name.as_str()),
            Some("Wash 7Q5 (corrected)")
        );
        assert_eq!(library.len(), 2, "and not four: the key is the same");
    }

    /// A directory that is not there is not an error, and neither is a file
    /// that will not parse. A desk starts.
    #[test]
    fn nothing_about_a_missing_or_broken_directory_stops_anything() {
        let mut library = FixtureLibrary::generic();
        library.read_ofl_tree(std::path::Path::new("no/such/directory"));
        assert_eq!(library.len(), generic_profiles().len());

        let dir = on_disk(&[
            ("robe/broken.json", "not json at all"),
            ("robe/wash-7q5.json", HEAD),
            ("robe/notes.txt", "ignored: only .json is read"),
        ]);
        library.read_ofl_tree(dir.path());
        assert_eq!(library.len(), generic_profiles().len() + 2);
        assert_eq!(library.conversion().files_rejected, 1);
    }

    /// One flat directory of files, which is what the operator's own folder is:
    /// no manufacturer directory to make, because *drop it in* is the whole
    /// convenience.
    #[test]
    fn a_flat_directory_is_read_under_one_manufacturer() {
        let dir = on_disk(&[("mine.json", HEAD)]);
        let mut library = FixtureLibrary::default();
        library.read_fixture_dir(dir.path(), "custom", "Custom");
        assert_eq!(library.len(), 2);
        let profile = library
            .profile("custom/mine/4ch")
            .expect("keyed by the stem");
        assert_eq!(profile.manufacturer, "Custom");
    }

    /* -- searching ---------------------------------------------------------- */

    /// A library big enough to have to be searched rather than listed.
    fn searchable() -> FixtureLibrary {
        let mut library = FixtureLibrary::default();
        let entry = |id: &str, manufacturer: &str, name: &str, mode: &str, footprint: u16| {
            (
                LibraryEntry {
                    id: id.to_owned(),
                    manufacturer: manufacturer.to_owned(),
                    name: name.to_owned(),
                    mode: mode.to_owned(),
                    footprint,
                },
                prism_domain::FixtureType {
                    id: id.to_owned(),
                    manufacturer: manufacturer.to_owned(),
                    name: name.to_owned(),
                    mode: mode.to_owned(),
                    footprint,
                    attributes: Vec::new(),
                },
            )
        };
        for (entry, profile) in [
            entry("robe/mmx-spot/16ch", "Robe", "MMX Spot 600", "16ch", 16),
            entry("robe/mmx-spot/24ch", "Robe", "MMX Spot 600", "24ch", 24),
            entry("robe/robin-600/8ch", "Robe", "Robin LEDWash 600", "8ch", 8),
            entry("chauvet/slimpar/4ch", "Chauvet", "SlimPAR Pro", "4ch", 4),
            entry("martin/mac-600/12ch", "Martin", "MAC 600", "12ch", 12),
        ] {
            library.insert(entry, profile);
        }
        library
    }

    /// **Words, in any order**, which is how a person types the name of a light
    /// they are holding.
    #[test]
    fn a_search_matches_every_word_wherever_it_appears() {
        let library = searchable();
        let ids = |text: &str| -> Vec<String> {
            library
                .search(text, DEFAULT_SEARCH_LIMIT)
                .into_iter()
                .map(|entry| entry.id)
                .collect()
        };
        // Both words, in either order, with anything between them.
        assert_eq!(
            ids("robe 600"),
            vec![
                "robe/mmx-spot/16ch",
                "robe/mmx-spot/24ch",
                "robe/robin-600/8ch"
            ]
        );
        assert_eq!(ids("600 robe"), ids("robe 600"), "order does not matter");
        // The manufacturer alone, the name alone, and the mode alone.
        assert_eq!(ids("chauvet"), vec!["chauvet/slimpar/4ch"]);
        assert_eq!(ids("slimpar"), vec!["chauvet/slimpar/4ch"]);
        assert_eq!(ids("24ch"), vec!["robe/mmx-spot/24ch"]);
        // A word nothing has.
        assert!(ids("nothing at all").is_empty());
        // Case does not matter, and neither does extra space.
        assert_eq!(ids("  ROBE   Spot "), ids("robe spot"));
    }

    /// Ranked, so the answer is useful when it is cut off at the limit.
    ///
    /// Two rules, and both are what an operator would expect. A word in the
    /// **name** beats the same word in the key, because the name is what is
    /// printed on the light. And a word **earlier** in a field beats one later
    /// in it, which is why `MAC 600` comes before `MMX Spot 600` for `600`.
    #[test]
    fn a_match_in_the_name_beats_one_only_in_the_key() {
        let library = searchable();
        // "robin" is in one fixture's name and in another's key. The name wins.
        let found = library.search("robin", DEFAULT_SEARCH_LIMIT);
        assert_eq!(
            found.first().map(|entry| entry.name.as_str()),
            Some("Robin LEDWash 600")
        );

        // Earlier in the field beats later in it: `600` is at index 4 of
        // "MAC 600" and at index 9 of "MMX Spot 600".
        let for_600 = library.search("600", DEFAULT_SEARCH_LIMIT);
        assert_eq!(
            for_600.first().map(|entry| entry.name.as_str()),
            Some("MAC 600")
        );

        // And one fixture's modes come out smallest first, rather than in
        // whatever order the file listed them.
        let modes: Vec<u16> = library
            .search("mmx", DEFAULT_SEARCH_LIMIT)
            .into_iter()
            .map(|entry| entry.footprint)
            .collect();
        assert_eq!(modes, vec![16, 24]);
    }

    /// An empty query is *show me something*, not *show me nothing*: a client
    /// that has not been typed into yet has a list.
    #[test]
    fn an_empty_search_answers_with_the_first_of_them() {
        let library = searchable();
        assert_eq!(library.search("", 2).len(), 2);
        assert_eq!(library.search("   ", DEFAULT_SEARCH_LIMIT).len(), 5);
    }

    /// **The limit is the daemon's**, because the answer has to fit in a frame
    /// (`docs/IPC_PROTOCOL.md` §3) and a client that asked for two thousand
    /// would otherwise get them.
    #[test]
    fn the_limit_is_clamped_at_both_ends() {
        let library = searchable();
        assert_eq!(library.search("", 0).len(), 1, "zero is not an answer");
        assert_eq!(library.search("", 3).len(), 3);
        assert_eq!(library.search("", usize::MAX).len(), 5, "all there are");
        const { assert!(MAX_SEARCH_LIMIT >= DEFAULT_SEARCH_LIMIT) };
    }
}
