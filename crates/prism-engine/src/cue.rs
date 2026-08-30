//! Cues, compiled into the form the tick can run.
//!
//! `ARCHITECTURE_SPEC.md` §5 steps 2 and 3: advance the fades, evaluate the
//! executors into attribute values. This module is the *plan* half of that — the
//! part that can be worked out before the tick starts — and `crate::player` is
//! the part that runs.
//!
//! # Why a cue cannot enter the tick
//!
//! A [`prism_domain::Cue`] owns a `String` for its number, a `String` for its
//! name and a `Vec` of parts. `ARCHITECTURE_SPEC.md` §3.1 forbids allocation
//! inside the tick, and **dropping** an owned field allocates exactly as surely
//! as creating one — the mistake S2 already met with `TickCommand`. So a cue is
//! compiled once, off the tick: fixtures and attributes become slot indices into
//! the [`MergePlan`], seconds become whole ticks, and what is left is flat,
//! `Copy` and indexed.
//!
//! # The shape of a compiled sequence
//!
//! ```text
//!   SequencePlan
//!     slots  [CueSlot]   one per plan slot the *whole sequence* touches
//!     cues   [CuePlan]   in cue-number order, each naming a range of parts
//!     parts  [CueValue]  (slot index, value), grouped by cue
//! ```
//!
//! The `slots` table is what bounds the player's working memory: a playback
//! needs one entry per attribute its own sequence can touch, not one per
//! attribute in the show. It also carries each slot's home value and merge mode,
//! so releasing a playback needs no lookup back into the merge plan.

use core::fmt;
use std::collections::BTreeMap;

use prism_domain::{
    Cue, CueChange, CueTrack, CueTrigger, GoDirection, MergeMode, PlaybackId, Sequence, SequenceId,
};

use crate::plan::MergePlan;
use crate::tick::TICK_HZ;

/// Upper bound on cues in one sequence.
///
/// Not a product limit — no show has ten thousand cues in one list. It exists so
/// that a corrupt show becomes a rejected sequence rather than an allocation
/// nobody asked for.
pub const MAX_CUES: usize = 10_000;

/// Upper bound on the total number of resolved parts in one sequence.
pub const MAX_CUE_PARTS: usize = 1_048_576;

/// Why a sequence cannot be compiled or loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueError {
    /// More cues than [`MAX_CUES`].
    TooManyCues(usize),
    /// More parts than [`MAX_CUE_PARTS`].
    TooManyParts(usize),
    /// The sequence was loaded onto a playback this engine does not have.
    UnknownPlayback(PlaybackId),
}

impl fmt::Display for CueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCues(count) => {
                write!(f, "{count} cues exceeds the limit of {MAX_CUES}")
            }
            Self::TooManyParts(count) => {
                write!(f, "{count} cue parts exceeds the limit of {MAX_CUE_PARTS}")
            }
            Self::UnknownPlayback(playback) => {
                write!(f, "{playback} is not in this patch")
            }
        }
    }
}

impl std::error::Error for CueError {}

/// Ticks in `seconds`, rounded to the nearest whole tick.
///
/// The tick is the time base for everything (`ARCHITECTURE_SPEC.md` §3.2), so a
/// cue time is converted once, when the sequence is compiled, and no float ever
/// reaches the tick. Rounds rather than truncating: truncation would bias every
/// time in the show the same way.
///
/// Monotone in its argument, including at the edges: a time that is negative,
/// zero or not a number is no fade at all, and one too large to count — up to
/// and including infinity — saturates rather than wrapping. `prism-domain`
/// refuses non-finite times on the wire, so neither edge can arrive from a show
/// file; this function is public and must not be able to panic on one anyway.
#[must_use]
pub fn ticks_from_seconds(seconds: f64) -> u64 {
    if seconds.is_nan() || seconds <= 0.0 {
        return 0;
    }
    // `as` on a float saturates at the integer bounds and maps NaN to zero, so
    // an absurd fade time becomes a fade nobody outlives rather than a short one.
    (seconds * TICK_HZ as f64).round() as u64
}

/// Where a fade has got to: `from` at `elapsed` 0, `to` from `duration` onwards.
///
/// Sixteen-bit throughout, whatever resolution the attribute is patched at.
/// `docs/DMX_MERGE.md` §5: an 8-bit dimmer fading over ten seconds still moves
/// in 16-bit steps and only quantises at the final write, which is what keeps a
/// slow fade from stepping visibly.
///
/// A duration of zero is a fade that is already finished, which is what a cue
/// with no fade time means.
#[must_use]
pub fn interpolate(from: u16, to: u16, elapsed: u64, duration: u64) -> u16 {
    if elapsed >= duration {
        return to;
    }
    // `elapsed < duration`, so the quotient is below the span and cannot
    // overflow the u16 either way round. u128 because `duration` is a tick count
    // and a show file may legitimately ask for a very long fade.
    let span = u128::from(from.abs_diff(to));
    let moved = (span * u128::from(elapsed)).div_euclid(u128::from(duration)) as u16;
    if to >= from {
        from.saturating_add(moved)
    } else {
        from.saturating_sub(moved)
    }
}

/// One plan slot a sequence touches, with everything the player needs about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CueSlot {
    /// Index into the [`MergePlan`] this sequence was compiled against.
    pub slot: usize,
    /// The value this attribute falls back to — where a release fades it to.
    pub home: u16,
    /// Whether the attribute merges HTP. Only an intensity is faded out on
    /// release; `docs/DMX_MERGE.md` §2.3 is the same asymmetry.
    pub htp: bool,
}

/// One value a cue provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CueValue {
    /// Index into [`SequencePlan::slots`] — **not** into the merge plan.
    pub slot: u32,
    /// The value to fade to, `0..=65535`.
    pub value: u16,
}

/// Where a slot's **tracked** value changes, and what it changes to — S48.
///
/// One of these per cue at which the value a walk from the top of the list would
/// leave a slot at moves. Everything between two of them is the same value, so a
/// list of a thousand cues over a slot two of them touch is two entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackPoint {
    /// The cue, by playback index, from which the value below is in force.
    pub cue: u32,
    /// What the list holds the slot at from that cue on.
    ///
    /// `None` is **not held at all** — a `prism_domain::CueTracking::CueOnly`
    /// value taken back with nothing underneath it. A slot a playback does not
    /// provide falls through to whatever is below it in the merge, which is a
    /// different thing from being held at zero.
    pub value: Option<u16>,
}

/// One cue, compiled: times in ticks, parts as a range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuePlan {
    number: Box<str>,
    fade_in: u64,
    fade_out: u64,
    delay: u64,
    trigger: CueTrigger,
    trigger_ticks: Option<u64>,
    first: usize,
    len: usize,
}

impl CuePlan {
    /// The cue number as the operator typed it.
    #[must_use]
    pub fn number(&self) -> &str {
        &self.number
    }

    /// Fade-in time in ticks: how long this cue takes to reach its values.
    #[must_use]
    pub const fn fade_in(&self) -> u64 {
        self.fade_in
    }

    /// Fade-out time in ticks: how long this cue's intensities take to go away
    /// when the playback is switched off.
    #[must_use]
    pub const fn fade_out(&self) -> u64 {
        self.fade_out
    }

    /// Delay in ticks before the fade starts.
    #[must_use]
    pub const fn delay(&self) -> u64 {
        self.delay
    }

    /// What starts this cue.
    #[must_use]
    pub const fn trigger(&self) -> CueTrigger {
        self.trigger
    }

    /// Trigger time in ticks, for [`CueTrigger::Time`].
    #[must_use]
    pub const fn trigger_ticks(&self) -> Option<u64> {
        self.trigger_ticks
    }

    /// Ticks from this cue's start to the moment it has finished fading — what
    /// a [`CueTrigger::Follow`] on the next cue waits for.
    #[must_use]
    pub const fn transition_ticks(&self) -> u64 {
        self.delay.saturating_add(self.fade_in)
    }
}

/// A cue list, compiled against one patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequencePlan {
    id: SequenceId,
    slots: Box<[CueSlot]>,
    cues: Box<[CuePlan]>,
    parts: Box<[CueValue]>,
    /// The tracking table, grouped by slot and ordered by cue within a slot.
    track: Box<[TrackPoint]>,
    /// Where each slot's run of [`Self::track`] starts; `slot_count + 1` long,
    /// so a slot's range is `track_index[i]..track_index[i + 1]` and the last
    /// one needs no special case.
    track_index: Box<[u32]>,
    looping: bool,
    unresolved: usize,
}

impl SequencePlan {
    /// Compiles a sequence against a patch.
    ///
    /// Cues are ordered by [`Cue::compare_numbers`], not by their position in
    /// the list: `1`, `1.5`, `2`, `10` is the order an operator reads, and
    /// inserting a cue between two others is the entire reason cue numbers are
    /// decimal strings.
    ///
    /// A part naming a fixture or attribute this patch does not have is dropped
    /// and counted in [`Self::unresolved`]. A show outlives the rig it was
    /// written on, and refusing to run a cue list because one fixture was
    /// unpatched would take the show down rather than one light.
    ///
    /// # Errors
    ///
    /// [`CueError::TooManyCues`] or [`CueError::TooManyParts`] if the sequence
    /// is implausibly large.
    pub fn build(plan: &MergePlan, sequence: &Sequence) -> Result<Self, CueError> {
        if sequence.cues.len() > MAX_CUES {
            return Err(CueError::TooManyCues(sequence.cues.len()));
        }

        let mut order: Vec<&Cue> = sequence.cues.iter().collect();
        // Stable, so two cues with the same number keep their file order.
        order.sort_by(|left, right| Cue::compare_numbers(&left.number, &right.number));

        // Every slot the whole sequence touches, once, in merge-plan order.
        let mut slots: BTreeMap<usize, CueSlot> = BTreeMap::new();
        let mut unresolved = 0usize;
        let mut resolved = 0usize;
        for cue in &order {
            for part in &cue.parts {
                let Some((index, slot)) = plan
                    .index_of(part.fixture, part.attribute)
                    .and_then(|index| Some((index, plan.slot(index)?)))
                else {
                    unresolved += 1;
                    continue;
                };
                resolved += 1;
                if resolved > MAX_CUE_PARTS {
                    return Err(CueError::TooManyParts(resolved));
                }
                slots.entry(index).or_insert(CueSlot {
                    slot: index,
                    home: slot.home,
                    htp: slot.merge_mode == MergeMode::Htp,
                });
            }
        }
        let positions: BTreeMap<usize, u32> = slots
            .keys()
            .enumerate()
            .map(|(position, slot)| (*slot, position as u32))
            .collect();

        let mut cues: Vec<CuePlan> = Vec::with_capacity(order.len());
        let mut parts: Vec<CueValue> = Vec::with_capacity(resolved);
        let mut scratch: BTreeMap<u32, u16> = BTreeMap::new();
        for cue in &order {
            scratch.clear();
            for part in &cue.parts {
                let Some(position) = plan
                    .index_of(part.fixture, part.attribute)
                    .and_then(|index| positions.get(&index))
                else {
                    continue;
                };
                // Later wins: a cue that names one attribute twice says the
                // second thing, exactly as a second keystroke would.
                scratch.insert(*position, part.value);
            }
            let first = parts.len();
            parts.extend(scratch.iter().map(|(slot, value)| CueValue {
                slot: *slot,
                value: *value,
            }));
            cues.push(CuePlan {
                number: cue.number.clone().into_boxed_str(),
                fade_in: ticks_from_seconds(cue.fade_in),
                fade_out: ticks_from_seconds(cue.fade_out),
                delay: ticks_from_seconds(cue.delay),
                trigger: cue.trigger,
                trigger_ticks: cue.trigger_time.map(ticks_from_seconds),
                first,
                len: parts.len() - first,
            });
        }

        // **The tracking table** — S48. Built here, on the core thread, because
        // this is where a sequence is compiled and a sequence is compiled
        // whenever it changes: at load, and again when a cue is stored, edited,
        // deleted, renumbered or moved. The tick reads it; the tick never builds
        // it.
        //
        // It is held **by slot** rather than by cue, and that is the whole
        // reason it fits. A cue-by-slot table is `cues x slots` cells — forty
        // megabytes for a large list over a large rig, most of them repeats of
        // the cell above. Held the other way round it is one entry per *change*,
        // which is bounded by the number of cue parts and is therefore the same
        // size as the edits it is derived from. Reading one slot at one cue is
        // then a binary search over that slot's own run rather than an index,
        // and it is allocation-free either way.
        let mut runs: Vec<Vec<TrackPoint>> = vec![Vec::new(); positions.len()];
        let mut walk = CueTrack::new();
        let mut changes: Vec<CueChange> = Vec::new();
        for (index, cue) in order.iter().enumerate() {
            walk.enter(cue, &mut changes);
            for change in &changes {
                let Some(position) = plan
                    .index_of(change.fixture, change.attribute)
                    .and_then(|slot| positions.get(&slot))
                else {
                    continue;
                };
                let Some(run) = runs.get_mut(*position as usize) else {
                    continue;
                };
                run.push(TrackPoint {
                    cue: index as u32,
                    value: change.value,
                });
            }
        }
        let mut track: Vec<TrackPoint> = Vec::new();
        let mut track_index: Vec<u32> = Vec::with_capacity(runs.len() + 1);
        for run in runs {
            track_index.push(track.len() as u32);
            track.extend(run);
        }
        track_index.push(track.len() as u32);

        Ok(Self {
            id: sequence.id,
            slots: slots.into_values().collect::<Vec<_>>().into_boxed_slice(),
            cues: cues.into_boxed_slice(),
            parts: parts.into_boxed_slice(),
            track: track.into_boxed_slice(),
            track_index: track_index.into_boxed_slice(),
            looping: sequence.looping,
            unresolved,
        })
    }

    /// What a walk from the first cue down to `cue` leaves slot `position` at -
    /// **S48**, and the answer this whole session exists to have.
    ///
    /// `position` is an index into [`Self::slots`], the same one
    /// [`Self::parts_of`] yields; `cue` is a playback index. `None` means the
    /// list does not hold that slot at that cue at all — either no cue up to
    /// there has named it, or the one that did took it back
    /// (`prism_domain::CueTracking::CueOnly`). A slot the playback does not hold
    /// falls through to whatever is under it in the merge, which is not the same
    /// answer as zero.
    ///
    /// **This runs on the tick** and makes no allocator call: a binary search
    /// over one slot's run of the flat table, and nothing else. It is the ninth
    /// path `crates/prism-engine/tests/tick_allocations.rs` measures.
    #[must_use]
    pub fn tracked(&self, position: usize, cue: usize) -> Option<u16> {
        let from = *self.track_index.get(position)? as usize;
        let to = *self.track_index.get(position.checked_add(1)?)? as usize;
        let run = self.track.get(from..to)?;
        // The last point at or before this cue. `partition_point` is a binary
        // search that allocates nothing and cannot panic on an empty slice.
        let found = run.partition_point(|point| point.cue as usize <= cue);
        run.get(found.checked_sub(1)?)?.value
    }

    /// How many entries the tracking table holds, for the measurement in
    /// `PROGRESS.md`: it is the number of *changes*, not `cues x slots`.
    #[must_use]
    pub const fn track_len(&self) -> usize {
        self.track.len()
    }

    /// Which sequence this is.
    #[must_use]
    pub const fn id(&self) -> SequenceId {
        self.id
    }

    /// Whether an automatic chain of follows runs off the end and round again.
    ///
    /// **Not whether a Go wraps** — a Go always does. See [`Self::step`].
    #[must_use]
    pub const fn looping(&self) -> bool {
        self.looping
    }

    /// How many cue parts named something this patch does not have.
    #[must_use]
    pub const fn unresolved(&self) -> usize {
        self.unresolved
    }

    /// How many cues this sequence has.
    #[must_use]
    pub const fn cue_count(&self) -> usize {
        self.cues.len()
    }

    /// One cue, by playback position.
    #[must_use]
    pub fn cue(&self, index: usize) -> Option<&CuePlan> {
        self.cues.get(index)
    }

    /// How many distinct plan slots the whole sequence touches — the size of the
    /// working memory a player over it needs.
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Every slot the sequence touches, in merge-plan order.
    #[must_use]
    pub const fn slots(&self) -> &[CueSlot] {
        &self.slots
    }

    /// One slot by its index within this sequence.
    #[must_use]
    pub fn slot(&self, index: usize) -> Option<&CueSlot> {
        self.slots.get(index)
    }

    /// The values one cue provides, in slot order. Empty for a cue that is not
    /// there, so a caller cannot index past the list.
    #[must_use]
    pub fn parts_of(&self, index: usize) -> &[CueValue] {
        let Some(cue) = self.cues.get(index) else {
            return &[];
        };
        self.parts
            .get(cue.first..cue.first.saturating_add(cue.len))
            .unwrap_or(&[])
    }

    /// Where a **Go** from `current` lands.
    ///
    /// `None` means the playback is stopped: stepping forward from there enters
    /// at the first cue and stepping back enters at the last.
    ///
    /// # A Go always comes round, and that is not [`Self::looping`]
    ///
    /// Go+ on the last cue enters the first, and Go− on the first enters the
    /// last, **whether or not the list loops**. It used to hold at the end
    /// instead, and holding is the wrong answer to a key somebody pressed: an
    /// operator at the end of a busking list who presses Go expects the top of
    /// it, and a desk that does nothing is a desk that looks broken in the dark.
    /// Nothing goes to black either way — the wrap enters cue 1 with its own
    /// fade, exactly as a Go into it from cue 0 would.
    ///
    /// [`Self::looping`] is a different question and keeps its own answer:
    /// whether an **automatic** chain of `Follow` and `Time` cues runs off the
    /// end and round again, which is a list that never stops on its own. That is
    /// [`Self::follow_step`], and it is deliberately not this: a chase that
    /// repeats for ever is a decision somebody makes about a cue list, and a Go
    /// is a hand on a key.
    #[must_use]
    pub fn step(&self, current: Option<usize>, direction: GoDirection) -> Option<usize> {
        let last = self.cues.len().checked_sub(1)?;
        let Some(current) = current else {
            return Some(match direction {
                GoDirection::Next => 0,
                GoDirection::Prev => last,
            });
        };
        let current = current.min(last);
        Some(match direction {
            GoDirection::Next => {
                if current < last {
                    current + 1
                } else {
                    0
                }
            }
            GoDirection::Prev => {
                if current > 0 {
                    current - 1
                } else {
                    last
                }
            }
        })
    }

    /// Where an **automatic** trigger from `current` goes — `Follow` and `Time`.
    ///
    /// `None` where the chain ends: the last cue of a list that does not loop,
    /// or a list with no cues at all. That is the whole difference from
    /// [`Self::step`], and the type says it — a Go always has somewhere to go
    /// and a follow chain does not.
    ///
    /// A **one-cue looping list** answers `Some(0)` from cue 0, which is a cue
    /// that retriggers itself for ever. That is what the flag asks for and the
    /// player has always allowed it; it is spelled out here because the obvious
    /// `next != current` guard would silently forbid it.
    #[must_use]
    pub fn follow_step(&self, current: usize) -> Option<usize> {
        let last = self.cues.len().checked_sub(1)?;
        let current = current.min(last);
        if current < last {
            Some(current + 1)
        } else if self.looping {
            Some(0)
        } else {
            None
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::cue::{
        CueError, CuePlan, MAX_CUE_PARTS, MAX_CUES, SequencePlan, interpolate, ticks_from_seconds,
    };
    use crate::plan::MergePlan;
    use crate::testkit::{cue, cue_part, moving_head, sequence};
    use crate::tick::TICK_HZ;
    use prism_domain::{AttributeType, CueTrigger, FixtureId, GoDirection, SequenceId};
    use proptest::prelude::*;

    /// Three moving heads: six slots, alternating HTP dimmer and LTP pan.
    fn plan() -> MergePlan {
        let head = moving_head();
        MergePlan::build((1..=3).map(|id| (FixtureId::new(id), &head, false))).unwrap()
    }

    fn slot(plan: &MergePlan, fixture: u32, attribute: AttributeType) -> usize {
        plan.index_of(FixtureId::new(fixture), attribute).unwrap()
    }

    /// Where one attribute sits in a compiled sequence's own slot table, which
    /// is what `SequencePlan::tracked` is indexed by — not the merge plan's.
    fn position(
        compiled: &SequencePlan,
        plan: &MergePlan,
        fixture: u32,
        attribute: AttributeType,
    ) -> usize {
        let wanted = slot(plan, fixture, attribute);
        compiled
            .slots()
            .iter()
            .position(|entry| entry.slot == wanted)
            .expect("the sequence touches that attribute")
    }

    #[test]
    fn a_fade_time_in_seconds_becomes_a_whole_number_of_ticks() {
        // The tick is the time base for every fade (ARCHITECTURE_SPEC.md 3.2),
        // so a cue time is converted once, when the plan is built, and the tick
        // itself never sees a float.
        assert_eq!(ticks_from_seconds(10.0), 440);
        assert_eq!(ticks_from_seconds(1.0), TICK_HZ);
        assert_eq!(ticks_from_seconds(0.0), 0);
        // Rounded to the nearest tick rather than truncated: half a period early
        // is as good as half a period late, and truncating biases every cue in
        // the show in the same direction.
        assert_eq!(ticks_from_seconds(0.5), 22);
        assert_eq!(ticks_from_seconds(1.0 / 88.0), 1);
        assert_eq!(ticks_from_seconds(1.0 / 200.0), 0);
    }

    #[test]
    fn an_impossible_fade_time_is_no_fade_rather_than_a_panic() {
        // prism-domain refuses non-finite times on the wire, but this function
        // is public and the tick must not be able to panic on one.
        for seconds in [-1.0, -0.0, f64::NAN, f64::NEG_INFINITY] {
            assert_eq!(ticks_from_seconds(seconds), 0, "{seconds}");
        }
        assert_eq!(ticks_from_seconds(f64::INFINITY), u64::MAX);
        assert_eq!(ticks_from_seconds(1e300), u64::MAX);
    }

    #[test]
    fn a_ten_second_fade_is_at_exactly_half_after_five_seconds() {
        // IMPLEMENTATION_PLAN.md S5, the headline criterion, as arithmetic.
        // 65535 is odd, so half of it is 32767 - the same number
        // docs/DMX_MERGE.md 7 gets from "65535 x 0.5".
        let ticks = ticks_from_seconds(10.0);
        assert_eq!(interpolate(0, 65_535, ticks / 2, ticks), 32_767);
        // And one tick either side is one tick's worth away, not a jump.
        let step = 65_535 / ticks as u16;
        assert!(interpolate(0, 65_535, ticks / 2 - 1, ticks).abs_diff(32_767) <= step + 1);
        assert!(interpolate(0, 65_535, ticks / 2 + 1, ticks).abs_diff(32_767) <= step + 1);
    }

    #[test]
    fn a_fade_starts_at_its_start_and_ends_at_its_target() {
        assert_eq!(interpolate(1_000, 60_000, 0, 100), 1_000);
        assert_eq!(interpolate(1_000, 60_000, 100, 100), 60_000);
        // Past the end it stays at the target rather than running on.
        assert_eq!(interpolate(1_000, 60_000, u64::MAX, 100), 60_000);
    }

    #[test]
    fn a_fade_of_no_length_is_already_finished() {
        assert_eq!(interpolate(0, 60_000, 0, 0), 60_000);
    }

    #[test]
    fn fading_down_is_the_mirror_of_fading_up() {
        assert_eq!(interpolate(65_535, 0, 220, 440), 32_768);
        assert_eq!(interpolate(60_000, 20_000, 1, 4), 50_000);
    }

    /// The tracking state, read the way the player reads it: by slot position
    /// and cue index, off a table built when the sequence was compiled.
    #[test]
    fn a_slot_a_later_cue_never_names_keeps_the_value_the_earlier_one_gave_it() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![
                    cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 32_768)]),
                    cue("2", 0.0, vec![cue_part(2, AttributeType::Dimmer, 65_535)]),
                    cue("3", 0.0, vec![cue_part(3, AttributeType::Dimmer, 20_000)]),
                    cue("4", 0.0, vec![cue_part(1, AttributeType::Dimmer, 65_535)]),
                ],
                false,
            ),
        )
        .unwrap();
        let one = position(&compiled, &plan, 1, AttributeType::Dimmer);

        // Cue 1 sets it, cues 2 and 3 do not mention it, cue 4 sets it again.
        assert_eq!(compiled.tracked(one, 0), Some(32_768));
        assert_eq!(compiled.tracked(one, 1), Some(32_768));
        assert_eq!(compiled.tracked(one, 2), Some(32_768));
        assert_eq!(compiled.tracked(one, 3), Some(65_535));

        // And **nothing before the cue that first names it**, which is a
        // different answer from zero: a slot the playback does not hold falls
        // through to whatever is under it in the merge.
        let three = position(&compiled, &plan, 3, AttributeType::Dimmer);
        assert_eq!(compiled.tracked(three, 0), None);
        assert_eq!(compiled.tracked(three, 1), None);
        assert_eq!(compiled.tracked(three, 2), Some(20_000));
    }

    /// The one-off, in the table: a cue-only value is in force at its own cue
    /// and gone at the next, back to whatever an earlier cue left underneath it.
    #[test]
    fn a_cue_only_value_is_in_the_table_at_its_own_cue_and_nowhere_else() {
        let plan = plan();
        let mut list = sequence(
            vec![
                cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 32_768)]),
                cue("2", 0.0, vec![cue_part(1, AttributeType::Dimmer, 65_535)]),
                cue("3", 0.0, vec![cue_part(2, AttributeType::Dimmer, 10_000)]),
                cue("4", 0.0, vec![cue_part(2, AttributeType::Dimmer, 20_000)]),
            ],
            false,
        );
        list.cues[1].parts[0].tracking = prism_domain::CueTracking::CueOnly;
        // And one with nothing underneath it at all, so the table has to carry a
        // release rather than a value.
        list.cues[2].parts[0].tracking = prism_domain::CueTracking::CueOnly;
        let compiled = SequencePlan::build(&plan, &list).unwrap();

        let one = position(&compiled, &plan, 1, AttributeType::Dimmer);
        assert_eq!(compiled.tracked(one, 0), Some(32_768));
        assert_eq!(compiled.tracked(one, 1), Some(65_535));
        assert_eq!(
            compiled.tracked(one, 2),
            Some(32_768),
            "the cue-only value did not go back to what was underneath it"
        );

        let two = position(&compiled, &plan, 2, AttributeType::Dimmer);
        assert_eq!(compiled.tracked(two, 2), Some(10_000));
        assert_eq!(
            compiled.tracked(two, 3),
            Some(20_000),
            "cue 4 asserts it, so the take-back is overwritten rather than empty"
        );

        // Take cue 4 away and the release is what is left: nothing held.
        list.cues.pop();
        let shorter = SequencePlan::build(&plan, &list).unwrap();
        let two = position(&shorter, &plan, 2, AttributeType::Dimmer);
        assert_eq!(shorter.tracked(two, 2), Some(10_000));
        assert_eq!(shorter.cue_count(), 3);
    }

    /// **The table is the size of the edits, not of the grid.**
    ///
    /// The reason the tracking state is held by slot rather than by cue. A
    /// hundred cues over six slots is six hundred cells; if two of those cues
    /// touch one slot, that slot costs **two** entries and not a hundred.
    #[test]
    fn a_tracking_table_is_the_size_of_the_edits_not_of_the_grid() {
        let plan = plan();
        let mut cues = vec![cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 100)])];
        for number in 2..=100u32 {
            // Ninety-nine cues that name nothing at all.
            cues.push(cue(&number.to_string(), 0.0, Vec::new()));
        }
        cues.push(cue(
            "101",
            0.0,
            vec![cue_part(1, AttributeType::Dimmer, 200)],
        ));
        let compiled = SequencePlan::build(&plan, &sequence(cues, false)).unwrap();
        assert_eq!(compiled.cue_count(), 101);
        assert_eq!(compiled.slot_count(), 1);
        assert_eq!(
            compiled.track_len(),
            2,
            "the table grew with the cues rather than with the changes"
        );
        let one = position(&compiled, &plan, 1, AttributeType::Dimmer);
        assert_eq!(compiled.tracked(one, 50), Some(100));
        assert_eq!(compiled.tracked(one, 100), Some(200));
    }

    /// A slot or a cue that is not there answers `None` rather than panicking:
    /// this runs on the tick, and `prism-engine` denies itself `unwrap`
    /// (`ARCHITECTURE_SPEC.md` section 3.1).
    #[test]
    fn asking_the_table_about_something_that_is_not_there_is_not_a_panic() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 100)])],
                false,
            ),
        )
        .unwrap();
        assert_eq!(compiled.tracked(99, 0), None);
        assert_eq!(compiled.tracked(0, usize::MAX), Some(100));
        assert_eq!(compiled.tracked(usize::MAX, 0), None);

        // And a sequence with no cues at all has no table and no opinion.
        let empty = SequencePlan::build(&plan, &sequence(Vec::new(), false)).unwrap();
        assert_eq!(empty.track_len(), 0);
        assert_eq!(empty.tracked(0, 0), None);
    }

    /// Cues are tracked in **playback** order, which is by number: `1`, `1.5`,
    /// `2`, `10`. A cue inserted between two others therefore inherits from the
    /// one above it in the running order rather than from the one above it in
    /// the file.
    #[test]
    fn the_table_follows_cue_numbers_and_not_the_order_of_the_file() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![
                    cue("2", 0.0, vec![cue_part(2, AttributeType::Dimmer, 2)]),
                    cue("10", 0.0, vec![cue_part(2, AttributeType::Dimmer, 10)]),
                    cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 1)]),
                ],
                false,
            ),
        )
        .unwrap();
        let one = position(&compiled, &plan, 1, AttributeType::Dimmer);
        let two = position(&compiled, &plan, 2, AttributeType::Dimmer);
        // Playback order is 1, 2, 10 - so fixture 1 is held from the *first*
        // played cue on, which is the one written last in the file.
        assert_eq!(compiled.cue(0).map(CuePlan::number), Some("1"));
        assert_eq!(compiled.tracked(one, 0), Some(1));
        assert_eq!(compiled.tracked(two, 0), None);
        assert_eq!(compiled.tracked(two, 1), Some(2));
        assert_eq!(compiled.tracked(two, 2), Some(10));
    }

    /// A cue that names one attribute twice says the second thing, and the
    /// second part is what decides whether it tracks — it is the one that was
    /// written last, exactly as a second keystroke would be.
    #[test]
    fn the_last_part_of_a_cue_wins_and_its_tracking_is_what_governs() {
        let plan = plan();
        let mut list = sequence(
            vec![
                cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 100)]),
                cue(
                    "2",
                    0.0,
                    vec![
                        cue_part(1, AttributeType::Dimmer, 200),
                        cue_part(1, AttributeType::Dimmer, 300),
                    ],
                ),
                cue("3", 0.0, vec![cue_part(2, AttributeType::Dimmer, 1)]),
            ],
            false,
        );
        list.cues[1].parts[1].tracking = prism_domain::CueTracking::CueOnly;
        let compiled = SequencePlan::build(&plan, &list).unwrap();
        let one = position(&compiled, &plan, 1, AttributeType::Dimmer);
        assert_eq!(compiled.tracked(one, 1), Some(300));
        assert_eq!(
            compiled.tracked(one, 2),
            Some(100),
            "the first part's tracking mode was used instead of the last part's"
        );
    }

    #[test]
    fn a_cue_resolves_its_parts_into_slot_indices() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![cue(
                    "1",
                    3.0,
                    vec![
                        cue_part(2, AttributeType::Pan, 45_000),
                        cue_part(1, AttributeType::Dimmer, 65_535),
                    ],
                )],
                false,
            ),
        )
        .unwrap();

        assert_eq!(compiled.cue_count(), 1);
        assert_eq!(compiled.unresolved(), 0);
        assert_eq!(compiled.slot_count(), 2);
        assert_eq!(compiled.id(), SequenceId::new(1));
        assert!(!compiled.looping());
        let values: Vec<(usize, u16)> = compiled
            .parts_of(0)
            .iter()
            .map(|part| (compiled.slot(part.slot as usize).unwrap().slot, part.value))
            .collect();
        assert_eq!(
            values,
            [
                (slot(&plan, 1, AttributeType::Dimmer), 65_535),
                (slot(&plan, 2, AttributeType::Pan), 45_000),
            ]
        );
    }

    #[test]
    fn a_compiled_slot_carries_the_home_value_and_merge_mode_it_falls_back_to() {
        // The release fades an intensity down to its home value, so the plan has
        // to know what that is without reaching back into the merge plan on the
        // tick.
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![cue(
                    "1",
                    0.0,
                    vec![
                        cue_part(1, AttributeType::Dimmer, 1),
                        cue_part(1, AttributeType::Pan, 1),
                    ],
                )],
                false,
            ),
        )
        .unwrap();
        let dimmer = compiled.slot(0).unwrap();
        assert_eq!(dimmer.slot, slot(&plan, 1, AttributeType::Dimmer));
        assert_eq!(dimmer.home, 0);
        assert!(dimmer.htp);
        let pan = compiled.slot(1).unwrap();
        assert_eq!(pan.home, 32_768);
        assert!(!pan.htp);
        assert!(compiled.slot(2).is_none());
    }

    #[test]
    fn a_part_for_something_that_is_not_patched_is_dropped_and_counted() {
        // A show can outlive the rig it was written on. A cue naming a fixture
        // that is no longer patched must not stop the sequence from running -
        // and must not vanish silently either, or the operator has no way to
        // know why a cue does nothing.
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![cue(
                    "1",
                    0.0,
                    vec![
                        cue_part(9, AttributeType::Dimmer, 100),
                        cue_part(1, AttributeType::Tilt, 100),
                        cue_part(1, AttributeType::Dimmer, 100),
                    ],
                )],
                false,
            ),
        )
        .unwrap();
        assert_eq!(compiled.unresolved(), 2);
        assert_eq!(compiled.parts_of(0).len(), 1);
        assert_eq!(compiled.slot_count(), 1);
    }

    #[test]
    fn one_slot_set_twice_in_a_cue_keeps_the_last_value() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![cue(
                    "1",
                    0.0,
                    vec![
                        cue_part(1, AttributeType::Dimmer, 100),
                        cue_part(1, AttributeType::Dimmer, 200),
                    ],
                )],
                false,
            ),
        )
        .unwrap();
        assert_eq!(compiled.parts_of(0).len(), 1);
        assert_eq!(compiled.parts_of(0).first().unwrap().value, 200);
    }

    #[test]
    fn a_slot_two_cues_share_is_listed_once() {
        // The player's working memory is one entry per slot the *sequence*
        // touches, which is what bounds it without a dense per-slot array.
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                vec![
                    cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 100)]),
                    cue("2", 0.0, vec![cue_part(1, AttributeType::Dimmer, 200)]),
                ],
                false,
            ),
        )
        .unwrap();
        assert_eq!(compiled.slot_count(), 1);
        assert_eq!(compiled.parts_of(0).first().unwrap().slot, 0);
        assert_eq!(compiled.parts_of(1).first().unwrap().slot, 0);
    }

    #[test]
    fn cues_play_in_cue_number_order_not_in_list_order() {
        // IMPLEMENTATION_PLAN.md S5: 1, 1.5, 2, 10 - the order an operator reads,
        // which is `Cue::compare_numbers` and not the lexical one. Inserting a
        // cue between two others is the whole reason cue numbers are decimals.
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                ["10", "2", "1.5", "1"]
                    .into_iter()
                    .map(|number| cue(number, 0.0, Vec::new()))
                    .collect(),
                false,
            ),
        )
        .unwrap();
        let numbers: Vec<&str> = (0..compiled.cue_count())
            .map(|index| compiled.cue(index).unwrap().number())
            .collect();
        assert_eq!(numbers, ["1", "1.5", "2", "10"]);
    }

    #[test]
    fn a_cue_number_that_is_not_a_number_still_has_a_place_in_the_order() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                ["oops", "2", "1"]
                    .into_iter()
                    .map(|number| cue(number, 0.0, Vec::new()))
                    .collect(),
                false,
            ),
        )
        .unwrap();
        let numbers: Vec<&str> = (0..compiled.cue_count())
            .map(|index| compiled.cue(index).unwrap().number())
            .collect();
        assert_eq!(numbers, ["1", "2", "oops"]);
    }

    #[test]
    fn a_cue_carries_its_times_as_ticks_and_its_trigger() {
        let plan = plan();
        let mut source = cue("1", 2.0, Vec::new());
        source.fade_in = 10.0;
        source.fade_out = 4.0;
        source.delay = 1.0;
        source.trigger = CueTrigger::Time;
        source.trigger_time = Some(3.0);
        let compiled = SequencePlan::build(&plan, &sequence(vec![source], false)).unwrap();
        let compiled = compiled.cue(0).unwrap();
        assert_eq!(compiled.fade_in(), 440);
        assert_eq!(compiled.fade_out(), 176);
        assert_eq!(compiled.delay(), 44);
        assert_eq!(compiled.trigger(), CueTrigger::Time);
        assert_eq!(compiled.trigger_ticks(), Some(132));
        // A cue is finished fading when its delay and its fade-in are over -
        // which is what a Follow on the next cue waits for.
        assert_eq!(compiled.transition_ticks(), 484);
    }

    /// **A Go always comes round**, on a list that does not loop as much as on
    /// one that does.
    ///
    /// It used to hold at the end, and holding is the wrong answer to a key
    /// somebody pressed — an operator at the bottom of a busking list who
    /// presses Go wants the top of it. `looping` governs the *automatic* chain
    /// and is asserted separately below, on a plan built with it off.
    #[test]
    fn stepping_forward_and_back_walks_the_list_and_comes_round_at_its_ends() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                ["1", "2", "3"]
                    .into_iter()
                    .map(|number| cue(number, 0.0, Vec::new()))
                    .collect(),
                false,
            ),
        )
        .unwrap();
        assert!(!compiled.looping(), "the wrap is not this flag");
        assert_eq!(compiled.step(None, GoDirection::Next), Some(0));
        assert_eq!(compiled.step(Some(0), GoDirection::Next), Some(1));
        assert_eq!(compiled.step(Some(2), GoDirection::Next), Some(0));
        // Stepping back into a stopped list enters at the end.
        assert_eq!(compiled.step(None, GoDirection::Prev), Some(2));
        assert_eq!(compiled.step(Some(1), GoDirection::Prev), Some(0));
        assert_eq!(compiled.step(Some(0), GoDirection::Prev), Some(2));

        // And the automatic chain, which is the half `looping` still decides:
        // it walks to the end of this list and stops there.
        assert_eq!(compiled.follow_step(0), Some(1));
        assert_eq!(compiled.follow_step(1), Some(2));
        assert_eq!(compiled.follow_step(2), None);
    }

    #[test]
    fn a_looping_sequence_wraps_at_both_ends() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                ["1", "2", "3"]
                    .into_iter()
                    .map(|number| cue(number, 0.0, Vec::new()))
                    .collect(),
                true,
            ),
        )
        .unwrap();
        assert!(compiled.looping());
        assert_eq!(compiled.step(Some(2), GoDirection::Next), Some(0));
        assert_eq!(compiled.step(Some(0), GoDirection::Prev), Some(2));
        // The chain comes round too, which is the whole of what the flag buys.
        assert_eq!(compiled.follow_step(2), Some(0));
    }

    /// A **one-cue looping list** follows itself for ever, and a one-cue list
    /// that does not loop stops after it.
    ///
    /// Spelled out because the obvious `next != current` guard — which is what
    /// the player used to carry — would forbid the first of these silently.
    #[test]
    fn a_single_cue_follows_itself_only_when_the_list_loops() {
        let plan = plan();
        let one = || vec![cue("1", 0.0, Vec::new())];
        let round = SequencePlan::build(&plan, &sequence(one(), true)).unwrap();
        let once = SequencePlan::build(&plan, &sequence(one(), false)).unwrap();
        assert_eq!(round.follow_step(0), Some(0));
        assert_eq!(once.follow_step(0), None);
        // A Go on either is the same key on the same one cue.
        assert_eq!(round.step(Some(0), GoDirection::Next), Some(0));
        assert_eq!(once.step(Some(0), GoDirection::Next), Some(0));
        assert_eq!(once.step(Some(0), GoDirection::Prev), Some(0));
    }

    /// An index past the end — which a client can send, since it names a cue by
    /// number and the list may have shrunk since — is clamped rather than
    /// panicking or wrapping from nowhere.
    #[test]
    fn an_index_past_the_end_is_read_as_the_end() {
        let plan = plan();
        let compiled = SequencePlan::build(
            &plan,
            &sequence(
                ["1", "2"]
                    .into_iter()
                    .map(|number| cue(number, 0.0, Vec::new()))
                    .collect(),
                false,
            ),
        )
        .unwrap();
        assert_eq!(compiled.step(Some(99), GoDirection::Next), Some(0));
        assert_eq!(compiled.step(Some(99), GoDirection::Prev), Some(0));
        assert_eq!(compiled.follow_step(99), None);
    }

    #[test]
    fn an_empty_sequence_has_nowhere_to_step() {
        let plan = plan();
        let compiled = SequencePlan::build(&plan, &sequence(Vec::new(), true)).unwrap();
        assert_eq!(compiled.cue_count(), 0);
        assert_eq!(compiled.step(None, GoDirection::Next), None);
        assert_eq!(compiled.step(None, GoDirection::Prev), None);
        assert_eq!(compiled.step(Some(0), GoDirection::Next), None);
        // A list with no cues has no chain either, looping or not.
        assert_eq!(compiled.follow_step(0), None);
        assert!(compiled.cue(0).is_none());
        assert!(compiled.parts_of(0).is_empty());
    }

    #[test]
    fn an_absurd_sequence_is_rejected_rather_than_compiled() {
        let plan = plan();
        let cues = (0..=MAX_CUES)
            .map(|number| cue(&number.to_string(), 0.0, Vec::new()))
            .collect();
        assert_eq!(
            SequencePlan::build(&plan, &sequence(cues, false)).unwrap_err(),
            CueError::TooManyCues(MAX_CUES + 1)
        );
    }

    #[test]
    fn a_cue_with_an_absurd_number_of_parts_is_rejected_rather_than_compiled() {
        // The limit counts parts as they resolve and stops there, rather than
        // collecting an implausible list first and measuring it afterwards.
        let plan = plan();
        let parts = (0..=MAX_CUE_PARTS)
            .map(|_| cue_part(1, AttributeType::Dimmer, 1))
            .collect();
        assert_eq!(
            SequencePlan::build(&plan, &sequence(vec![cue("1", 0.0, parts)], false)).unwrap_err(),
            CueError::TooManyParts(MAX_CUE_PARTS + 1)
        );
    }

    #[test]
    fn a_rejected_sequence_says_why_in_words() {
        assert_eq!(
            CueError::TooManyCues(20_000).to_string(),
            "20000 cues exceeds the limit of 10000"
        );
        assert_eq!(
            CueError::TooManyParts(9_000_000).to_string(),
            "9000000 cue parts exceeds the limit of 1048576"
        );
        assert_eq!(
            CueError::UnknownPlayback(prism_domain::PlaybackId::of_sequence(SequenceId::new(7)))
                .to_string(),
            "sequence 7 is not in this patch"
        );
        let as_error: &dyn std::error::Error = &CueError::TooManyCues(1);
        assert!(!as_error.to_string().is_empty());
    }

    proptest! {
        /// A fade never leaves the interval between where it started and where
        /// it is going. An operator seeing a value outside that interval sees a
        /// light doing something no cue asked for.
        #[test]
        fn a_fade_never_leaves_the_interval_it_is_fading_across(
            from in any::<u16>(),
            to in any::<u16>(),
            elapsed in any::<u64>(),
            duration in any::<u64>(),
        ) {
            let value = interpolate(from, to, elapsed, duration);
            prop_assert!(value >= from.min(to));
            prop_assert!(value <= from.max(to));
        }

        /// And it only ever moves towards the target: a fade that went back on
        /// itself for one tick would be a visible flicker.
        #[test]
        fn a_fade_is_monotone_in_the_time_that_has_passed(
            from in any::<u16>(),
            to in any::<u16>(),
            elapsed in 0u64..1_000,
            duration in 1u64..1_000,
        ) {
            let earlier = interpolate(from, to, elapsed, duration);
            let later = interpolate(from, to, elapsed + 1, duration);
            if to >= from {
                prop_assert!(later >= earlier);
            } else {
                prop_assert!(later <= earlier);
            }
        }

        /// Ordering cues is `Cue::compare_numbers` and nothing else, whatever
        /// order the show file happens to hold them in.
        #[test]
        fn compiling_a_sequence_orders_it_by_cue_number(
            numbers in proptest::collection::vec(0u32..50, 0..8),
        ) {
            let plan = plan();
            let cues = numbers
                .iter()
                .map(|number| cue(&number.to_string(), 0.0, Vec::new()))
                .collect();
            let compiled = SequencePlan::build(&plan, &sequence(cues, false)).unwrap();
            let mut expected = numbers.clone();
            expected.sort_unstable();
            let actual: Vec<u32> = (0..compiled.cue_count())
                .map(|index| compiled.cue(index).unwrap().number().parse().unwrap())
                .collect();
            prop_assert_eq!(actual, expected);
        }
    }
}
