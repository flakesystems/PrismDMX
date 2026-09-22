//! What a device is made of and where its light leaves it — **S61**, rebuilt
//! in **S30b** once a published file had been read.
//!
//! # What GDTF states and this reads
//!
//! A fixture's `<Geometries>` is a tree: a base, a yoke that turns on it, a head
//! that tilts in the yoke, a `<Beam>` in the head. Every node carries a
//! `Position` — a transform relative to its parent — and a `Model`, which names
//! the 3D file it is drawn with, the primitive to draw when there is no file,
//! and how big it is.
//!
//! This reads the tree **whole**, as [`prism_domain::GeometryNode`]s with each
//! node's transform relative to its parent — which is what a viewer turns a
//! yoke by — and also flattened into [`prism_domain::FixtureBeam`]s at their
//! home positions, which is what S61's readers and the 2D fallback use.
//!
//! # The matrix, as the specification states it — and S61 did not
//!
//! The value-type table of `gdtf-spec.md` says of `Matrix`: *stored in a
//! row-major order … the mathematical definition of the matrix is in a
//! column-major order … the translation is stored in the 4th column*. So a
//! published file writes
//!
//! ```text
//! {r00,r01,r02,tx}{r10,r11,r12,ty}{r20,r21,r22,tz}{0,0,0,1}
//! ```
//!
//! — the groups are the matrix's **rows**, the rotation's columns are the
//! turned axes, and the translation is the last number of each of the first
//! three rows, **in metres**. A Robe Robin T1 Profile, read on 2026-09-21,
//! states its yoke at `-0.074`, its head at `-0.335` and its lens at
//! `-0.291636` below its parent — a 55 cm head, in metres. S61 read the fourth
//! *group* as the translation and multiplied it by a thousandth, and with no
//! published file to hand every test agreed with it, because the same
//! assumption wrote them. Every node of every real fixture sat at its root.
//!
//! # Which way the world is
//!
//! GDTF is Z-up: X to the right, Y away from the operator, Z up.
//! [`prism_domain::Vec3`] is Y-up with `z` upstage (`prism_domain::placement`).
//! [`to_show_axes`] swaps the last two, and a rotation `R` becomes `P R P` —
//! which, written as axes, is [`Matrix::show_axes`].

use prism_domain::{BeamShape, FixtureBeam, FixturePhysical, GeometryNode, Vec3};

use super::xml::Node;
use super::{non_empty, number};

/// How deep a geometry tree may be walked.
///
/// A moving head is four deep. This stops a file whose geometry references form
/// a loop — `Pixel` referring to `Bar` referring to `Pixel` — from walking for
/// ever, which no published file does and a hand-edited one can.
const MAX_DEPTH: usize = 12;

/// The most beams one device may contribute.
///
/// An LED bar or a matrix blinder is a few hundred pixels, each one a beam. A
/// thousand is past anything published and stops a file whose geometry
/// references multiply from filling memory with beams nobody will draw.
const MAX_BEAMS: usize = 1_000;

/// The most geometry nodes one device may contribute, for the same reason.
const MAX_NODES: usize = 4_000;

/// A transform, as GDTF writes one: a 3×3 rotation by **rows** and a
/// translation in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Matrix {
    /// `rows[row][column]`.
    rows: [[f64; 3]; 3],
    /// Where the origin moved to, in metres.
    origin: [f64; 3],
}

impl Matrix {
    /// The transform that changes nothing.
    const IDENTITY: Self = Self {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        origin: [0.0, 0.0, 0.0],
    };

    /// `self · child` — the child's transform expressed in the parent's frame.
    fn then(&self, child: &Self) -> Self {
        let mut rows = [[0.0; 3]; 3];
        for (row, out) in rows.iter_mut().enumerate() {
            for (column, cell) in out.iter_mut().enumerate() {
                *cell = (0..3)
                    .map(|inner| self.rows[row][inner] * child.rows[inner][column])
                    .sum();
            }
        }
        let mut origin = self.origin;
        for (row, cell) in origin.iter_mut().enumerate() {
            *cell += (0..3)
                .map(|inner| self.rows[row][inner] * child.origin[inner])
                .sum::<f64>();
        }
        Self { rows, origin }
    }

    /// The matrix a `Position` attribute states — see the module documentation
    /// for the layout.
    ///
    /// Three groups of at least four numbers are needed: the fourth row is
    /// always `{0,0,0,1}` for a fixture's geometry and is not read. Anything
    /// that is not that — an attribute that is absent, a group short, a number
    /// that will not parse — is the identity, because a geometry whose position
    /// could not be read sits where its parent does rather than at infinity.
    fn parse(value: &str) -> Self {
        let mut groups = Vec::new();
        for group in value.split('{').skip(1) {
            let Some((body, _)) = group.split_once('}') else {
                continue;
            };
            let numbers: Vec<f64> = body.split(',').filter_map(number).collect();
            groups.push(numbers);
        }
        if groups.len() < 3 || groups[..3].iter().any(|group| group.len() < 4) {
            return Self::IDENTITY;
        }
        let mut rows = [[0.0; 3]; 3];
        let mut origin = [0.0; 3];
        for (index, out) in rows.iter_mut().enumerate() {
            out.copy_from_slice(&groups[index][..3]);
            origin[index] = groups[index][3];
        }
        Self { rows, origin }
    }

    /// Column `index` of the rotation: where the local axis points.
    fn column(&self, index: usize) -> [f64; 3] {
        [
            self.rows[0][index],
            self.rows[1][index],
            self.rows[2][index],
        ]
    }

    /// Where this transform's **−Z** points, as a unit vector.
    ///
    /// Straight down when the matrix does not have a length to normalise —
    /// which is where a hanging light points, and never the zero vector that a
    /// viewer would have to special-case.
    fn beam_direction(&self) -> [f64; 3] {
        let z = self.column(2);
        let length = (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt();
        if !length.is_finite() || length <= f64::EPSILON {
            return [0.0, 0.0, -1.0];
        }
        [-z[0] / length, -z[1] / length, -z[2] / length]
    }

    /// The rotation in show axes, as the images of show's X, Y and Z.
    ///
    /// Show's Y is GDTF's Z and show's Z is GDTF's Y, so show's Y axis is
    /// where GDTF's Z column points, converted — `P R P`, column by column.
    fn show_axes(&self) -> (Vec3, Vec3, Vec3) {
        (
            to_show_axes(self.column(0)),
            to_show_axes(self.column(2)),
            to_show_axes(self.column(1)),
        )
    }
}

/// GDTF's axes, as this desk's.
///
/// GDTF is Z-up and [`Vec3`] is Y-up. See the module documentation: this is the
/// only place the two meet.
fn to_show_axes(vector: [f64; 3]) -> Vec3 {
    Vec3 {
        x: vector[0],
        y: vector[2],
        z: vector[1],
    }
}

/// A fixture's models and geometries, ready to be asked about a mode.
#[derive(Debug, Default)]
pub struct Geometries {
    /// Every `Model`, by name.
    models: Vec<(String, Model)>,
    /// The top-level geometries, in the order the file lists them.
    roots: Vec<Node>,
}

/// One `Model`: the file it is drawn from, its primitive and its box.
#[derive(Debug, Clone, Default)]
struct Model {
    /// The file GDTF names, without a directory or an extension — the format
    /// ships the same model in several formats under one name.
    file: Option<String>,
    /// `PrimitiveType`, where it is not `Undefined`.
    primitive: Option<String>,
    /// Length (X), width (Y) and height (Z), in metres, as GDTF states them.
    size: [f64; 3],
}

/// What a walk is building.
#[derive(Default)]
struct Walk {
    nodes: Vec<GeometryNode>,
    beams: Vec<FixtureBeam>,
}

impl Geometries {
    /// Reads a fixture's `Models` and `Geometries`.
    #[must_use]
    pub fn of(fixture: &Node) -> Self {
        let mut models = Vec::new();
        if let Some(node) = fixture.child("Models") {
            for model in node.children_named("Model") {
                let name = model.get("Name").to_owned();
                if name.is_empty() {
                    continue;
                }
                let primitive = non_empty(model.get("PrimitiveType"))
                    .filter(|kind| !kind.eq_ignore_ascii_case("Undefined"))
                    .map(str::to_owned);
                models.push((
                    name,
                    Model {
                        file: non_empty(model.get("File")).map(str::to_owned),
                        primitive,
                        size: [
                            number(model.get("Length")).unwrap_or(0.0),
                            number(model.get("Width")).unwrap_or(0.0),
                            number(model.get("Height")).unwrap_or(0.0),
                        ],
                    },
                ));
            }
        }
        let roots = fixture
            .child("Geometries")
            .map(|node| node.children.clone())
            .unwrap_or_default();
        Self { models, roots }
    }

    /// What one DMX mode's geometry says the device is.
    ///
    /// `geometry` is the mode's own `Geometry` attribute — GDTF lets a file
    /// describe several devices and a mode names which of them it drives. A
    /// mode naming none, or naming one that is not there, gets the whole tree:
    /// a file with one device in it is the ordinary case and naming its root is
    /// optional.
    #[must_use]
    pub fn physical(&self, geometry: &str, fixture_type_id: &str) -> FixturePhysical {
        let named = non_empty(geometry).and_then(|name| {
            self.roots
                .iter()
                .find(|root| root.get("Name") == name)
                .map(std::slice::from_ref)
        });
        let roots: &[Node] = named.unwrap_or(&self.roots);
        if roots.is_empty() {
            return super::empty_physical(fixture_type_id);
        }

        let mut walk = Walk::default();
        for root in roots {
            self.walk(root, None, Matrix::IDENTITY, 0, &mut walk);
        }
        // The body is the first root's model: the base of a moving head, the
        // box of a PAR. The whole device is `geometries`; this stays for the
        // readers that only want one box.
        let body = roots.first().and_then(|root| self.model_of(root));
        FixturePhysical {
            fixture_type_id: fixture_type_id.trim().to_owned(),
            size: body
                .as_ref()
                .map_or(Vec3::ZERO, |model| to_show_axes(model.size)),
            model: body.and_then(|model| model.file),
            beams: walk.beams,
            geometries: walk.nodes,
            channels: Vec::new(),
            wheels: Vec::new(),
        }
    }

    /// The model one geometry node names, if the file defines it.
    fn model_of(&self, node: &Node) -> Option<Model> {
        let name = non_empty(node.get("Model"))?;
        self.models
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, model)| model.clone())
    }

    /// Walks one geometry and everything under it.
    ///
    /// `parent` is the index of the node this one hangs from and `world` the
    /// parent's transform from the device's origin. A `GeometryReference` is
    /// followed — it is how an LED bar states *and here are the same eight
    /// pixels again* — as a node of its own at the reference's position with
    /// the referenced geometry's children under it, and [`MAX_DEPTH`] is what
    /// stops a file whose references point at each other.
    fn walk(&self, node: &Node, parent: Option<u32>, world: Matrix, depth: usize, walk: &mut Walk) {
        if depth >= MAX_DEPTH || walk.nodes.len() >= MAX_NODES {
            return;
        }
        let local = Matrix::parse(node.get("Position"));
        let here = world.then(&local);

        let reference = (node.name == "GeometryReference")
            .then(|| {
                non_empty(node.get("Geometry"))
                    .and_then(|name| self.roots.iter().find(|root| root.get("Name") == name))
            })
            .flatten();
        // A reference is drawn as what it refers to: its model and its kind.
        let described = reference.unwrap_or(node);
        let model = self.model_of(described);
        let (x_axis, y_axis, z_axis) = local.show_axes();
        let beam = (described.name == "Beam").then(|| BeamShape {
            beam_type: non_empty(described.get("BeamType"))
                .unwrap_or("Wash")
                .to_owned(),
            beam_angle: number(described.get("BeamAngle")).unwrap_or(0.0),
            field_angle: number(described.get("FieldAngle")).unwrap_or(0.0),
            beam_radius: number(described.get("BeamRadius")).unwrap_or(0.0),
            luminous_flux: number(described.get("LuminousFlux")).unwrap_or(0.0),
            color_temperature: number(described.get("ColorTemperature")).unwrap_or(0.0),
            rectangle_ratio: number(described.get("RectangleRatio")).unwrap_or(1.0),
        });
        let index = u32::try_from(walk.nodes.len()).unwrap_or(u32::MAX);
        walk.nodes.push(GeometryNode {
            name: non_empty(node.get("Name"))
                .map_or_else(|| format!("Geometry {}", index + 1), str::to_owned),
            parent,
            kind: if reference.is_some() {
                described.name.clone()
            } else {
                node.name.clone()
            },
            model: model.as_ref().and_then(|model| model.file.clone()),
            primitive: model.as_ref().and_then(|model| model.primitive.clone()),
            size: model
                .as_ref()
                .map_or(Vec3::ZERO, |model| to_show_axes(model.size)),
            position: to_show_axes(local.origin),
            x_axis,
            y_axis,
            z_axis,
            beam: beam.clone(),
        });

        if let Some(shape) = beam
            && walk.beams.len() < MAX_BEAMS
        {
            walk.beams.push(FixtureBeam {
                name: non_empty(node.get("Name"))
                    .map_or_else(|| format!("Beam {}", walk.beams.len() + 1), str::to_owned),
                position: to_show_axes(here.origin),
                direction: to_show_axes(here.beam_direction()),
                beam_angle: shape.beam_angle,
                luminous_flux: shape.luminous_flux,
                color_temperature: shape.color_temperature,
            });
        }

        // A reference's own children are its DMX break overrides and not
        // geometry, so the target's children are walked instead — from this
        // node, which is what puts the eighth pixel of a bar where it is.
        let children = reference.map_or(&node.children, |target| &target.children);
        for child in children {
            self.walk(child, Some(index), here, depth + 1, walk);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Geometries, Matrix, to_show_axes};
    use crate::library::gdtf::xml;

    /// The identity, as GDTF writes it.
    const IDENTITY: &str = "{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}";

    fn fixture(body: &str) -> xml::Node {
        let source = format!("<GDTF><FixtureType>{body}</FixtureType></GDTF>");
        xml::parse(source.as_bytes())
            .expect("the test document is XML")
            .child("FixtureType")
            .expect("it has a fixture")
            .clone()
    }

    fn close(value: f64, expected: f64) -> bool {
        (value - expected).abs() < 1e-9
    }

    /// **The layout a published file uses** — a Robe Robin T1 Profile's head,
    /// verbatim: the translation is the fourth number of the third row, in
    /// metres. S61 read the fourth group and would have put it at nought.
    #[test]
    fn a_position_is_read_as_the_specification_states_it() {
        let matrix = Matrix::parse(
            "{1.000000,0.000000,0.000000,0.000000}{0.000000,1.000000,0.000000,0.000000}\
             {0.000000,0.000000,1.000000,-0.335000}{0,0,0,1}",
        );
        assert!(close(matrix.origin[2], -0.335), "{matrix:?}");
        let shown = to_show_axes(matrix.origin);
        assert!(close(shown.y, -0.335), "GDTF's Z is this desk's height");
        assert!(close(shown.x, 0.0));
        assert!(close(shown.z, 0.0));
    }

    #[test]
    fn a_position_that_will_not_parse_is_the_identity() {
        assert_eq!(Matrix::parse(""), Matrix::IDENTITY);
        assert_eq!(Matrix::parse("{1,0,0,0}"), Matrix::IDENTITY);
        assert_eq!(Matrix::parse("nonsense"), Matrix::IDENTITY);
        assert_eq!(
            Matrix::parse("{1,0,0}{0,1,0}{0,0,1}{0,0,0}"),
            Matrix::IDENTITY
        );
        assert_eq!(
            Matrix::parse("{a,b,c,d}{e,f,g,h}{i,j,k,l}{m,n,o,p}"),
            Matrix::IDENTITY
        );
    }

    #[test]
    fn a_beam_at_home_points_straight_down() {
        assert_eq!(Matrix::IDENTITY.beam_direction(), [0.0, 0.0, -1.0]);
        let shown = to_show_axes(Matrix::IDENTITY.beam_direction());
        assert!(close(shown.y, -1.0), "down is negative height");
    }

    /// The rotation is read by **columns**: a quarter turn about GDTF's X,
    /// written as rows, sends local −Z to +Y — this desk's upstage.
    #[test]
    fn a_turned_geometry_turns_its_beam() {
        // Rx(+90°) by rows: {1,0,0}{0,0,-1}{0,1,0}. Its Z column is (0,-1,0),
        // so −Z points along +Y.
        let matrix = Matrix::parse("{1,0,0,0}{0,0,-1,0}{0,1,0,0}{0,0,0,1}");
        let direction = to_show_axes(matrix.beam_direction());
        assert!(close(direction.z, 1.0), "{direction:?}");
        assert!(close(direction.x, 0.0));
        assert!(close(direction.y, 0.0));
        let (x, y, z) = matrix.show_axes();
        assert!(close(x.x, 1.0));
        // Show's Y axis is where GDTF's Z column went: (0,-1,0) in GDTF, which
        // is (0,0,-1) in show axes.
        assert!(close(y.z, -1.0), "{y:?}");
        assert!(close(z.y, 1.0), "{z:?}");
    }

    /// The Robin T1's tree, as its file states it: the lens is 0.074 + 0.335 +
    /// 0.292 m below the base, and every node keeps its **own** offset.
    #[test]
    fn the_transforms_multiply_down_the_tree_and_each_node_keeps_its_own() {
        let node = fixture(
            r#"<Models>
                 <Model Name="Base" File="base" Length="0.384" Width="0.229" Height="0.101"/>
                 <Model Name="Yoke" File="yoke" Length="0.399" Width="0.113" Height="0.394"/>
                 <Model Name="Head" File="head" Length="0.278" Width="0.257" Height="0.535"/>
                 <Model Name="Beam" File="" PrimitiveType="Cylinder" Length="0.127" Width="0.127" Height="0.001"/>
               </Models>
               <Geometries>
                 <Geometry Model="Base" Name="Base" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
                   <Axis Model="Yoke" Name="Yoke" Position="{1,0,0,0}{0,1,0,0}{0,0,1,-0.074}{0,0,0,1}">
                     <Axis Model="Head" Name="Head" Position="{1,0,0,0}{0,1,0,0}{0,0,1,-0.335}{0,0,0,1}">
                       <Beam BeamAngle="45" BeamRadius="0.0635" BeamType="Spot" FieldAngle="45"
                             ColorTemperature="8000" LuminousFlux="10075" Model="Beam" Name="Beam"
                             RectangleRatio="1.7777"
                             Position="{1,0,0,0}{0,1,0,0}{0,0,1,-0.291636}{0,0,0,1}"/>
                     </Axis>
                   </Axis>
                 </Geometry>
               </Geometries>"#,
        );
        let physical = Geometries::of(&node).physical("", "GUID");
        let names: Vec<&str> = physical
            .geometries
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(names, ["Base", "Yoke", "Head", "Beam"]);
        let parents: Vec<Option<u32>> = physical.geometries.iter().map(|n| n.parent).collect();
        assert_eq!(parents, [None, Some(0), Some(1), Some(2)]);
        let kinds: Vec<&str> = physical
            .geometries
            .iter()
            .map(|n| n.kind.as_str())
            .collect();
        assert_eq!(kinds, ["Geometry", "Axis", "Axis", "Beam"]);

        let yoke = &physical.geometries[1];
        assert_eq!(yoke.model.as_deref(), Some("yoke"));
        assert!(close(yoke.position.y, -0.074), "{:?}", yoke.position);
        // GDTF's length is across, its width is depth and its height is up.
        assert!(
            close(yoke.size.x, 0.399) && close(yoke.size.y, 0.394) && close(yoke.size.z, 0.113)
        );
        let lens = &physical.geometries[3];
        assert_eq!(lens.primitive.as_deref(), Some("Cylinder"));
        assert_eq!(lens.model, None, "an empty File is no file");
        let shape = lens.beam.as_ref().expect("a beam has optics");
        assert_eq!(shape.beam_type, "Spot");
        assert!(close(shape.beam_radius, 0.0635));
        assert!(close(shape.rectangle_ratio, 1.7777));

        assert_eq!(physical.beams.len(), 1);
        let beam = &physical.beams[0];
        assert!(
            close(beam.position.y, -(0.074 + 0.335 + 0.291_636)),
            "{:?}",
            beam.position
        );
        assert!(close(beam.direction.y, -1.0));
        assert!(close(beam.beam_angle, 45.0));
        assert_eq!(physical.model.as_deref(), Some("base"));
        assert!(close(physical.size.x, 0.384));
        assert_eq!(physical.fixture_type_id, "GUID");
    }

    #[test]
    fn a_mode_names_which_device_it_drives() {
        let node = fixture(&format!(
            r#"<Geometries>
                 <Geometry Name="One" Position="{IDENTITY}"><Beam Name="A" Position="{IDENTITY}"/></Geometry>
                 <Geometry Name="Two" Position="{IDENTITY}"><Beam Name="B" Position="{IDENTITY}"/></Geometry>
               </Geometries>"#
        ));
        let geometries = Geometries::of(&node);
        let named: Vec<String> = geometries
            .physical("Two", "")
            .beams
            .into_iter()
            .map(|beam| beam.name)
            .collect();
        assert_eq!(named, ["B"]);
        assert_eq!(geometries.physical("", "").beams.len(), 2);
        assert_eq!(geometries.physical("Three", "").beams.len(), 2);
    }

    #[test]
    fn a_geometry_reference_repeats_a_pixel_where_it_is_referenced() {
        let node = fixture(
            r#"<Geometries>
                 <Geometry Name="Pixel" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
                   <Beam Name="Cell" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}"/>
                 </Geometry>
                 <Geometry Name="Bar" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
                   <GeometryReference Name="P1" Geometry="Pixel"
                                      Position="{1,0,0,0.1}{0,1,0,0}{0,0,1,0}{0,0,0,1}"/>
                   <GeometryReference Name="P2" Geometry="Pixel"
                                      Position="{1,0,0,0.2}{0,1,0,0}{0,0,1,0}{0,0,0,1}"/>
                 </Geometry>
               </Geometries>"#,
        );
        let physical = Geometries::of(&node).physical("Bar", "");
        let across: Vec<f64> = physical.beams.iter().map(|beam| beam.position.x).collect();
        assert_eq!(across.len(), 2);
        assert!(close(across[0], 0.1), "{across:?}");
        assert!(close(across[1], 0.2), "{across:?}");
        // In the tree the reference is a node of its own, named for itself,
        // with the referenced geometry's beam under it.
        let names: Vec<&str> = physical
            .geometries
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(names, ["Bar", "P1", "Cell", "P2", "Cell"]);
        assert_eq!(physical.geometries[2].parent, Some(1));
    }

    #[test]
    fn a_reference_that_points_at_itself_stops() {
        let node = fixture(
            r#"<Geometries>
                 <Geometry Name="Loop" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
                   <GeometryReference Name="Again" Geometry="Loop"
                                      Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}"/>
                   <Beam Name="Cell" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}"/>
                 </Geometry>
               </Geometries>"#,
        );
        let physical = Geometries::of(&node).physical("Loop", "");
        assert!(!physical.beams.is_empty());
    }

    #[test]
    fn a_fixture_with_no_geometry_has_none() {
        let physical = Geometries::of(&fixture("")).physical("", " GUID ");
        assert!(physical.beams.is_empty());
        assert!(physical.geometries.is_empty());
        assert_eq!(physical.model, None);
        assert_eq!(physical.size, prism_domain::Vec3::ZERO);
        assert_eq!(physical.fixture_type_id, "GUID", "it is trimmed");
    }
}
