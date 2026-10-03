//! The geometry a written GDTF carries — **B65**, the second half.
//!
//! A planner that is handed an OFL fixture's GDTF has two questions the
//! channels cannot answer: *what does it look like*, and *which part moves when
//! pan and tilt do*. GDTF answers both with a **geometry tree** — `Base`, an
//! `Axis` for the yoke, an `Axis` for the head, a `Beam` — and a channel names
//! the geometry it turns. A tree of one `Cube` and a `Beam`, with every channel
//! on the cube, answers neither: it is a box that does not move, which is what
//! this module replaced.
//!
//! # Three trees, by what the profile knows
//!
//! - **Described**: the profile carries a geometry tree (it came from a GDTF
//!   file, S30b). It is written back as it was — names, kinds, places, axes and
//!   beams — and every channel keeps the geometry it acted on. The 3D models
//!   are *not* in the profile, so a node that had a model file and no primitive
//!   is written as a `Cube` of its box: the room it took, not its shape.
//! - **Flat**: the profile states beams and no tree (an older embedded one): a
//!   body and those beams.
//! - **Stand-in**: the profile says nothing about a device — every Open Fixture
//!   Library profile and every generic. What the desk's own viewer draws for it
//!   (`ui/src/viewer/gl/primitives.ts`), in GDTF's own primitives and no files:
//!   a **moving head** (`Base`, a `Yoke` axis, a `Head` axis, a `Beam`) when
//!   some mode has pan or tilt, and a `Conventional` can with a beam out of its
//!   foot when none has. Pan is on the yoke and tilt on the head, so a planner
//!   moves the right part; and because the viewer reads the file back as it
//!   reads any GDTF, an exported-and-imported OFL fixture is still the moving
//!   head it was.
//!
//! Sizes and places are the viewer's (a base 0.34 × 0.24 × 0.10 m, a yoke 0.34 ×
//! 0.10 × 0.30, a head 0.24 × 0.24 × 0.34), laid out the way GDTF hangs a head:
//! base at the top, yoke arms down, beam along −Z.

use std::fmt::Write as _;

use prism_domain::{AttributeDef, AttributeType, FixtureBeam, FixturePhysical, FixtureType, Vec3};

use super::{DEFAULT_BEAM_ANGLE, DEFAULT_BODY, escape, number};

/// A model of a part: which primitive it is and how big, in GDTF's axes
/// (length X, width Y, height Z).
#[derive(Debug, Clone)]
struct Model {
    primitive: String,
    size: [f64; 3],
}

/// What a beam says about its light.
#[derive(Debug, Clone)]
struct BeamOut {
    kind: String,
    beam_angle: f64,
    field_angle: f64,
    radius: f64,
    flux: f64,
    temperature: f64,
    ratio: f64,
}

impl BeamOut {
    /// A wash of the usual size, for a beam the profile does not describe.
    fn plain(angle: f64) -> Self {
        Self {
            kind: "Wash".to_owned(),
            beam_angle: angle,
            field_angle: angle,
            radius: 0.025,
            flux: 0.0,
            temperature: 6000.0,
            ratio: 1.0,
        }
    }
}

/// One geometry.
#[derive(Debug, Clone)]
struct Part {
    name: String,
    /// GDTF's element: `Geometry`, `Axis`, `Beam`, …
    element: &'static str,
    parent: Option<usize>,
    model: Option<Model>,
    /// `Position`: three rows of the rotation, each with its translation.
    rows: [[f64; 4]; 3],
    beam: Option<BeamOut>,
}

/// The tree, and how the channels are put on it.
#[derive(Debug)]
pub(super) struct Tree {
    parts: Vec<Part>,
    /// Which kind of tree it is, which decides where a channel goes.
    shape: Shape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Described,
    MovingHead,
    Plain,
}

/// The elements GDTF has a geometry kind for that this writer will name.
const KINDS: [&str; 7] = [
    "Geometry",
    "Axis",
    "Beam",
    "FilterBeam",
    "FilterColor",
    "FilterGobo",
    "FilterShaper",
];

const IDENTITY: [[f64; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

/// A pure translation.
const fn at(x: f64, y: f64, z: f64) -> [[f64; 4]; 3] {
    [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]]
}

/// The geometry for the first mode's profile of a fixture; `moves` is whether
/// any mode of it has pan or tilt.
pub(super) fn tree_of(first: &FixtureType, moves: bool) -> Tree {
    match &first.physical {
        Some(physical) if !physical.geometries.is_empty() => described(physical),
        Some(physical) if !physical.beams.is_empty() => flat(physical),
        _ if moves => moving_head(),
        _ => conventional(),
    }
}

/// The tree the profile carries, written back.
fn described(physical: &FixturePhysical) -> Tree {
    let mut names: Vec<String> = Vec::new();
    let mut parts: Vec<Part> = Vec::new();
    for (index, node) in physical.geometries.iter().enumerate() {
        let mut name = if node.name.trim().is_empty() {
            format!("Geometry {}", index + 1)
        } else {
            node.name.trim().to_owned()
        };
        while names.contains(&name) {
            name = format!("{name} {}", index + 1);
        }
        names.push(name.clone());
        let element = KINDS
            .iter()
            .find(|kind| **kind == node.kind)
            .copied()
            .unwrap_or("Geometry");
        let boxed = node.size.x > 0.0 && node.size.y > 0.0 && node.size.z > 0.0;
        // A model file that is not here is a box of the room it took.
        let primitive = node
            .primitive
            .clone()
            .or_else(|| (node.model.is_some() && boxed).then(|| "Cube".to_owned()));
        let model = primitive.map(|primitive| Model {
            primitive,
            size: [node.size.x, node.size.z, node.size.y],
        });
        // The node's axes in its parent's frame, swapped back into GDTF's. The
        // reader names them as the images of *show's* X, Y and Z
        // (`Matrix::show_axes`): show's Y is GDTF's Z column and show's Z its Y
        // column, so the columns come back in that order.
        let column = |axis: Vec3| [axis.x, axis.z, axis.y];
        let (x, y, z) = (
            column(node.x_axis),
            column(node.z_axis),
            column(node.y_axis),
        );
        let place = [node.position.x, node.position.z, node.position.y];
        let rows = [
            [x[0], y[0], z[0], place[0]],
            [x[1], y[1], z[1], place[1]],
            [x[2], y[2], z[2], place[2]],
        ];
        parts.push(Part {
            name,
            element,
            parent: node.parent.and_then(|parent| usize::try_from(parent).ok()),
            model,
            rows,
            beam: node.beam.as_ref().map(|shape| BeamOut {
                kind: shape.beam_type.clone(),
                beam_angle: shape.beam_angle,
                field_angle: shape.field_angle,
                radius: shape.beam_radius,
                flux: shape.luminous_flux,
                temperature: shape.color_temperature,
                ratio: shape.rectangle_ratio,
            }),
        });
    }
    // A beam is the element that has one, whatever the profile called its kind.
    for part in &mut parts {
        if part.beam.is_some() {
            part.element = "Beam";
        }
    }
    Tree {
        parts,
        shape: Shape::Described,
    }
}

/// A body and the beams a profile states, with no tree.
fn flat(physical: &FixturePhysical) -> Tree {
    let size = Some(physical.size)
        .filter(|size| size.x > 0.0 && size.y > 0.0 && size.z > 0.0)
        .unwrap_or(Vec3 {
            x: DEFAULT_BODY,
            y: DEFAULT_BODY,
            z: DEFAULT_BODY,
        });
    let mut parts = vec![Part {
        name: "Body".to_owned(),
        element: "Geometry",
        parent: None,
        model: Some(Model {
            primitive: "Cube".to_owned(),
            size: [size.x, size.z, size.y],
        }),
        rows: IDENTITY,
        beam: None,
    }];
    let mut names = vec!["Body".to_owned()];
    for (index, beam) in physical.beams.iter().enumerate() {
        // Geometry names are unique in a file, and two pixels may share one.
        let mut name = if beam.name.trim().is_empty() {
            format!("Beam {}", index + 1)
        } else {
            beam.name.trim().to_owned()
        };
        while names.contains(&name) {
            name = format!("{name} {}", index + 1);
        }
        names.push(name.clone());
        parts.push(beam_part(name, 0, beam));
    }
    Tree {
        parts,
        shape: Shape::Plain,
    }
}

/// A beam at a place, turned so that it leaves along its direction.
fn beam_part(name: String, parent: usize, beam: &FixtureBeam) -> Part {
    let angle = if beam.beam_angle > 0.0 {
        beam.beam_angle
    } else {
        DEFAULT_BEAM_ANGLE
    };
    let mut out = BeamOut::plain(angle);
    out.flux = beam.luminous_flux.max(0.0);
    if beam.color_temperature > 0.0 {
        out.temperature = beam.color_temperature;
    }
    Part {
        name,
        element: "Beam",
        parent: Some(parent),
        model: Some(lens()),
        rows: beam_rows(beam),
        beam: Some(out),
    }
}

/// What a lens is drawn as in a planner that draws one.
fn lens() -> Model {
    Model {
        primitive: "Cylinder".to_owned(),
        size: [0.05, 0.05, 0.01],
    }
}

/// A moving head: base, yoke, head, beam — the viewer's own stand-in.
fn moving_head() -> Tree {
    let part = |name: &str, element, parent, primitive: &str, size, rows| Part {
        name: name.to_owned(),
        element,
        parent,
        model: Some(Model {
            primitive: primitive.to_owned(),
            size,
        }),
        rows,
        beam: None,
    };
    let mut beam = part(
        "Beam",
        "Beam",
        Some(2),
        "Cylinder",
        [0.05, 0.05, 0.01],
        at(0.0, 0.0, -0.17),
    );
    beam.beam = Some(BeamOut::plain(DEFAULT_BEAM_ANGLE));
    Tree {
        parts: vec![
            part(
                "Base",
                "Geometry",
                None,
                "Base",
                [0.34, 0.24, 0.10],
                IDENTITY,
            ),
            // Under the base, its arms down, turning about the vertical.
            part(
                "Yoke",
                "Axis",
                Some(0),
                "Yoke",
                [0.34, 0.10, 0.30],
                at(0.0, 0.0, -0.20),
            ),
            // Between the arms, turning about the across axis; the lens at its
            // front, which is GDTF's −Z.
            part(
                "Head",
                "Axis",
                Some(1),
                "Head",
                [0.24, 0.24, 0.34],
                at(0.0, 0.0, -0.07),
            ),
            beam,
        ],
        shape: Shape::MovingHead,
    }
}

/// A can with a beam out of its foot: what has no pan or tilt.
fn conventional() -> Tree {
    let mut beam = Part {
        name: "Beam".to_owned(),
        element: "Beam",
        parent: Some(0),
        model: Some(lens()),
        rows: at(0.0, 0.0, -0.15),
        beam: None,
    };
    beam.beam = Some(BeamOut::plain(DEFAULT_BEAM_ANGLE));
    Tree {
        parts: vec![
            Part {
                name: "Body".to_owned(),
                element: "Geometry",
                parent: None,
                model: Some(Model {
                    primitive: "Conventional".to_owned(),
                    size: [0.22, 0.22, 0.30],
                }),
                rows: IDENTITY,
                beam: None,
            },
            beam,
        ],
        shape: Shape::Plain,
    }
}

impl Tree {
    /// The first root, which is what a mode names.
    pub(super) fn root(&self) -> &str {
        self.parts
            .iter()
            .find(|part| part.parent.is_none())
            .map_or("Body", |part| part.name.as_str())
    }

    /// The geometry a channel acts on.
    pub(super) fn geometry_of(&self, definition: &AttributeDef, first: &FixtureType) -> &str {
        let named = |name: &str| {
            self.parts
                .iter()
                .find(|part| part.name == name)
                .map(|part| part.name.as_str())
        };
        let found = match self.shape {
            Shape::MovingHead => match definition.attribute {
                AttributeType::Pan => named("Yoke"),
                AttributeType::Tilt => named("Head"),
                _ => None,
            },
            Shape::Described => first
                .physical
                .as_ref()
                .and_then(|physical| {
                    physical
                        .channels
                        .iter()
                        .find(|channel| channel.offset == definition.coarse_offset)
                })
                .and_then(|channel| channel.geometry.as_deref())
                .and_then(named),
            Shape::Plain => None,
        };
        found.unwrap_or_else(|| self.root())
    }

    /// `<Models>` and `<Geometries>`.
    pub(super) fn write(&self, xml: &mut String) {
        xml.push_str("    <Models>\n");
        for part in &self.parts {
            if let Some(model) = &part.model {
                let _ = writeln!(
                    xml,
                    "      <Model Name=\"{}\" Length=\"{}\" Width=\"{}\" Height=\"{}\" PrimitiveType=\"{}\"/>",
                    escape(&part.name),
                    number(model.size[0]),
                    number(model.size[1]),
                    number(model.size[2]),
                    escape(&model.primitive),
                );
            }
        }
        xml.push_str("    </Models>\n    <Geometries>\n");
        for (index, part) in self.parts.iter().enumerate() {
            // A parent that is not an earlier node is no parent: a root.
            if part.parent.is_none_or(|parent| parent >= index) {
                self.element(xml, index, 3);
            }
        }
        xml.push_str("    </Geometries>\n");
    }

    fn element(&self, xml: &mut String, index: usize, depth: usize) {
        let part = &self.parts[index];
        let pad = "  ".repeat(depth);
        let mut open = format!("{pad}<{} Name=\"{}\"", part.element, escape(&part.name));
        if part.model.is_some() {
            let _ = write!(open, " Model=\"{}\"", escape(&part.name));
        }
        let _ = write!(open, " Position=\"{}\"", matrix_text(&part.rows));
        if let Some(beam) = &part.beam {
            let _ = write!(
                open,
                " LampType=\"LED\" PowerConsumption=\"0\" LuminousFlux=\"{}\" ColorTemperature=\"{}\" \
                 BeamAngle=\"{}\" FieldAngle=\"{}\" BeamRadius=\"{}\" BeamType=\"{}\" \
                 ColorRenderingIndex=\"100\" RectangleRatio=\"{}\"",
                number(beam.flux),
                number(beam.temperature),
                number(beam.beam_angle),
                number(beam.field_angle),
                number(beam.radius),
                escape(&beam.kind),
                number(if beam.ratio > 0.0 { beam.ratio } else { 1.0 }),
            );
        }
        let children: Vec<usize> = (index + 1..self.parts.len())
            .filter(|child| self.parts[*child].parent == Some(index))
            .collect();
        if children.is_empty() {
            let _ = writeln!(xml, "{open}/>");
            return;
        }
        let _ = writeln!(xml, "{open}>");
        for child in children {
            self.element(xml, child, depth + 1);
        }
        let _ = writeln!(xml, "{pad}</{}>", part.element);
    }
}

/// GDTF's `Position`: four rows of four.
fn matrix_text(rows: &[[f64; 4]; 3]) -> String {
    let row = |row: &[f64; 4]| {
        format!(
            "{{{},{},{},{}}}",
            number(row[0]),
            number(row[1]),
            number(row[2]),
            number(row[3])
        )
    };
    format!(
        "{}{}{}{{0,0,0,1}}",
        row(&rows[0]),
        row(&rows[1]),
        row(&rows[2])
    )
}

/// A beam's `Position`: where it sits, and turned so that it leaves along
/// `direction`.
///
/// GDTF's beam leaves along its geometry's **−Z** and its Z is this desk's
/// height; the layout is the specification's, as `geometry::Matrix` reads it.
fn beam_rows(beam: &FixtureBeam) -> [[f64; 4]; 3] {
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
    [
        [rotation[0][0], rotation[0][1], rotation[0][2], place[0]],
        [rotation[1][0], rotation[1][1], rotation[1][2], place[1]],
        [rotation[2][0], rotation[2][1], rotation[2][2], place[2]],
    ]
}
