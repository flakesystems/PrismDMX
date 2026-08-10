//! Playback: cues running on the tick's time base.
//!
//! `ARCHITECTURE_SPEC.md` §5 steps 2 and 3. A [`CuePlayer`] owns one executor's
//! position in a cue list and the fade it is part way through; once a tick it
//! works out a value for every attribute it is holding and writes them into that
//! executor's [`PlaybackSource`], which is where the merge picks them up.
//!
//! Time comes from the tick index and from nowhere else. A player never reads a
//! clock, so a ten-minute cue list can be run in a millisecond by a test and
//! produce the same numbers it would produce on stage.
//!
//! # The rules this module decides
//!
//! `docs/DMX_MERGE.md` says what a *set* of values merges to. It does not say
//! what an executor holds in the first place, and four of those answers are
//! choices rather than deductions. They are stated here because a console whose
//! playback behaviour is folded into an implementation is a console nobody can
//! predict.
//!
//! **Cues track.** A cue holds the values it names and nothing else; an
//! attribute it does not mention keeps whatever the playback was already holding
//! it at. The alternative — every cue is a complete look — is not available:
//! `Command::StoreCue` stores the programmer, which `docs/DMX_MERGE.md` §3 makes
//! sparse by specification, so a cue recorded after moving one head would black
//! the rest of the stage out.
//!
//! **A Go during a running fade overtakes it.** Every attribute re-bases from
//! the value it is holding at that instant and moves to the new cue's target
//! over the new cue's fade time. The running fade is neither completed first nor
//! abandoned, and no attribute jumps. This is the one rule that has to be
//! decided rather than discovered, because the obvious alternatives — snap to
//! the old target first, or start from the old target — both produce a visible
//! step in the middle of a crossfade.
//!
//! **A release fades the light out and leaves the rig still.** Switching a
//! playback off fades its intensities down to their home values over the current
//! cue's fade-out time and holds everything else where it is until that is over;
//! then the whole source leaves the merge. A moving head must not swing back to
//! centre while it is still lit, and an LTP attribute that faded would go on
//! winning its slot for the whole fade, so the fade would be a fade of the
//! *winner* rather than of the value — the same asymmetry as §2.3.
//!
//! **At most one cue starts per tick.** A chain of follow cues with no times in
//! it advances one cue per tick rather than spinning inside one. It is a fuse:
//! the tick has a 22.7 ms budget and a cue list is show data.

use prism_domain::{CueTrigger, ExecutorId, GoDirection};

use crate::cue::{CuePlan, SequencePlan, interpolate};
use crate::playback::{PlaybackLayer, PlaybackSource};

/// One attribute a playback is holding, and the fade it is part way through.
///
/// Flat and `Copy`: the whole of a player's working memory is one of these per
/// slot its own sequence can touch, allocated when the sequence is loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Entry {
    /// Whether the playback is holding this attribute at all.
    live: bool,
    /// Value at the start of the current transition.
    from: u16,
    /// Value the transition is heading for.
    to: u16,
    /// Value right now — what was written into the source last tick, and what a
    /// Go re-bases from.
    value: u16,
    /// Ticks the transition takes.
    duration: u64,
}

/// What a tick's worth of playback did to the source's place in the merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// Nothing: the source was already where it should be.
    None,
    /// The playback started, so its executor goes active and is stamped with a
    /// fresh activation counter — the top of the LTP order.
    Activate,
    /// The playback finished releasing and leaves the merge.
    Deactivate,
}

/// One executor's playback: where it is in its cue list, and what it is holding.
#[derive(Debug, Clone)]
pub struct CuePlayer {
    executor: ExecutorId,
    sequence: Option<SequencePlan>,
    entries: Box<[Entry]>,
    /// The cue being played, or `None` when the playback is stopped or releasing.
    current: Option<usize>,
    /// Tick the current transition started on.
    started: u64,
    /// Ticks of delay before the current transition moves anything.
    delay: u64,
    /// Set by a command, consumed by the next [`Self::advance`]: commands are
    /// applied before the tick that renders them and do not know its index.
    restart: bool,
    /// Whether this player has told the layer its source is active.
    active: bool,
    /// Set by [`Self::unload`], so the source it was driving is cleaned up once.
    flush: bool,
}

impl CuePlayer {
    /// A player for one executor, with no sequence on it.
    #[must_use]
    fn new(executor: ExecutorId) -> Self {
        Self {
            executor,
            sequence: None,
            entries: Box::default(),
            current: None,
            started: 0,
            delay: 0,
            restart: false,
            active: false,
            flush: false,
        }
    }

    /// Which executor this playback belongs to.
    #[must_use]
    pub const fn executor(&self) -> ExecutorId {
        self.executor
    }

    /// The compiled cue list, if one is loaded.
    #[must_use]
    pub const fn sequence(&self) -> Option<&SequencePlan> {
        self.sequence.as_ref()
    }

    /// Whether a sequence is loaded. A player without one does not touch its
    /// source at all, which is what lets a host write values into an executor
    /// by hand.
    #[must_use]
    pub const fn is_loaded(&self) -> bool {
        self.sequence.is_some()
    }

    /// The cue being played, as an index into the sequence. `None` while the
    /// playback is stopped or releasing.
    #[must_use]
    pub const fn current_cue(&self) -> Option<usize> {
        self.current
    }

    /// What this playback is holding for one slot of the merge plan.
    #[must_use]
    pub fn provides(&self, slot: usize) -> Option<u16> {
        let plan = self.sequence.as_ref()?;
        let position = plan
            .slots()
            .binary_search_by_key(&slot, |slot| slot.slot)
            .ok()?;
        let entry = self.entries.get(position)?;
        entry.live.then_some(entry.value)
    }

    /// Puts a compiled sequence on this executor.
    ///
    /// Allocates the working memory, so this is a set-up operation and not
    /// something to do while the tick is running. The playback is left stopped;
    /// whatever it was holding is released without a fade on the next tick,
    /// because the values belonged to a cue list that is no longer there.
    pub fn load(&mut self, sequence: SequencePlan) {
        self.entries = vec![Entry::default(); sequence.slot_count()].into_boxed_slice();
        self.sequence = Some(sequence);
        self.current = None;
        self.started = 0;
        self.delay = 0;
        self.restart = false;
        self.flush = false;
        // `active` deliberately survives: it records what the *layer* was last
        // told, and the next tick has to take the old source back out of the
        // merge.
    }

    /// Takes the sequence off this executor and gives the source back.
    ///
    /// The next tick clears what the playback was holding and takes it out of
    /// the merge; after that the source is the host's again.
    pub fn unload(&mut self) {
        if self.sequence.take().is_none() {
            return;
        }
        self.entries = Box::default();
        self.current = None;
        self.restart = false;
        self.flush = true;
    }

    /// Steps to the next or previous cue, starting the list if it was stopped.
    ///
    /// Returns `false` if there is no sequence or it has no cues. The transition
    /// begins on the next tick to be rendered — commands are applied before the
    /// frame they affect, and do not know its index.
    pub fn go(&mut self, direction: GoDirection) -> bool {
        let Self {
            sequence,
            entries,
            current,
            delay,
            restart,
            ..
        } = self;
        let Some(plan) = sequence.as_ref() else {
            return false;
        };
        let Some(next) = plan.step(*current, direction) else {
            return false;
        };
        *delay = begin(plan, entries, next);
        *current = Some(next);
        *restart = true;
        true
    }

    /// Starts the sequence at its first cue.
    ///
    /// Returns `false` if there is nothing to start or the playback is already
    /// running: `ExecutorButtonFunction::On` pressed twice must not restart the
    /// list under the operator, exactly as [`PlaybackLayer::activate`] does not
    /// re-stamp an executor that is already on.
    pub fn on(&mut self) -> bool {
        if self.current.is_some() {
            return false;
        }
        self.go(GoDirection::Next)
    }

    /// Stops the playback: intensities fade to home over the current cue's
    /// fade-out time, everything else holds, and the source leaves the merge
    /// when the fade is over.
    ///
    /// Returns `false` if no cue was running.
    pub fn off(&mut self) -> bool {
        let Self {
            sequence,
            entries,
            current,
            delay,
            restart,
            ..
        } = self;
        let Some(plan) = sequence.as_ref() else {
            return false;
        };
        let Some(outgoing) = current.take() else {
            return false;
        };
        release(plan, entries, outgoing);
        *delay = 0;
        *restart = true;
        true
    }

    /// Moves the playback on to tick `tick` and works out what it is holding.
    ///
    /// Allocation-free, lock-free and clock-free: this runs on the tick.
    fn advance(&mut self, tick: u64) -> Change {
        let Self {
            sequence,
            entries,
            current,
            started,
            delay,
            restart,
            active,
            ..
        } = self;
        let Some(plan) = sequence.as_ref() else {
            return Change::None;
        };
        if *restart {
            *started = tick;
            *restart = false;
        }
        let elapsed = tick.saturating_sub(*started);

        // Where every held attribute has got to. Evaluated *before* the trigger
        // check below, so a cue that ends on this tick reaches its target and
        // the cue that follows it starts from there rather than from one step
        // short of it.
        for entry in entries.iter_mut().filter(|entry| entry.live) {
            entry.value = if elapsed < *delay {
                entry.from
            } else {
                interpolate(
                    entry.from,
                    entry.to,
                    elapsed.saturating_sub(*delay),
                    entry.duration,
                )
            };
        }

        // Follow and Time, once per tick at most.
        if let Some(index) = *current
            && let Some(next) = plan.step(Some(index), GoDirection::Next)
            && (next != index || plan.looping())
            && triggers(plan, index, next, elapsed)
        {
            *delay = begin(plan, entries, next);
            *current = Some(next);
            *started = tick;
        }

        // A release that has run its course drops everything at once, so the
        // attributes fall back to whatever is underneath them in the merge.
        if current.is_none()
            && entries
                .iter()
                .filter(|entry| entry.live)
                .all(|entry| elapsed >= delay.saturating_add(entry.duration))
        {
            for entry in entries.iter_mut() {
                entry.live = false;
            }
        }

        let wanted = current.is_some() || entries.iter().any(|entry| entry.live);
        if wanted == *active {
            return Change::None;
        }
        *active = wanted;
        if wanted {
            Change::Activate
        } else {
            Change::Deactivate
        }
    }

    /// Writes what the playback is holding into its source.
    fn write(&self, source: &mut PlaybackSource) {
        source.clear();
        let Some(plan) = self.sequence.as_ref() else {
            return;
        };
        for (position, entry) in self.entries.iter().enumerate() {
            if !entry.live {
                continue;
            }
            if let Some(slot) = plan.slot(position) {
                source.set(slot.slot, entry.value);
            }
        }
    }
}

/// Starts cue `index`: every held attribute re-bases from where it is now, and
/// the cue's own attributes take their new targets. Returns the cue's delay.
///
/// Attributes the cue does not mention keep their targets and move on to the new
/// cue's time base — one transition has one clock.
fn begin(plan: &SequencePlan, entries: &mut [Entry], index: usize) -> u64 {
    let Some(cue) = plan.cue(index) else {
        return 0;
    };
    let fade = cue.fade_in();
    for entry in entries.iter_mut().filter(|entry| entry.live) {
        entry.from = entry.value;
        entry.duration = fade;
    }
    for part in plan.parts_of(index) {
        let position = part.slot as usize;
        let (Some(entry), Some(slot)) = (entries.get_mut(position), plan.slot(position)) else {
            continue;
        };
        if !entry.live {
            // An attribute this playback was not holding fades in from the value
            // it falls back to, so a cue with a fade time fades rather than
            // snapping on its first tick.
            entry.live = true;
            entry.from = slot.home;
            entry.value = slot.home;
        }
        entry.to = part.value;
        entry.duration = fade;
    }
    cue.delay()
}

/// Turns everything the playback holds into a release over the outgoing cue's
/// fade-out time: intensities to home, everything else held where it is.
fn release(plan: &SequencePlan, entries: &mut [Entry], outgoing: usize) {
    let fade = plan.cue(outgoing).map_or(0, CuePlan::fade_out);
    for (position, entry) in entries.iter_mut().enumerate() {
        if !entry.live {
            continue;
        }
        entry.from = entry.value;
        entry.to = match plan.slot(position) {
            Some(slot) if slot.htp => slot.home,
            _ => entry.value,
        };
        entry.duration = fade;
    }
}

/// Whether cue `next` starts by itself, `elapsed` ticks after cue `current`
/// started.
///
/// `Sound` is deferred past V1 (`IMPLEMENTATION_PLAN.md` S5) and a `Time` cue
/// with no trigger time is under-specified. Neither fires: a cue that runs away
/// by itself during a show is a worse fault than one that waits for a Go.
fn triggers(plan: &SequencePlan, current: usize, next: usize, elapsed: u64) -> bool {
    let (Some(current), Some(next)) = (plan.cue(current), plan.cue(next)) else {
        return false;
    };
    match next.trigger() {
        CueTrigger::Follow => elapsed >= current.transition_ticks(),
        CueTrigger::Time => next.trigger_ticks().is_some_and(|ticks| elapsed >= ticks),
        CueTrigger::Go | CueTrigger::Sound => false,
    }
}

/// One player per executor, kept in step with a [`PlaybackLayer`].
#[derive(Debug, Clone)]
pub struct CueLayer {
    /// Sorted by executor number, like the layer's sources, so a lookup is a
    /// binary search and the two can be walked together.
    players: Box<[CuePlayer]>,
}

impl CueLayer {
    /// A player for every source in a layer, none of them loaded.
    ///
    /// Built from the layer rather than from a list of executors, so the two
    /// cannot end up describing different sets.
    #[must_use]
    pub fn for_layer(layer: &PlaybackLayer) -> Self {
        Self {
            players: layer
                .sources()
                .iter()
                .map(|source| CuePlayer::new(source.executor()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    /// How many players this layer holds.
    #[must_use]
    pub const fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Every player, in executor order.
    #[must_use]
    pub const fn players(&self) -> &[CuePlayer] {
        &self.players
    }

    fn index_of(&self, executor: ExecutorId) -> Option<usize> {
        self.players
            .binary_search_by_key(&executor, CuePlayer::executor)
            .ok()
    }

    /// One player by executor number.
    #[must_use]
    pub fn player(&self, executor: ExecutorId) -> Option<&CuePlayer> {
        self.players.get(self.index_of(executor)?)
    }

    /// One player by executor number, for loading a sequence or stepping it.
    pub fn player_mut(&mut self, executor: ExecutorId) -> Option<&mut CuePlayer> {
        let index = self.index_of(executor)?;
        self.players.get_mut(index)
    }

    /// Runs every playback on to tick `tick` and writes the result into `layer`.
    ///
    /// This is step 2 and step 3 of `ARCHITECTURE_SPEC.md` §5, and it runs on
    /// the tick: no allocation, no locks, no clock.
    pub fn advance(&mut self, tick: u64, layer: &mut PlaybackLayer) {
        for player in &mut self.players {
            if !player.is_loaded() {
                // A player with no sequence leaves its source alone - except
                // once, immediately after the sequence was taken off it.
                if player.flush {
                    player.flush = false;
                    player.active = false;
                    if let Some(source) = layer.source_mut(player.executor) {
                        source.clear();
                    }
                    layer.deactivate(player.executor);
                }
                continue;
            }
            match player.advance(tick) {
                Change::Activate => {
                    layer.activate(player.executor);
                }
                Change::Deactivate => {
                    layer.deactivate(player.executor);
                }
                Change::None => {}
            }
            if let Some(source) = layer.source_mut(player.executor) {
                player.write(source);
            }
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::cue::SequencePlan;
    use crate::plan::MergePlan;
    use crate::playback::{MergeScratch, PlaybackLayer};
    use crate::player::{Change, CueLayer, Entry, begin, release, triggers};
    use crate::testkit::{cue, cue_part, moving_head, sequence};
    use prism_domain::{
        AttributeType, Cue, CueTrigger, ExecutorId, FixtureId, GoDirection, Sequence,
    };

    /// Three moving heads: six slots, alternating HTP dimmer and LTP pan.
    fn plan() -> MergePlan {
        let head = moving_head();
        MergePlan::build((1..=3).map(|id| (FixtureId::new(id), &head))).unwrap()
    }

    fn slot(plan: &MergePlan, fixture: u32, attribute: AttributeType) -> usize {
        plan.index_of(FixtureId::new(fixture), attribute).unwrap()
    }

    struct Rig {
        plan: MergePlan,
        layer: PlaybackLayer,
        cues: CueLayer,
    }

    impl Rig {
        fn new(executors: u32) -> Self {
            let plan = plan();
            let layer = PlaybackLayer::new(&plan, (1..=executors).map(ExecutorId::new)).unwrap();
            let cues = CueLayer::for_layer(&layer);
            Self { plan, layer, cues }
        }

        fn load(&mut self, executor: u32, sequence: &Sequence) {
            let compiled = SequencePlan::build(&self.plan, sequence).unwrap();
            self.cues
                .player_mut(ExecutorId::new(executor))
                .unwrap()
                .load(compiled);
        }

        fn go(&mut self, executor: u32, direction: GoDirection) -> bool {
            self.cues
                .player_mut(ExecutorId::new(executor))
                .unwrap()
                .go(direction)
        }

        fn off(&mut self, executor: u32) -> bool {
            self.cues
                .player_mut(ExecutorId::new(executor))
                .unwrap()
                .off()
        }

        fn tick(&mut self, index: u64) {
            self.cues.advance(index, &mut self.layer);
        }

        /// Runs every tick from `from` up to and including `to`.
        fn run(&mut self, from: u64, to: u64) {
            for index in from..=to {
                self.tick(index);
            }
        }

        fn values(&self) -> Vec<u16> {
            let mut scratch = MergeScratch::new(&self.plan);
            let mut out = vec![0u16; self.plan.slot_count()];
            self.layer.resolve(&self.plan, &mut scratch, &mut out);
            out
        }

        fn value(&self, fixture: u32, attribute: AttributeType) -> u16 {
            self.values()[slot(&self.plan, fixture, attribute)]
        }

        fn current(&self, executor: u32) -> Option<usize> {
            self.cues
                .player(ExecutorId::new(executor))
                .unwrap()
                .current_cue()
        }

        fn activation(&self, executor: u32) -> Option<u64> {
            self.layer.source(ExecutorId::new(executor))?.activation()
        }
    }

    /// One cue: the dimmer of fixture 1, at `value`, over `fade` seconds.
    fn dimmer_cue(number: &str, value: u16, fade: f64) -> Cue {
        cue(
            number,
            fade,
            vec![cue_part(1, AttributeType::Dimmer, value)],
        )
    }

    #[test]
    fn a_playback_with_no_sequence_does_not_touch_its_source() {
        // S3 and S4 write values into a source directly, and the tick harness
        // in tests/tick_allocations.rs does too. A player owns its source only
        // once a sequence is loaded onto it.
        let mut rig = Rig::new(2);
        let dimmer = slot(&rig.plan, 1, AttributeType::Dimmer);
        rig.layer
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(dimmer, 40_000);
        rig.layer.activate(ExecutorId::new(1));
        rig.run(0, 10);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 40_000);
        assert!(!rig.cues.player(ExecutorId::new(1)).unwrap().is_loaded());
    }

    #[test]
    fn a_go_starts_the_first_cue_and_puts_the_playback_into_the_ltp_order() {
        let mut rig = Rig::new(2);
        rig.load(1, &sequence(vec![dimmer_cue("1", 40_000, 0.0)], false));
        assert_eq!(rig.activation(1), None);
        assert!(rig.go(1, GoDirection::Next));
        rig.tick(0);
        assert_eq!(rig.current(1), Some(0));
        assert_eq!(rig.activation(1), Some(0));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 40_000);
    }

    #[test]
    fn a_ten_second_fade_stands_at_exactly_half_after_five_seconds() {
        // IMPLEMENTATION_PLAN.md S5's headline criterion, on the tick grid:
        // 10 s is 440 ticks, so 5 s is tick 220. 65535 is odd, so half of it is
        // 32767 - the same arithmetic docs/DMX_MERGE.md 7 uses for "65535 x 0.5".
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 10.0)], false));
        rig.go(1, GoDirection::Next);

        rig.run(0, 219);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_618);
        rig.tick(220);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_767);
        rig.tick(221);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_916);
        // And it arrives, once, at the end.
        rig.run(222, 440);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
        rig.tick(441);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
    }

    #[test]
    fn a_fade_moves_every_single_tick_so_nothing_steps_visibly() {
        // The 16-bit half of "fades interpolate in 16 bit even on 8-bit patched
        // channels": over ten seconds the internal value moves on every one of
        // the 440 ticks. The 8-bit half - that the byte it quantises to never
        // jumps - is tests/cue_playback.rs, where the encoder is in the picture.
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 10.0)], false));
        rig.go(1, GoDirection::Next);
        let mut previous = 0u16;
        for index in 0..=440 {
            rig.tick(index);
            let value = rig.value(1, AttributeType::Dimmer);
            if index > 0 {
                assert!(value > previous, "tick {index} did not move: {value}");
                assert!(value - previous <= 150, "tick {index} jumped: {value}");
            }
            previous = value;
        }
    }

    #[test]
    fn a_delay_holds_the_previous_look_and_then_fades_on_time() {
        let mut rig = Rig::new(1);
        let mut delayed = dimmer_cue("1", 65_535, 1.0);
        delayed.delay = 2.0;
        rig.load(1, &sequence(vec![delayed], false));
        rig.go(1, GoDirection::Next);

        // 2 s of delay is 88 ticks, and nothing moves until they are over.
        rig.run(0, 87);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        // Then 1 s of fade: half way through it is half way up.
        rig.run(88, 110);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_767);
        rig.run(111, 132);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
    }

    #[test]
    fn a_go_during_a_running_fade_carries_on_from_where_the_fade_had_reached() {
        // IMPLEMENTATION_PLAN.md S5: "a Go during a running fade behaves
        // deterministically". The rule is that it does *not* jump: every
        // attribute re-bases from the value it is holding at that instant and
        // moves to the new cue's target over the new cue's fade time. The
        // running fade is neither completed nor abandoned - it is overtaken.
        let mut rig = Rig::new(1);
        rig.load(
            1,
            &sequence(
                vec![dimmer_cue("1", 65_535, 10.0), dimmer_cue("2", 0, 5.0)],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.run(0, 220);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_767);

        rig.go(1, GoDirection::Next);
        rig.tick(220);
        // No jump: the second cue starts from exactly where the first had got to.
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_767);
        assert_eq!(rig.current(1), Some(1));
        // And then it fades down over the new cue's five seconds, not the old
        // cue's ten: half way is 110 ticks later, not 220.
        rig.run(221, 330);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 16_384);
        rig.run(331, 440);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
    }

    #[test]
    fn a_cue_that_does_not_mention_an_attribute_leaves_it_where_it_was() {
        // Cues track. A cue holds a delta, not a whole rig: `StoreCue` stores
        // the programmer, which is sparse by specification, so a cue that
        // changed only the position would black the stage out if a cue were
        // read as a complete look.
        let mut rig = Rig::new(1);
        rig.load(
            1,
            &sequence(
                vec![
                    cue(
                        "1",
                        0.0,
                        vec![
                            cue_part(1, AttributeType::Dimmer, 65_535),
                            cue_part(1, AttributeType::Pan, 45_000),
                        ],
                    ),
                    dimmer_cue("2", 20_000, 0.0),
                ],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.value(1, AttributeType::Pan), 45_000);

        rig.go(1, GoDirection::Next);
        rig.tick(1);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 20_000);
        assert_eq!(rig.value(1, AttributeType::Pan), 45_000);
    }

    #[test]
    fn switching_a_playback_off_fades_the_light_out_and_leaves_the_head_still() {
        // A release fades intensity to home over the cue's fade-out time and
        // holds every other attribute where it is until that is over: a moving
        // head must not swing back to centre while it is still lit.
        let mut rig = Rig::new(1);
        let mut look = cue(
            "1",
            0.0,
            vec![
                cue_part(1, AttributeType::Dimmer, 65_535),
                cue_part(1, AttributeType::Pan, 45_000),
            ],
        );
        look.fade_out = 2.0;
        rig.load(1, &sequence(vec![look], false));
        rig.go(1, GoDirection::Next);
        rig.tick(0);

        assert!(rig.off(1));
        rig.tick(1);
        assert_eq!(rig.current(1), None);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);

        // Half way through the two-second fade out.
        rig.run(2, 45);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_768);
        assert_eq!(rig.value(1, AttributeType::Pan), 45_000);

        // And at the end of it the source leaves the merge altogether, so both
        // attributes fall back to home rather than sitting at their last value.
        rig.run(46, 89);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.value(1, AttributeType::Pan), 32_768);
        assert_eq!(rig.activation(1), None);
    }

    #[test]
    fn a_playback_with_no_fade_out_time_releases_in_the_tick_it_is_switched_off() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 0.0)], false));
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
        rig.off(1);
        rig.tick(1);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.activation(1), None);
    }

    #[test]
    fn switching_off_a_playback_that_is_not_running_changes_nothing() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 1, 0.0)], false));
        assert!(!rig.off(1));
        rig.tick(0);
        assert_eq!(rig.activation(1), None);
    }

    #[test]
    fn a_playback_switched_on_starts_its_sequence_and_switching_it_on_again_does_not() {
        // `SetExecutorActive { on: true }` on a loaded executor is "start the
        // sequence" - ExecutorButtonFunction::On. Pressing it twice must not
        // restart the list under the operator, exactly as PlaybackLayer::activate
        // does not re-stamp an executor that is already on.
        let mut rig = Rig::new(1);
        rig.load(
            1,
            &sequence(
                vec![dimmer_cue("1", 10_000, 0.0), dimmer_cue("2", 20_000, 0.0)],
                false,
            ),
        );
        let player = rig.cues.player_mut(ExecutorId::new(1)).unwrap();
        assert!(player.on());
        rig.tick(0);
        assert_eq!(rig.current(1), Some(0));

        rig.go(1, GoDirection::Next);
        rig.tick(1);
        assert_eq!(rig.current(1), Some(1));

        let player = rig.cues.player_mut(ExecutorId::new(1)).unwrap();
        assert!(!player.on());
        rig.tick(2);
        assert_eq!(rig.current(1), Some(1));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 20_000);
    }

    #[test]
    fn a_go_on_a_running_playback_does_not_move_it_in_the_ltp_order() {
        // S3's decision, carried into S5: activation order is about *going*
        // active. Stepping to the next cue is not switching a playback on, so it
        // must not reorder the rig underneath the operator.
        let mut rig = Rig::new(2);
        rig.load(
            1,
            &sequence(
                vec![
                    cue("1", 0.0, vec![cue_part(1, AttributeType::Pan, 10_000)]),
                    cue("2", 0.0, vec![cue_part(1, AttributeType::Pan, 20_000)]),
                ],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        let first = rig.activation(1);

        // Executor 2 goes on afterwards, so it is later in the LTP order.
        rig.layer
            .source_mut(ExecutorId::new(2))
            .unwrap()
            .set(slot(&rig.plan, 1, AttributeType::Pan), 50_000);
        rig.layer.activate(ExecutorId::new(2));
        rig.tick(1);
        assert_eq!(rig.value(1, AttributeType::Pan), 50_000);

        // A Go on executor 1 advances its cue and leaves the order alone.
        rig.go(1, GoDirection::Next);
        rig.tick(2);
        assert_eq!(rig.current(1), Some(1));
        assert_eq!(rig.activation(1), first);
        assert_eq!(rig.value(1, AttributeType::Pan), 50_000);
    }

    #[test]
    fn a_playback_that_was_released_and_started_again_goes_to_the_top_of_the_order() {
        let mut rig = Rig::new(2);
        rig.load(1, &sequence(vec![dimmer_cue("1", 30_000, 0.0)], false));
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        let first = rig.activation(1).unwrap();
        rig.layer.activate(ExecutorId::new(2));
        let second = rig.activation(2).unwrap();
        assert!(second > first);

        rig.off(1);
        rig.tick(1);
        assert_eq!(rig.activation(1), None);
        rig.go(1, GoDirection::Next);
        rig.tick(2);
        assert!(rig.activation(1).unwrap() > second);
    }

    #[test]
    fn stepping_back_returns_to_the_previous_cue() {
        let mut rig = Rig::new(1);
        rig.load(
            1,
            &sequence(
                vec![dimmer_cue("1", 10_000, 0.0), dimmer_cue("2", 20_000, 0.0)],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.current(1), Some(1));
        rig.go(1, GoDirection::Prev);
        rig.tick(1);
        assert_eq!(rig.current(1), Some(0));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 10_000);
    }

    #[test]
    fn two_gos_inside_one_tick_advance_two_cues() {
        // Commands are drained before the frame they affect is rendered, so two
        // presses inside one 23 ms slot are two steps. They share one start
        // instant, which is what keeps the behaviour deterministic.
        let mut rig = Rig::new(1);
        rig.load(
            1,
            &sequence(
                vec![
                    dimmer_cue("1", 10_000, 0.0),
                    dimmer_cue("2", 20_000, 0.0),
                    dimmer_cue("3", 30_000, 0.0),
                ],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.current(1), Some(1));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 20_000);
    }

    #[test]
    fn a_follow_cue_starts_when_the_cue_before_it_has_finished_fading() {
        let mut rig = Rig::new(1);
        let mut follower = dimmer_cue("2", 40_000, 0.0);
        follower.trigger = CueTrigger::Follow;
        rig.load(
            1,
            &sequence(vec![dimmer_cue("1", 10_000, 1.0), follower], false),
        );
        rig.go(1, GoDirection::Next);

        // One second of fade in, so 44 ticks.
        rig.run(0, 43);
        assert_eq!(rig.current(1), Some(0));
        rig.tick(44);
        // The fade finished on this tick, and the cue that follows it starts
        // from the value it finished at.
        assert_eq!(rig.current(1), Some(1));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 10_000);
        rig.tick(45);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 40_000);
    }

    #[test]
    fn a_follow_waits_for_the_delay_as_well_as_the_fade() {
        let mut rig = Rig::new(1);
        let mut first = dimmer_cue("1", 10_000, 1.0);
        first.delay = 1.0;
        let mut follower = dimmer_cue("2", 40_000, 0.0);
        follower.trigger = CueTrigger::Follow;
        rig.load(1, &sequence(vec![first, follower], false));
        rig.go(1, GoDirection::Next);
        rig.run(0, 87);
        assert_eq!(rig.current(1), Some(0));
        rig.tick(88);
        assert_eq!(rig.current(1), Some(1));
    }

    #[test]
    fn a_timed_cue_starts_that_long_after_the_cue_before_it_started() {
        let mut rig = Rig::new(1);
        let mut timed = dimmer_cue("2", 40_000, 0.0);
        timed.trigger = CueTrigger::Time;
        timed.trigger_time = Some(2.0);
        rig.load(
            1,
            &sequence(vec![dimmer_cue("1", 10_000, 0.0), timed], false),
        );
        rig.go(1, GoDirection::Next);
        rig.run(0, 87);
        assert_eq!(rig.current(1), Some(0));
        rig.tick(88);
        assert_eq!(rig.current(1), Some(1));
    }

    #[test]
    fn a_timed_cue_with_no_time_and_a_sound_cue_both_wait_for_a_go() {
        // `Sound` is deferred past V1 (IMPLEMENTATION_PLAN.md S5), and a `Time`
        // cue with no time is under-specified. Neither may fire by itself: a cue
        // that runs away on its own during a show is worse than one that waits.
        for trigger in [CueTrigger::Time, CueTrigger::Sound] {
            let mut rig = Rig::new(1);
            let mut waiting = dimmer_cue("2", 40_000, 0.0);
            waiting.trigger = trigger;
            waiting.trigger_time = None;
            rig.load(
                1,
                &sequence(vec![dimmer_cue("1", 10_000, 0.0), waiting], false),
            );
            rig.go(1, GoDirection::Next);
            rig.run(0, 500);
            assert_eq!(rig.current(1), Some(0), "{trigger:?}");
            rig.go(1, GoDirection::Next);
            rig.tick(501);
            assert_eq!(rig.current(1), Some(1), "{trigger:?}");
        }
    }

    #[test]
    fn a_chain_of_instant_follows_advances_by_one_cue_per_tick() {
        // The fuse: a follow chain with no times in it must not spin inside one
        // tick. At most one cue starts per tick, whatever the times say.
        let mut rig = Rig::new(1);
        let cues = (1..=4)
            .map(|number| {
                let mut cue = dimmer_cue(&number.to_string(), number * 1_000, 0.0);
                cue.trigger = CueTrigger::Follow;
                cue
            })
            .collect();
        rig.load(1, &sequence(cues, false));
        rig.go(1, GoDirection::Next);
        for index in 0..4u64 {
            rig.tick(index);
            assert_eq!(
                rig.value(1, AttributeType::Dimmer),
                (index as u16 + 1) * 1_000
            );
        }
        // The last cue of a list that does not loop stays put.
        rig.run(4, 10);
        assert_eq!(rig.current(1), Some(3));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 4_000);
    }

    #[test]
    fn a_looping_sequence_of_follows_comes_back_round_to_the_first_cue() {
        let mut rig = Rig::new(1);
        let cues = (1..=3)
            .map(|number| {
                let mut cue = dimmer_cue(&number.to_string(), number * 1_000, 0.0);
                cue.trigger = CueTrigger::Follow;
                cue
            })
            .collect();
        rig.load(1, &sequence(cues, true));
        rig.go(1, GoDirection::Next);
        for index in 0..9u64 {
            rig.tick(index);
            let expected = (index % 3) as u16 * 1_000 + 1_000;
            assert_eq!(
                rig.value(1, AttributeType::Dimmer),
                expected,
                "tick {index}"
            );
        }
    }

    #[test]
    fn a_player_over_a_sequence_with_no_cues_has_nothing_to_do() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(Vec::new(), false));
        assert!(!rig.go(1, GoDirection::Next));
        rig.run(0, 5);
        assert_eq!(rig.current(1), None);
        assert_eq!(rig.activation(1), None);
        assert_eq!(rig.values(), [0, 32_768, 0, 32_768, 0, 32_768]);
    }

    #[test]
    fn a_running_cue_that_provides_nothing_still_holds_the_playback_active() {
        // An empty cue is a legitimate placeholder. It contributes no values, so
        // every attribute stays at home - but the executor is running, and its
        // place in the activation order is real.
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![cue("1", 0.0, Vec::new())], false));
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.activation(1), Some(0));
        assert_eq!(rig.values(), [0, 32_768, 0, 32_768, 0, 32_768]);
    }

    #[test]
    fn loading_a_new_sequence_replaces_what_the_playback_was_holding() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 50_000, 0.0)], false));
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 50_000);

        rig.load(
            1,
            &sequence(
                vec![cue("1", 0.0, vec![cue_part(2, AttributeType::Pan, 1_000)])],
                false,
            ),
        );
        rig.tick(1);
        assert_eq!(rig.current(1), None);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.activation(1), None);
        rig.go(1, GoDirection::Next);
        rig.tick(2);
        assert_eq!(rig.value(2, AttributeType::Pan), 1_000);
    }

    #[test]
    fn unloading_a_sequence_takes_its_playback_out_of_the_merge() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 50_000, 0.0)], false));
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 50_000);

        rig.cues.player_mut(ExecutorId::new(1)).unwrap().unload();
        rig.tick(1);
        assert!(!rig.cues.player(ExecutorId::new(1)).unwrap().is_loaded());
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.activation(1), None);
        // And an unloaded player leaves the source alone from then on.
        rig.layer
            .source_mut(ExecutorId::new(1))
            .unwrap()
            .set(slot(&rig.plan, 1, AttributeType::Dimmer), 12_345);
        rig.layer.activate(ExecutorId::new(1));
        rig.tick(2);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 12_345);
        // Unloading twice is not an error, and does not re-clear the source.
        rig.cues.player_mut(ExecutorId::new(1)).unwrap().unload();
        rig.tick(3);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 12_345);
    }

    #[test]
    fn a_player_reports_the_value_it_is_holding_for_a_slot() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 50_000, 10.0)], false));
        let player = rig.cues.player(ExecutorId::new(1)).unwrap();
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Dimmer)),
            None
        );
        rig.go(1, GoDirection::Next);
        rig.run(0, 220);
        let player = rig.cues.player(ExecutorId::new(1)).unwrap();
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Dimmer)),
            Some(25_000)
        );
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Pan)),
            None
        );
        assert_eq!(player.provides(usize::MAX), None);
        assert_eq!(player.executor(), ExecutorId::new(1));
        assert!(player.sequence().is_some());
    }

    #[test]
    fn an_executor_the_layer_does_not_have_has_no_player() {
        let mut rig = Rig::new(2);
        assert_eq!(rig.cues.player_count(), 2);
        assert!(rig.cues.player(ExecutorId::new(9)).is_none());
        assert!(rig.cues.player_mut(ExecutorId::new(9)).is_none());
    }

    #[test]
    fn a_player_with_no_sequence_answers_no_to_everything() {
        let mut rig = Rig::new(1);
        let player = rig.cues.player_mut(ExecutorId::new(1)).unwrap();
        assert!(!player.go(GoDirection::Next));
        assert!(!player.on());
        assert!(!player.off());
        assert!(player.sequence().is_none());
        assert_eq!(player.current_cue(), None);
        assert_eq!(player.provides(0), None);
    }

    #[test]
    fn a_release_leaves_the_attributes_the_playback_was_not_holding_alone() {
        // Only the second cue touches the pan, and it was never played, so the
        // release has nothing to do for that slot - and must not invent a value
        // for it on the way out.
        let mut rig = Rig::new(1);
        let mut first = dimmer_cue("1", 65_535, 0.0);
        first.fade_out = 1.0;
        rig.load(
            1,
            &sequence(
                vec![
                    first,
                    cue("2", 0.0, vec![cue_part(1, AttributeType::Pan, 60_000)]),
                ],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        let player = rig.cues.player(ExecutorId::new(1)).unwrap();
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Pan)),
            None
        );

        rig.off(1);
        rig.run(1, 23);
        // Half way through the one-second fade out, the dimmer is on its way
        // down and the pan is still nobody's business.
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_768);
        let player = rig.cues.player(ExecutorId::new(1)).unwrap();
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Pan)),
            None
        );
        assert_eq!(rig.value(1, AttributeType::Pan), 32_768);
    }

    #[test]
    fn an_impossible_cue_index_or_a_short_buffer_degrades_instead_of_panicking() {
        // `begin`, `release` and `triggers` are only ever handed indices that
        // `step` produced and a buffer the sequence sized, so none of this can
        // happen. It is asserted anyway: CLAUDE.md's zero-crash invariant costs
        // a frame for every panic on the tick, and these three functions are
        // where an off-by-one in a later session would land.
        let plan = SequencePlan::build(
            &plan(),
            &sequence(
                vec![cue("1", 0.0, vec![cue_part(1, AttributeType::Dimmer, 500)])],
                false,
            ),
        )
        .unwrap();
        let mut entries = vec![Entry::default(); plan.slot_count()];
        assert_eq!(begin(&plan, &mut entries, 99), 0);
        release(&plan, &mut entries, 99);
        assert!(!triggers(&plan, 99, 0, u64::MAX));
        assert!(!triggers(&plan, 0, 99, u64::MAX));

        // A buffer too short for the sequence loses values rather than the show.
        let mut short: Vec<Entry> = Vec::new();
        assert_eq!(begin(&plan, &mut short, 0), 0);
        assert!(short.is_empty());
    }

    #[test]
    fn a_player_with_no_sequence_advances_to_nothing_and_writes_nothing() {
        // `CueLayer::advance` never calls these on an unloaded player. They are
        // reachable on their own, so they answer for themselves.
        let plan = plan();
        let mut layer = PlaybackLayer::new(&plan, [ExecutorId::new(1)]).unwrap();
        let mut cues = CueLayer::for_layer(&layer);
        let player = cues.player_mut(ExecutorId::new(1)).unwrap();
        assert_eq!(player.advance(7), Change::None);

        let source = layer.source_mut(ExecutorId::new(1)).unwrap();
        source.set(0, 1_234);
        player.write(source);
        assert!(source.is_empty());
    }

    #[test]
    fn two_playbacks_run_their_own_cue_lists_at_the_same_time() {
        let mut rig = Rig::new(2);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 10.0)], false));
        rig.load(
            2,
            &sequence(
                vec![cue(
                    "1",
                    10.0,
                    vec![cue_part(2, AttributeType::Pan, 65_535)],
                )],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.go(2, GoDirection::Next);
        rig.run(0, 220);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_767);
        // Fixture 2's pan homes at 32768 and fades to full, so half way up it is
        // three quarters of the way across.
        assert_eq!(rig.value(2, AttributeType::Pan), 49_151);
    }
}
