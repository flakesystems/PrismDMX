//! Calibrating a head from what its beam was seen to hit — **B70**.
//!
//! The solver is pure and is held in `prism-domain`. What this file holds is
//! the part that is the *show's*: reading a head's pan and tilt **values** as
//! angles through the profile's travel, its inverted channels and its mirrored
//! axes, and the way back — the same reading the follow layer aims through, so
//! a head driven to a mark by this and a head aimed at a tracker by that land
//! in the same place.

mod common;

use common::{attribute, fixture};
use prism_core::Show;
use prism_domain::{
    AttributeType, CalibrationAnswer, CalibrationSighting, FixtureType, Mirror, Vec3, beam,
    orientation, turn,
};

/// A moving head with a 540° pan and a 270° tilt, both centred on nought.
fn moving_head() -> FixtureType {
    let mut pan = attribute(AttributeType::Pan, 0, 32_768);
    pan.physical_from = -270.0;
    pan.physical_to = 270.0;
    let mut tilt = attribute(AttributeType::Tilt, 1, 32_768);
    tilt.physical_from = -135.0;
    tilt.physical_to = 135.0;
    FixtureType {
        id: "test.head".to_owned(),
        manufacturer: "Test".to_owned(),
        name: "Head".to_owned(),
        mode: "2ch".to_owned(),
        footprint: 2,
        attributes: vec![pan, tilt],
        physical: None,
    }
}

fn show_with(invert_pan: bool, mirror: Mirror) -> Show {
    let mut show = Show::new();
    show.embed_fixture_type(moving_head()).unwrap();
    let mut head = fixture(1, "test.head", 1, 1);
    head.invert_pan = invert_pan;
    head.mirror = mirror;
    show.patch_fixture(head).unwrap();
    show
}

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

/// What the values of a head standing in a known place put its beam through,
/// `reach` metres out, worked out from the **viewer's** angles.
fn land(show: &Show, position: Vec3, rotation: Vec3, pan: u16, tilt: u16, reach: f64) -> Vec3 {
    let (pan_axis, tilt_axis) = show.pan_tilt_axes(prism_domain::FixtureId::new(1)).unwrap();
    let direction = turn(
        &orientation(rotation),
        beam(pan_axis.degrees_of(pan), tilt_axis.degrees_of(tilt)),
    );
    v(
        position.x + direction.x * reach,
        position.y + direction.y * reach,
        position.z + direction.z * reach,
    )
}

fn sightings(show: &Show, position: Vec3, rotation: Vec3) -> Vec<CalibrationSighting> {
    [
        (30_000_u16, 24_000_u16, 6.0),
        (36_000, 26_500, 7.5),
        (42_000, 23_000, 6.5),
        (26_000, 28_000, 9.0),
        (47_000, 27_000, 8.0),
    ]
    .into_iter()
    .map(|(pan, tilt, reach)| CalibrationSighting {
        target: land(show, position, rotation, pan, tilt, reach),
        pan,
        tilt,
    })
    .collect()
}

fn solved(answer: CalibrationAnswer) -> (Vec3, Vec3, f64) {
    match answer {
        CalibrationAnswer::Solved {
            position,
            rotation,
            rms,
            ..
        } => (position, rotation, rms),
        other => panic!("not solved: {other:?}"),
    }
}

#[test]
fn a_head_is_found_where_the_values_say_it_hangs() {
    let show = show_with(false, Mirror::default());
    let truth = (v(1.5, 6.0, -2.0), v(10.0, 35.0, -3.0));
    let seen = sightings(&show, truth.0, truth.1);
    let answer = show.calibrate_fixture(
        prism_domain::FixtureId::new(1),
        v(1.0, 5.5, -1.5),
        Vec3::ZERO,
        false,
        &seen,
    );
    let (position, rotation, rms) = solved(answer);
    assert!(rms < 1e-4, "rms {rms}");
    assert!((position.x - truth.0.x).abs() < 1e-3, "{position:?}");
    assert!((position.y - truth.0.y).abs() < 1e-3, "{position:?}");
    assert!((position.z - truth.0.z).abs() < 1e-3, "{position:?}");
    let probe = beam(20.0, 45.0);
    let found = turn(&orientation(rotation), probe);
    let real = turn(&orientation(truth.1), probe);
    assert!((found.x - real.x).abs() < 1e-4 && (found.z - real.z).abs() < 1e-4);
}

/// The same hang read through an inverted cable and a mirrored motor: the values
/// the operator typed are different, the pose they give back is not. If this
/// reading and the follow layer's differed, one of them would put a head where
/// the other had calibrated it away from.
#[test]
fn an_inverted_cable_and_a_mirrored_motor_give_the_same_pose() {
    let truth = (v(-2.0, 7.0, 1.0), v(0.0, 50.0, 0.0));
    for (invert_pan, mirror) in [
        (false, Mirror::default()),
        (true, Mirror::default()),
        (
            false,
            Mirror {
                pan: true,
                tilt: false,
            },
        ),
        (
            false,
            Mirror {
                pan: false,
                tilt: true,
            },
        ),
        (
            true,
            Mirror {
                pan: true,
                tilt: true,
            },
        ),
    ] {
        let show = show_with(invert_pan, mirror);
        let seen = sightings(&show, truth.0, truth.1);
        let answer = show.calibrate_fixture(
            prism_domain::FixtureId::new(1),
            truth.0,
            truth.1,
            true,
            &seen,
        );
        let (_, _, rms) = solved(answer);
        assert!(
            rms < 1e-4,
            "invert {invert_pan} mirror {mirror:?}: rms {rms}"
        );
    }
}

/// The two directions are one reading: drive a head to a mark by the aim, and
/// the beam is on the mark.
#[test]
fn the_aim_puts_the_beam_on_the_mark() {
    let position = v(0.5, 6.5, -1.0);
    let rotation = v(5.0, -20.0, 2.0);
    for (invert_pan, mirror) in [
        (false, Mirror::default()),
        (
            true,
            Mirror {
                pan: false,
                tilt: true,
            },
        ),
    ] {
        let show = show_with(invert_pan, mirror);
        let id = prism_domain::FixtureId::new(1);
        for target in [v(2.0, 0.0, 3.0), v(-3.0, 0.0, 1.0), v(0.0, 0.0, -4.0)] {
            let aimed = show
                .calibration_aim(id, target, position, rotation, None)
                .unwrap();
            assert!(aimed.reached);
            let landed = land(&show, position, rotation, aimed.pan, aimed.tilt, 1.0);
            let to = v(
                target.x - position.x,
                target.y - position.y,
                target.z - position.z,
            );
            let length = (to.x * to.x + to.y * to.y + to.z * to.z).sqrt();
            let along = v(
                (landed.x - position.x) * length,
                (landed.y - position.y) * length,
                (landed.z - position.z) * length,
            );
            // 16 bits of a 540° pan is 0.008° a step: a few millimetres at 8 m.
            assert!(
                (along.x - to.x).abs() < 0.05
                    && (along.y - to.y).abs() < 0.05
                    && (along.z - to.z).abs() < 0.05,
                "{target:?}: {along:?} vs {to:?}"
            );
        }
    }
}

#[test]
fn a_fixture_with_no_pan_and_tilt_cannot_be_calibrated_or_driven() {
    let mut show = Show::new();
    show.embed_fixture_type(common::par_type()).unwrap();
    show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
        .unwrap();
    let id = prism_domain::FixtureId::new(1);
    assert!(
        show.calibration_aim(id, v(1.0, 0.0, 1.0), Vec3::ZERO, Vec3::ZERO, None)
            .is_none()
    );
    assert!(matches!(
        show.calibrate_fixture(id, Vec3::ZERO, Vec3::ZERO, false, &[]),
        CalibrationAnswer::Refused { .. }
    ));
    assert!(matches!(
        show.calibrate_fixture(
            prism_domain::FixtureId::new(9),
            Vec3::ZERO,
            Vec3::ZERO,
            false,
            &[]
        ),
        CalibrationAnswer::Refused { .. }
    ));
}

#[test]
fn too_few_sightings_say_how_many_are_needed_in_words() {
    let show = show_with(false, Mirror::default());
    let seen = sightings(&show, v(0.0, 6.0, 0.0), Vec3::ZERO);
    let answer = show.calibrate_fixture(
        prism_domain::FixtureId::new(1),
        v(0.0, 6.0, 0.0),
        Vec3::ZERO,
        false,
        &seen[..2],
    );
    match answer {
        CalibrationAnswer::Refused { reason } => {
            assert!(reason.contains("at least 3"), "{reason}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_calibration_changes_nothing_in_the_show() {
    let show = show_with(false, Mirror::default());
    let before = rmp_serde::to_vec_named(&show).unwrap();
    let seen = sightings(&show, v(0.0, 6.0, 0.0), Vec3::ZERO);
    let _ = show.calibrate_fixture(
        prism_domain::FixtureId::new(1),
        v(0.2, 6.0, 0.0),
        Vec3::ZERO,
        false,
        &seen,
    );
    let _ = show.calibration_aim(
        prism_domain::FixtureId::new(1),
        v(1.0, 0.0, 1.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Some((10, 10)),
    );
    assert_eq!(rmp_serde::to_vec_named(&show).unwrap(), before);
}
