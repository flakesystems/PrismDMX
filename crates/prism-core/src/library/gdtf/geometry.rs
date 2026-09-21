//! Where a device's beams are, and how big its body is — **S60**, and the
//! reason S30 can be built at all.
//!
//! # What GDTF states and this reads
//!
//! A fixture's `<Geometries>` is a tree: a base, a yoke that turns on it, a head
//! that tilts on the yoke, and a `<Beam>` in the head. Every node carries a
//! `Position` — a 4×4 matrix relative to its parent — and a `Model`, which
//! names the 3D file it is drawn with and states how big it is.
//!
//! This walks that tree with the matrices multiplied down it and comes out with
//! [`prism_domain::FixturePhysical`]: the body's size and model, and one
//! [`prism_domain::FixtureBeam`] per `<Beam>`, with where it sits and which way
//! it points.
//!
//! # Two conventions, written down once
//!
//! **Which way a beam points.** GDTF's beam leaves along its geometry's
//! **−Z**. So a beam's direction is the matrix's third column, negated, and a
//! device whose head is at home with an identity matrix points straight down —
//! which is where a hanging light points.
//!
//! **Which way the world is.** GDTF is Z-up: X to the right, Y away from the
//! operator, Z up. [`prism_domain::Vec3`] is Y-up — `x` across, `y` height,
//! `z` depth — because that is what `prism_domain::Fixture::position` has meant
//! since S1 and a viewer may not hold two opinions about which way up a stage
//! is. [`to_show_axes`] is the one place that conversion happens.
//!
//! # The unit, and the one thing here that is not settled
//!
//! GDTF states lengths in metres and the **translation part of a matrix in
//! millimetres**, which is why [`MATRIX_TO_METRES`] exists and is applied in
//! exactly one place. It is the single assumption in this module that was not
//! checked against a published archive while it was written — see
//! `PROGRESS.md` §5. Everything else here is arithmetic the tests pin down, and
//! if that number is ever found to be wrong, one constant is what changes.

use prism_domain::{FixtureBeam, FixturePhysical, Vec3};

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

/// Millimetres to metres.
///
/// GDTF's matrices state their translation in millimetres while everything else
/// in the format — a model's length, width and height — is in metres. This is
/// the one place the two meet, and it is deliberately a named constant rather
/// than a `0.001` in an expression: see this module's documentation.
const MATRIX_TO_METRES: f64 = 0.001;

/// A 4×4 transform, as GDTF writes one.
///
/// Held as the three basis vectors and the translation, which is all this needs
/// of it: the fourth row of a GDTF matrix is `{0,0,0,1}` in every file the
/// format produces, and a projective transform is not a thing a lighting
/// fixture's geometry is.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Matrix {
    /// The X, Y and Z basis vectors, as columns.
    basis: [[f64; 3]; 3],
    /// Where the origin moved to, in metres.
    origin: [f64; 3],
}

impl Matrix {
    /// The transform that changes nothing.
    const IDENTITY: Self = Self {
        basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        origin: [0.0, 0.0, 0.0],
    };

    /// `self` followed by `child` — the child's transform expressed in the
    /// parent's frame.
    fn then(&self, child: &Self) -> Self {
        let mut basis = [[0.0; 3]; 3];
        for (column, out) in basis.iter_mut().enumerate() {
            for (row, cell) in out.iter_mut().enumerate() {
                *cell = (0..3)
                    .map(|inner| self.basis[inner][row] * child.basis[column][inner])
                    .sum();
            }
        }
        let mut origin = self.origin;
        for (row, cell) in origin.iter_mut().enumerate() {
            *cell += (0..3)
                .map(|inner| self.basis[inner][row] * child.origin[inner])
                .sum::<f64>();
        }
        Self { basis, origin }
    }

    /// The matrix a `Position` attribute states.
    ///
    /// GDTF writes it as four brace-delimited groups of four numbers, the
    /// first three the basis vectors and the last the translation. Anything
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
            if numbers.len() < 3 {
                return Self::IDENTITY;
            }
            groups.push(numbers);
        }
        if groups.len() < 4 {
            return Self::IDENTITY;
        }
        let mut basis = [[0.0; 3]; 3];
        for (index, out) in basis.iter_mut().enumerate() {
            out.copy_from_slice(&groups[index][..3]);
        }
        let origin = [
            groups[3][0] * MATRIX_TO_METRES,
            groups[3][1] * MATRIX_TO_METRES,
            groups[3][2] * MATRIX_TO_METRES,
        ];
        Self { basis, origin }
    }

    /// Where this transform's **−Z** points, as a unit vector.
    ///
    /// Straight down when the matrix does not have a length to normalise —
    /// which is where a hanging light points, and never the zero vector that a
    /// viewer would have to special-case.
    fn beam_direction(&self) -> [f64; 3] {
        let z = self.basis[2];
        let length = (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt();
        if !length.is_finite() || length <= f64::EPSILON {
            return [0.0, 0.0, -1.0];
        }
        [-z[0] / length, -z[1] / length, -z[2] / length]
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
    /// Every `Model`, by name: the file it names and how big it is.
    models: Vec<(String, Model)>,
    /// The top-level geometries, in the order the file lists them.
    roots: Vec<Node>,
}

/// One `Model`: the file it is drawn from and the box it fits in.
#[derive(Debug, Clone, Default)]
struct Model {
    /// The file GDTF names, without a directory or an extension — the format
    /// ships the same model in several formats under one name.
    file: Option<String>,
    /// Length (X), width (Y) and height (Z), in metres, as GDTF states them.
    size: [f64; 3],
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
                models.push((
                    name,
                    Model {
                        file: non_empty(model.get("File")).map(str::to_owned),
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

        let mut beams = Vec::new();
        for root in roots {
            self.walk(root, Matrix::IDENTITY, 0, &mut beams);
        }
        // The body is the first root's model: the base of a moving head, the
        // box of a PAR. A device whose root names no model has no size here,
        // which a viewer reads as *size it yourself*.
        let body = roots.first().and_then(|root| self.model_of(root));
        FixturePhysical {
            fixture_type_id: fixture_type_id.trim().to_owned(),
            size: body.as_ref().map_or(Vec3::ZERO, |model| {
                to_show_axes([model.size[0], model.size[1], model.size[2]])
            }),
            model: body.and_then(|model| model.file),
            beams,
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

    /// Walks one geometry and everything under it, collecting beams.
    ///
    /// A `GeometryReference` is followed — it is how an LED bar states *and
    /// here are the same eight pixels again* — with the reference's own
    /// position applied, and with [`MAX_DEPTH`] as the thing that stops a file
    /// whose references point at each other.
    fn walk(&self, node: &Node, parent: Matrix, depth: usize, beams: &mut Vec<FixtureBeam>) {
        if depth >= MAX_DEPTH || beams.len() >= MAX_BEAMS {
            return;
        }
        let here = parent.then(&Matrix::parse(node.get("Position")));
        if node.name == "Beam" {
            beams.push(FixtureBeam {
                name: non_empty(node.get("Name"))
                    .map_or_else(|| format!("Beam {}", beams.len() + 1), str::to_owned),
                position: to_show_axes(here.origin),
                direction: to_show_axes(here.beam_direction()),
                beam_angle: number(node.get("BeamAngle")).unwrap_or(0.0),
                luminous_flux: number(node.get("LuminousFlux")).unwrap_or(0.0),
                color_temperature: number(node.get("ColorTemperature")).unwrap_or(0.0),
            });
        }
        if node.name == "GeometryReference" {
            let target = non_empty(node.get("Geometry"))
                .and_then(|name| self.roots.iter().find(|root| root.get("Name") == name));
            if let Some(target) = target {
                // The reference's own children are its DMX break overrides and
                // not geometry, so only the target is walked — from **this**
                // node's transform, which is what puts the eighth pixel of a
                // bar where the eighth pixel is.
                for child in &target.children {
                    self.walk(child, here, depth + 1, beams);
                }
                if target.name == "Beam" {
                    self.walk(target, parent, depth + 1, beams);
                }
            }
            return;
        }
        for child in &node.children {
            self.walk(child, here, depth + 1, beams);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Geometries, MATRIX_TO_METRES, Matrix, to_show_axes};
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

    #[test]
    fn a_position_is_read_as_a_matrix_in_metres() {
        // 1 200 mm up in GDTF's Z, which is 1.2 m of this desk's height.
        let matrix = Matrix::parse("{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,1200,1}");
        assert!((matrix.origin[2] - 1.2).abs() < 1e-9);
        assert!((MATRIX_TO_METRES - 0.001).abs() < f64::EPSILON);
        let shown = to_show_axes(matrix.origin);
        assert!(
            (shown.y - 1.2).abs() < 1e-9,
            "GDTF's Z is this desk's height"
        );
        assert!(shown.x.abs() < 1e-9);
        assert!(shown.z.abs() < 1e-9);
    }

    #[test]
    fn a_position_that_will_not_parse_is_the_identity() {
        assert_eq!(Matrix::parse(""), Matrix::IDENTITY);
        assert_eq!(Matrix::parse("{1,0,0,0}"), Matrix::IDENTITY);
        assert_eq!(Matrix::parse("nonsense"), Matrix::IDENTITY);
        assert_eq!(
            Matrix::parse("{a,b,c,d}{e,f,g,h}{i,j,k,l}{m,n,o,p}"),
            Matrix::IDENTITY
        );
    }

    #[test]
    fn a_beam_at_home_points_straight_down() {
        assert_eq!(Matrix::IDENTITY.beam_direction(), [0.0, 0.0, -1.0]);
        let shown = to_show_axes(Matrix::IDENTITY.beam_direction());
        assert!((shown.y + 1.0).abs() < 1e-9, "down is negative height");
    }

    #[test]
    fn a_turned_geometry_turns_its_beam() {
        // A quarter turn about GDTF's X: −Z becomes +Y, which is this desk's
        // depth — a light on the floor pointing away from the operator.
        let matrix = Matrix::parse("{1,0,0,0}{0,0,1,0}{0,-1,0,0}{0,0,0,1}");
        let direction = to_show_axes(matrix.beam_direction());
        assert!((direction.z - 1.0).abs() < 1e-9, "{direction:?}");
        assert!(direction.x.abs() < 1e-9);
        assert!(direction.y.abs() < 1e-9);
    }

    #[test]
    fn the_transforms_multiply_down_the_tree() {
        // A yoke 500 mm up on a base, and a beam 300 mm up in the yoke: the
        // beam is 800 mm up. A reader that took only the innermost matrix
        // would put it at 300.
        let node = fixture(
            r#"<Geometries>
                 <Geometry Name="Base" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
                   <Axis Name="Yoke" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,500,1}">
                     <Beam Name="Beam" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,300,1}"
                           BeamAngle="14" LuminousFlux="9000" ColorTemperature="6500"/>
                   </Axis>
                 </Geometry>
               </Geometries>"#,
        );
        let physical = Geometries::of(&node).physical("", "GUID");
        assert_eq!(physical.beams.len(), 1);
        let beam = &physical.beams[0];
        assert_eq!(beam.name, "Beam");
        assert!((beam.position.y - 0.8).abs() < 1e-9, "{:?}", beam.position);
        assert!((beam.beam_angle - 14.0).abs() < f64::EPSILON);
        assert!((beam.luminous_flux - 9000.0).abs() < f64::EPSILON);
        assert!((beam.color_temperature - 6500.0).abs() < f64::EPSILON);
        assert_eq!(physical.fixture_type_id, "GUID");
    }

    #[test]
    fn the_body_s_model_is_its_size_and_its_file() {
        let node = fixture(
            r#"<Models>
                 <Model Name="Base" File="base" Length="0.3" Width="0.4" Height="0.5"/>
               </Models>
               <Geometries>
                 <Geometry Name="Base" Model="Base" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}"/>
               </Geometries>"#,
        );
        let physical = Geometries::of(&node).physical("", "");
        assert_eq!(physical.model.as_deref(), Some("base"));
        // GDTF's length is across, its width is depth and its height is up.
        assert!((physical.size.x - 0.3).abs() < 1e-9);
        assert!((physical.size.z - 0.4).abs() < 1e-9);
        assert!((physical.size.y - 0.5).abs() < 1e-9);
        assert!(physical.beams.is_empty());
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
        // Naming none takes the lot, which is what a one-device file wants.
        assert_eq!(geometries.physical("", "").beams.len(), 2);
        // Naming one that is not there is the same answer rather than none.
        assert_eq!(geometries.physical("Three", "").beams.len(), 2);
    }

    #[test]
    fn a_geometry_reference_repeats_a_pixel_where_it_is_referenced() {
        // How an LED bar states its pixels: one `Pixel` geometry, referenced
        // twice at two places. A reader that ignored the reference would give
        // the bar one beam at the origin.
        let node = fixture(
            r#"<Geometries>
                 <Geometry Name="Pixel" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
                   <Beam Name="Cell" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}"/>
                 </Geometry>
                 <Geometry Name="Bar" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
                   <GeometryReference Name="P1" Geometry="Pixel"
                                      Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{100,0,0,1}"/>
                   <GeometryReference Name="P2" Geometry="Pixel"
                                      Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{200,0,0,1}"/>
                 </Geometry>
               </Geometries>"#,
        );
        let physical = Geometries::of(&node).physical("Bar", "");
        let across: Vec<f64> = physical.beams.iter().map(|beam| beam.position.x).collect();
        assert_eq!(across.len(), 2);
        assert!((across[0] - 0.1).abs() < 1e-9, "{across:?}");
        assert!((across[1] - 0.2).abs() < 1e-9, "{across:?}");
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
        // It terminates, which is the assertion; how many beams a loop yields
        // is not a fact worth freezing.
        let physical = Geometries::of(&node).physical("Loop", "");
        assert!(!physical.beams.is_empty());
    }

    #[test]
    fn a_fixture_with_no_geometry_has_none() {
        let physical = Geometries::of(&fixture("")).physical("", " GUID ");
        assert!(physical.beams.is_empty());
        assert_eq!(physical.model, None);
        assert_eq!(physical.size, prism_domain::Vec3::ZERO);
        assert_eq!(physical.fixture_type_id, "GUID", "it is trimmed");
    }
}
