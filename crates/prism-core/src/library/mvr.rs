//! A venue's rig as its planner exported it — **S62**.
//!
//! # Why this is the most useful of the four ways to get a library
//!
//! `docs/FIXTURE_LIBRARY.md` lays out the problem S61 left: the desk reads GDTF
//! and GDTF's upstream hands nothing out without an account, so *where does a
//! venue get the files* is a real question with an awkward answer.
//!
//! An `.mvr` answers it without asking anybody for anything. **My Virtual Rig**
//! is the format a lighting plan is exchanged in, and it is a ZIP holding a
//! `GeneralSceneDescription.xml` **and the `.gdtf` file of every fixture in the
//! plan**. So a venue that has been sent its own rig has been sent exactly the
//! profiles it needs, already licensed to it as part of its own production. No
//! account, no network, no clause to read.
//!
//! # What this reads, and what it deliberately does not
//!
//! **It fills the library.** Every `.gdtf` inside the archive goes through
//! [`super::gdtf`], exactly as a loose one would, and lands under the same key —
//! so a fixture that arrives twice, once in an MVR and once on its own, is one
//! row and not two.
//!
//! **It reads the scene, and does not act on it.** [`Rig`] carries what the plan
//! says — which fixture, which mode, at which address, where on the stage — and
//! nothing here patches any of it. That is a separate decision with a separate
//! consequence: filling a library adds profiles an operator may ignore, while
//! patching a rig *replaces the show they have open*. The plan entry for S62
//! says that decision is the session's to make and to justify; until it is made,
//! this module hands the information over and stops.
//!
//! # The archive is a ZIP of ZIPs
//!
//! An `.mvr` is a ZIP; a `.gdtf` inside it is another ZIP. [`super::zip`] reads
//! both, because it takes bytes rather than a file, and a member's bytes are
//! bytes. There is no recursion beyond that — an MVR does not nest MVRs — and
//! the members are read once each.

use std::collections::BTreeMap;

use prism_domain::{FixtureType, Vec3};

use super::LibraryEntry;
use super::gdtf::{self, xml::Node};
use super::zip::Archive;

/// The root document every `.mvr` carries.
const SCENE: &str = "GeneralSceneDescription.xml";

/// The extension of a fixture profile inside the archive.
const GDTF_EXTENSION: &str = ".gdtf";

/// Millimetres to metres, for the translation part of a `Matrix`.
///
/// The same convention GDTF states and the same one `library::gdtf::geometry`
/// applies, named again here rather than reached across a module boundary —
/// and carrying the same caveat: it is the one number in either reader that
/// could not be checked against a published archive. `PROGRESS.md` §5 says how
/// to settle it, and settling it settles both.
const MATRIX_TO_METRES: f64 = 0.001;

/// How many fixtures one plan may describe.
///
/// A large touring rig is a few thousand. Ten thousand is past anything a
/// venue this desk is for will open and stops a hand-edited document from
/// filling memory with fixtures nobody will patch.
const MAX_FIXTURES: usize = 10_000;

/// What an `.mvr` cost to read, beside what its profiles cost.
///
/// Counted rather than asserted, for the reason the GDTF conversion counts:
/// an archive this desk reads badly should be **visible** in a number rather
/// than fatal, and the numbers are what a re-import is read by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Conversion {
    /// `.gdtf` members found in the archive.
    pub profiles: u32,
    /// Members that looked like a profile and would not read.
    pub profiles_rejected: u32,
    /// Fixtures the scene document describes.
    pub fixtures: u32,
    /// Fixtures naming a profile that is **not** in the archive.
    ///
    /// Allowed by the format — an exporter may leave out what it expects the
    /// other end to have — and the reason a plan can arrive with fewer
    /// profiles than fixtures.
    pub fixtures_without_profile: u32,
}

impl Conversion {
    /// Folds another archive's counts into these.
    pub fn absorb(&mut self, other: Self) {
        self.profiles += other.profiles;
        self.profiles_rejected += other.profiles_rejected;
        self.fixtures += other.fixtures;
        self.fixtures_without_profile += other.fixtures_without_profile;
    }
}

/// One fixture of a plan, as the scene document states it.
///
/// Read but not acted on — see the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct RigFixture {
    /// What the plan calls it.
    pub name: String,
    /// The profile file it names, as written in the document.
    pub spec: String,
    /// The DMX mode it is patched in.
    pub mode: String,
    /// The number the plan gives it, where it gives one.
    pub fixture_id: Option<u32>,
    /// The library key of the profile this fixture is patched with, where the
    /// archive carried one — **S62**.
    ///
    /// Resolved here rather than by the caller, because only this module knows
    /// which member of the archive produced which profile: the plan names a
    /// **file** (`GDTFSpec`) and a **mode**, and a key is what a patch needs.
    /// `None` is the ordinary case of a plan that names a profile it did not
    /// carry, and such a fixture is not patchable.
    pub type_id: Option<String>,
    /// Its addresses, one per DMX break, in the order stated.
    pub addresses: Vec<RigAddress>,
    /// Where it hangs, in this desk's axes and in metres, where the plan says.
    pub position: Option<Vec3>,
}

/// One DMX break's address, as MVR writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RigAddress {
    /// Which break — 1 unless the fixture has more than one DMX start.
    pub break_number: u16,
    /// The absolute address: `(universe - 1) * 512 + address`, as the format
    /// states it in one number.
    pub absolute: u32,
}

impl RigAddress {
    /// The universe and address a desk would patch, one-based.
    ///
    /// MVR states one absolute number and this is the only place it is taken
    /// apart. An address of nought is not a patch and answers `None`.
    #[must_use]
    pub fn universe_and_address(self) -> Option<(u16, u16)> {
        if self.absolute == 0 {
            return None;
        }
        let zero_based = self.absolute - 1;
        let universe = u16::try_from(zero_based / 512).ok()?.checked_add(1)?;
        let address = u16::try_from(zero_based % 512).ok()?.checked_add(1)?;
        Some((universe, address))
    }
}

/// What a plan says, without a word about what to do with it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rig {
    /// The fixtures, in the order the document lists them.
    pub fixtures: Vec<RigFixture>,
}

/// Reads an `.mvr`: every profile in it, and what its plan says.
///
/// `own` marks the profiles as the venue's, which is what an imported archive
/// always is — it came off the venue's own plan, so it wins its keys against
/// the installed library exactly as a hand-placed file does (B43).
///
/// An archive that is not a ZIP, or that holds no profile, yields nothing and
/// says so in the counts. That is not an error a desk stops on: see the module
/// documentation of [`super::gdtf`].
#[must_use]
pub fn read_archive(
    bytes: &[u8],
    own: bool,
) -> (Vec<(LibraryEntry, FixtureType)>, Conversion, Rig) {
    let mut counts = Conversion::default();
    let Some(archive) = Archive::read(bytes) else {
        return (Vec::new(), counts, Rig::default());
    };

    // The profiles first, so the rig below can be told which of the fixtures
    // it names actually arrived with one.
    let members: Vec<String> = archive
        .names()
        .filter(|name| name.to_ascii_lowercase().ends_with(GDTF_EXTENSION))
        .map(str::to_owned)
        .collect();

    let mut built = Vec::new();
    // Which member produced which profiles, by the member's own file name —
    // what a `GDTFSpec` refers to. One member yields one profile per DMX mode.
    let mut present: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for member in &members {
        counts.profiles += 1;
        let Some(inner) = archive.file(member) else {
            counts.profiles_rejected += 1;
            continue;
        };
        let (profiles, _) = gdtf::read_archive(&inner, own);
        if profiles.is_empty() {
            counts.profiles_rejected += 1;
            continue;
        }
        // Keyed by the member's own file name, because that is the name a
        // `GDTFSpec` refers to — the fixture's identity comes out of the
        // profile, but *which member is which* is a matter of the archive.
        present.insert(
            file_name_of(member).to_ascii_lowercase(),
            profiles
                .iter()
                .map(|(entry, _)| (entry.mode.clone(), entry.id.clone()))
                .collect(),
        );
        built.extend(profiles);
    }

    let rig = archive
        .file(SCENE)
        .and_then(|document| read_scene(&document, &present, &mut counts))
        .unwrap_or_default();

    (built, counts, rig)
}

/// The member's own name, without the directories an exporter may have used.
fn file_name_of(member: &str) -> &str {
    member.rsplit(['/', '\\']).next().unwrap_or(member)
}

/// Reads `GeneralSceneDescription.xml`.
fn read_scene(
    document: &[u8],
    present: &BTreeMap<String, Vec<(String, String)>>,
    counts: &mut Conversion,
) -> Option<Rig> {
    let root = gdtf::xml::parse(document)?;
    let mut fixtures = Vec::new();
    // Every `Fixture` anywhere under the scene: a plan nests them in layers,
    // and a layer may nest groups, and neither nesting means anything to a
    // library. What matters is the set of fixtures, not the tree they sit in.
    collect_fixtures(&root, &mut fixtures);

    let mut rig = Rig::default();
    for node in fixtures.iter().take(MAX_FIXTURES) {
        let spec = node.child_text("GDTFSpec").to_owned();
        let mode = node.child_text("GDTFMode").to_owned();
        counts.fixtures += 1;
        let type_id = present
            .get(&file_name_of(&spec).to_ascii_lowercase())
            .and_then(|modes| resolve_mode(modes, &mode));
        if type_id.is_none() {
            counts.fixtures_without_profile += 1;
        }
        rig.fixtures.push(RigFixture {
            name: node.get("name").to_owned(),
            spec,
            type_id,
            mode,
            fixture_id: node.child_text("FixtureID").parse::<u32>().ok(),
            addresses: read_addresses(node),
            position: position_of(node.child_text("Matrix")),
        });
    }
    Some(rig)
}

/// Which of a profile's modes a plan's `GDTFMode` means.
///
/// The name as written wins. Where it names no mode this desk built — an
/// exporter that wrote the mode differently, or a profile whose modes this desk
/// could not read — a profile with **exactly one** mode answers that one,
/// because a plan naming a single-mode fixture can only mean it. Anything else
/// is `None`: guessing between two modes would patch a footprint nobody chose.
fn resolve_mode(modes: &[(String, String)], wanted: &str) -> Option<String> {
    if let Some((_, id)) = modes.iter().find(|(name, _)| name == wanted) {
        return Some(id.clone());
    }
    if let Some((_, id)) = modes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
    {
        return Some(id.clone());
    }
    match modes {
        [(_, only)] => Some(only.clone()),
        _ => None,
    }
}

/// Every `Fixture` under a node, however deeply a plan nested it.
fn collect_fixtures<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    for child in &node.children {
        if child.name == "Fixture" {
            out.push(child);
        }
        collect_fixtures(child, out);
    }
}

/// A fixture's `Addresses`.
fn read_addresses(node: &Node) -> Vec<RigAddress> {
    let Some(addresses) = node.child("Addresses") else {
        return Vec::new();
    };
    addresses
        .children_named("Address")
        .filter_map(|address| {
            let absolute = address.text.trim().parse::<u32>().ok()?;
            let break_number = address.get("break").trim().parse::<u16>().unwrap_or(1);
            Some(RigAddress {
                break_number,
                absolute,
            })
        })
        .collect()
}

/// Where a fixture hangs, out of MVR's `Matrix`.
///
/// The same brace-delimited shape GDTF uses for a geometry's position, and the
/// same two conventions apply: the translation is the **fourth** group and is
/// stated in **millimetres**, and MVR is Z-up where this desk is Y-up. Both are
/// written down once in [`super::gdtf::geometry`]; this is the second place
/// they are applied, and it says so rather than restating why.
fn position_of(text: &str) -> Option<Vec3> {
    let mut groups = Vec::new();
    for group in text.split('{').skip(1) {
        let body = group.split_once('}')?.0;
        let numbers: Vec<f64> = body
            .split(',')
            .filter_map(|part| part.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .collect();
        if numbers.len() < 3 {
            return None;
        }
        groups.push(numbers);
    }
    let translation = groups.get(3)?;
    Some(Vec3 {
        x: translation[0] * MATRIX_TO_METRES,
        y: translation[2] * MATRIX_TO_METRES,
        z: translation[1] * MATRIX_TO_METRES,
    })
}

#[cfg(test)]
mod tests;
