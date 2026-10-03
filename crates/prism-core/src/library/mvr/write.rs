//! Writing a rig as an `.mvr` — **B65**.
//!
//! [`super::read_archive`] takes a planner's rig in (S62); this sends the desk's
//! back out: **which fixture, in which mode, at which address, hanging where and
//! facing which way, with the GDTF of every profile it uses** — one archive a
//! planner opens as it opens any other.
//!
//! # The profiles: the manufacturer's own, or one this desk writes
//!
//! A profile the library holds as a published GDTF is **copied as it is** —
//! models, gobo pictures and all — provided it is exactly what the show
//! embedded (`FixtureLibrary::published_archive` checks that, mode by mode: a
//! show patched against an older revision must not be exported with a newer
//! file whose footprint differs from the addresses written beside it).
//!
//! Everything else — a profile from the **Open Fixture Library**, which has no
//! GDTF of its own, a venue's hand-written one, or a GDTF the desk no longer has
//! the file of — is **written** by [`crate::library::gdtf::write`]: its channels,
//! ranges and defaults, and a device (a moving head with its yoke and head axes, or
//! a can) in GDTF's own primitives. **That is
//! the owner's answer to *what happens to the OFL fixtures*: none is dropped,
//! none is left out of the archive, and the report says how many were written
//! rather than published.**
//!
//! # What is on the wire in the matrix
//!
//! The format's `Matrix` is `{u}{v}{w}{o}` — the three axes the fixture's own
//! axes are turned **to**, then its origin — in millimetres, Z up and
//! right-handed (the MVR specification, *Matrix*). This desk is Y up and left
//! handed, so the two meet in one swap of the last two axes (the one constant `SWAP`) and
//! nowhere else, in both directions: [`rotation_of_matrix`] is what the reader
//! applies and [`matrix_of`] what this writes, and the tests hold them to one
//! another over arbitrary rotations.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use prism_domain::{Fixture, FixtureType, Vec3};
use prism_domain::{Orientation, orientation};

use super::MATRIX_TO_METRES;
use crate::library::gdtf::write as gdtf_write;
use crate::library::gdtf::write::escape as escape_attribute;
use crate::library::zip::Writer;

/// What an export did — the counts a notice is made of.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportReport {
    /// Fixtures written into the plan.
    pub fixtures: u32,
    /// `.gdtf` files in the archive.
    pub profiles: u32,
    /// Of them, the manufacturer's own file, copied as it was published.
    pub published: u32,
    /// Of them, **written by this desk** out of the profile the show holds —
    /// every Open Fixture Library profile and every hand-written one.
    pub written: u32,
}

/// A rig as an archive, and what it took.
#[derive(Debug)]
pub struct Exported {
    /// The `.mvr`.
    pub bytes: Vec<u8>,
    /// What went into it.
    pub report: ExportReport,
}

/// The one layer the plan has, and the name a planner shows it under.
const LAYER_NAME: &str = "PrismDMX";

/// The root of every identity this writes — `PRISMDMX` in ASCII, so a planner's
/// user can tell where an object came from.
const UUID_PREFIX: &str = "50524953-4d44-4d58-8000";

/// Writes the plan.
///
/// `fixtures` are the show's patched fixtures with their profiles, in the order
/// the plan lists them. `published` is asked, for each **fixture** of the
/// library (a manufacturer and a name, with every mode the show uses of it),
/// for the manufacturer's own file; `None` means *write one*.
///
/// `None` where the archive would not fit the plain ZIP format, or where there
/// is nothing to write.
#[must_use]
pub fn write_archive<'a>(
    fixtures: impl IntoIterator<Item = (&'a Fixture, &'a FixtureType)>,
    published: &dyn Fn(&[&FixtureType]) -> Option<Vec<u8>>,
) -> Option<Exported> {
    let fixtures: Vec<(&Fixture, &FixtureType)> = fixtures.into_iter().collect();
    if fixtures.is_empty() {
        return None;
    }

    // One `.gdtf` per fixture of the library, holding the modes used.
    let mut devices: BTreeMap<(String, String), Vec<&FixtureType>> = BTreeMap::new();
    for (_, profile) in &fixtures {
        let modes = devices
            .entry((profile.manufacturer.clone(), profile.name.clone()))
            .or_default();
        if !modes.iter().any(|mode| mode.id == profile.id) {
            modes.push(profile);
        }
    }

    let mut report = ExportReport::default();
    let mut archive = Writer::new();
    // Which member each profile key is in, for the scene to name.
    let mut member_of: BTreeMap<&str, String> = BTreeMap::new();
    let mut taken: Vec<String> = Vec::new();
    for modes in devices.values() {
        let (bytes, was_published) = match published(modes) {
            Some(bytes) => (bytes, true),
            None => (gdtf_write::write_archive(modes)?, false),
        };
        let mut name = gdtf_write::file_name(modes[0]);
        // Two fixtures whose names differ only in what a file name cannot say.
        while taken.iter().any(|other| other.eq_ignore_ascii_case(&name)) {
            name = format!(
                "{} ({}).gdtf",
                name.trim_end_matches(".gdtf"),
                taken.len() + 1
            );
        }
        archive.add_stored(&name, &bytes);
        taken.push(name.clone());
        for mode in modes {
            member_of.insert(mode.id.as_str(), name.clone());
        }
        report.profiles += 1;
        if was_published {
            report.published += 1;
        } else {
            report.written += 1;
        }
    }

    let mut scene = String::new();
    scene.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    let _ = writeln!(
        scene,
        "<GeneralSceneDescription verMajor=\"1\" verMinor=\"6\" provider=\"PrismDMX\" \
         providerVersion=\"{}\">",
        env!("CARGO_PKG_VERSION")
    );
    scene.push_str("  <UserData/>\n  <Scene>\n    <Layers>\n");
    let _ = writeln!(
        scene,
        "      <Layer name=\"{LAYER_NAME}\" uuid=\"{UUID_PREFIX}-000000000000\">\n        <ChildList>"
    );
    for (fixture, profile) in &fixtures {
        let file = member_of.get(profile.id.as_str())?;
        let absolute = u32::from(fixture.address.saturating_sub(1))
            + fixture.universe.get().saturating_sub(1) * 512
            + 1;
        let _ = writeln!(
            scene,
            "          <Fixture name=\"{name}\" uuid=\"{UUID_PREFIX}-{id:012X}\">\n\
             \x20           <Matrix>{matrix}</Matrix>\n\
             \x20           <GDTFSpec>{file}</GDTFSpec>\n\
             \x20           <GDTFMode>{mode}</GDTFMode>\n\
             \x20           <Addresses>\n\
             \x20             <Address break=\"0\">{absolute}</Address>\n\
             \x20           </Addresses>\n\
             \x20           <FixtureID>{id_text}</FixtureID>\n\
             \x20           <UnitNumber>0</UnitNumber>\n\
             \x20         </Fixture>",
            name = escape_attribute(&fixture.name),
            id = fixture.id.get(),
            id_text = fixture.id.get(),
            matrix = matrix_of(fixture.position, fixture.rotation),
            file = escape_attribute(file),
            mode = escape_attribute(&profile.mode),
        );
        report.fixtures += 1;
    }
    scene.push_str("        </ChildList>\n      </Layer>\n    </Layers>\n  </Scene>\n</GeneralSceneDescription>\n");

    archive.add_deflated(super::SCENE, scene.as_bytes());
    Some(Exported {
        bytes: archive.finish()?,
        report,
    })
}

/// The desk's axes as MVR's, and the other way: **the last two swapped**.
///
/// Y up and left-handed against Z up and right-handed. The swap is its own
/// inverse, so one table does both directions — for a position, and for the rows
/// and the columns of a rotation alike.
const SWAP: [usize; 3] = [0, 2, 1];

/// A vector in the other frame.
const fn axes(vector: [f64; 3]) -> [f64; 3] {
    [vector[SWAP[0]], vector[SWAP[1]], vector[SWAP[2]]]
}

/// A fixture's `Matrix`, as MVR writes it.
#[must_use]
pub fn matrix_of(position: Vec3, rotation: Vec3) -> String {
    let desk: Orientation = orientation(rotation);
    // The same rotation in MVR's axes: the desk's matrix with its rows and its
    // columns both swapped.
    let swapped = |row: usize, column: usize| desk[SWAP[row]][SWAP[column]];
    let group = |column: usize| {
        format!(
            "{{{},{},{}}}",
            number(swapped(0, column)),
            number(swapped(1, column)),
            number(swapped(2, column)),
        )
    };
    let origin = axes([position.x, position.y, position.z]);
    format!(
        "{}{}{}{{{},{},{}}}",
        // `u`, `v`, `w` are where the fixture's own X, Y and Z are turned to —
        // the **columns** of the rotation.
        group(0),
        group(1),
        group(2),
        number(origin[0] / MATRIX_TO_METRES),
        number(origin[1] / MATRIX_TO_METRES),
        number(origin[2] / MATRIX_TO_METRES),
    )
}

/// The rotation a `Matrix` states, as this desk's three angles — the reader's
/// half of [`matrix_of`].
///
/// `None` unless the text is the three turned axes and an origin. A matrix that
/// does not turn the fixture at all answers [`Vec3::ZERO`].
#[must_use]
pub fn rotation_of_matrix(text: &str) -> Option<Vec3> {
    let groups = groups_of(text)?;
    let axes_of_fixture = groups.get(..3)?;
    // MVR's rotation, columns u, v, w — and this desk's, by the swap.
    let mvr = |row: usize, column: usize| axes_of_fixture[column][row];
    let mut desk: Orientation = [[0.0; 3]; 3];
    for (row, line) in desk.iter_mut().enumerate() {
        for (column, cell) in line.iter_mut().enumerate() {
            *cell = mvr(SWAP[row], SWAP[column]);
        }
    }
    // A planner may state a scale; the rotation is what is left once it is
    // taken out, and a matrix with none to give (a collapsed axis) is no
    // rotation at all.
    for column in 0..3 {
        let length = (0..3)
            .map(|row| desk[row][column] * desk[row][column])
            .sum::<f64>()
            .sqrt();
        if !(length.is_finite() && length > 1e-9) {
            return None;
        }
        for row in &mut desk {
            row[column] /= length;
        }
    }
    Some(prism_domain::rotation_of(&desk))
}

/// The four groups of three numbers of a `Matrix`, or `None` if it is not that.
pub(super) fn groups_of(text: &str) -> Option<Vec<[f64; 3]>> {
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
        groups.push([numbers[0], numbers[1], numbers[2]]);
    }
    (groups.len() >= 4).then_some(groups)
}

/// A number as an attribute value: twelve places, no trailing noise, no `-0`.
///
/// Twelve, because a rotation read back near the vertical divides by a cosine
/// and turns a rounding into an angle. Nine was enough for every fixture hung at
/// a sensible tilt and not for one tipped to `90.0018` degrees, where the cosine
/// is `3e-5` and the ninth place became an error of a thousandth of a degree -
/// found by a proptest seed in S32, recorded in `proptest-regressions`.
fn number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let text = format!("{value:.12}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text {
        "" | "-0" => "0".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests;
