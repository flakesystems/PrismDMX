//! The follow layer: a head that is pointed at its tracker - **S32**.
//!
//! `ARCHITECTURE_SPEC.md` §8. A fixture the show has given a tracker
//! (`prism_domain::Fixture::follow`) and whose *Follow* value
//! (`prism_domain::AttributeType::Follow`) is above nought is aimed at where that
//! tracker is, and the aim is the **way back from the viewer's yoke**
//! (`prism_domain::aim`): pan and tilt that put the beam through the point.
//!
//! # Where it sits, and what it does not beat
//!
//! Between the playbacks and the programmer. It runs after the playback merge
//! has produced a pan and a tilt and **mixes the aim into them by the Follow
//! value**: at nought the cues' pan and tilt stand, at full the tracker's aim
//! replaces them, and between the two the head is part of the way - which is
//! what a cue's fade time does when it turns following on, and why *Follow* is a
//! fraction rather than a switch.
//!
//! The programmer beats it, as it beats every playback (`docs/DMX_MERGE.md`
//! §3): **a pan or a tilt the operator is holding is left alone**, whatever the
//! Follow value says. The operator's own *Follow* value is read the same way -
//! it is the programmer's if the programmer holds one, the playbacks' if not -
//! so putting a head on its tracker by hand is one value, and `Clear` hands it
//! back to the cues.
//!
//! # What it reads
//!
//! The [`TrackerTable`], once per head per tick, and nothing else that moves.
//! Everything that is a function of the show - where the head hangs, which way
//! it faces, its pan and tilt travel, which merge slots those are - is resolved
//! when the layer is **built**, on the core thread, and arrives behind the tick as
//! a table (§3.1, *nothing derived is computed here - it is read*). The tick does
//! the trigonometry of one aim per head per tick, which is a handful of `sin`s and
//! allocates nothing.
//!
//! A tracker nobody has heard has no position, and a head following it is left
//! to the cues - there is no point at which it could be aimed. One that went
//! quiet **keeps its last position** in the table and the head stays aimed at
//! it, which is the specification's *hold, and do not jump*.

use prism_domain::{Aim, AttributeType, Fixture, FixtureType, Orientation, Travel, Vec3, aim};

use crate::cue::interpolate;
use crate::plan::MergePlan;
use crate::programmer::ProgrammerLayer;
use crate::sync::Arc;
use crate::tracker::TrackerTable;

/// One axis of one head: where its value lives and what it means.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Axis {
    /// The merge slot.
    slot: usize,
    /// The travel in degrees: value `0` and value `65535`.
    travel: Travel,
    /// Whether the encoder will invert this channel on the way out.
    ///
    /// The merge works **before** the invert and the head turns **after** it, so
    /// to put the head at an angle the value to ask for is the mirror of the
    /// angle's own when the channel is inverted. Mixing happens on the
    /// pre-invert values, and the mirror is exact over a mix.
    invert: bool,
    /// Whether this axis turns the other way from the viewer's (`Mirror`).
    ///
    /// The aim works in the **viewer's** angles - the ones `aim` is the inverse
    /// of - so a mirrored axis's real angle is the negative of the one the aim
    /// asks for. Unlike `invert` it is not undone by the encoder: it is a fact
    /// about the head and not about the cable.
    mirrored: bool,
}

impl Axis {
    /// The merge value that puts the axis at `degrees` (the viewer's angle)
    /// once the encoder has had its say.
    fn value_for(&self, degrees: f64) -> u16 {
        let real = if self.mirrored { -degrees } else { degrees };
        let value = self.travel.value(real);
        if self.invert { u16::MAX - value } else { value }
    }

    /// The viewer's angle a merge value turns the axis to.
    fn degrees_of(&self, value: u16) -> f64 {
        let real = self
            .travel
            .degrees(if self.invert { u16::MAX - value } else { value });
        if self.mirrored { -real } else { real }
    }

    /// The travel in the viewer's angles, which is what `aim` chooses within.
    fn model_travel(&self) -> Travel {
        if self.mirrored {
            Travel {
                from: -self.travel.to,
                to: -self.travel.from,
            }
        } else {
            self.travel
        }
    }
}

/// One head that follows a tracker, and everything about it that is decided
/// before the tick.
#[derive(Debug, Clone)]
struct Head {
    follow: usize,
    pan: Axis,
    tilt: Axis,
    tracker: u16,
    position: Vec3,
    orientation: Orientation,
    offset: Vec3,
    /// Where the head was pointed on the last tick it was aimed, which is what
    /// the next aim is chosen nearest to - so a head does not spin the long way
    /// round because a performer crossed the line behind it.
    aimed: Option<(f64, f64)>,
}

/// Every head the show has given a tracker, and the table they read.
///
/// Built on the core thread by [`Self::build`] and handed to the tick whole.
#[derive(Debug, Clone)]
pub struct FollowLayer {
    heads: Box<[Head]>,
    trackers: Arc<TrackerTable>,
}

impl FollowLayer {
    /// A layer with no heads on it, over a table nobody writes.
    ///
    /// What a body has until it is given one, and what a rig with no tracker
    /// assigned has for good: [`Self::apply`] is then a loop over nothing.
    #[must_use]
    pub fn none() -> Self {
        Self {
            heads: Box::new([]),
            trackers: Arc::new(TrackerTable::new()),
        }
    }

    /// Resolves the show's follow assignments against a plan.
    ///
    /// Each entry is a patched fixture and its type. A fixture that has no
    /// tracker, whose type cannot be aimed, or whose pan, tilt or *Follow* the
    /// plan does not hold is **left out** rather than refused: one head that
    /// cannot follow is a head that does not, and the rest of the rig is
    /// unaffected. Allocates - set-up work, not the tick's.
    #[must_use]
    pub fn build<'a, I>(plan: &MergePlan, fixtures: I, trackers: Arc<TrackerTable>) -> Self
    where
        I: IntoIterator<Item = (&'a Fixture, &'a FixtureType)>,
    {
        let mut heads = Vec::new();
        for (fixture, fixture_type) in fixtures {
            let Some(target) = fixture.follow else {
                continue;
            };
            let slot = |attribute: AttributeType| plan.index_of(fixture.id, attribute.into());
            let (Some(follow), Some(pan), Some(tilt)) = (
                slot(AttributeType::Follow),
                slot(AttributeType::Pan),
                slot(AttributeType::Tilt),
            ) else {
                continue;
            };
            let axis =
                |attribute: AttributeType, slot: usize, fixture_invert: bool, mirrored: bool| {
                    fixture_type
                        .attributes
                        .iter()
                        .find(|def| def.attribute == attribute && def.occurrence == 0)
                        .map(|def| Axis {
                            slot,
                            travel: Travel {
                                from: def.physical_from,
                                to: def.physical_to,
                            },
                            invert: def.invert ^ fixture_invert,
                            mirrored,
                        })
                };
            let (Some(pan), Some(tilt)) = (
                axis(
                    AttributeType::Pan,
                    pan,
                    fixture.invert_pan,
                    fixture.mirror.pan,
                ),
                axis(
                    AttributeType::Tilt,
                    tilt,
                    fixture.invert_tilt,
                    fixture.mirror.tilt,
                ),
            ) else {
                continue;
            };
            heads.push(Head {
                follow,
                pan,
                tilt,
                tracker: target.tracker,
                position: fixture.position,
                orientation: prism_domain::orientation(fixture.rotation),
                offset: target.offset,
                aimed: None,
            });
        }
        Self {
            heads: heads.into_boxed_slice(),
            trackers,
        }
    }

    /// How many heads are following a tracker.
    #[must_use]
    pub fn len(&self) -> usize {
        self.heads.len()
    }

    /// Whether no head is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.heads.is_empty()
    }

    /// The table this layer reads.
    #[must_use]
    pub const fn trackers(&self) -> &Arc<TrackerTable> {
        &self.trackers
    }

    /// Carries what a replaced layer was doing over to this one, so a head that
    /// is already aimed does not start from nowhere.
    ///
    /// The *nearest way round* rule needs to know where each head points; a
    /// layer built because somebody moved one fixture should not make every
    /// other follower forget. Matching is by merge slot, which is stable for as
    /// long as the patch is - and a patch that changed rebuilds the body whole.
    pub fn inherit(&mut self, from: &Self) {
        for head in &mut *self.heads {
            if let Some(old) = from.heads.iter().find(|old| old.follow == head.follow) {
                head.aimed = old.aimed;
            }
        }
    }

    /// Mixes each following head's aim into the pan and tilt the playbacks
    /// resolved. Allocation-free: the tick's work.
    ///
    /// `values` is the merge as it stands after the playbacks and **before** the
    /// programmer is applied; `programmer` is read for what it holds and not
    /// written.
    pub fn apply(&mut self, values: &mut [u16], programmer: &ProgrammerLayer) {
        for head in &mut *self.heads {
            let Some(&playback_follow) = values.get(head.follow) else {
                continue;
            };
            let amount = programmer.get(head.follow).unwrap_or(playback_follow);
            if amount == 0 {
                // Not following: forget where it pointed, so that the next time
                // it follows it starts from what the cues have it doing.
                head.aimed = None;
                continue;
            }
            let Some(tracker) = self.trackers.read(head.tracker) else {
                continue;
            };
            let target = Vec3 {
                x: tracker.x + head.offset.x,
                y: tracker.y + head.offset.y,
                z: tracker.z + head.offset.z,
            };
            // Where it points now, for choosing between two ways of arriving.
            let near = head.aimed.unwrap_or_else(|| {
                let pan = values.get(head.pan.slot).copied().unwrap_or(0);
                let tilt = values.get(head.tilt.slot).copied().unwrap_or(0);
                (head.pan.degrees_of(pan), head.tilt.degrees_of(tilt))
            });
            let Some(Aim { pan, tilt, .. }) = aim(
                head.position,
                &head.orientation,
                target,
                head.pan.model_travel(),
                head.tilt.model_travel(),
                near,
            ) else {
                continue;
            };
            head.aimed = Some((pan, tilt));
            for (axis, degrees) in [(head.pan, pan), (head.tilt, tilt)] {
                // What the operator is holding on this axis is theirs.
                if programmer.get(axis.slot).is_some() {
                    continue;
                }
                if let Some(value) = values.get_mut(axis.slot) {
                    *value = interpolate(
                        *value,
                        axis.value_for(degrees),
                        u64::from(amount),
                        u64::from(u16::MAX),
                    );
                }
            }
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use prism_domain::{
        AttributeKey, AttributeType, Fixture, FixtureId, FixtureType, FollowTarget, Vec3,
    };

    use super::FollowLayer;
    use crate::plan::MergePlan;
    use crate::programmer::ProgrammerLayer;
    use crate::sync::Arc;
    use crate::testkit::{aimable_head, fixture};
    use crate::tracker::TrackerTable;

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    /// A moving head at `(0, 6, 0)` hanging at nought, following tracker `1`.
    fn rig() -> (Fixture, FixtureType) {
        let head = aimable_head();
        let mut following = fixture(1, &head.id, 0, 1);
        following.position = v(0.0, 6.0, 0.0);
        following.follow = Some(FollowTarget {
            tracker: 1,
            offset: Vec3::ZERO,
        });
        (following, head)
    }

    struct Bench {
        plan: MergePlan,
        layer: FollowLayer,
        table: Arc<TrackerTable>,
        values: Vec<u16>,
        programmer: ProgrammerLayer,
    }

    impl Bench {
        fn new() -> Self {
            Self::mirrored(prism_domain::Mirror::default())
        }

        fn mirrored(mirror: prism_domain::Mirror) -> Self {
            let (mut fixture, head) = rig();
            fixture.mirror = mirror;
            let plan = MergePlan::build([(fixture.id, &head, false)]).unwrap();
            let table = Arc::new(TrackerTable::new());
            let layer = FollowLayer::build(&plan, [(&fixture, &head)], Arc::clone(&table));
            let values = plan.slots().iter().map(|slot| slot.home).collect();
            let programmer = ProgrammerLayer::new(&plan);
            Self {
                plan,
                layer,
                table,
                values,
                programmer,
            }
        }

        fn slot(&self, attribute: AttributeType) -> usize {
            self.plan
                .index_of(FixtureId::new(1), AttributeKey::first(attribute))
                .unwrap()
        }

        fn set(&mut self, attribute: AttributeType, value: u16) {
            let slot = self.slot(attribute);
            self.values[slot] = value;
        }

        fn get(&self, attribute: AttributeType) -> u16 {
            self.values[self.slot(attribute)]
        }

        fn run(&mut self) {
            self.layer.apply(&mut self.values, &self.programmer);
        }
    }

    #[test]
    fn a_head_with_a_tracker_is_a_head_on_the_layer_and_one_without_is_not() {
        let head = aimable_head();
        let plain = fixture(2, &head.id, 0, 20);
        let plan = MergePlan::build([(plain.id, &head, false)]).unwrap();
        let layer = FollowLayer::build(&plan, [(&plain, &head)], Arc::new(TrackerTable::new()));
        assert!(layer.is_empty());
        assert_eq!(Bench::new().layer.len(), 1);
    }

    #[test]
    fn follow_at_nought_leaves_pan_and_tilt_to_the_cues() {
        let mut bench = Bench::new();
        bench.table.publish(1, v(3.0, 0.0, 0.0));
        bench.set(AttributeType::Pan, 1000);
        bench.set(AttributeType::Tilt, 2000);
        bench.run();
        assert_eq!(bench.get(AttributeType::Pan), 1000);
        assert_eq!(bench.get(AttributeType::Tilt), 2000);
    }

    #[test]
    fn follow_at_full_puts_the_head_on_the_tracker() {
        let mut bench = Bench::new();
        // Straight below the head: tilt nought, the middle of the tilt travel.
        bench.table.publish(1, v(0.0, 0.0, 0.0));
        bench.set(AttributeType::Follow, u16::MAX);
        bench.set(AttributeType::Pan, 5);
        bench.set(AttributeType::Tilt, 5);
        bench.run();
        assert!(
            bench.get(AttributeType::Tilt).abs_diff(32768) <= 1,
            "{}",
            bench.get(AttributeType::Tilt)
        );
    }

    #[test]
    fn follow_part_way_is_part_of_the_way() {
        let mut bench = Bench::new();
        bench.table.publish(1, v(0.0, 0.0, -6.0)); // downstage: tilt +45
        bench.set(AttributeType::Tilt, 0);
        bench.set(AttributeType::Follow, 32768);
        bench.run();
        let half = bench.get(AttributeType::Tilt);

        let mut full = Bench::new();
        full.table.publish(1, v(0.0, 0.0, -6.0));
        full.set(AttributeType::Tilt, 0);
        full.set(AttributeType::Follow, u16::MAX);
        full.run();
        let all = full.get(AttributeType::Tilt);
        assert!(all > half && half > 0, "{half} of {all}");
        assert!(half.abs_diff(all / 2) <= 2, "{half} of {all}");
    }

    /// **A head whose motor runs the other way is aimed the other way**, so the
    /// viewer - which draws it mirrored - and the real head agree about where it
    /// points. The travel is symmetric, so the mirror of an angle is the mirror
    /// of its value.
    #[test]
    fn a_mirrored_axis_is_aimed_the_other_way() {
        let aimed = |mirror| {
            let mut bench = Bench::mirrored(mirror);
            bench.table.publish(1, v(2.0, 0.0, -2.0));
            bench.set(AttributeType::Follow, u16::MAX);
            bench.run();
            (
                bench.get(AttributeType::Pan),
                bench.get(AttributeType::Tilt),
            )
        };
        let (pan, tilt) = aimed(prism_domain::Mirror::default());
        assert!(
            pan.abs_diff(32768) > 1_000,
            "a pan that is not the middle: {pan}"
        );
        assert!(
            tilt.abs_diff(32768) > 1_000,
            "a tilt that is not the middle: {tilt}"
        );

        let (mirrored_pan, same_tilt) = aimed(prism_domain::Mirror {
            pan: true,
            tilt: false,
        });
        assert!(
            u32::from(pan).abs_diff(u32::from(u16::MAX) - u32::from(mirrored_pan)) <= 2,
            "{pan} and {mirrored_pan}"
        );
        assert_eq!(same_tilt, tilt, "tilt is not mirrored");

        let (same_pan, mirrored_tilt) = aimed(prism_domain::Mirror {
            pan: false,
            tilt: true,
        });
        assert_eq!(same_pan, pan, "pan is not mirrored");
        assert!(
            u32::from(tilt).abs_diff(u32::from(u16::MAX) - u32::from(mirrored_tilt)) <= 2,
            "{tilt} and {mirrored_tilt}"
        );
    }

    #[test]
    fn a_tracker_nobody_has_heard_leaves_the_head_to_the_cues() {
        let mut bench = Bench::new();
        bench.set(AttributeType::Follow, u16::MAX);
        bench.set(AttributeType::Pan, 111);
        bench.run();
        assert_eq!(bench.get(AttributeType::Pan), 111);
    }

    #[test]
    fn a_tracker_that_goes_quiet_leaves_the_head_where_it_was() {
        let mut bench = Bench::new();
        bench.table.publish(1, v(2.0, 0.0, -2.0));
        bench.set(AttributeType::Follow, u16::MAX);
        bench.run();
        let (pan, tilt) = (
            bench.get(AttributeType::Pan),
            bench.get(AttributeType::Tilt),
        );
        // Many ticks and no new position: the head does not move and does not
        // jump, because there is nothing to jump to.
        for _ in 0..200 {
            bench.set(AttributeType::Pan, 0);
            bench.set(AttributeType::Tilt, 0);
            bench.run();
            assert_eq!(bench.get(AttributeType::Pan), pan);
            assert_eq!(bench.get(AttributeType::Tilt), tilt);
        }
    }

    #[test]
    fn what_the_programmer_holds_on_an_axis_is_not_overwritten() {
        let mut bench = Bench::new();
        bench.table.publish(1, v(0.0, 0.0, -6.0));
        bench.set(AttributeType::Follow, u16::MAX);
        let pan = bench.slot(AttributeType::Pan);
        bench.programmer.set(pan, 777);
        bench.set(AttributeType::Pan, 777);
        bench.set(AttributeType::Tilt, 0);
        bench.run();
        assert_eq!(
            bench.get(AttributeType::Pan),
            777,
            "the operator's pan stands"
        );
        assert!(
            bench.get(AttributeType::Tilt) > 0,
            "and the tilt, which they did not touch, follows"
        );
    }

    #[test]
    fn the_programmers_own_follow_value_is_the_one_that_counts() {
        let mut bench = Bench::new();
        bench.table.publish(1, v(0.0, 0.0, -6.0));
        // The cues say follow, the operator says no.
        bench.set(AttributeType::Follow, u16::MAX);
        let follow = bench.slot(AttributeType::Follow);
        bench.programmer.set(follow, 0);
        bench.set(AttributeType::Tilt, 100);
        bench.run();
        assert_eq!(bench.get(AttributeType::Tilt), 100);
    }

    #[test]
    fn an_inverted_channel_is_pointed_through_its_mirror() {
        let (mut fixture, head) = rig();
        fixture.invert_tilt = true;
        let plan = MergePlan::build([(fixture.id, &head, false)]).unwrap();
        let table = Arc::new(TrackerTable::new());
        table.publish(1, v(0.0, 0.0, -6.0));
        let mut layer = FollowLayer::build(&plan, [(&fixture, &head)], Arc::clone(&table));
        let slot = |attribute| {
            plan.index_of(FixtureId::new(1), AttributeKey::first(attribute))
                .unwrap()
        };
        let mut values: Vec<u16> = plan.slots().iter().map(|slot| slot.home).collect();
        values[slot(AttributeType::Follow)] = u16::MAX;
        layer.apply(&mut values, &ProgrammerLayer::new(&plan));
        let inverted = values[slot(AttributeType::Tilt)];

        let mut plain = Bench::new();
        plain.table.publish(1, v(0.0, 0.0, -6.0));
        plain.set(AttributeType::Follow, u16::MAX);
        plain.run();
        assert_eq!(
            inverted,
            u16::MAX - plain.get(AttributeType::Tilt),
            "the encoder will mirror it back"
        );
    }

    #[test]
    fn a_layer_built_over_an_old_one_remembers_where_each_head_pointed() {
        let mut bench = Bench::new();
        bench.table.publish(1, v(0.0, 0.0, -6.0));
        bench.set(AttributeType::Follow, u16::MAX);
        bench.run();
        let (fixture, head) = rig();
        let mut next =
            FollowLayer::build(&bench.plan, [(&fixture, &head)], Arc::clone(&bench.table));
        assert!(next.heads.first().unwrap().aimed.is_none());
        next.inherit(&bench.layer);
        assert!(next.heads.first().unwrap().aimed.is_some());
    }
}
