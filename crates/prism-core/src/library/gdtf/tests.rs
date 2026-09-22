//! What a GDTF file becomes — **S61**.
//!
//! Every case here is a `description.xml` written out in full, because the
//! thing being tested is a reading of a document and a fixture built out of
//! fragments would be a reading of this file's idea of one. The archive-level
//! tests go through a real ZIP built byte by byte (`super::super::zip`), so the
//! whole path a `.gdtf` takes is exercised and not only its XML.

use prism_domain::{AttributeType, MergeMode};

use super::{Conversion, dmx_value, read_archive, read_description, slug};

/// The identity, as GDTF writes a `Position`.
const IDENTITY: &str = "{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}";

/// A whole `description.xml` around a body.
fn description(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
           <GDTF DataVersion="1.2">
             <FixtureType Name="Robin T1 Profile" ShortName="T1"
                          Manufacturer="Robe Lighting"
                          FixtureTypeID="7B2B4EBA-1234-4E2F-9B1A-0A0B0C0D0E0F">
               {body}
             </FixtureType>
           </GDTF>"#
    )
}

/// A one-mode fixture whose channels are written out by the caller.
fn one_mode(channels: &str) -> String {
    description(&format!(
        r#"<DMXModes>
             <DMXMode Name="Standard" Geometry="Base">
               <DMXChannels>{channels}</DMXChannels>
             </DMXMode>
           </DMXModes>"#
    ))
}

/// Reads a description and insists it produced exactly one mode.
fn only_mode(source: &str) -> (prism_domain::FixtureType, Conversion) {
    let (built, counts) = read_description(source.as_bytes(), false);
    assert_eq!(built.len(), 1, "one mode was written");
    let (_, profile) = built.into_iter().next().expect("one mode");
    (profile, counts)
}

#[test]
fn a_fixture_is_keyed_by_what_it_says_it_is() {
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="Dimmer">
               <ChannelFunction Name="Dimmer 1" Attribute="Dimmer"
                                PhysicalFrom="0" PhysicalTo="100"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (built, counts) = read_description(source.as_bytes(), false);
    assert_eq!(counts.fixtures, 1);
    assert_eq!(counts.modes, 1);
    let (entry, profile) = &built[0];
    assert_eq!(entry.id, "robe-lighting/robin-t1-profile/standard");
    assert_eq!(entry.manufacturer, "Robe Lighting");
    assert_eq!(entry.name, "Robin T1 Profile");
    assert_eq!(entry.mode, "Standard");
    assert_eq!(entry.footprint, 1);
    assert!(!entry.own);
    assert_eq!(profile.id, entry.id);
    assert_eq!(profile.attributes.len(), 1);
    assert_eq!(profile.attributes[0].attribute, AttributeType::Dimmer);
    assert_eq!(profile.attributes[0].coarse_offset, 0);
    assert_eq!(profile.attributes[0].fine_offset, None);
    assert_eq!(profile.attributes[0].label.as_deref(), Some("Dimmer"));
}

#[test]
fn a_sixteen_bit_channel_keeps_both_bytes_and_its_physical_range() {
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="3,4">
             <LogicalChannel Attribute="Pan">
               <ChannelFunction Name="Pan 1" Attribute="Pan" Default="32768/2"
                                PhysicalFrom="-270" PhysicalTo="270"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, _) = only_mode(&source);
    let pan = &profile.attributes[0];
    assert_eq!(pan.attribute, AttributeType::Pan);
    assert_eq!(pan.coarse_offset, 2, "GDTF's offsets are one-based");
    assert_eq!(pan.fine_offset, Some(3));
    assert_eq!(pan.default_value, 32_768);
    assert!((pan.physical_from + 270.0).abs() < f64::EPSILON);
    assert!((pan.physical_to - 270.0).abs() < f64::EPSILON);
    assert_eq!(pan.merge_mode, MergeMode::Ltp);
    // The footprint is how far the channels reach, not how many there are.
    assert_eq!(profile.footprint, 4);
}

#[test]
fn a_channel_with_no_offset_is_not_on_the_wire() {
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="Dimmer">
               <ChannelFunction Attribute="Dimmer"/>
             </LogicalChannel>
           </DMXChannel>
           <DMXChannel DMXBreak="1" Offset="None">
             <LogicalChannel Attribute="Zoom">
               <ChannelFunction Attribute="Zoom"/>
             </LogicalChannel>
           </DMXChannel>
           <DMXChannel DMXBreak="1">
             <LogicalChannel Attribute="Focus1">
               <ChannelFunction Attribute="Focus1"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, counts) = only_mode(&source);
    assert_eq!(counts.channels_virtual, 2);
    assert_eq!(profile.footprint, 1);
    assert_eq!(profile.attributes.len(), 1);
}

#[test]
fn a_channel_on_a_second_break_is_counted_and_left() {
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="Dimmer">
               <ChannelFunction Attribute="Dimmer"/>
             </LogicalChannel>
           </DMXChannel>
           <DMXChannel DMXBreak="2" Offset="1">
             <LogicalChannel Attribute="ColorAdd_R">
               <ChannelFunction Attribute="ColorAdd_R"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, counts) = only_mode(&source);
    assert_eq!(counts.channels_other_break, 1);
    assert_eq!(profile.footprint, 1, "the second break is a second address");
    assert_eq!(profile.attributes.len(), 1);
}

#[test]
fn two_names_for_one_knob_are_two_knobs() {
    // `Shutter1` and `Shutter1Strobe` are both this desk's `Shutter`, and two
    // attributes of one fixture may not carry one key — the merge refuses a rig
    // that does. The second takes the next number.
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="Shutter1">
               <ChannelFunction Attribute="Shutter1"/>
             </LogicalChannel>
           </DMXChannel>
           <DMXChannel DMXBreak="1" Offset="2">
             <LogicalChannel Attribute="Shutter1Strobe">
               <ChannelFunction Attribute="Shutter1Strobe"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, counts) = only_mode(&source);
    assert_eq!(counts.channels_renumbered, 1);
    let keys: Vec<_> = profile
        .attributes
        .iter()
        .map(|def| (def.attribute, def.occurrence))
        .collect();
    assert_eq!(
        keys,
        [(AttributeType::Shutter, 0), (AttributeType::Shutter, 1),]
    );
}

#[test]
fn a_second_gobo_wheel_is_the_second_because_the_name_says_so() {
    // And not because it is second in the channel list: this mode carries only
    // the second wheel, and it is still `Gobo 2`.
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="Gobo2">
               <ChannelFunction Attribute="Gobo2"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, _) = only_mode(&source);
    assert_eq!(profile.attributes[0].attribute, AttributeType::Gobo);
    assert_eq!(profile.attributes[0].occurrence, 1);
}

#[test]
fn an_attribute_this_desk_has_no_word_for_is_a_knob_with_its_name_on_it() {
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="MagicSparkleMode">
               <ChannelFunction Attribute="MagicSparkleMode"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, counts) = only_mode(&source);
    assert_eq!(counts.channels_raw, 1);
    assert_eq!(profile.attributes[0].attribute, AttributeType::Raw);
    assert_eq!(
        profile.attributes[0].label.as_deref(),
        Some("Magic Sparkle Mode")
    );
    assert_eq!(profile.footprint, 1, "it still occupies its channel");
}

#[test]
fn a_gobo_wheel_s_slots_become_named_ranges_with_their_pictures() {
    // The whole point of the format for this desk: *Gobo 3* is a picture, and
    // the picture's name travels with the profile.
    let source = description(
        r#"<Wheels>
             <Wheel Name="Gobo1">
               <Slot Name="Open"/>
               <Slot Name="Triangles" MediaFileName="gobo_triangles"/>
               <Slot Name="Dots" MediaFileName="gobo_dots"/>
             </Wheel>
           </Wheels>
           <DMXModes>
             <DMXMode Name="Standard">
               <DMXChannels>
                 <DMXChannel DMXBreak="1" Offset="1">
                   <LogicalChannel Attribute="Gobo1">
                     <ChannelFunction Attribute="Gobo1" Wheel="Gobo1">
                       <ChannelSet DMXFrom="0/1" WheelSlotIndex="1"/>
                       <ChannelSet DMXFrom="10/1" WheelSlotIndex="2"/>
                       <ChannelSet DMXFrom="20/1" WheelSlotIndex="3"/>
                     </ChannelFunction>
                   </LogicalChannel>
                 </DMXChannel>
               </DMXChannels>
             </DMXMode>
           </DMXModes>"#,
    );
    let (profile, counts) = only_mode(&source);
    assert_eq!(counts.wheel_media, 2, "one slot has no picture");
    let ranges = &profile.attributes[0].ranges;
    let named: Vec<(&str, Option<&str>)> = ranges
        .iter()
        .map(|range| (range.name.as_str(), range.media.as_deref()))
        .collect();
    assert_eq!(
        named,
        [
            ("Open", None),
            ("Triangles", Some("gobo_triangles")),
            ("Dots", Some("gobo_dots")),
        ]
    );
    // The ranges meet with nothing between them and the last runs to the top,
    // so an encoder standing anywhere names something.
    assert_eq!(ranges[0].from, 0);
    assert_eq!(ranges[1].from, dmx_value("10/1").expect("a value"));
    assert_eq!(ranges[0].to, ranges[1].from - 1);
    assert_eq!(ranges[2].to, u16::MAX);
}

#[test]
fn a_channel_set_that_names_itself_keeps_its_own_name() {
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="Shutter1">
               <ChannelFunction Attribute="Shutter1">
                 <ChannelSet Name="Closed" DMXFrom="0/1"/>
                 <ChannelSet Name="Open" DMXFrom="32/1"/>
               </ChannelFunction>
               <ChannelFunction Attribute="Shutter1Strobe" DMXFrom="64/1">
                 <ChannelSet Name="Strobe slow" DMXFrom="64/1"/>
               </ChannelFunction>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, _) = only_mode(&source);
    let names: Vec<&str> = profile.attributes[0]
        .ranges
        .iter()
        .map(|range| range.name.as_str())
        .collect();
    assert_eq!(names, ["Closed", "Open", "Strobe slow"]);
    // The channel *is* its first function's attribute, and the rest of the
    // range is still readable.
    assert_eq!(profile.attributes[0].attribute, AttributeType::Shutter);
}

#[test]
fn a_colour_with_no_stated_default_rests_open() {
    // B1's rule, applied in the converter and nowhere else.
    let source = one_mode(
        r#"<DMXChannel DMXBreak="1" Offset="1">
             <LogicalChannel Attribute="ColorAdd_R">
               <ChannelFunction Attribute="ColorAdd_R"/>
             </LogicalChannel>
           </DMXChannel>
           <DMXChannel DMXBreak="1" Offset="2">
             <LogicalChannel Attribute="Dimmer">
               <ChannelFunction Attribute="Dimmer"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (profile, _) = only_mode(&source);
    assert_eq!(profile.attributes[0].default_value, u16::MAX);
    assert_eq!(profile.attributes[1].default_value, 0);
}

#[test]
fn every_mode_of_a_file_is_a_profile_of_its_own() {
    let source = description(
        r#"<DMXModes>
             <DMXMode Name="Mode 1">
               <DMXChannels>
                 <DMXChannel Offset="1">
                   <LogicalChannel Attribute="Dimmer">
                     <ChannelFunction Attribute="Dimmer"/>
                   </LogicalChannel>
                 </DMXChannel>
               </DMXChannels>
             </DMXMode>
             <DMXMode Name="Mode 2">
               <DMXChannels>
                 <DMXChannel Offset="1">
                   <LogicalChannel Attribute="Dimmer">
                     <ChannelFunction Attribute="Dimmer"/>
                   </LogicalChannel>
                 </DMXChannel>
                 <DMXChannel Offset="2">
                   <LogicalChannel Attribute="Zoom">
                     <ChannelFunction Attribute="Zoom"/>
                   </LogicalChannel>
                 </DMXChannel>
               </DMXChannels>
             </DMXMode>
           </DMXModes>"#,
    );
    let (built, counts) = read_description(source.as_bytes(), true);
    assert_eq!(counts.modes, 2);
    assert_eq!(counts.fixtures, 1, "one file is one fixture");
    let keys: Vec<(&str, u16, bool)> = built
        .iter()
        .map(|(entry, _)| (entry.id.as_str(), entry.footprint, entry.own))
        .collect();
    assert_eq!(
        keys,
        [
            ("robe-lighting/robin-t1-profile/mode-1", 1, true),
            ("robe-lighting/robin-t1-profile/mode-2", 2, true),
        ]
    );
}

#[test]
fn the_physical_description_travels_with_every_mode() {
    let source = description(&format!(
        r#"<Models><Model Name="Base" File="base" Length="0.3" Width="0.3" Height="0.6"/></Models>
           <Geometries>
             <Geometry Name="Base" Model="Base" Position="{IDENTITY}">
               <Beam Name="Beam" Position="{IDENTITY}" BeamAngle="12" LuminousFlux="12000"/>
             </Geometry>
           </Geometries>
           <DMXModes>
             <DMXMode Name="Standard" Geometry="Base">
               <DMXChannels>
                 <DMXChannel Offset="1">
                   <LogicalChannel Attribute="Dimmer">
                     <ChannelFunction Attribute="Dimmer"/>
                   </LogicalChannel>
                 </DMXChannel>
               </DMXChannels>
             </DMXMode>
           </DMXModes>"#
    ));
    let (profile, counts) = only_mode(&source);
    assert_eq!(counts.beams, 1);
    assert_eq!(counts.models, 1);
    let physical = profile.physical.expect("a GDTF profile has one");
    assert_eq!(
        physical.fixture_type_id,
        "7B2B4EBA-1234-4E2F-9B1A-0A0B0C0D0E0F"
    );
    assert_eq!(physical.model.as_deref(), Some("base"));
    assert!((physical.size.y - 0.6).abs() < 1e-9);
    assert_eq!(physical.beams.len(), 1);
    assert!((physical.beams[0].beam_angle - 12.0).abs() < f64::EPSILON);
}

#[test]
fn a_file_that_is_not_a_fixture_is_counted_and_left() {
    for source in [
        "",
        "not xml at all",
        "<GDTF DataVersion=\"1.2\"></GDTF>",
        "<html><body/></html>",
    ] {
        let (built, counts) = read_description(source.as_bytes(), false);
        assert!(built.is_empty(), "{source:?}");
        assert_eq!(counts.files_rejected, 1, "{source:?}");
        assert_eq!(counts.fixtures, 0, "{source:?}");
    }
}

#[test]
fn a_mode_with_no_channels_is_not_a_profile() {
    let source = description(
        r#"<DMXModes>
             <DMXMode Name="Empty"><DMXChannels/></DMXMode>
           </DMXModes>"#,
    );
    let (built, counts) = read_description(source.as_bytes(), false);
    assert!(built.is_empty());
    assert_eq!(counts.fixtures, 1, "the file itself was understood");
    assert_eq!(counts.modes, 0);
}

#[test]
fn a_fixture_that_names_nothing_still_has_a_key() {
    let source = r#"<GDTF><FixtureType>
        <DMXModes><DMXMode><DMXChannels>
          <DMXChannel Offset="1">
            <LogicalChannel Attribute="Dimmer">
              <ChannelFunction Attribute="Dimmer"/>
            </LogicalChannel>
          </DMXChannel>
        </DMXChannels></DMXMode></DMXModes>
      </FixtureType></GDTF>"#;
    let (built, _) = read_description(source.as_bytes(), false);
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].0.id, "unknown/fixture/mode");
    assert_eq!(built[0].0.manufacturer, "Unknown");
}

#[test]
fn the_manufacturer_s_own_word_is_what_the_encoder_reads() {
    // GDTF's `AttributeDefinitions` table states a `Pretty` beside each
    // attribute. It is the manufacturer's word, which is what S53 put `label`
    // on the definition for; where the file states none, this desk spaces out
    // the format's own name.
    let source = description(
        r#"<AttributeDefinitions>
             <Attributes>
               <Attribute Name="Dimmer" Pretty="Dim"/>
               <Attribute Name="Gobo1" Pretty="G1"/>
             </Attributes>
           </AttributeDefinitions>
           <DMXModes><DMXMode Name="Standard"><DMXChannels>
             <DMXChannel Offset="1">
               <LogicalChannel Attribute="Dimmer">
                 <ChannelFunction Attribute="Dimmer"/>
               </LogicalChannel>
             </DMXChannel>
             <DMXChannel Offset="2">
               <LogicalChannel Attribute="Shutter1Strobe">
                 <ChannelFunction Attribute="Shutter1Strobe"/>
               </LogicalChannel>
             </DMXChannel>
           </DMXChannels></DMXMode></DMXModes>"#,
    );
    let (profile, _) = only_mode(&source);
    let labels: Vec<Option<&str>> = profile
        .attributes
        .iter()
        .map(|def| def.label.as_deref())
        .collect();
    assert_eq!(labels, [Some("Dim"), Some("Shutter 1 Strobe")]);
}

#[test]
fn a_slug_is_lower_case_and_dashed() {
    assert_eq!(slug("Robe Lighting"), "robe-lighting");
    assert_eq!(slug("Mac 700 Profile"), "mac-700-profile");
    assert_eq!(slug("  A  B  "), "a-b");
    assert_eq!(slug("!!!"), "fixture");
    assert_eq!(slug(""), "fixture");
    // A manufacturer with a non-ASCII name keeps its letters.
    assert_eq!(slug("Lichttechnik Müller"), "lichttechnik-müller");
}

#[test]
fn a_dmx_value_is_scaled_from_the_bytes_it_is_written_in() {
    assert_eq!(dmx_value("0/1"), Some(0));
    assert_eq!(dmx_value("255/1"), Some(u16::MAX));
    assert_eq!(dmx_value("65535/2"), Some(u16::MAX));
    assert_eq!(dmx_value("128/1"), Some(32_896));
    assert_eq!(dmx_value("32768/2"), Some(32_768));
    // A trailing `s` asks for a shift rather than a scale; this desk scales,
    // and the two agree at both ends.
    assert_eq!(dmx_value("255/1s"), Some(u16::MAX));
    // A value larger than its own byte count is clamped rather than wrapped.
    assert_eq!(dmx_value("999/1"), Some(u16::MAX));
    assert_eq!(dmx_value(""), None);
    assert_eq!(dmx_value("nonsense"), None);
    assert_eq!(dmx_value("1/0"), None);
}

/* -------------------------------------------------------------------------- */
/* The archive                                                                */
/* -------------------------------------------------------------------------- */

/// A `.gdtf` file: a ZIP archive with a `description.xml` in it, built byte by
/// byte so that the whole path a real file takes is what is exercised.
fn archive(description: &str) -> Vec<u8> {
    crate::library::zip::testkit::one_file("description.xml", description.as_bytes())
}

#[test]
fn a_gdtf_archive_reads_the_same_as_its_description() {
    let source = one_mode(
        r#"<DMXChannel Offset="1">
             <LogicalChannel Attribute="Dimmer">
               <ChannelFunction Attribute="Dimmer"/>
             </LogicalChannel>
           </DMXChannel>"#,
    );
    let (from_archive, counts) = read_archive(&archive(&source), false);
    let (from_xml, _) = read_description(source.as_bytes(), false);
    assert_eq!(from_archive, from_xml);
    assert_eq!(counts.files_rejected, 0);
    assert_eq!(counts.fixtures, 1);
}

#[test]
fn a_file_that_is_not_an_archive_is_counted_and_left() {
    for bytes in [
        &b""[..],
        b"not a zip",
        // A ZIP with no description in it.
        &crate::library::zip::testkit::one_file("readme.txt", b"hello")[..],
    ] {
        let (built, counts) = read_archive(bytes, false);
        assert!(built.is_empty());
        assert_eq!(counts.files_rejected, 1);
    }
}

#[test]
fn counts_add_up() {
    let mut total = Conversion::default();
    total.absorb(Conversion {
        fixtures: 1,
        modes: 2,
        channels_raw: 3,
        beams: 4,
        ..Conversion::default()
    });
    total.absorb(Conversion {
        fixtures: 1,
        modes: 1,
        channels_raw: 1,
        beams: 1,
        ..Conversion::default()
    });
    assert_eq!(total.fixtures, 2);
    assert_eq!(total.modes, 3);
    assert_eq!(total.channels_raw, 4);
    assert_eq!(total.beams, 5);
}

/// **A published archive, read whole** — S30b.
///
/// There is no GDTF corpus in this repository and there cannot be one (D12),
/// so this reads a file the person running it supplies:
///
/// ```text
/// PRISMDMX_GDTF_SAMPLE=path/to/Robe@Robin_T1_Profile.gdtf \
///   cargo test -p prism-core --lib a_published_archive -- --ignored --nocapture
/// ```
///
/// It asserts what every published moving head has — a tree with an axis in
/// it, a lens that is **not** at the base, channels that name the geometry they
/// move, a shutter with more than one function — and prints the rest, so the
/// person who ran it can compare it with the file.
#[test]
#[ignore = "reads a published .gdtf named by PRISMDMX_GDTF_SAMPLE"]
// Printing what it read is the point: the person who ran it compares it.
#[allow(clippy::print_stdout)]
fn a_published_archive_is_read_whole() {
    let Ok(path) = std::env::var("PRISMDMX_GDTF_SAMPLE") else {
        panic!("set PRISMDMX_GDTF_SAMPLE to a published .gdtf");
    };
    let bytes = std::fs::read(&path).expect("the sample can be read");
    let (built, counts) = read_archive(&bytes, true);
    assert!(!built.is_empty(), "no mode was read: {counts:?}");
    let (entry, profile) = &built[0];
    let physical = profile
        .physical
        .as_ref()
        .expect("a GDTF profile is physical");
    println!(
        "{} — {} channels, {} geometries, {} wheels",
        entry.id,
        profile.footprint,
        physical.geometries.len(),
        physical.wheels.len()
    );
    for node in &physical.geometries {
        println!(
            "  {:>2} {:<10} {:<8} parent {:?} model {:?}/{:?} at {:?}",
            node.name.len(),
            node.name,
            node.kind,
            node.parent,
            node.model,
            node.primitive,
            node.position
        );
    }
    for beam in &physical.beams {
        println!(
            "  beam {} at {:?} pointing {:?}",
            beam.name, beam.position, beam.direction
        );
    }
    assert!(
        physical.geometries.iter().any(|node| node.kind == "Axis"),
        "a head has axes"
    );
    assert!(
        physical
            .beams
            .iter()
            .all(|beam| beam.position.y.abs() > 0.05),
        "the lens is not at the base: {:?}",
        physical.beams
    );
    assert!(
        physical
            .channels
            .iter()
            .any(|detail| detail.geometry.is_some())
    );
    let shutter = physical
        .channels
        .iter()
        .find(|detail| detail.attribute.starts_with("Shutter"))
        .expect("a moving head has a shutter");
    println!(
        "  shutter: {:?}",
        shutter
            .functions
            .iter()
            .map(|f| (&f.attribute, f.from, f.to, f.physical_from, f.physical_to))
            .collect::<Vec<_>>()
    );
    assert!(
        shutter.functions.len() > 1,
        "open/closed and strobe are two functions"
    );
    for wheel in &physical.wheels {
        println!(
            "  wheel {}: {:?}",
            wheel.name,
            wheel
                .slots
                .iter()
                .map(|slot| (
                    &slot.name,
                    slot.color,
                    slot.media.as_deref(),
                    slot.facets.len()
                ))
                .collect::<Vec<_>>()
        );
    }
}

/// **Every function of a channel is carried for the visualiser** — S30b.
///
/// A shutter is closed-and-open, then a strobe with a rate; a gobo's index
/// channel is an index while its master stands in one range and a rotation in
/// another. Each function keeps its attribute, its physical range and its
/// sets; a function ends where the next one **under the same master mode**
/// starts, so the index and the rotation overlap on purpose.
#[test]
fn a_channel_s_functions_are_carried_whole_for_the_visualiser() {
    let source = description(&format!(
        r#"<Geometries>
             <Geometry Name="Base" Position="{IDENTITY}">
               <Axis Name="Head" Position="{IDENTITY}">
                 <Beam Name="Beam" Position="{IDENTITY}" BeamAngle="20"/>
               </Axis>
             </Geometry>
           </Geometries>
           <DMXModes>
             <DMXMode Name="Standard" Geometry="Base">
               <DMXChannels>
                 <DMXChannel DMXBreak="1" Offset="1" Geometry="Beam">
                   <LogicalChannel Attribute="Shutter1">
                     <ChannelFunction Attribute="Shutter1" DMXFrom="0/1">
                       <ChannelSet Name="Closed" DMXFrom="0/1"/>
                       <ChannelSet Name="Open" DMXFrom="32/1"/>
                     </ChannelFunction>
                     <ChannelFunction Attribute="Shutter1Strobe" DMXFrom="64/1"
                                      PhysicalFrom="0.3" PhysicalTo="20"/>
                   </LogicalChannel>
                 </DMXChannel>
                 <DMXChannel DMXBreak="1" Offset="2" Geometry="Head">
                   <LogicalChannel Attribute="Gobo1">
                     <ChannelFunction Attribute="Gobo1" DMXFrom="0/1"/>
                     <ChannelFunction Attribute="Gobo1" DMXFrom="32/1"/>
                   </LogicalChannel>
                 </DMXChannel>
                 <DMXChannel DMXBreak="1" Offset="3,4" Geometry="Head">
                   <LogicalChannel Attribute="Gobo1Pos">
                     <ChannelFunction Attribute="Gobo1Pos" DMXFrom="0/2"
                                      PhysicalFrom="-180" PhysicalTo="180"
                                      ModeMaster="Head_Gobo1" ModeFrom="0/1" ModeTo="31/1"/>
                     <ChannelFunction Attribute="Gobo1PosRotate" DMXFrom="0/2"
                                      PhysicalFrom="-760" PhysicalTo="760"
                                      ModeMaster="Head_Gobo1" ModeFrom="32/1" ModeTo="255/1"/>
                   </LogicalChannel>
                 </DMXChannel>
               </DMXChannels>
             </DMXMode>
           </DMXModes>"#
    ));
    let (profile, _) = only_mode(&source);
    let physical = profile.physical.expect("a GDTF profile is physical");
    let channels = &physical.channels;
    assert_eq!(channels.len(), 3);

    let shutter = &channels[0];
    assert_eq!((shutter.offset, shutter.fine), (0, None));
    assert_eq!(shutter.geometry.as_deref(), Some("Beam"));
    let strobe_from = dmx_value("64/1").expect("a value");
    assert_eq!(shutter.functions[0].to, strobe_from - 1);
    assert_eq!(shutter.functions[1].attribute, "Shutter1Strobe");
    assert_eq!(shutter.functions[1].from, strobe_from);
    assert_eq!(shutter.functions[1].to, u16::MAX);
    assert!((shutter.functions[1].physical_from - 0.3).abs() < 1e-9);
    assert!((shutter.functions[1].physical_to - 20.0).abs() < 1e-9);
    let sets: Vec<(&str, u16, u16)> = shutter.functions[0]
        .sets
        .iter()
        .map(|set| (set.name.as_str(), set.from, set.to))
        .collect();
    let open_from = dmx_value("32/1").expect("a value");
    assert_eq!(
        sets,
        [
            ("Closed", 0, open_from - 1),
            ("Open", open_from, strobe_from - 1)
        ]
    );

    // Two functions under the same master mode (none) share the channel.
    let gobo = &channels[1];
    assert_eq!(gobo.functions[0].to, open_from - 1);

    // Under a master, each mode's function runs the whole channel.
    let index = &channels[2];
    assert_eq!((index.offset, index.fine), (2, Some(3)));
    for function in &index.functions {
        assert_eq!((function.from, function.to), (0, u16::MAX));
        assert_eq!(function.mode_master.as_deref(), Some("Head_Gobo1"));
    }
    assert_eq!(
        index.functions[0].mode_to,
        dmx_value("31/1").expect("a value")
    );
    assert_eq!(index.functions[1].mode_from, open_from);
    assert_eq!(index.functions[1].attribute, "Gobo1PosRotate");
}

/// **A wheel slot's colour is its CIE colour in sRGB**, and its transmission
/// is the Y — S30b. White (D65) is no filter at all; a prism's facets are where
/// each pushes its beam; and only the wheels a mode uses travel with it.
#[test]
fn a_wheel_slot_is_read_with_its_colour_and_its_facets() {
    let source = description(
        r#"<Wheels>
             <Wheel Name="Color1">
               <Slot Name="Open" Color="0.3127,0.3290,100.0"/>
               <Slot Name="Red" Color="0.6400,0.3300,21.26"/>
               <Slot Name="Blue" Color="0.1500,0.0600,7.22"/>
             </Wheel>
             <Wheel Name="Prism1">
               <Slot Name="Open"/>
               <Slot Name="3-facet">
                 <Facet Color="0.3127,0.3290,100.0" Rotation="{1,0,0}{0,1,0}{0.2,-0.1,1}"/>
                 <Facet Color="0.3127,0.3290,100.0" Rotation="{1,0,0}{0,1,0}{-0.2,-0.1,1}"/>
                 <Facet Color="0.3127,0.3290,100.0" Rotation="{1,0,0}{0,1,0}{0,0.2,1}"/>
               </Slot>
             </Wheel>
             <Wheel Name="Unused">
               <Slot Name="Nothing"/>
             </Wheel>
           </Wheels>
           <DMXModes>
             <DMXMode Name="Standard">
               <DMXChannels>
                 <DMXChannel DMXBreak="1" Offset="1">
                   <LogicalChannel Attribute="Color1">
                     <ChannelFunction Attribute="Color1" Wheel="Color1">
                       <ChannelSet DMXFrom="0/1" WheelSlotIndex="1"/>
                       <ChannelSet DMXFrom="10/1" WheelSlotIndex="2"/>
                       <ChannelSet DMXFrom="20/1" WheelSlotIndex="3"/>
                     </ChannelFunction>
                   </LogicalChannel>
                 </DMXChannel>
                 <DMXChannel DMXBreak="1" Offset="2">
                   <LogicalChannel Attribute="Prism1">
                     <ChannelFunction Attribute="Prism1" Wheel="Prism1">
                       <ChannelSet DMXFrom="0/1" WheelSlotIndex="1"/>
                       <ChannelSet DMXFrom="128/1" WheelSlotIndex="2"/>
                     </ChannelFunction>
                   </LogicalChannel>
                 </DMXChannel>
               </DMXChannels>
             </DMXMode>
           </DMXModes>"#,
    );
    let (profile, _) = only_mode(&source);
    let physical = profile.physical.expect("a GDTF profile is physical");
    let names: Vec<&str> = physical
        .wheels
        .iter()
        .map(|wheel| wheel.name.as_str())
        .collect();
    assert_eq!(names, ["Color1", "Prism1"]);

    let colours = &physical.wheels[0].slots;
    assert_eq!(colours[0].color, None, "D65 is white: no filter");
    assert!((colours[0].transmission - 1.0).abs() < 1e-9);
    let red = colours[1].color.expect("red is a colour");
    assert_eq!(red.r, 255);
    assert!(red.g < 40 && red.b < 40, "{red:?}");
    assert!((colours[1].transmission - 0.2126).abs() < 1e-9);
    let blue = colours[2].color.expect("blue is a colour");
    assert_eq!(blue.b, 255);
    assert!(blue.r < 40 && blue.g < 40, "{blue:?}");

    let prism = &physical.wheels[1].slots;
    assert!(prism[0].facets.is_empty());
    let pushes: Vec<(f64, f64)> = prism[1]
        .facets
        .iter()
        .map(|facet| (facet.x, facet.y))
        .collect();
    assert_eq!(pushes, [(0.2, -0.1), (-0.2, -0.1), (0.0, 0.2)]);
    let sets = &physical.channels[1].functions[0].sets;
    assert_eq!(sets[1].slot, Some(2));
}
