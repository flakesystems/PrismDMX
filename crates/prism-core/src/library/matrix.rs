//! Matrix channel inserts, resolved — **S52**.
//!
//! # What this is for
//!
//! An Open Fixture Library mode does not have to write its channels out. A
//! fixture with a pixel matrix may say *repeat these template channels once per
//! pixel* instead:
//!
//! ```json
//! { "insert": "matrixChannels", "repeatFor": "eachPixelABC",
//!   "channelOrder": "perPixel",
//!   "templateChannels": ["Red $pixelKey", "Green $pixelKey", "Blue $pixelKey"] }
//! ```
//!
//! Until S52 a mode containing one of those was **skipped whole** — the
//! footprint depended on state the reader did not resolve, and a footprint may
//! not depend on state. That was **90 of the 634 installed profiles**, and they
//! could not be patched at all.
//!
//! They can be resolved, and the reason it is this session that does it is that
//! resolving them makes a fixture with **eight reds** — which is what
//! `prism_domain::AttributeKey`'s occurrence is for. Before S52 an expanded
//! matrix would have produced one red and seven counted duplicates, which is a
//! worse answer than skipping the mode.
//!
//! # What it deliberately does not do
//!
//! Everything here is a **substitution**: an insert becomes the flat list of
//! channel names it stands for, and from there the ordinary reader
//! (`super::ofl::Channels`) resolves each name against `templateChannels` and
//! `availableChannels` exactly as it already did. The matrix itself — where a
//! pixel is in space — reaches no further than the order the channels come out
//! in. This desk has no pixel model, and inventing one here would be a model
//! nothing else in the program knows about.
//!
//! **A switching channel is still not resolved.** Those are a footprint that
//! depends on another channel's *value*, which is a different question from one
//! that depends on the fixture's own geometry; `docs/ISSUES.md` carries it.

use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// The placeholder a template channel's name carries.
pub(super) const PIXEL_KEY: &str = "$pixelKey";

/// One pixel of a matrix: what it is called and where it is.
#[derive(Debug, Clone)]
struct Pixel {
    /// The key a template channel's `$pixelKey` is replaced with.
    key: String,
    /// Position, one-based, in the axis order `[x, y, z]`.
    at: [u32; 3],
}

/// A fixture's pixel matrix, as far as a footprint depends on it.
#[derive(Debug, Default)]
pub(super) struct Matrix {
    /// The pixels, in the file's own traversal order.
    pixels: Vec<Pixel>,
    /// Pixel group keys, **in the order the file writes them** — which is what
    /// `eachPixelGroup` means and what a `serde_json::Value` cannot answer,
    /// since its maps are sorted. See [`GroupOrder`].
    groups: Vec<String>,
}

impl Matrix {
    /// The matrix a fixture declares, or an empty one when it declares none.
    ///
    /// `source` is the file's own text and not only the parsed value, because
    /// the order of `pixelGroups` is a fact about the *file* — see
    /// [`Matrix::groups`].
    pub(super) fn of(fixture: &serde_json::Map<String, Value>, source: &str) -> Self {
        let Some(matrix) = fixture.get("matrix") else {
            return Self::default();
        };
        Self {
            pixels: pixels_of(matrix),
            groups: serde_json::from_str::<MatrixOrder>(source)
                .ok()
                .map(|order| order.matrix.pixel_groups.0)
                .unwrap_or_default(),
        }
    }

    /// One mode's channel list with every matrix insert written out.
    ///
    /// `None` when an insert names something this reader cannot resolve — an
    /// unknown `repeatFor`, a `channelOrder` the format has grown since this
    /// was written, a matrix the fixture does not declare. The caller counts
    /// that and skips the mode, which is what it did with **every** insert
    /// before S52: a footprint guessed at is worse than a mode left out.
    pub(super) fn expand(&self, entries: &[Value]) -> Option<Vec<Value>> {
        let mut out = Vec::new();
        for entry in entries {
            if entry.is_string() || entry.is_null() {
                out.push(entry.clone());
                continue;
            }
            let object = entry.as_object()?;
            if object.get("insert").and_then(Value::as_str) != Some("matrixChannels") {
                return None;
            }
            let repeat = self.repeat_for(object.get("repeatFor")?)?;
            let templates: Vec<&str> = object
                .get("templateChannels")?
                .as_array()?
                .iter()
                .map(|name| name.as_str())
                .collect::<Option<_>>()?;
            // `perPixel` walks the templates inside each pixel; `perChannel`
            // walks the pixels inside each template. The whole installed
            // library writes `perPixel`, and the other is here because the
            // format defines it — not resolving it would be the same silent
            // guess this module exists to stop.
            match object.get("channelOrder").and_then(Value::as_str)? {
                "perPixel" => {
                    for key in &repeat {
                        for template in &templates {
                            out.push(Value::String(template.replace(PIXEL_KEY, key)));
                        }
                    }
                }
                "perChannel" => {
                    for template in &templates {
                        for key in &repeat {
                            out.push(Value::String(template.replace(PIXEL_KEY, key)));
                        }
                    }
                }
                _ => return None,
            }
        }
        Some(out)
    }

    /// The pixel or pixel-group keys one insert repeats over, in order.
    ///
    /// The four keyword forms the format defines, and an explicit array — which
    /// the installed library uses 480 times, and whose entries may name either
    /// a pixel or a **pixel group**. Both are only a name to substitute, so
    /// they need not be told apart.
    fn repeat_for(&self, repeat: &Value) -> Option<Vec<String>> {
        if let Some(list) = repeat.as_array() {
            return list
                .iter()
                .map(|name| name.as_str().map(ToOwned::to_owned))
                .collect();
        }
        match repeat.as_str()? {
            // "Gets computed into an alphanumerically sorted list of all
            // pixelKeys" — the format's own words, and `alphanumerically` is
            // why this is not a plain string sort: pixel `10` comes after pixel
            // `9` on a tube, and lexically it does not.
            "eachPixelABC" => {
                let mut keys: Vec<String> =
                    self.pixels.iter().map(|pixel| pixel.key.clone()).collect();
                keys.sort_by(|left, right| alphanumeric(left, right));
                Some(keys)
            }
            // "ordered by appearance in the JSON file".
            "eachPixelGroup" => Some(self.groups.clone()),
            other => {
                let axes = axis_order(other.strip_prefix("eachPixel")?)?;
                let mut pixels = self.pixels.clone();
                // The **last** named axis is the outermost: `XYZ` reads the
                // matrix like a book — left to right first (`X`, letter by
                // letter), then top to bottom, then front to back.
                pixels.sort_by_key(|pixel| {
                    (
                        pixel.at[axes[2] as usize],
                        pixel.at[axes[1] as usize],
                        pixel.at[axes[0] as usize],
                    )
                });
                Some(pixels.into_iter().map(|pixel| pixel.key).collect())
            }
        }
    }
}

/// The axis indices an `XYZ`-style suffix names, or `None` for anything else.
///
/// Each of the three letters exactly once; a suffix that repeats one or brings
/// a fourth is not a permutation and is refused rather than half-read.
fn axis_order(suffix: &str) -> Option<[u8; 3]> {
    let mut axes = [0u8; 3];
    let mut seen = [false; 3];
    if suffix.chars().count() != 3 {
        return None;
    }
    for (slot, letter) in suffix.chars().enumerate() {
        let axis = match letter {
            'X' => 0,
            'Y' => 1,
            'Z' => 2,
            _ => return None,
        };
        if core::mem::replace(&mut seen[axis], true) {
            return None;
        }
        axes[slot] = u8::try_from(axis).ok()?;
    }
    Some(axes)
}

/// The pixels a `matrix` object declares, in the file's own traversal order.
///
/// Two shapes, and the format allows exactly these: `pixelKeys`, a `Z` of `Y`
/// of `X` array with a name or `null` in each cell, and `pixelCount`, three
/// counts from which the names are **generated**. A cell that is `null` is a
/// hole in the frame and is not a pixel — that is what it is written for.
fn pixels_of(matrix: &Value) -> Vec<Pixel> {
    if let Some(planes) = matrix.get("pixelKeys").and_then(Value::as_array) {
        let mut pixels = Vec::new();
        for (z, plane) in planes.iter().enumerate() {
            for (y, row) in plane.as_array().into_iter().flatten().enumerate() {
                for (x, cell) in row.as_array().into_iter().flatten().enumerate() {
                    if let Some(key) = cell.as_str() {
                        pixels.push(Pixel {
                            key: key.to_owned(),
                            at: one_based(x, y, z),
                        });
                    }
                }
            }
        }
        return pixels;
    }
    let Some(counts) = matrix.get("pixelCount").and_then(Value::as_array) else {
        return Vec::new();
    };
    let count = |index: usize| -> usize {
        counts
            .get(index)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0)
    };
    let (width, height, depth) = (count(0), count(1), count(2));
    // Which axes the fixture actually has more than one of. The generated name
    // depends on it, and that is the format's rule rather than this reader's —
    // see `default_key`.
    let defined: Vec<usize> = [width, height, depth]
        .into_iter()
        .enumerate()
        .filter_map(|(axis, size)| (size > 1).then_some(axis))
        .collect();
    let mut pixels = Vec::new();
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let at = one_based(x, y, z);
                pixels.push(Pixel {
                    key: default_key(at, &defined),
                    at,
                });
            }
        }
    }
    pixels
}

/// A position as the format counts it — from one, in `[x, y, z]` order.
fn one_based(x: usize, y: usize, z: usize) -> [u32; 3] {
    [
        u32::try_from(x).unwrap_or(u32::MAX).saturating_add(1),
        u32::try_from(y).unwrap_or(u32::MAX).saturating_add(1),
        u32::try_from(z).unwrap_or(u32::MAX).saturating_add(1),
    ]
}

/// The name a pixel gets when the fixture declared only a `pixelCount`.
///
/// **The Open Fixture Library's own rule** (`lib/model/Matrix.js`,
/// `_getPixelDefaultKey`), and not a scheme invented here — a mode's explicit
/// `repeatFor` array names pixels by these strings, so a different rule would
/// mean an insert repeating over pixels that do not exist:
///
/// | Axes with more than one pixel | Name |
/// |---|---|
/// | one | the coordinate along it, e.g. `"7"` |
/// | two | `"(x, y)"` — the two defined coordinates |
/// | three | `"(x, y, z)"` |
///
/// A one-by-one-by-one matrix defines no axis at all; it is named like the
/// one-axis case, which is `"1"`.
fn default_key(at: [u32; 3], defined: &[usize]) -> String {
    match defined {
        [] => "1".to_owned(),
        [only] => at[*only].to_string(),
        [first, last] => format!("({}, {})", at[*first], at[*last]),
        _ => format!("({}, {}, {})", at[0], at[1], at[2]),
    }
}

/// Compare two keys the way the format's *alphanumeric* sort does.
///
/// Runs of digits compare as numbers and everything else as text, so `2` comes
/// before `10` — which a plain string sort gets wrong on every tube with more
/// than nine pixels, and which is the whole reason the format names the sort
/// rather than saying *sorted*.
fn alphanumeric(left: &str, right: &str) -> core::cmp::Ordering {
    let mut left = left.chars().peekable();
    let mut right = right.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return core::cmp::Ordering::Equal,
            (None, Some(_)) => return core::cmp::Ordering::Less,
            (Some(_), None) => return core::cmp::Ordering::Greater,
            (Some(a), Some(b)) => {
                if a.is_ascii_digit() && b.is_ascii_digit() {
                    let order = number(&mut left).cmp(&number(&mut right));
                    if order != core::cmp::Ordering::Equal {
                        return order;
                    }
                } else {
                    if a != b {
                        return a.cmp(&b);
                    }
                    left.next();
                    right.next();
                }
            }
        }
    }
}

/// The run of digits at the head, as a number. Saturates rather than wrapping.
fn number(chars: &mut core::iter::Peekable<core::str::Chars<'_>>) -> u128 {
    let mut value: u128 = 0;
    while let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
        value = value.saturating_mul(10).saturating_add(u128::from(digit));
        chars.next();
    }
    value
}

/* -------------------------------------------------------------------------- */
/* The order of `pixelGroups`, which a parsed value cannot answer              */
/* -------------------------------------------------------------------------- */

/// Just enough of a fixture file to read the **order** of its pixel groups.
///
/// `serde_json::Map` is a `BTreeMap` in this build, so an object's keys come
/// back sorted and the file's own order is gone. `eachPixelGroup` is defined as
/// *ordered by appearance in the JSON file*, so it is read here — off the
/// stream, where the order still exists — rather than guessed at from the
/// sorted names.
#[derive(Deserialize, Default)]
struct MatrixOrder {
    #[serde(default)]
    matrix: MatrixGroups,
}

/// The `pixelGroups` member of a `matrix`, as an order.
#[derive(Deserialize, Default)]
struct MatrixGroups {
    #[serde(rename = "pixelGroups", default)]
    pixel_groups: GroupOrder,
}

/// An object's keys, in the order they were written.
#[derive(Default)]
struct GroupOrder(Vec<String>);

impl<'de> Deserialize<'de> for GroupOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Keys;
        impl<'de> Visitor<'de> for Keys {
            type Value = GroupOrder;

            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("an object of pixel groups")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<GroupOrder, M::Error> {
                let mut keys = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    map.next_value::<IgnoredAny>()?;
                    keys.push(key);
                }
                Ok(GroupOrder(keys))
            }
        }
        deserializer.deserialize_map(Keys)
    }
}

#[cfg(test)]
mod tests {
    use super::{Matrix, alphanumeric, axis_order, default_key};
    use serde_json::{Value, json};

    /// The two shapes a matrix comes in, and the names the format gives them.
    #[test]
    fn a_pixel_count_is_named_by_the_librarys_own_rule() {
        // One axis is the number on its own; that is every LED tube in the
        // library and the case an explicit `repeatFor` array names.
        assert_eq!(default_key([7, 1, 1], &[0]), "7");
        assert_eq!(default_key([1, 3, 1], &[1]), "3");
        assert_eq!(default_key([2, 3, 1], &[0, 1]), "(2, 3)");
        assert_eq!(default_key([2, 3, 4], &[0, 1, 2]), "(2, 3, 4)");
        // A one-by-one-by-one matrix defines no axis; it is still one pixel.
        assert_eq!(default_key([1, 1, 1], &[]), "1");
    }

    /// `eachPixelABC` sorts **alphanumerically**, so pixel 10 follows pixel 9.
    #[test]
    fn ten_comes_after_nine() {
        let mut keys = vec!["10", "2", "1", "9"];
        keys.sort_by(|left, right| alphanumeric(left, right));
        assert_eq!(keys, vec!["1", "2", "9", "10"]);
        // And text still compares as text.
        let mut mixed = vec!["Bottom", "Top", "Center"];
        mixed.sort_by(|left, right| alphanumeric(left, right));
        assert_eq!(mixed, vec!["Bottom", "Center", "Top"]);
    }

    /// `XYZ` reads the matrix like a book: X first, then Y, then Z.
    #[test]
    fn the_first_named_axis_is_the_one_that_varies_fastest() {
        let fixture = json!({
            "matrix": { "pixelCount": [2, 2, 1] }
        });
        let matrix = Matrix::of(fixture.as_object().unwrap(), &fixture.to_string());
        let order = |name: &str| {
            matrix
                .repeat_for(&Value::String(name.to_owned()))
                .expect("a named order")
        };
        assert_eq!(
            order("eachPixelXYZ"),
            vec!["(1, 1)", "(2, 1)", "(1, 2)", "(2, 2)"]
        );
        // `YXZ` turns the page ninety degrees: down a column, then across.
        assert_eq!(
            order("eachPixelYXZ"),
            vec!["(1, 1)", "(1, 2)", "(2, 1)", "(2, 2)"]
        );
        // Anything that is not a permutation of the three axes is refused
        // rather than half-read.
        assert!(axis_order("XXY").is_none());
        assert!(axis_order("XY").is_none());
        assert!(
            matrix
                .repeat_for(&Value::String("eachPixelWat".into()))
                .is_none()
        );
    }

    /// A `null` in `pixelKeys` is a hole in the frame, not a pixel.
    #[test]
    fn a_hole_in_the_frame_is_not_a_pixel() {
        let fixture = json!({
            "matrix": {
                "pixelKeys": [[[null, "Top", null], ["Left", "Centre", "Right"]]]
            }
        });
        let matrix = Matrix::of(fixture.as_object().unwrap(), &fixture.to_string());
        assert_eq!(
            matrix
                .repeat_for(&Value::String("eachPixelABC".into()))
                .unwrap(),
            vec!["Centre", "Left", "Right", "Top"]
        );
    }

    /// The insert becomes the channels it stands for, in `perPixel` order.
    #[test]
    fn an_insert_becomes_the_channels_it_stands_for() {
        let fixture = json!({
            "matrix": { "pixelCount": [3, 1, 1] }
        });
        let matrix = Matrix::of(fixture.as_object().unwrap(), &fixture.to_string());
        let mode = json!([
            "Master",
            {
                "insert": "matrixChannels",
                "repeatFor": "eachPixelABC",
                "channelOrder": "perPixel",
                "templateChannels": ["Red $pixelKey", "Green $pixelKey"]
            }
        ]);
        let expanded = matrix.expand(mode.as_array().unwrap()).unwrap();
        assert_eq!(
            expanded
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "Master", "Red 1", "Green 1", "Red 2", "Green 2", "Red 3", "Green 3"
            ]
        );
        // `perChannel` walks the pixels inside each template instead.
        let per_channel = json!([{
            "insert": "matrixChannels",
            "repeatFor": "eachPixelABC",
            "channelOrder": "perChannel",
            "templateChannels": ["Red $pixelKey", "Green $pixelKey"]
        }]);
        assert_eq!(
            matrix
                .expand(per_channel.as_array().unwrap())
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["Red 1", "Red 2", "Red 3", "Green 1", "Green 2", "Green 3"]
        );
    }

    /// `eachPixelGroup` is the file's order, which the sorted map has lost.
    ///
    /// The whole reason [`super::GroupOrder`] exists: `serde_json::Map` is a
    /// `BTreeMap` here, so `Zone`, `Middle`, `Outer` would come back
    /// alphabetically and an insert would repeat over the wrong pixels.
    #[test]
    fn a_pixel_group_keeps_the_order_the_file_wrote_it_in() {
        let source = r#"{
            "matrix": {
                "pixelCount": [4, 1, 1],
                "pixelGroups": { "Outer": "all", "Middle": "all", "Centre": "all" }
            }
        }"#;
        let fixture: Value = serde_json::from_str(source).unwrap();
        let matrix = Matrix::of(fixture.as_object().unwrap(), source);
        assert_eq!(
            matrix
                .repeat_for(&Value::String("eachPixelGroup".into()))
                .unwrap(),
            vec!["Outer", "Middle", "Centre"],
            "the sorted map would have said Centre, Middle, Outer"
        );
    }

    /// An insert this reader cannot resolve is refused, never guessed at.
    #[test]
    fn an_insert_it_cannot_read_is_refused() {
        let fixture = json!({ "matrix": { "pixelCount": [2, 1, 1] } });
        let matrix = Matrix::of(fixture.as_object().unwrap(), &fixture.to_string());
        // A `channelOrder` the format grows later.
        let unknown_order = json!([{
            "insert": "matrixChannels",
            "repeatFor": "eachPixelABC",
            "channelOrder": "perUniverse",
            "templateChannels": ["Red $pixelKey"]
        }]);
        assert!(matrix.expand(unknown_order.as_array().unwrap()).is_none());
        // And an entry that is an object but not an insert at all.
        let not_an_insert = json!([{ "switch": "Programs" }]);
        assert!(matrix.expand(not_an_insert.as_array().unwrap()).is_none());
    }

    /// A fixture with no matrix has none, and says so.
    #[test]
    fn a_fixture_without_a_matrix_has_none() {
        let fixture = json!({ "name": "A PAR" });
        let matrix = Matrix::of(fixture.as_object().unwrap(), &fixture.to_string());
        // A plain channel list still passes straight through it, so a caller
        // need not ask whether there is a matrix before expanding.
        let mode = json!(["Dimmer", null, "Red"]);
        assert_eq!(
            matrix.expand(mode.as_array().unwrap()).unwrap(),
            mode.as_array().unwrap().clone()
        );
    }
}
