//! What a written GDTF is read back as — **B65**.
//!
//! Every case writes a profile and reads the archive with the **reader this
//! desk already has**, which is the one check available on a build machine
//! against a planner it does not own. What the reader cannot say — that the
//! file is valid GDTF — is `docs/RELEASE_TEST_0.9.3.md`'s kind of test and not
//! this one's.

use prism_domain::{
    AttributeDef, AttributeRange, AttributeType, FixtureBeam, FixturePhysical, FixtureType, Vec3,
};

use super::{file_name, guid_of, is_guid, write_archive};
use crate::library::gdtf::read_archive;

fn def(attribute: AttributeType, coarse: u16, fine: Option<u16>) -> AttributeDef {
    AttributeDef {
        attribute,
        label: None,
        occurrence: 0,
        feature_group: attribute.feature_group(),
        coarse_offset: coarse,
        fine_offset: fine,
        default_value: 0,
        merge_mode: attribute.default_merge_mode(),
        invert: false,
        physical_from: 0.0,
        physical_to: 1.0,
        ranges: Vec::new(),
        switched: None,
    }
}

fn profile(mode: &str, attributes: Vec<AttributeDef>) -> FixtureType {
    let footprint = attributes
        .iter()
        .map(|a| a.fine_offset.unwrap_or(a.coarse_offset) + 1)
        .max()
        .unwrap_or(1);
    FixtureType {
        id: format!("acme/spot/{}", mode.to_lowercase()),
        manufacturer: "Acme Lighting".to_owned(),
        name: "Spot 5000".to_owned(),
        mode: mode.to_owned(),
        footprint,
        attributes,
        physical: None,
    }
}

fn range(name: &str, from: u16, to: u16) -> AttributeRange {
    AttributeRange {
        name: name.to_owned(),
        from,
        to,
        media: None,
    }
}

fn physical(size: Vec3, beams: Vec<FixtureBeam>, id: &str) -> FixturePhysical {
    FixturePhysical {
        fixture_type_id: id.to_owned(),
        size,
        model: None,
        beams,
        geometries: Vec::new(),
        channels: Vec::new(),
        wheels: Vec::new(),
    }
}

/// A head with a bit of everything a channel can be.
fn head() -> FixtureType {
    let dimmer = def(AttributeType::Dimmer, 0, None);
    let mut pan = def(AttributeType::Pan, 1, Some(2));
    pan.default_value = 32_768;
    pan.physical_from = -270.0;
    pan.physical_to = 270.0;
    let mut red = def(AttributeType::Red, 3, None);
    red.default_value = u16::MAX;
    let mut gobo = def(AttributeType::Gobo, 4, None);
    gobo.label = Some("Rotating Gobo".to_owned());
    gobo.ranges = vec![
        // The steps of an 8-bit channel — multiples of 257, the way the
        // reader widens them.
        range("Open", 0, 2569),
        range("Stars", 2570, 5139),
        range("Tunnel", 5140, u16::MAX),
    ];
    let mut second_gobo = def(AttributeType::Gobo, 5, None);
    second_gobo.occurrence = 1;
    let mut knob = def(AttributeType::Raw, 6, None);
    knob.label = Some("Lamp Hours".to_owned());
    profile("Extended", vec![dimmer, pan, red, gobo, second_gobo, knob])
}

fn round_trip(modes: &[&FixtureType]) -> Vec<FixtureType> {
    let bytes = write_archive(modes).expect("a small archive");
    let (built, counts) = read_archive(&bytes, true);
    assert_eq!(
        counts.files_rejected, 0,
        "the reader accepts what is written"
    );
    built.into_iter().map(|(_, profile)| profile).collect()
}

#[test]
fn every_channel_comes_back_where_it_was_with_what_it_was() {
    let written = head();
    let read = round_trip(&[&written]);
    assert_eq!(read.len(), 1);
    let read = &read[0];
    assert_eq!(read.footprint, written.footprint);
    assert_eq!(read.mode, "Extended");
    assert_eq!(read.manufacturer, "Acme Lighting");
    assert_eq!(read.name, "Spot 5000");
    assert_eq!(read.attributes.len(), written.attributes.len());
    for (was, now) in written.attributes.iter().zip(&read.attributes) {
        let what = format!("{:?} at {}", was.attribute, was.coarse_offset);
        assert_eq!(now.attribute, was.attribute, "{what}");
        assert_eq!(now.occurrence, was.occurrence, "{what}");
        assert_eq!(now.coarse_offset, was.coarse_offset, "{what}");
        assert_eq!(now.fine_offset, was.fine_offset, "{what}");
        assert_eq!(now.default_value, was.default_value, "{what}");
        assert!(
            (now.physical_from - was.physical_from).abs() < 1e-9,
            "{what}"
        );
        assert!((now.physical_to - was.physical_to).abs() < 1e-9, "{what}");
        assert_eq!(now.ranges, was.ranges, "{what}");
        assert_eq!(now.feature_group, was.feature_group, "{what}");
    }
    // The manufacturer's own words survive as the label — an OFL profile's
    // *Rotating Gobo* and a raw knob's name both.
    assert_eq!(read.attributes[3].label.as_deref(), Some("Rotating Gobo"));
    assert_eq!(read.attributes[5].label.as_deref(), Some("Lamp Hours"));
    assert_eq!(read.attributes[5].attribute, AttributeType::Raw);
}

#[test]
fn an_eight_bit_value_that_came_from_an_eight_bit_file_comes_back_exactly() {
    let mut dimmer = def(AttributeType::Dimmer, 0, None);
    // 128 of 255, the way the reader widens it.
    dimmer.default_value = 128 * 257;
    let read = round_trip(&[&profile("One", vec![dimmer])]);
    assert_eq!(read[0].attributes[0].default_value, 128 * 257);
}

#[test]
fn every_mode_a_show_uses_is_one_file() {
    let a = profile("Basic", vec![def(AttributeType::Dimmer, 0, None)]);
    let b = profile(
        "Full",
        vec![
            def(AttributeType::Dimmer, 0, None),
            def(AttributeType::Red, 1, None),
        ],
    );
    let read = round_trip(&[&a, &b]);
    let modes: Vec<(&str, u16)> = read
        .iter()
        .map(|p| (p.mode.as_str(), p.footprint))
        .collect();
    assert_eq!(modes, [("Basic", 1), ("Full", 2)]);
    assert_eq!(read[0].id, "acme-lighting/spot-5000/basic");
}

/// **The viewer's own stand-in, written as GDTF** (B65, second half): a profile
/// with pan or tilt and no device of its own comes back as a moving head — base,
/// a yoke axis, a head axis, a beam — and pan is on the yoke and tilt on the
/// head, so a planner moves the right part and this desk's viewer draws the same
/// head it drew before the export.
#[test]
fn a_profile_with_pan_and_tilt_and_no_device_is_written_as_a_moving_head() {
    let mut written = head();
    written.attributes.push(def(AttributeType::Tilt, 7, None));
    written.footprint = 8;
    let read = round_trip(&[&written]);
    let physical = read[0].physical.as_ref().expect("a GDTF always has one");

    let kinds: Vec<(&str, &str, Option<&str>)> = physical
        .geometries
        .iter()
        .map(|g| (g.name.as_str(), g.kind.as_str(), g.primitive.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("Base", "Geometry", Some("Base")),
            ("Yoke", "Axis", Some("Yoke")),
            ("Head", "Axis", Some("Head")),
            ("Beam", "Beam", Some("Cylinder")),
        ]
    );
    let parents: Vec<Option<u32>> = physical.geometries.iter().map(|g| g.parent).collect();
    assert_eq!(parents, [None, Some(0), Some(1), Some(2)]);

    // Pan turns the yoke and tilt the head; everything else is on the base.
    let geometry = |offset: u16| {
        physical
            .channels
            .iter()
            .find(|channel| channel.offset == offset)
            .and_then(|channel| channel.geometry.as_deref())
            .map(str::to_owned)
    };
    assert_eq!(geometry(1).as_deref(), Some("Yoke"), "pan");
    assert_eq!(geometry(7).as_deref(), Some("Head"), "tilt");
    assert_eq!(geometry(0).as_deref(), Some("Base"), "dimmer");

    // It hangs: the beam leaves downward from below the head.
    assert_eq!(physical.beams.len(), 1);
    let beam = &physical.beams[0];
    assert!(
        (beam.direction.y + 1.0).abs() < 1e-9,
        "straight down: {beam:?}"
    );
    assert!(beam.position.y < -0.3, "under the base: {beam:?}");
    assert!((beam.beam_angle - 25.0).abs() < 1e-9);
    // The body is the base, as the viewer sizes it.
    assert!((physical.size.x - 0.34).abs() < 1e-9);
}

/// What has no pan or tilt is a can with a beam out of its foot, not a box.
#[test]
fn a_profile_that_does_not_move_is_written_as_a_can() {
    let written = profile(
        "Fixed",
        vec![
            def(AttributeType::Dimmer, 0, None),
            def(AttributeType::Red, 1, None),
        ],
    );
    let read = round_trip(&[&written]);
    let physical = read[0].physical.as_ref().expect("physical");
    assert_eq!(physical.geometries.len(), 2);
    assert_eq!(
        physical.geometries[0].primitive.as_deref(),
        Some("Conventional")
    );
    assert!(
        physical.geometries.iter().all(|g| g.kind != "Axis"),
        "nothing to turn"
    );
    let beam = &physical.beams[0];
    assert!((beam.direction.y + 1.0).abs() < 1e-9);
    assert!(
        (beam.position.y + 0.15).abs() < 1e-9,
        "at the foot: {beam:?}"
    );
}

/// A tree that was written is a tree that is written again: a profile that came
/// out of a GDTF keeps its geometry — names, kinds, places, axes, beams and the
/// geometry every channel acts on — through another export.
#[test]
fn a_geometry_tree_the_profile_carries_is_written_back_as_it_was() {
    let mut written = head();
    written.attributes.push(def(AttributeType::Tilt, 7, None));
    written.footprint = 8;
    let once = round_trip(&[&written]).remove(0);
    let twice = round_trip(&[&once]).remove(0);
    let (a, b) = (once.physical.unwrap(), twice.physical.unwrap());
    assert_eq!(a.geometries, b.geometries);
    assert_eq!(a.beams, b.beams);
    let geometries = |p: &FixturePhysical| -> Vec<(u16, Option<String>)> {
        p.channels
            .iter()
            .map(|c| (c.offset, c.geometry.clone()))
            .collect()
    };
    assert_eq!(geometries(&a), geometries(&b));
}

/// A turned node comes back turned the same way — the axes go out in the order
/// the reader takes them in, which an identity matrix cannot tell.
#[test]
fn a_turned_geometry_comes_back_turned_the_same_way() {
    let mut written = head();
    let mut device = physical(Vec3::ZERO, Vec::new(), "");
    // A quarter turn about GDTF's Z, in the reader's names for the axes.
    device.geometries = vec![prism_domain::GeometryNode {
        name: "Turned".to_owned(),
        parent: None,
        kind: "Geometry".to_owned(),
        model: None,
        primitive: Some("Cube".to_owned()),
        size: Vec3 {
            x: 0.2,
            y: 0.4,
            z: 0.1,
        },
        position: Vec3 {
            x: 0.1,
            y: -0.2,
            z: 0.3,
        },
        x_axis: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        y_axis: Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        z_axis: Vec3 {
            x: -1.0,
            y: 0.0,
            z: 0.0,
        },
        beam: None,
    }];
    written.physical = Some(device.clone());
    let read = round_trip(&[&written]);
    let node = &read[0].physical.as_ref().unwrap().geometries[0];
    let close = |a: Vec3, b: Vec3| {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9
    };
    let was = &device.geometries[0];
    assert!(close(node.x_axis, was.x_axis), "{:?}", node.x_axis);
    assert!(close(node.y_axis, was.y_axis), "{:?}", node.y_axis);
    assert!(close(node.z_axis, was.z_axis), "{:?}", node.z_axis);
    assert!(close(node.position, was.position), "{:?}", node.position);
}

/// A node whose model file the profile cannot carry is written as the box of
/// the room it took, so it is still a body and not a hole.
#[test]
fn a_model_that_is_not_carried_is_written_as_the_box_it_filled() {
    let mut written = head();
    let mut physical = physical(Vec3::ZERO, Vec::new(), "");
    physical.geometries = vec![prism_domain::GeometryNode {
        name: "Housing".to_owned(),
        parent: None,
        kind: "Geometry".to_owned(),
        model: Some("housing".to_owned()),
        primitive: None,
        size: Vec3 {
            x: 0.2,
            y: 0.4,
            z: 0.1,
        },
        position: Vec3::ZERO,
        x_axis: Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        y_axis: Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        z_axis: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        beam: None,
    }];
    written.physical = Some(physical);
    let read = round_trip(&[&written]);
    let node = &read[0].physical.as_ref().unwrap().geometries[0];
    assert_eq!(node.primitive.as_deref(), Some("Cube"));
    assert!((node.size.y - 0.4).abs() < 1e-9 && (node.size.x - 0.2).abs() < 1e-9);
}

/// The device the profile states is the device the file states: its size, and
/// every beam where it sits and which way it points — in every direction a
/// head can hang, including the two the rotation maths treats separately.
#[test]
fn a_device_and_its_beams_come_back_in_the_same_places() {
    let v = |x: f64, y: f64, z: f64| Vec3 { x, y, z };
    let directions = [
        v(0.0, -1.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(0.0, 0.0, 1.0),
        v(-0.6, -0.48, 0.64),
    ];
    let beams: Vec<FixtureBeam> = directions
        .iter()
        .enumerate()
        .map(|(index, direction)| FixtureBeam {
            name: format!("Pixel {}", index + 1),
            position: v(index as f64 * 0.1, -0.35, 0.05),
            direction: *direction,
            beam_angle: 12.5,
            luminous_flux: 1000.0,
            color_temperature: 3200.0,
        })
        .collect();
    let mut written = head();
    written.physical = Some(physical(
        v(0.4, 0.7, 0.5),
        beams.clone(),
        "7b2b4eba-1234-4e2f-9b1a-0a0b0c0d0e0f",
    ));
    let read = round_trip(&[&written]);
    let now = read[0].physical.as_ref().expect("physical");
    assert_eq!(
        now.fixture_type_id, "7B2B4EBA-1234-4E2F-9B1A-0A0B0C0D0E0F",
        "the identity it came with, as GDTF writes a GUID"
    );
    let close = |a: Vec3, b: Vec3| {
        (a.x - b.x).abs() < 1e-5 && (a.y - b.y).abs() < 1e-5 && (a.z - b.z).abs() < 1e-5
    };
    assert!(close(now.size, v(0.4, 0.7, 0.5)), "{:?}", now.size);
    assert_eq!(now.beams.len(), beams.len());
    for (was, got) in beams.iter().zip(&now.beams) {
        assert_eq!(got.name, was.name);
        assert!(close(got.position, was.position), "{was:?} {got:?}");
        let d = was.direction;
        let length = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
        let unit = v(d.x / length, d.y / length, d.z / length);
        assert!(
            close(got.direction, unit),
            "{:?} came back {:?}",
            was.direction,
            got.direction
        );
        assert!((got.beam_angle - 12.5).abs() < 1e-9);
        assert!((got.color_temperature - 3200.0).abs() < 1e-9);
    }
}

#[test]
fn names_with_characters_xml_cares_about_survive() {
    let mut written = profile(
        "Mode \"A\" & <B>",
        vec![def(AttributeType::Dimmer, 0, None)],
    );
    written.name = "Tom's <Spot> & \"Co\"".to_owned();
    written.manufacturer = "Müller & Söhne".to_owned();
    let read = round_trip(&[&written]);
    assert_eq!(read[0].name, written.name);
    assert_eq!(read[0].manufacturer, written.manufacturer);
    assert_eq!(read[0].mode, written.mode);
}

#[test]
fn a_range_is_a_channel_set_and_a_gap_is_closed_as_the_reader_closes_it() {
    let mut gobo = def(AttributeType::Gobo, 0, None);
    gobo.ranges = vec![range("Open", 0, 2569), range("One", 5140, 7709)];
    let read = round_trip(&[&profile("M", vec![gobo])]);
    let ranges = &read[0].attributes[0].ranges;
    assert_eq!(ranges[0].name, "Open");
    assert_eq!(ranges[0].from, 0);
    assert_eq!(ranges[1].name, "One");
    assert_eq!(ranges[1].from, 5140);
    assert_eq!(ranges[1].to, u16::MAX, "GDTF states starts only");
}

#[test]
fn a_guid_is_kept_where_there_is_one_and_made_the_same_way_every_time_where_there_is_not() {
    let mut a = head();
    assert!(is_guid(&guid_of(&a)));
    assert_eq!(guid_of(&a), guid_of(&a.clone()), "stable");
    let mut b = head();
    b.name = "Spot 6000".to_owned();
    assert_ne!(guid_of(&a), guid_of(&b));
    a.physical = Some(physical(Vec3::ZERO, Vec::new(), "not a guid"));
    assert!(is_guid(&guid_of(&a)), "a bad one is replaced, not copied");
}

#[test]
fn a_file_name_is_manufacturer_at_fixture_and_nothing_a_file_system_refuses() {
    let mut p = head();
    assert_eq!(file_name(&p), "Acme Lighting@Spot 5000.gdtf");
    p.manufacturer = "A/B:C".to_owned();
    p.name = "..".to_owned();
    assert_eq!(file_name(&p), "A_B_C@Unknown.gdtf");
}

#[test]
fn nothing_to_write_is_nothing() {
    assert!(write_archive(&[]).is_none());
}
