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
//! # What is in it (S44, and **GDTF since S61**)
//!
//! Three sources, in the order a key is resolved:
//!
//! 1. **The operator's own**, in `fixtures/` inside the daemon's data
//!    directory. Read at start-up, and a key here **wins**, so a venue can
//!    correct a profile without editing installed data and without losing the
//!    correction on the next import. Both formats are read here: a `.gdtf`
//!    file the manufacturer published, and a JSON file in the Open Fixture
//!    Library's own format — which is what a light nobody has published a GDTF
//!    for gets written in, because a channel list in JSON is a far kinder thing
//!    to write by hand than a ZIP archive of XML.
//! 2. **The installed library**, in `profiles/fixtures/` — **GDTF since S61**.
//!    See [`gdtf`] for what a `.gdtf` file becomes and why the library moved to
//!    it; [`ofl`] is still read from the same tree, so a desk whose library was
//!    installed before S61 keeps working and a venue may mix the two. Which is
//!    installed, and how, is `profiles/fixtures/SOURCE.md`.
//! 3. **Four generic profiles** built in Rust: a dimmer, two PARs and a moving
//!    head. They stay because a rig is often patched before anybody knows what
//!    is actually hanging in it, and because a one-channel dimmer is not a
//!    thing either format has a sensible entry for.
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
//! There is no way to author a profile in the interface, and no GDTF is
//! **written** — this desk reads the format, it does not publish in it. What
//! each reader cannot use is counted rather than hidden: see [`gdtf`] and
//! [`ofl`] for the whole of what each conversion costs.

pub mod gdtf;
pub mod index;
mod matrix;
pub mod mvr;
pub mod ofl;
pub mod zip;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub use index::LibraryIndex;

use prism_domain::{
    AttributeDef, AttributeType, FeatureGroup, FixtureType, LibraryEntry, LibraryFixture,
    LibraryMode, MergeMode, ResourceKind,
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
    // **`is_additive_emitter` and not the bank** — S51, B38. The four generics
    // are all additive, so this changes nothing for them; it is written this way
    // because it is the same rule the OFL converter applies, and a fifth generic
    // with a cyan flag on it must not come out resting opaque.
    let home = if attribute.is_additive_emitter() {
        u16::MAX
    } else {
        0
    };
    eight_bit(attribute, coarse_offset, home)
}

fn eight_bit(attribute: AttributeType, coarse_offset: u16, default_value: u16) -> AttributeDef {
    AttributeDef {
        switched: None,
        attribute,
        // A generic profile stands in for a light nobody has told the desk
        // about, so there is no manufacturer's word to carry — S53.
        label: None,
        // A generic profile has one of each parameter, so every one is the
        // first of its kind — S52.
        occurrence: 0,
        feature_group: attribute.feature_group(),
        coarse_offset,
        fine_offset: None,
        default_value,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 100.0,
        // A generic profile has no named ranges: it stands in for a light
        // nobody has told the desk about, so there is nothing to name.
        ranges: Vec::new(),
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
        switched: None,
        attribute,
        label: None,
        occurrence: 0,
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
        ranges: Vec::new(),
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
        physical: None,
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
        physical: None,
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
        physical: None,
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
        physical: None,
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
    /// key. A GDTF profile is held as its summary alone and read back out of
    /// its file when it is wanted whole — see [`Held`].
    profiles: BTreeMap<String, Held>,
    /// The GDTF profiles read back most recently — see [`Self::profile`].
    recent: RecentProfiles,
    /// What the last start read, so this one need not — see [`index`].
    index: IndexSlot,
    /// The searchable form of each, in the order they were added.
    entries: Vec<LibraryEntry>,
    /// The entries **by fixture** — S57, punch-list B60 — as indices into
    /// `entries`, each fixture in the order it was first met and its modes in
    /// the order its file lists them.
    fixtures: Vec<Vec<usize>>,
    /// Which of `fixtures` a fixture key is, so a mode added later joins the
    /// fixture it belongs to. See [`fixture_key`].
    fixture_index: BTreeMap<(String, bool), usize>,
    /// What reading the Open Fixture Library files cost and what they could
    /// not use.
    conversion: ofl::Conversion,
    /// The same, for the GDTF files — **S61**. Two counters rather than one
    /// because the two formats lose different things, and a single total would
    /// say neither.
    gdtf_conversion: gdtf::Conversion,
    /// And for the rig plans — **S62**. A third for the same reason as the
    /// second: an `.mvr` loses its own things (a fixture naming a profile the
    /// archive does not carry), and folding that into the GDTF count would
    /// make a plan look like a broken profile.
    mvr_conversion: mvr::Conversion,
    /// The fixture keys — `manufacturer/fixture`, without the mode — the venue
    /// supplied itself, so a vendored file of the same name is skipped whole.
    ///
    /// **Per fixture and not per mode**, and that is the decision B43 had to
    /// make. Keeping the first profile for each *mode* key would leave a venue
    /// that corrected a Mac 700 with its own 9-channel mode standing beside the
    /// vendored 16-channel one — two profiles for one lamp, one of them the
    /// thing the correction was written to replace. A venue's file is the
    /// venue's answer about that fixture, all of it.
    own_fixtures: BTreeSet<String>,
    /// Redirects met while walking, resolved once the whole tree is read.
    ///
    /// Held rather than followed on the spot, because the fixture a redirect
    /// points at is usually under another manufacturer and may not have been
    /// read yet.
    pending: Vec<PendingRedirect>,
    /// Where each GDTF profile's archive is — **S30b** — by profile key and by
    /// `guid:<FixtureTypeID>`, so a viewer can be sent the models and the gobo
    /// pictures the profile names ([`Self::resource`]).
    sources: BTreeMap<String, Source>,
}

/// One profile as the library holds it (2026-09-22).
///
/// A published GDTF library is twelve thousand fixtures in fifty-five thousand
/// modes, and held whole — every mode's geometry tree, every channel's
/// functions, every wheel — it was 1.8 GB of a desk's memory for a menu. What a
/// menu shows is two facts per mode, kept here; what patching needs is the
/// whole profile, and for a GDTF that is read back out of its file in a few
/// milliseconds when a fixture is patched ([`FixtureLibrary::profile`]). A
/// profile that is not cheap to read again — an Open Fixture Library file, a
/// rig plan, a built-in generic — is kept whole.
#[derive(Debug, Clone, PartialEq)]
struct Held {
    /// Whether it has an intensity of its own — the patch window's question.
    has_intensity: bool,
    /// How many beams its device has — the patch window's other column.
    beams: u16,
    /// The profile, when it is kept; `None` when it is read from its source.
    profile: Option<Box<FixtureType>>,
}

impl Held {
    /// The summary of `profile`, keeping it whole or not.
    fn of(profile: FixtureType, keep: bool) -> Self {
        Self {
            has_intensity: profile.has_dimmer(),
            beams: profile.physical.as_ref().map_or(0, |physical| {
                u16::try_from(physical.beams.len()).unwrap_or(u16::MAX)
            }),
            profile: keep.then(|| Box::new(profile)),
        }
    }
}

/// How many GDTF profiles read back from their files are kept.
///
/// A patch window asks for the same profile on every keystroke of its preview;
/// a rig is a handful of fixture types. Sixteen covers both.
const RECENT_PROFILES: usize = 16;

/// The GDTF profiles read back most recently, newest last.
///
/// Behind a mutex because [`FixtureLibrary::profile`] is a read: a library is
/// shared by reference and a profile being read back is not a change to it.
/// Two libraries compare equal whatever each has read back, and a clone starts
/// with nothing read back, because this is a cache and not the library.
#[derive(Debug, Default)]
struct RecentProfiles(std::sync::Mutex<Vec<FixtureType>>);

impl Clone for RecentProfiles {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for RecentProfiles {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The library's [`LibraryIndex`], while it reads. Not part of what a library
/// *is*: two libraries compare equal whatever index each read with, and a
/// clone has none.
#[derive(Debug, Default)]
struct IndexSlot(Option<LibraryIndex>);

impl Clone for IndexSlot {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for IndexSlot {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// Where a profile's files are — **S30b**.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// A `.gdtf` archive.
    Archive(std::path::PathBuf),
    /// An unpacked GDTF: the directory `description.xml` is in.
    Unpacked(std::path::PathBuf),
    /// A rig plan: the `.gdtf` members inside it are searched by GUID.
    Plan(std::path::PathBuf),
}

/// The largest file [`FixtureLibrary::resource`] serves — a gobo picture of a
/// published fixture is about a megabyte; a model a few hundred kilobytes.
const MAX_RESOURCE_BYTES: usize = 32 * 1024 * 1024;

/// The files a [`ResourceKind`] and a name may be, most wanted first.
fn resource_candidates(kind: ResourceKind, name: &str) -> Vec<String> {
    match kind {
        ResourceKind::Model => vec![
            format!("models/gltf/{name}.glb"),
            format!("models/3ds/{name}.3ds"),
        ],
        ResourceKind::Wheel => ["png", "jpg", "jpeg", "svg"]
            .iter()
            .map(|extension| format!("wheels/{name}.{extension}"))
            .collect(),
    }
}

/// The first candidate an archive holds, and its bytes.
fn resource_in(archive: &zip::Archive<'_>, candidates: &[String]) -> Option<(String, Vec<u8>)> {
    for candidate in candidates {
        let found = archive
            .names()
            .find(|name| name.eq_ignore_ascii_case(candidate))
            .map(str::to_owned);
        if let Some(found) = found
            && let Some(bytes) = archive.file(&found)
        {
            return Some((found, bytes));
        }
    }
    None
}

/// Where a file of a fixture's archive is, found by
/// [`FixtureLibrary::resource_lookup`] and read by [`Self::read`] — **S30b**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLookup {
    source: Source,
    guid: String,
    candidates: Vec<String>,
}

impl ResourceLookup {
    /// Reads the file: its path inside the archive and its bytes, or `None`
    /// when the archive has no such file or it is larger than a fixture's file
    /// has any business being.
    #[must_use]
    pub fn read(&self) -> Option<(String, Vec<u8>)> {
        let candidates = &self.candidates;
        let guid = &self.guid;
        let found = match &self.source {
            Source::Archive(path) => {
                let bytes = std::fs::read(path).ok()?;
                resource_in(&zip::Archive::read(&bytes)?, candidates)
            }
            Source::Unpacked(directory) => candidates.iter().find_map(|candidate| {
                let bytes = std::fs::read(directory.join(candidate)).ok()?;
                Some((candidate.clone(), bytes))
            }),
            Source::Plan(path) => {
                let bytes = std::fs::read(path).ok()?;
                let plan = zip::Archive::read(&bytes)?;
                let members: Vec<String> = plan
                    .names()
                    .filter(|member| member.to_ascii_lowercase().ends_with(".gdtf"))
                    .map(str::to_owned)
                    .collect();
                members.iter().find_map(|member| {
                    let inner = plan.file(member)?;
                    let archive = zip::Archive::read(&inner)?;
                    let description = archive.file(archive.find(GDTF_DESCRIPTION)?)?;
                    let text = String::from_utf8_lossy(&description).to_ascii_uppercase();
                    let wanted = if guid.is_empty() {
                        false
                    } else {
                        text.contains(&format!("FIXTURETYPEID=\"{guid}\""))
                    };
                    // A plan with one fixture type needs no GUID to be sure.
                    (wanted || members.len() == 1)
                        .then(|| resource_in(&archive, candidates))
                        .flatten()
                })
            }
        }?;
        (found.1.len() <= MAX_RESOURCE_BYTES).then_some(found)
    }
}

/// One file of a tree, waiting to be read — see
/// [`FixtureLibrary::read_gdtf_files`].
#[derive(Debug, Clone)]
enum GdtfFile {
    /// A `.gdtf` archive.
    Archive(std::path::PathBuf),
    /// An unpacked GDTF's `description.xml`.
    Unpacked(std::path::PathBuf),
    /// A rig plan.
    Plan(std::path::PathBuf),
}

impl GdtfFile {
    /// The file whose length and time say whether it changed — the archive, or
    /// an unpacked GDTF's description. A plan is read whole every time.
    fn indexed_path(&self) -> Option<&Path> {
        match self {
            Self::Archive(path) | Self::Unpacked(path) => Some(path),
            Self::Plan(_) => None,
        }
    }

    /// Where the files of what it holds are, for [`FixtureLibrary::resource`].
    fn source(&self) -> Source {
        match self {
            Self::Archive(path) => Source::Archive(path.clone()),
            Self::Unpacked(description) => {
                Source::Unpacked(description.parent().unwrap_or(description).to_path_buf())
            }
            Self::Plan(path) => Source::Plan(path.clone()),
        }
    }
}

/// One profile a file produced, reduced to what the library holds.
#[derive(Debug)]
struct Reduced {
    entry: LibraryEntry,
    held: Held,
    /// The device's GDTF GUID, upper case, for [`FixtureLibrary::resource`].
    guid: Option<String>,
}

impl Reduced {
    /// What the index remembered, as the library holds it: never whole, since
    /// only a GDTF is indexed and a GDTF is read back from its file.
    fn from_index(indexed: index::IndexedProfile) -> Self {
        Self {
            entry: indexed.entry,
            held: Held {
                has_intensity: indexed.has_intensity,
                beams: indexed.beams,
                profile: None,
            },
            guid: indexed.guid,
        }
    }

    /// What the index remembers of it.
    fn to_index(&self) -> index::IndexedProfile {
        index::IndexedProfile {
            entry: self.entry.clone(),
            has_intensity: self.held.has_intensity,
            beams: self.held.beams,
            guid: self.guid.clone(),
        }
    }

    fn of(entry: LibraryEntry, profile: FixtureType, keep: bool) -> Self {
        let guid = profile
            .physical
            .as_ref()
            .map(|physical| physical.fixture_type_id.to_ascii_uppercase())
            .filter(|guid| !guid.is_empty());
        Self {
            entry,
            held: Held::of(profile, keep),
            guid,
        }
    }
}

/// What reading one file produced.
enum ReadFile {
    Gdtf(Vec<Reduced>, gdtf::Conversion),
    Plan(Vec<Reduced>, mvr::Conversion),
}

/// Reads one file of a tree — on a worker thread, so nothing here touches the
/// library. `None` for a file that could not be read off the disk at all.
///
/// A `.gdtf` is read **without reading the archive**: its end record, its
/// central directory and its description, and none of its models or pictures
/// ([`zip::read_from_file`]). Its profiles are read back from it when they are
/// wanted whole; a plan's are kept, because a plan is read once whole.
fn read_one(file: &GdtfFile, own: bool) -> Option<ReadFile> {
    let reduce = |built: Vec<(LibraryEntry, FixtureType)>, keep: bool| -> Vec<Reduced> {
        built
            .into_iter()
            .map(|(entry, profile)| Reduced::of(entry, profile, keep))
            .collect()
    };
    match file {
        GdtfFile::Archive(path) => {
            let (built, counts) = gdtf::read_archive_file(path, own);
            Some(ReadFile::Gdtf(reduce(built, false), counts))
        }
        GdtfFile::Unpacked(description) => {
            let bytes = std::fs::read(description).ok()?;
            let (built, counts) = gdtf::read_description(&bytes, own);
            Some(ReadFile::Gdtf(reduce(built, false), counts))
        }
        GdtfFile::Plan(path) => {
            let bytes = std::fs::read(path).ok()?;
            let (built, counts, _) = mvr::read_archive(&bytes, own);
            Some(ReadFile::Plan(reduce(built, true), counts))
        }
    }
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
    /// Whether the *redirecting* file is the venue's own — B43. An alias is as
    /// much the venue's as the file that declares it, whatever it points at.
    own: bool,
}

/// The manufacturer key a loose file in the venue's own directory is filed
/// under, and the name it shows — **B43**.
///
/// A file dropped straight into `fixtures/` needs no directory of its own,
/// which is the whole convenience of *drop it in and restart*. A venue that
/// wants a real manufacturer key — because it is *correcting* a vendored
/// profile rather than adding one — puts the file in a directory named after
/// that manufacturer instead. See [`FixtureLibrary::read_own_tree`].
const CUSTOM_KEY: &str = "custom";
const CUSTOM_NAME: &str = "Custom";

/// The file stem of the manufacturer names table, which is not a fixture.
const MANUFACTURERS_STEM: &str = "manufacturers";

/// What a GDTF archive is called — **S61**.
const GDTF_EXTENSION: &str = "gdtf";

/// A venue's rig as its planner exported it — **S62**.
///
/// Read wherever a `.gdtf` is read, and for the same reason: it is a file an
/// operator was sent and put in their own folder. See [`mvr`].
const MVR_EXTENSION: &str = "mvr";

/// The document at the top of one, packed or unpacked.
const GDTF_DESCRIPTION: &str = "description.xml";

/// How far into a tree a fixture is looked for — **S61**.
///
/// The installed library is one directory per manufacturer, so one level is
/// what it takes; three leaves room for an installer that files by
/// manufacturer and then by range, and for the Open Fixture Library corpus in
/// a tree of its own beside the GDTF. What it is really for is stopping a
/// directory linked back to one of its own parents from being walked for ever
/// at start-up.
const MAX_LIBRARY_DEPTH: usize = 3;

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

    /// Reads the **installed library** into this library — S44, GDTF since
    /// S61.
    ///
    /// Two shapes live in one tree, and which of them a file is decided by its
    /// extension rather than by where it sits:
    ///
    /// - a **`.gdtf`** archive, whose key is what the file says the fixture is
    ///   and not what it is called. This is what `tools/fetch-fixtures`
    ///   installs since S61;
    /// - a **`.json`** in the Open Fixture Library's own layout — one directory
    ///   per manufacturer, one file per fixture, a `manufacturers.json` beside
    ///   them naming each directory. A desk whose library was installed before
    ///   S61 still reads, and a venue may mix the two.
    ///
    /// A directory holding a `description.xml` is an **unpacked** GDTF and is
    /// read as one, which is the shape an installer leaves behind when it wants
    /// the models and the gobo pictures on disk beside the description rather
    /// than inside an archive.
    ///
    /// A directory that is not there is not an error — a daemon started from a
    /// build tree without an installed library keeps the profiles it has.
    ///
    /// **A key already in this library is kept.** So the caller reads the
    /// operator's own folder *first* and the installed tree second, and a
    /// correction in the data directory wins without anything having to know
    /// which of the two it came from.
    ///
    /// Nothing here fails. A file that will not parse is counted — in
    /// [`Self::conversion`] or [`Self::gdtf_conversion`] — and skipped, because
    /// a desk must start with a corrupt profile in its folder.
    pub fn read_installed_tree(&mut self, root: &Path) {
        self.read_gdtf_dir(root, false);
        self.read_tree(root, false);
        self.resolve_redirects();
    }

    /// Reads **the venue's own** profile directory — punch-list entry **B43**.
    ///
    /// # Where it is, and why it is not where the library is
    ///
    /// `prismd::paths::fixtures_dir` — `fixtures/` inside the daemon's data
    /// directory, which since S29 is the desk's identity: the show, the machine
    /// configuration, the rig and the lock all live there and an installer does
    /// not touch it. The installed library is the opposite: `tools/fetch-fixtures`
    /// **empties its destination** on every run, deliberately, because a
    /// half-replaced copy of somebody else's data is worse than none. A venue's
    /// own profile put there would survive exactly until the next download,
    /// which is the state B43 reports.
    ///
    /// # Two shapes, because a venue has two reasons to put a file here
    ///
    /// - **A loose `.json` at the top**, filed under `custom/<file stem>` — a
    ///   light nobody has a profile for, dropped in and restarted. The
    ///   convenience S44 built this for, and **the reason the Open Fixture
    ///   Library's format is still read at all after S61**: a channel list in
    ///   JSON is a far kinder thing to write by hand than a ZIP archive of XML.
    /// - **A manufacturer directory**, exactly as the Open Fixture Library lays
    ///   one out, filed under `<directory>/<file stem>` — which is what makes
    ///   the override S44 documented actually work. A key already in this
    ///   library is kept ([`Self::read_installed_tree`]), and this is read
    ///   *first*, so `martin/mac-700-profile.json` in here replaces the
    ///   installed Mac 700. Until S51 there was no way to write that key at
    ///   all: everything went under `custom/`, so the documented correction was
    ///   impossible.
    /// - **A `.gdtf` file**, anywhere in here — S61. Its key is what the file
    ///   itself says the fixture is, so a manufacturer's own published archive
    ///   dropped in this directory overrides the installed copy of the same
    ///   fixture whatever either of them is called. That is a better identity
    ///   than a file name and it is the format's own.
    ///
    /// A `manufacturers.json` here names the directories, as it does in the
    /// installed tree; without one a directory is its own display name.
    ///
    /// Every profile from here is marked [`LibraryEntry::own`], because the key
    /// cannot say where it came from once a venue is allowed to reuse one.
    pub fn read_own_tree(&mut self, root: &Path) {
        // The venue's GDTF first, so that its key is in `own_fixtures` before
        // anything else claims it.
        self.read_gdtf_dir(root, true);
        // The loose files next, so a top-level `foo.json` and a
        // `custom/foo.json` resolve the way every other collision does: first
        // one wins, and the flat drop-in is the one this directory is for.
        self.read_manufacturer(root, CUSTOM_KEY, CUSTOM_NAME, true);
        self.read_tree(root, true);
        self.resolve_redirects();
    }

    /// Every `.gdtf` file and every unpacked GDTF in a tree — **S61**.
    ///
    /// Walked to [`MAX_LIBRARY_DEPTH`], so an installer may lay the library out
    /// one directory per manufacturer, or flat, or not at all: a GDTF's key
    /// comes out of the file, so where it sits says nothing and nothing has to
    /// agree about it.
    fn read_gdtf_dir(&mut self, root: &Path, own: bool) {
        let mut files = Vec::new();
        Self::walk_gdtf(root, 0, &mut files);
        self.read_gdtf_files(files, own);
    }

    /// One level of [`Self::read_gdtf_dir`]: every GDTF and rig plan under
    /// `directory`, in the order they are filed, as work to be read — each with
    /// its [`index::stamp_of`].
    ///
    /// **Everything asked of a file is asked of the directory listing**
    /// (2026-09-22). On Windows the listing carries each entry's kind, length
    /// and time, and asking the file itself opens it: twelve thousand of those,
    /// with a virus scanner watching each, were fifteen seconds of a start
    /// that read nothing else.
    fn walk_gdtf(directory: &Path, depth: usize, into: &mut Vec<(GdtfFile, Option<(u64, u64)>)>) {
        if depth > MAX_LIBRARY_DEPTH {
            return;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        // Sorted, so a library built twice on two machines holds the same
        // profiles in the same order and a recording of it is stable.
        let mut listed: Vec<std::fs::DirEntry> = entries.flatten().collect();
        listed.sort_by_key(std::fs::DirEntry::path);

        // An unpacked GDTF is a directory with a description in it, and its
        // subdirectories are its models and its gobo pictures rather than more
        // fixtures — so it is read here and not descended into.
        if depth > 0
            && let Some(description) = listed
                .iter()
                .find(|entry| entry.file_name().eq_ignore_ascii_case(GDTF_DESCRIPTION))
        {
            let stamp = description
                .metadata()
                .ok()
                .as_ref()
                .and_then(index::stamp_of);
            into.push((GdtfFile::Unpacked(description.path()), stamp));
            return;
        }
        for entry in listed {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            // A link is followed, as a directory walk always has: a venue may
            // put its library on another drive and link it here.
            let is_dir = kind.is_dir() || (kind.is_symlink() && path.is_dir());
            if is_dir {
                Self::walk_gdtf(&path, depth + 1, into);
                continue;
            }
            let extension = path.extension();
            if extension.is_some_and(|extension| extension.eq_ignore_ascii_case(GDTF_EXTENSION)) {
                let stamp = entry.metadata().ok().as_ref().and_then(index::stamp_of);
                into.push((GdtfFile::Archive(path), stamp));
            } else if extension
                .is_some_and(|extension| extension.eq_ignore_ascii_case(MVR_EXTENSION))
            {
                into.push((GdtfFile::Plan(path), None));
            }
        }
    }

    /// Reads every file of a tree **in parallel**, and files what each
    /// produced **in order** (2026-09-22).
    ///
    /// Parsing a GDTF is a megabyte of XML and the files are independent, so
    /// the work is spread over the machine's cores; filing is not, because the
    /// first profile for a key wins and which is first has to be the same on
    /// every machine. Each worker reduces what it read to what the library
    /// holds ([`Held`]) before handing it on, so a library of twelve thousand
    /// is never all in memory at once.
    fn read_gdtf_files(&mut self, listed: Vec<(GdtfFile, Option<(u64, u64)>)>, own: bool) {
        // What the index already knows, unchanged since it was read, is not
        // read again; the stamps came with the listing.
        let (files, stamps): (Vec<GdtfFile>, Vec<Option<(u64, u64)>>) = listed.into_iter().unzip();
        let mut read: Vec<Option<ReadFile>> = Vec::new();
        read.resize_with(files.len(), || None);
        let mut todo: Vec<usize> = Vec::new();
        for (index, (file, stamp)) in files.iter().zip(&stamps).enumerate() {
            let known = match (&mut self.index.0, file.indexed_path(), stamp) {
                (Some(library_index), Some(path), Some(stamp)) => {
                    library_index.lookup(path, own, *stamp)
                }
                _ => None,
            };
            match known {
                Some((profiles, counts)) => {
                    let reduced = profiles.into_iter().map(Reduced::from_index).collect();
                    if let Some(slot) = read.get_mut(index) {
                        *slot = Some(ReadFile::Gdtf(reduced, counts));
                    }
                }
                None => todo.push(index),
            }
        }

        let workers = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .clamp(1, 16)
            .min(todo.len().max(1));
        let next = std::sync::atomic::AtomicUsize::new(0);
        let slots: Vec<std::sync::Mutex<Option<ReadFile>>> =
            files.iter().map(|_| std::sync::Mutex::new(None)).collect();
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&index) = todo.get(at) else {
                            break;
                        };
                        let Some(file) = files.get(index) else {
                            break;
                        };
                        let done = read_one(file, own);
                        if let Some(slot) = slots.get(index)
                            && let Ok(mut slot) = slot.lock()
                        {
                            *slot = done;
                        }
                    }
                });
            }
        });
        for (slot, place) in slots.into_iter().zip(read.iter_mut()) {
            if let Some(done) = slot.into_inner().ok().flatten() {
                *place = Some(done);
            }
        }
        for ((file, done), stamp) in files.iter().zip(read).zip(stamps) {
            match done {
                Some(ReadFile::Gdtf(built, counts)) => {
                    if let (Some(library_index), Some(path), Some(stamp)) =
                        (&mut self.index.0, file.indexed_path(), stamp)
                    {
                        let profiles = built.iter().map(Reduced::to_index).collect();
                        library_index.record(path, own, stamp, profiles, counts);
                    }
                    self.absorb_gdtf(built, counts, own, &file.source());
                }
                Some(ReadFile::Plan(built, counts)) => self.absorb_plan(built, counts, own, file),
                None => self.gdtf_conversion.files_rejected += 1,
            }
        }
    }

    /// What one rig plan produced — **S62**.
    ///
    /// Every profile the plan carries, filed exactly as a loose `.gdtf` would
    /// be, so a fixture that arrives both ways is one row. What the plan *says*
    /// — the addresses, the positions — is read and **not acted on**; see
    /// [`mvr`] for why that is a separate decision.
    fn absorb_plan(
        &mut self,
        built: Vec<Reduced>,
        counts: mvr::Conversion,
        own: bool,
        file: &GdtfFile,
    ) {
        self.mvr_conversion.absorb(counts);
        // Each profile of the archive is filed on its own: they are separate
        // fixtures that happened to travel together, and B43's rule is per
        // fixture rather than per file.
        let source = file.source();
        for reduced in built {
            let key = fixture_key(&reduced.entry.id).to_owned();
            if own {
                self.own_fixtures.insert(key);
            } else if self.own_fixtures.contains(&key) {
                continue;
            }
            self.insert_from(reduced, &source);
        }
    }

    /// Files what one GDTF file produced, honouring B43's rule.
    ///
    /// The key is taken from what was built rather than read a second time out
    /// of the archive: it is the same string, and parsing a megabyte of XML
    /// twice to learn a fixture's name would be the whole cost of the import
    /// again.
    fn absorb_gdtf(
        &mut self,
        built: Vec<Reduced>,
        counts: gdtf::Conversion,
        own: bool,
        source: &Source,
    ) {
        self.gdtf_conversion.absorb(counts);
        let Some(key) = built
            .first()
            .map(|reduced| fixture_key(&reduced.entry.id).to_owned())
        else {
            return;
        };
        if own {
            self.own_fixtures.insert(key);
        } else if self.own_fixtures.contains(&key) {
            // The venue has its own answer about this fixture — B43. Skipped
            // whole rather than mode by mode; see `own_fixtures`.
            return;
        }
        for reduced in built {
            self.insert_from(reduced, source);
        }
    }

    /// [`Self::insert`], remembering where the profile's files are — S30b.
    fn insert_from(&mut self, reduced: Reduced, source: &Source) {
        let Reduced { entry, held, guid } = reduced;
        let key = entry.id.clone();
        if self.insert_held(entry, held) {
            self.sources.insert(key, source.clone());
            if let Some(guid) = guid {
                self.sources
                    .entry(format!("guid:{guid}"))
                    .or_insert_with(|| source.clone());
            }
        }
    }

    /// One file out of a fixture's own archive — **S30b**, and what
    /// `Query::FixtureResource` answers with.
    ///
    /// The archive is found by the device's GUID first — the same across
    /// revisions of a fixture and across desks, so a show from another desk
    /// finds this desk's copy — and by the profile key second. `name` is a
    /// name and never a path: one with a separator or a `..` in it is refused,
    /// because an unpacked GDTF is a directory and a name is joined to it.
    ///
    /// Answers the member's path inside the archive, so a client knows the
    /// format, and its bytes; `None` when the desk has no such file. The same
    /// as [`Self::resource_lookup`] and [`ResourceLookup::read`] one after the
    /// other, which is what a caller holding a lock should do instead.
    #[must_use]
    pub fn resource(
        &self,
        fixture_type_id: &str,
        type_id: &str,
        kind: ResourceKind,
        name: &str,
    ) -> Option<(String, Vec<u8>)> {
        self.resource_lookup(fixture_type_id, type_id, kind, name)?
            .read()
    }

    /// **Where** a file of a fixture's archive would be, without reading it.
    ///
    /// The daemon asks this under its core lock and reads the file after it
    /// has let go (`prismd`'s `Desk::query`): a disk read of a published
    /// archive is milliseconds, and a command waiting behind it is an operator
    /// waiting behind it.
    #[must_use]
    pub fn resource_lookup(
        &self,
        fixture_type_id: &str,
        type_id: &str,
        kind: ResourceKind,
        name: &str,
    ) -> Option<ResourceLookup> {
        if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
            return None;
        }
        let guid = fixture_type_id.trim().to_ascii_uppercase();
        let source = (!guid.is_empty())
            .then(|| self.sources.get(&format!("guid:{guid}")))
            .flatten()
            .or_else(|| self.sources.get(type_id))?;
        Some(ResourceLookup {
            source: source.clone(),
            guid,
            candidates: resource_candidates(kind, name),
        })
    }

    /// One directory of manufacturer directories, in the Open Fixture Library's
    /// own layout. Shared by the installed tree and the venue's own.
    ///
    /// **A subdirectory that has a `manufacturers.json` of its own is a tree
    /// and not a manufacturer** — S61. `tools/fetch-fixtures/fetch-ofl` puts
    /// the corpus in `profiles/fixtures/ofl/`, beside the GDTF rather than
    /// mixed into it, because the two installers each empty what they write
    /// and neither may take the other's data with it. Without this rule that
    /// corpus would be read as one manufacturer called *ofl* with no fixtures
    /// in it.
    fn read_tree(&mut self, root: &Path, own: bool) {
        self.read_tree_at(root, own, 0);
    }

    /// One level of [`Self::read_tree`], with the depth that stops a directory
    /// linked back to its own parent from being walked for ever.
    fn read_tree_at(&mut self, root: &Path, own: bool, depth: usize) {
        let names = manufacturer_names(root);
        let Ok(directory) = std::fs::read_dir(root) else {
            return;
        };
        // The kind from the listing, not from asking each path — see
        // `walk_gdtf`: a venue's folder of twelve thousand GDTF files is
        // listed here too, and every one of them would be opened to learn
        // that it is not a directory.
        let mut manufacturers: Vec<std::path::PathBuf> = directory
            .flatten()
            .filter(|entry| {
                entry
                    .file_type()
                    .is_ok_and(|kind| kind.is_dir() || (kind.is_symlink() && entry.path().is_dir()))
            })
            .map(|entry| entry.path())
            .collect();
        // Sorted, so a library built twice on two machines holds the same
        // profiles in the same order and a recording of it is stable.
        manufacturers.sort();
        for path in manufacturers {
            let Some(key) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if depth < MAX_LIBRARY_DEPTH && path.join("manufacturers.json").is_file() {
                self.read_tree_at(&path, own, depth + 1);
                continue;
            }
            let display = names.get(key).cloned().unwrap_or_else(|| key.to_owned());
            self.read_manufacturer(&path, key, &display, own);
        }
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
                // A redirect is the Open Fixture Library's, and its profiles
                // are kept whole; one pointing at a GDTF is not followed.
                .filter_map(|(id, held)| {
                    held.profile
                        .as_ref()
                        .map(|profile| (id.clone(), profile.as_ref().clone()))
                })
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
                        own: redirect.own,
                        // A redirect is an Open Fixture Library file: that
                        // format is where redirects exist at all.
                        gdtf: false,
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
    /// `own` says whether these are the venue's own profiles — see
    /// [`Self::read_own_tree`], which is what the daemon calls.
    pub fn read_fixture_dir(&mut self, path: &Path, key: &str, display: &str, own: bool) {
        self.read_manufacturer(path, key, display, own);
        self.resolve_redirects();
    }

    fn read_manufacturer(&mut self, path: &Path, key: &str, display: &str, own: bool) {
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
            // The names table is not a fixture. It is read by
            // `manufacturer_names` and counting it as a rejected file would put
            // one in every conversion report for no fault at all.
            if stem == MANUFACTURERS_STEM {
                continue;
            }
            let fixture_key = format!("{key}/{stem}");
            if own {
                self.own_fixtures.insert(fixture_key.clone());
            } else if self.own_fixtures.contains(&fixture_key) {
                // The venue has its own answer about this fixture — B43. Skipped
                // whole rather than mode by mode; see `own_fixtures`.
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&file) else {
                self.conversion.files_rejected += 1;
                continue;
            };
            if let Some(redirect) = ofl::read_redirect(&source) {
                self.conversion.redirects += 1;
                self.pending.push(PendingRedirect {
                    from: fixture_key,
                    manufacturer: display.to_owned(),
                    name: redirect.name,
                    to: redirect.to,
                    own,
                });
                continue;
            }
            let (built, counts) = ofl::read_fixture(key, display, stem, &source, own);
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
                // The four built-in generics and anything a test hands over:
                // shipped with the desk, so not the venue's.
                own: false,
                // and not GDTF, which is a claim about where a profile's
                // physical description came from — S61.
                gdtf: false,
            },
            profile,
        );
    }

    /// Adds a profile, kept whole, unless its key is taken. See
    /// [`Self::read_ofl_tree`].
    fn insert(&mut self, entry: LibraryEntry, profile: FixtureType) -> bool {
        self.insert_held(entry, Held::of(profile, true))
    }

    /// Adds a profile as the library holds it, unless its key is taken.
    fn insert_held(&mut self, entry: LibraryEntry, held: Held) -> bool {
        if self.profiles.contains_key(&entry.id) {
            return false;
        }
        self.profiles.insert(entry.id.clone(), held);
        let key = (fixture_key(&entry.id).to_owned(), entry.own);
        let index = self.entries.len();
        self.entries.push(entry);
        let next = self.fixtures.len();
        let fixture = *self.fixture_index.entry(key).or_insert(next);
        match self.fixtures.get_mut(fixture) {
            Some(modes) => modes.push(index),
            None => self.fixtures.push(vec![index]),
        }
        true
    }

    /// How many fixtures there are, counting each fixture once however many
    /// modes it has — S57.
    #[must_use]
    pub fn fixture_count(&self) -> usize {
        self.fixtures.len()
    }

    /// One page of the fixtures matching `text`, **one per fixture** with all
    /// of its modes, best first — S57, punch-list **B60**.
    ///
    /// A fixture matches when any of its modes does, by [`Self::search`]'s
    /// rule, and ranks by its best mode; an empty `text` lists the library in
    /// the order it was read, which is by manufacturer and then by fixture
    /// rather than smallest footprint first. The page is `offset` fixtures in
    /// and at most `limit` long, `limit` clamped as a search's is, so a list
    /// scrolled to its end has asked for every fixture there is without any
    /// one answer carrying two thousand of them.
    #[must_use]
    pub fn browse(&self, text: &str, offset: usize, limit: usize) -> LibraryPage {
        let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
        let words = words_of(text);
        let mut scored: Vec<(u32, usize)> = Vec::new();
        for (index, modes) in self.fixtures.iter().enumerate() {
            let best = modes
                .iter()
                .filter_map(|mode| self.entries.get(*mode))
                .filter_map(|entry| score(entry, &words))
                .min();
            if let Some(rank) = best {
                // No words: the order the library was read in, not the
                // footprint every mode ties on.
                scored.push((if words.is_empty() { 0 } else { rank }, index));
            }
        }
        scored.sort_unstable();
        let fixtures = scored
            .iter()
            .skip(offset)
            .take(limit)
            .filter_map(|(_, index)| self.fixture_at(*index))
            .collect();
        LibraryPage {
            fixtures,
            matched: scored.len(),
            total: self.fixtures.len(),
        }
    }

    /// The fixture one profile key is a mode of, with all its modes — S57.
    #[must_use]
    pub fn fixture_of(&self, type_id: &str) -> Option<LibraryFixture> {
        let own = self
            .entries
            .iter()
            .find(|entry| entry.id == type_id)
            .map(|entry| entry.own)?;
        let index = self
            .fixture_index
            .get(&(fixture_key(type_id).to_owned(), own))?;
        self.fixture_at(*index)
    }

    /// One fixture of `fixtures`, as a client is sent it.
    fn fixture_at(&self, index: usize) -> Option<LibraryFixture> {
        let modes = self.fixtures.get(index)?;
        let first = self.entries.get(*modes.first()?)?;
        Some(LibraryFixture {
            manufacturer: first.manufacturer.clone(),
            name: first.name.clone(),
            own: first.own,
            gdtf: first.gdtf,
            modes: modes
                .iter()
                .filter_map(|mode| self.entries.get(*mode))
                .map(|entry| LibraryMode {
                    id: entry.id.clone(),
                    mode: entry.mode.clone(),
                    footprint: entry.footprint,
                    has_intensity: self
                        .profiles
                        .get(&entry.id)
                        .is_some_and(|held| held.has_intensity),
                    beams: self.profiles.get(&entry.id).map_or(0, |held| held.beams),
                })
                .collect(),
        })
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

    /// What reading the Open Fixture Library files cost.
    #[must_use]
    pub const fn conversion(&self) -> ofl::Conversion {
        self.conversion
    }

    /// What reading the GDTF files cost — **S61**.
    #[must_use]
    pub const fn gdtf_conversion(&self) -> gdtf::Conversion {
        self.gdtf_conversion
    }

    /// What reading the rig plans cost — **S62**.
    #[must_use]
    pub const fn mvr_conversion(&self) -> mvr::Conversion {
        self.mvr_conversion
    }

    /// How many of the profiles offered came out of a GDTF file — **S61**.
    ///
    /// What the daemon logs at start-up, and what tells an installer whether
    /// the GDTF library is actually installed: a desk offering four profiles
    /// and none of them GDTF has not had its library downloaded.
    #[must_use]
    pub fn gdtf_profiles(&self) -> usize {
        self.entries.iter().filter(|entry| entry.gdtf).count()
    }

    /// Files the profiles of one GDTF archive the venue has just imported, as
    /// its own — **S62**'s import, without reading the library again
    /// (2026-09-22).
    ///
    /// A profile whose key is already here is left as it is: which of two
    /// copies of a fixture wins is the whole library's question, and a full
    /// read in the background answers it. What this answers is the operator's
    /// — a fixture imported is offered at once.
    pub fn file_imported(&mut self, built: Vec<(LibraryEntry, FixtureType)>, archive: &Path) {
        let source = Source::Archive(archive.to_path_buf());
        for (entry, profile) in built {
            self.own_fixtures.insert(fixture_key(&entry.id).to_owned());
            self.insert_from(Reduced::of(entry, profile, false), &source);
        }
    }

    /// Reads with `index` from here on: a file it knows, unchanged, is taken
    /// from it rather than parsed — see [`index`].
    pub fn use_index(&mut self, index: LibraryIndex) {
        self.index.0 = Some(index);
    }

    /// The index, with what this library read recorded in it, to be saved.
    pub fn take_index(&mut self) -> Option<LibraryIndex> {
        self.index.0.take()
    }

    /// One profile by key, ready to embed into a show.
    ///
    /// A GDTF profile is read back out of its file ([`Held`]) — a few
    /// milliseconds, once per patch rather than once per start — and the
    /// last few read are kept, so a patch window previewing on every
    /// keystroke reads the file once. `None` when there is no such key, or
    /// when the file it came from is gone or no longer holds it.
    #[must_use]
    pub fn profile(&self, id: &str) -> Option<FixtureType> {
        let held = self.profiles.get(id)?;
        if let Some(profile) = &held.profile {
            return Some(profile.as_ref().clone());
        }
        if let Ok(recent) = self.recent.0.lock()
            && let Some(found) = recent.iter().find(|profile| profile.id == id)
        {
            return Some(found.clone());
        }
        let own = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .is_some_and(|entry| entry.own);
        let built = match self.sources.get(id)? {
            Source::Archive(path) => gdtf::read_archive_file(path, own).0,
            Source::Unpacked(directory) => {
                let bytes = std::fs::read(directory.join(GDTF_DESCRIPTION)).ok()?;
                gdtf::read_description(&bytes, own).0
            }
            // Kept whole when filed, so never here.
            Source::Plan(_) => return None,
        };
        let profile = built
            .into_iter()
            .map(|(_, profile)| profile)
            .find(|profile| profile.id == id)?;
        if let Ok(mut recent) = self.recent.0.lock() {
            if recent.len() >= RECENT_PROFILES {
                recent.remove(0);
            }
            recent.push(profile.clone());
        }
        Some(profile)
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
        let words = words_of(text);

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

/// One page of [`FixtureLibrary::browse`] — S57.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryPage {
    /// The fixtures on this page, best first.
    pub fixtures: Vec<LibraryFixture>,
    /// How many fixtures matched in all.
    pub matched: usize,
    /// How many fixtures the library holds.
    pub total: usize,
}

/// Which fixture a profile key is a mode of — S57.
///
/// An Open Fixture Library key is `manufacturer/fixture/mode`, so the fixture
/// is everything before the last slash; the venue's own folder uses the same
/// layout. A key with fewer than two slashes — the four built-in generics are
/// `generic.dimmer` and the like — is a fixture of its own with one mode.
fn fixture_key(id: &str) -> &str {
    match id.rsplit_once('/') {
        Some((fixture, _)) if fixture.contains('/') => fixture,
        _ => id,
    }
}

/// What was typed, as the lower-case words a search matches.
fn words_of(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(str::to_lowercase)
        .filter(|word| !word.is_empty())
        .collect()
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
    use super::{
        DEFAULT_SEARCH_LIMIT, FixtureLibrary, MAX_SEARCH_LIMIT, fixture_key, generic_profiles,
    };
    use crate::Show;
    use prism_domain::{AttributeType, FeatureGroup, FixtureType, LibraryEntry};
    use std::collections::BTreeSet;

    /// A profile with a key and a footprint and nothing else to say.
    fn mode_of(id: &str, name: &str, mode: &str, footprint: u16) -> FixtureType {
        FixtureType {
            id: id.to_owned(),
            manufacturer: "Maker".to_owned(),
            name: name.to_owned(),
            mode: mode.to_owned(),
            footprint,
            attributes: Vec::new(),
            physical: None,
        }
    }

    /// **A fixture is listed once, with its modes** — S57, punch-list B60.
    #[test]
    fn the_library_is_browsed_one_fixture_at_a_time_and_a_page_at_a_time() {
        let mut library = FixtureLibrary::default();
        library.insert_profile(mode_of("maker/spot/16ch", "Spot", "16ch", 16));
        library.insert_profile(mode_of("maker/spot/8ch", "Spot", "8ch", 8));
        library.insert_profile(mode_of("maker/wash/4ch", "Wash", "4ch", 4));
        // A mode met later still joins its fixture.
        library.insert_profile(mode_of("maker/spot/24ch", "Spot", "24ch", 24));
        assert_eq!(library.len(), 4, "four profiles");
        assert_eq!(library.fixture_count(), 2, "two fixtures");

        let all = library.browse("", 0, 10);
        assert_eq!((all.matched, all.total), (2, 2));
        assert_eq!(all.fixtures[0].name, "Spot", "the order it was read in");
        assert_eq!(
            all.fixtures[0]
                .modes
                .iter()
                .map(|mode| mode.id.as_str())
                .collect::<Vec<_>>(),
            vec!["maker/spot/16ch", "maker/spot/8ch", "maker/spot/24ch"]
        );

        // One mode matching is the fixture matching, and it brings all of them.
        let found = library.browse("8ch", 0, 10);
        assert_eq!(found.matched, 1);
        assert_eq!(found.fixtures[0].modes.len(), 3);
        // Nothing matching is nothing, not everything.
        assert_eq!(library.browse("no such light", 0, 10).matched, 0);

        // A page at a time: one, then the other, then nothing past the end.
        assert_eq!(library.browse("", 0, 1).fixtures[0].name, "Spot");
        assert_eq!(library.browse("", 1, 1).fixtures[0].name, "Wash");
        assert!(library.browse("", 2, 1).fixtures.is_empty());
        assert_eq!(library.browse("", 2, 1).matched, 2);

        // And a mode finds its fixture.
        let spot = library.fixture_of("maker/spot/8ch").unwrap();
        assert_eq!(spot.modes.len(), 3);
        assert_eq!(library.fixture_of("maker/spot/9ch"), None);
    }

    #[test]
    fn a_key_without_a_fixture_in_it_is_a_fixture_of_its_own() {
        assert_eq!(fixture_key("robe/mmx-spot/16ch"), "robe/mmx-spot");
        assert_eq!(fixture_key("generic.dimmer"), "generic.dimmer");
        assert_eq!(fixture_key("venue/one"), "venue/one");
        // The generics are four fixtures of one mode each.
        let generics = FixtureLibrary::generic();
        assert_eq!(generics.fixture_count(), generic_profiles().len());
        assert!(
            generics
                .browse("", 0, 10)
                .fixtures
                .iter()
                .all(|fixture| fixture.modes.len() == 1)
        );
        // And whether a mode has an intensity is the profile's answer.
        let dimmer = generics.fixture_of("generic.dimmer").unwrap();
        assert!(dimmer.modes[0].has_intensity);
        let par = generics.fixture_of("generic.rgbw.par").unwrap();
        assert!(!par.modes[0].has_intensity);
    }

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

    /// The same fixture as the venue would correct it: one wider mode, and none
    /// of the vendored file's.
    const WIDE_HEAD: &str = r#"{
      "name": "Wash 7Q5 (as hung here)",
      "availableChannels": {
        "Pan": { "capability": { "type": "Pan", "angleStart": "0deg", "angleEnd": "540deg" } },
        "Tilt": { "capability": { "type": "Tilt", "angleStart": "0deg", "angleEnd": "180deg" } },
        "Dimmer": { "capability": { "type": "Intensity" } },
        "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } },
        "Green": { "capability": { "type": "ColorIntensity", "color": "Green" } },
        "Blue": { "capability": { "type": "ColorIntensity", "color": "Blue" } },
        "White": { "capability": { "type": "ColorIntensity", "color": "White" } },
        "Zoom": { "capability": { "type": "Zoom" } },
        "Shutter": { "capability": { "type": "ShutterStrobe", "shutterEffect": "Strobe" } }
      },
      "modes": [
        {
          "shortName": "9ch",
          "channels": [
            "Pan", "Tilt", "Dimmer", "Red", "Green", "Blue", "White", "Zoom", "Shutter"
          ]
        }
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
            assert_eq!(
                library.profile(key).map(|found| found.id),
                Some(key.clone())
            );
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
        library.read_installed_tree(dir.path());

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
        library.read_installed_tree(dir.path());
        assert_eq!(
            library
                .profile("nameless-co/thing/4ch")
                .map(|profile| profile.manufacturer),
            Some("nameless-co".to_owned())
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
        library.read_installed_tree(corrected.path());
        library.read_installed_tree(vendored.path());
        assert_eq!(
            library
                .profile("robe/wash-7q5/4ch")
                .map(|profile| profile.name),
            Some("Wash 7Q5 (corrected)".to_owned())
        );
        assert_eq!(library.len(), 2, "and not four: the key is the same");
    }

    /// A directory that is not there is not an error, and neither is a file
    /// that will not parse. A desk starts.
    #[test]
    fn nothing_about_a_missing_or_broken_directory_stops_anything() {
        let mut library = FixtureLibrary::generic();
        library.read_installed_tree(std::path::Path::new("no/such/directory"));
        assert_eq!(library.len(), generic_profiles().len());

        let dir = on_disk(&[
            ("robe/broken.json", "not json at all"),
            ("robe/wash-7q5.json", HEAD),
            ("robe/notes.txt", "ignored: only .json is read"),
        ]);
        library.read_installed_tree(dir.path());
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
        library.read_fixture_dir(dir.path(), "custom", "Custom", true);
        assert_eq!(library.len(), 2);
        let profile = library
            .profile("custom/mine/4ch")
            .expect("keyed by the stem");
        assert_eq!(profile.manufacturer, "Custom");
    }

    /* -- the venue's own directory (B43) ------------------------------------ */

    /// **The venue's own directory is both shapes at once** — B43.
    ///
    /// A loose file is filed under `custom/`, which is S44's *drop it in and
    /// restart*; a file inside a manufacturer directory is filed under that
    /// manufacturer's key, which is what a **correction** to a vendored profile
    /// needs and what was impossible before S51.
    #[test]
    fn the_venues_own_directory_takes_a_loose_file_and_a_manufacturer_directory() {
        let dir = on_disk(&[("mine.json", HEAD), ("martin/mac-700.json", HEAD)]);
        let mut library = FixtureLibrary::default();
        library.read_own_tree(dir.path());

        assert!(
            library.profile("custom/mine/4ch").is_some(),
            "the loose file"
        );
        assert!(
            library.profile("martin/mac-700/4ch").is_some(),
            "the manufacturer directory"
        );
    }

    /// **A venue's profile wins over the vendored one with the same key** —
    /// which is the override S44 documented and could not perform.
    ///
    /// Read first, and `insert` keeps the first profile it is given, so this is
    /// the whole of the rule. Before S51 a venue's file could only ever be
    /// keyed `custom/…`, so it could not collide with a vendored key at all and
    /// the documented correction quietly did nothing.
    #[test]
    fn a_venues_profile_replaces_the_vendored_one_it_names() {
        let vendored = on_disk(&[("martin/mac-700.json", HEAD)]);
        let mine = on_disk(&[("martin/mac-700.json", WIDE_HEAD)]);

        let mut library = FixtureLibrary::default();
        library.read_own_tree(mine.path());
        library.read_installed_tree(vendored.path());

        let profile = library
            .profile("martin/mac-700/9ch")
            .expect("the venue's own mode is the one that is there");
        assert_eq!(profile.footprint, 9);
        assert!(
            library.profile("martin/mac-700/4ch").is_none(),
            "the vendored copy came back beside the correction"
        );
    }

    /// **The picker can tell whose a profile is** — B43's *marked as the
    /// venue's own*.
    ///
    /// It cannot be read off the key, and that is the reason the flag exists: a
    /// correction deliberately carries a vendored key.
    #[test]
    fn an_entry_says_whether_it_is_the_venues_own() {
        let mine = on_disk(&[("martin/mac-700.json", HEAD)]);
        let vendored = on_disk(&[("robe/mmx.json", HEAD)]);
        let mut library = FixtureLibrary::default();
        library.read_own_tree(mine.path());
        library.read_installed_tree(vendored.path());
        for profile in generic_profiles() {
            library.insert_profile(profile);
        }

        let own = |id: &str| {
            library
                .entries()
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.own)
        };
        assert_eq!(own("martin/mac-700/4ch"), Some(true));
        assert_eq!(own("robe/mmx/4ch"), Some(false));
        assert_eq!(own("generic.dimmer"), Some(false), "shipped with the desk");
    }

    /// A `manufacturers.json` in the venue's directory is a names table, not a
    /// fixture, and is not counted as a file that could not be read.
    #[test]
    fn the_names_table_is_not_a_rejected_fixture() {
        let dir = on_disk(&[
            (
                "manufacturers.json",
                r#"{ "martin": { "name": "Martin" } }"#,
            ),
            ("martin/mac-700.json", HEAD),
        ]);
        let mut library = FixtureLibrary::default();
        library.read_own_tree(dir.path());
        assert_eq!(library.conversion().files_rejected, 0);
        assert_eq!(
            library
                .entries()
                .iter()
                .find(|entry| entry.id == "martin/mac-700/4ch")
                .map(|entry| entry.manufacturer.as_str()),
            Some("Martin"),
            "the names table is read here as it is in the vendored tree"
        );
    }

    /* -- GDTF, S61 ---------------------------------------------------------- */

    /// A `description.xml` for a one-mode fixture, with a beam in it.
    fn gdtf_source(manufacturer: &str, name: &str, footprint: u16) -> String {
        let channels: String = (1..=footprint)
            .map(|offset| {
                format!(
                    r#"<DMXChannel Offset="{offset}">
                         <LogicalChannel Attribute="Dimmer">
                           <ChannelFunction Attribute="Dimmer"/>
                         </LogicalChannel>
                       </DMXChannel>"#
                )
            })
            .collect();
        format!(
            r#"<GDTF DataVersion="1.2">
                 <FixtureType Name="{name}" Manufacturer="{manufacturer}" FixtureTypeID="GUID">
                   <Models><Model Name="Body" File="body" Length="0.2" Width="0.2" Height="0.3"/></Models>
                   <Geometries>
                     <Geometry Name="Body" Model="Body" Position="{IDENTITY}">
                       <Beam Name="Beam" Position="{IDENTITY}" BeamAngle="15"/>
                     </Geometry>
                   </Geometries>
                   <DMXModes>
                     <DMXMode Name="Standard" Geometry="Body">
                       <DMXChannels>{channels}</DMXChannels>
                     </DMXMode>
                   </DMXModes>
                 </FixtureType>
               </GDTF>"#
        )
    }

    /// The identity, as GDTF writes a `Position`.
    const IDENTITY: &str = "{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}";

    /// A `.gdtf` archive on disk, at a path of the caller's choosing.
    fn write_gdtf(root: &std::path::Path, at: &str, source: &str) {
        let full = root.join(at);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("it creates");
        std::fs::write(
            full,
            crate::library::zip::testkit::one_file("description.xml", source.as_bytes()),
        )
        .expect("it writes");
    }

    /// **A `.gdtf` file is a fixture, and its key is what the file says it
    /// is** — S61.
    #[test]
    fn a_gdtf_file_becomes_a_profile_keyed_by_its_contents() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        // Filed under a directory that says nothing, and named something else
        // again, which is what a download off a manufacturer's site looks
        // like. The key is neither of them.
        write_gdtf(
            dir.path(),
            "downloads/Robe@Robin T1 Profile@3.gdtf",
            &gdtf_source("Robe Lighting", "Robin T1 Profile", 2),
        );
        let mut library = FixtureLibrary::default();
        library.read_installed_tree(dir.path());

        assert_eq!(library.len(), 1);
        let profile = library
            .profile("robe-lighting/robin-t1-profile/standard")
            .expect("the key comes out of the file");
        assert_eq!(profile.manufacturer, "Robe Lighting");
        assert_eq!(profile.footprint, 2);
        let physical = profile.physical.as_ref().expect("a GDTF profile has one");
        assert_eq!(physical.beams.len(), 1);
        assert_eq!(physical.model.as_deref(), Some("body"));
        assert_eq!(library.gdtf_conversion().fixtures, 1);
        assert_eq!(library.gdtf_conversion().beams, 1);
        assert_eq!(library.gdtf_profiles(), 1);
        // And the entry says so, which is what the picker marks.
        assert!(library.entries()[0].gdtf);
    }

    /// An **unpacked** GDTF — the shape an installer leaves when it wants the
    /// models and the gobo pictures on disk beside the description.
    #[test]
    fn an_unpacked_gdtf_reads_the_same_as_an_archive() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let source = gdtf_source("Robe Lighting", "Robin T1 Profile", 2);
        std::fs::create_dir_all(dir.path().join("robe/t1/models")).expect("it creates");
        std::fs::write(dir.path().join("robe/t1/description.xml"), &source).expect("it writes");
        // A file under the unpacked fixture that is not a fixture, which must
        // not be walked into as if it were one.
        std::fs::write(dir.path().join("robe/t1/models/body.glb"), b"glb").expect("it writes");

        let mut library = FixtureLibrary::default();
        library.read_installed_tree(dir.path());
        assert_eq!(library.len(), 1);
        assert!(
            library
                .profile("robe-lighting/robin-t1-profile/standard")
                .is_some()
        );
        assert_eq!(library.gdtf_conversion().files_rejected, 0);
    }

    /// **The venue's own GDTF wins** — B43's rule, on the format's own
    /// identity rather than on a file name.
    #[test]
    fn a_venue_s_own_gdtf_replaces_the_installed_one_whatever_it_is_called() {
        let mine = tempfile::tempdir().expect("a temporary directory");
        let installed = tempfile::tempdir().expect("a temporary directory");
        // Same fixture, four channels instead of two, under a different file
        // name — which is the case a key taken from the file name gets wrong.
        write_gdtf(
            mine.path(),
            "my-corrected-t1.gdtf",
            &gdtf_source("Robe Lighting", "Robin T1 Profile", 4),
        );
        write_gdtf(
            installed.path(),
            "robe-lighting/robin-t1-profile.gdtf",
            &gdtf_source("Robe Lighting", "Robin T1 Profile", 2),
        );

        let mut library = FixtureLibrary::default();
        library.read_own_tree(mine.path());
        library.read_installed_tree(installed.path());

        assert_eq!(library.len(), 1, "one fixture, not two");
        let profile = library
            .profile("robe-lighting/robin-t1-profile/standard")
            .expect("the venue's");
        assert_eq!(profile.footprint, 4, "the venue's answer is the one kept");
        assert!(library.entries()[0].own);
    }

    /// **A venue's Open Fixture Library file still works** — the whole of what
    /// S61 promised to keep.
    #[test]
    fn the_two_formats_live_in_one_library() {
        let mine = tempfile::tempdir().expect("a temporary directory");
        let installed = tempfile::tempdir().expect("a temporary directory");
        // A light nobody has published a GDTF for, written by hand in the
        // kinder format.
        std::fs::write(mine.path().join("shop-special.json"), HEAD).expect("it writes");
        write_gdtf(
            installed.path(),
            "robe.gdtf",
            &gdtf_source("Robe Lighting", "Robin T1 Profile", 2),
        );

        let mut library = FixtureLibrary::default();
        library.read_own_tree(mine.path());
        library.read_installed_tree(installed.path());

        assert!(library.profile("custom/shop-special/4ch").is_some());
        assert!(
            library
                .profile("robe-lighting/robin-t1-profile/standard")
                .is_some()
        );
        assert_eq!(library.gdtf_profiles(), 1, "one of the three is GDTF");
        assert_eq!(
            library.len(),
            3,
            "two modes of the JSON and one of the GDTF"
        );
        // The fixture list marks which is which, which is what the picker
        // shows.
        let fixtures: Vec<(String, bool)> = (0..library.fixture_count())
            .filter_map(|index| library.fixture_at(index))
            .map(|fixture| (fixture.name, fixture.gdtf))
            .collect();
        assert_eq!(
            fixtures,
            [
                ("Wash 7Q5".to_owned(), false),
                ("Robin T1 Profile".to_owned(), true),
            ]
        );
    }

    /// A file that is not a fixture is counted and left, and the desk starts.
    #[test]
    fn a_broken_gdtf_is_counted_and_the_rest_still_reads() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(
            dir.path().join("truncated.gdtf"),
            b"PK\x03\x04 and then nothing",
        )
        .expect("it writes");
        write_gdtf(dir.path(), "good.gdtf", &gdtf_source("Maker", "Thing", 1));

        let mut library = FixtureLibrary::default();
        library.read_installed_tree(dir.path());
        assert_eq!(library.gdtf_conversion().files_rejected, 1);
        assert_eq!(library.len(), 1);
    }

    /// The number of beams travels to the picker, which is what tells an
    /// operator the viewer can draw this one properly.
    #[test]
    fn a_mode_carries_how_many_beams_its_device_has() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        write_gdtf(dir.path(), "a.gdtf", &gdtf_source("Maker", "Thing", 1));
        let mut library = FixtureLibrary::generic();
        library.read_installed_tree(dir.path());

        let gdtf = library
            .fixture_of("maker/thing/standard")
            .expect("it is in the library");
        assert_eq!(gdtf.modes[0].beams, 1);
        let generic = library
            .fixture_of("generic.dimmer")
            .expect("the desk carries one");
        assert_eq!(generic.modes[0].beams, 0, "a generic describes no device");
        assert!(!generic.gdtf);
    }

    /* -- MVR, S62 ----------------------------------------------------------- */

    /// **A rig plan in the venue's own folder fills the library** — S62.
    ///
    /// The whole point of reading MVR: a venue that was sent its own plan was
    /// sent the profiles it needs, and needs no account and no network to use
    /// them.
    #[test]
    fn a_profile_s_files_are_served_out_of_its_own_archive() {
        use crate::library::zip::testkit::Builder;
        use prism_domain::ResourceKind;

        let archive = Builder::new()
            .deflated(
                "description.xml",
                gdtf_source("Robe Lighting", "Robin T1 Profile", 3).as_bytes(),
            )
            .stored("models/gltf/body.glb", b"glTF-bytes")
            .stored("models/3ds/other.3ds", b"3ds-bytes")
            .stored("wheels/15020356.png", b"png-bytes")
            .build();
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(dir.path().join("t1.gdtf"), &archive).expect("it writes");
        let mut library = FixtureLibrary::default();
        library.read_own_tree(dir.path());

        let key = "robe-lighting/robin-t1-profile/standard";
        // By the device's GUID, whatever key a show carries it under — and in
        // either case of the GUID.
        assert_eq!(
            library.resource("guid", "some/other/key", ResourceKind::Model, "body"),
            Some(("models/gltf/body.glb".to_owned(), b"glTF-bytes".to_vec()))
        );
        // By the key where the GUID is unknown.
        assert_eq!(
            library.resource("", key, ResourceKind::Model, "other"),
            Some(("models/3ds/other.3ds".to_owned(), b"3ds-bytes".to_vec()))
        );
        assert_eq!(
            library.resource("GUID", key, ResourceKind::Wheel, "15020356"),
            Some(("wheels/15020356.png".to_owned(), b"png-bytes".to_vec()))
        );
        // Nothing the archive does not have, and nothing that is not a name.
        assert_eq!(
            library.resource("GUID", key, ResourceKind::Model, "head"),
            None
        );
        assert_eq!(
            library.resource("NOPE", "no/such/key", ResourceKind::Model, "body"),
            None
        );
        for sneaky in ["../t1", "models/gltf/body", r"a\b", ""] {
            assert_eq!(
                library.resource("GUID", key, ResourceKind::Model, sneaky),
                None,
                "{sneaky}"
            );
        }

        // And out of a rig plan, whose members are searched by GUID.
        let plan = Builder::new()
            .deflated("GeneralSceneDescription.xml", b"<GeneralSceneDescription/>")
            .stored("Robe@T1.gdtf", &archive)
            .build();
        let other = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(other.path().join("rig.mvr"), plan).expect("it writes");
        let mut from_plan = FixtureLibrary::default();
        from_plan.read_own_tree(other.path());
        assert_eq!(
            from_plan
                .resource("GUID", "", ResourceKind::Wheel, "15020356")
                .map(|(path, _)| path),
            Some("wheels/15020356.png".to_owned())
        );
    }

    #[test]
    fn an_mvr_in_the_venues_folder_is_read_like_a_gdtf() {
        use crate::library::zip::testkit::Builder;

        let inner = Builder::new()
            .deflated(
                "description.xml",
                gdtf_source("Robe Lighting", "Robin T1 Profile", 3).as_bytes(),
            )
            .build();
        let plan = r#"<GeneralSceneDescription verMajor="1" verMinor="6">
              <Scene><Layers><Layer name="Stage"><ChildList>
                <Fixture uuid="F1" name="Front left">
                  <GDTFSpec>Robe@T1.gdtf</GDTFSpec>
                  <GDTFMode>Standard</GDTFMode>
                  <FixtureID>1</FixtureID>
                  <Addresses><Address break="1">1</Address></Addresses>
                </Fixture>
              </ChildList></Layer></Layers></Scene>
            </GeneralSceneDescription>"#;
        let archive = Builder::new()
            .deflated("GeneralSceneDescription.xml", plan.as_bytes())
            .deflated("Robe@T1.gdtf", &inner)
            .build();

        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(dir.path().join("unser-rig.mvr"), archive).expect("it writes");

        let mut library = FixtureLibrary::default();
        library.read_own_tree(dir.path());

        assert_eq!(library.len(), 1, "the plan's profile is in the library");
        let profile = library
            .profile("robe-lighting/robin-t1-profile/standard")
            .expect("the key comes out of the profile, not out of the plan");
        assert_eq!(profile.footprint, 3);
        assert_eq!(library.mvr_conversion().profiles, 1);
        assert_eq!(library.mvr_conversion().fixtures, 1);
        assert_eq!(library.mvr_conversion().fixtures_without_profile, 0);
        // It is the venue's own, so it wins against the installed library —
        // B43's rule, reached through a third kind of file.
        assert!(library.entries()[0].own);
        assert!(library.entries()[0].gdtf);
    }

    /// A plan and a loose copy of the same fixture are **one** row.
    #[test]
    fn a_fixture_that_arrives_twice_is_one_profile() {
        use crate::library::zip::testkit::Builder;

        let dir = tempfile::tempdir().expect("a temporary directory");
        let source = gdtf_source("Robe Lighting", "Robin T1 Profile", 3);
        let inner = Builder::new()
            .stored("description.xml", source.as_bytes())
            .build();
        std::fs::write(
            dir.path().join("a-plan.mvr"),
            Builder::new().stored("Robe@T1.gdtf", &inner).build(),
        )
        .expect("it writes");
        write_gdtf(dir.path(), "the-same-light.gdtf", &source);

        let mut library = FixtureLibrary::default();
        library.read_own_tree(dir.path());
        assert_eq!(library.len(), 1, "one fixture, whichever file it came in");
    }

    /// Something that is not an archive is counted and left, and the rest of
    /// the folder still reads.
    #[test]
    fn a_broken_mvr_does_not_take_the_folder_with_it() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(dir.path().join("truncated.mvr"), b"PK and then nothing")
            .expect("it writes");
        write_gdtf(dir.path(), "good.gdtf", &gdtf_source("Maker", "Thing", 1));

        let mut library = FixtureLibrary::default();
        library.read_own_tree(dir.path());
        assert_eq!(library.len(), 1, "the good one still arrives");
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
                    own: false,
                    gdtf: false,
                },
                FixtureType {
                    id: id.to_owned(),
                    manufacturer: manufacturer.to_owned(),
                    name: name.to_owned(),
                    mode: mode.to_owned(),
                    footprint,
                    attributes: Vec::new(),
                    physical: None,
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

    /// A GDTF archive written into a fresh directory, for the start-up tests.
    fn gdtf_tree(files: &[(&str, &str, &str, u16)]) -> tempfile::TempDir {
        use crate::library::zip::testkit::Builder;
        let dir = tempfile::tempdir().expect("a temporary directory");
        for (file, manufacturer, name, footprint) in files {
            let archive = Builder::new()
                .deflated(
                    "description.xml",
                    gdtf_source(manufacturer, name, *footprint).as_bytes(),
                )
                .stored("models/gltf/body.glb", &[0_u8; 4096])
                .build();
            std::fs::write(dir.path().join(file), archive).expect("it writes");
        }
        dir
    }

    /// **A GDTF profile is held as its summary and read back whole** — the
    /// library of twelve thousand (2026-09-22). What comes back is exactly
    /// what reading the file gives, and the menu still knows the two facts it
    /// shows without reading anything.
    #[test]
    fn a_gdtf_profile_is_read_back_out_of_its_file_when_it_is_wanted() {
        let dir = gdtf_tree(&[("t1.gdtf", "Robe Lighting", "Robin T1 Profile", 3)]);
        let mut library = FixtureLibrary::default();
        library.read_installed_tree(dir.path());
        let key = "robe-lighting/robin-t1-profile/standard";

        let held = library.profiles.get(key).expect("filed");
        assert!(held.profile.is_none(), "not kept whole");
        let fixture = library.fixture_of(key).expect("a fixture");
        assert!(fixture.modes[0].has_intensity);

        let direct = super::gdtf::read_archive_file(&dir.path().join("t1.gdtf"), false)
            .0
            .into_iter()
            .map(|(_, profile)| profile)
            .find(|profile| profile.id == key)
            .expect("the file holds it");
        assert_eq!(library.profile(key), Some(direct.clone()));
        // Kept for the next question, which does not need the file.
        std::fs::remove_file(dir.path().join("t1.gdtf")).expect("removed");
        assert_eq!(library.profile(key), Some(direct));
        // A clone starts with nothing read back, so it has to ask the file.
        assert_eq!(library.clone().profile(key), None);
        assert_eq!(library.profile("no/such/key"), None);
    }

    /// **The second start reads nothing it read before** — and ends with the
    /// same library.
    #[test]
    fn a_library_read_with_its_index_is_the_library_read_without_it() {
        let dir = gdtf_tree(&[
            ("a.gdtf", "Robe Lighting", "Robin T1 Profile", 3),
            ("b.gdtf", "Martin", "MAC Aura", 14),
        ]);
        let index_at = tempfile::tempdir().expect("a directory");
        let index_path = index_at.path().join("library-index.json");

        let read = |expect_hits: usize| {
            let mut library = FixtureLibrary::default();
            library.use_index(super::LibraryIndex::open(&index_path));
            library.read_installed_tree(dir.path());
            let index = library.take_index().expect("the index");
            assert_eq!(index.hits(), expect_hits);
            index.save();
            library
        };
        let first = read(0);
        let second = read(2);
        assert_eq!(first.entries(), second.entries());
        assert_eq!(first, second, "the same library either way");
        assert_eq!(
            first.fixture_of("martin/mac-aura/standard"),
            second.fixture_of("martin/mac-aura/standard")
        );
        assert_eq!(
            first.gdtf_conversion(),
            second.gdtf_conversion(),
            "and the same counts"
        );

        // A file that changed is read again, and what it says now is filed.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let archive = crate::library::zip::testkit::Builder::new()
            .deflated(
                "description.xml",
                gdtf_source("Martin", "MAC Aura", 20).as_bytes(),
            )
            .build();
        std::fs::write(dir.path().join("b.gdtf"), archive).expect("rewritten");
        let third = read(1);
        assert_eq!(
            third
                .entries()
                .iter()
                .find(|entry| entry.id == "martin/mac-aura/standard")
                .map(|entry| entry.footprint),
            Some(20)
        );

        // An index that is not one is no index.
        std::fs::write(&index_path, b"not json").expect("written");
        let _ = read(0);
    }
}
