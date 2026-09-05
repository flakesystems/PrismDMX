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
//! **Cues track, and tracking is computed rather than accumulated** (S48). A cue
//! holds the values it names and nothing else; an attribute it does not mention
//! keeps whatever an *earlier cue of the same list* left it at. The alternative —
//! every cue is a complete look — is not available: `Command::StoreCue` stores
//! the programmer, which `docs/DMX_MERGE.md` §3 makes sparse by specification, so
//! a cue recorded after moving one head would black the rest of the stage out.
//!
//! What changed in S48 is the second half of that sentence. Until then an
//! unmentioned attribute kept whatever *this playback happened to be holding*,
//! which is the same thing only if the list was walked from the top. Jump to cue
//! 7 and then to cue 3, and cue 3 put out cue 5's values for everything it did
//! not name — the same cue, two outputs, and which one you got depended on where
//! you had been. So every cue entry now resolves through
//! [`SequencePlan::tracked`]: a table built off the tick that says, for each slot
//! and each cue, what a walk from the first cue would leave it at.
//!
//! **Resolving always is what makes it one rule rather than two.** A forward Go
//! lands on exactly the values it landed on before — the tracking state at cue
//! *i* is the state at *i−1* with cue *i*'s own parts over it, which is what the
//! old accumulation produced — so nothing about walking a list moved. A `Goto`,
//! a backwards Go and a start now land there too, and that is the whole session.
//!
//! **An attribute that leaves the tracking state is released, not held.** A
//! `prism_domain::CueTracking::CueOnly` value taken back, or a jump to a cue
//! above the one that introduced it, ends the playback's claim on that slot: an
//! intensity fades to home over the incoming cue's fade time and everything else
//! holds where it is, which is §2.3's asymmetry applied per attribute rather than
//! per playback. When that fade is over the slot is dropped and the merge falls
//! through to whatever is under it.
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
//!
//! # Speed, and why time is accumulated rather than subtracted (S34)
//!
//! Until S34 a player read its position as `tick - started`, which is exact and
//! has no rate in it. A *speed master* — `docs/DMX_MERGE.md` §4 item 3, named
//! there since S3 with nothing implementing it — is a rate, so the position is
//! now **accumulated**: every tick adds `speed` sixty-fourths… precisely,
//! `prism_domain::SPEED_UNITY`ths of a tick, and the remainder is carried. At
//! [`prism_domain::SPEED_UNITY`] that adds exactly one per tick and every fade
//! in the project is the number it was before, which is what the existing tests
//! assert; the accumulator is what makes any other rate possible at all.
//!
//! It advances by the number of tick *slots* that have passed rather than by
//! one, so a tick the scheduler missed still moves the fade forward by the time
//! it really took. A cue list that fell behind wall-clock time after a stall
//! would be a show drifting away from its own timing.
//!
//! **A tap learns the speed against the running cue.** Two taps within
//! [`TAP_WINDOW`] mean *this transition should take that long*, so the rate is
//! the cue's own transition time divided by the tapped interval. A cue with no
//! transition time has no rate to learn and a tap against one changes nothing.
//!
//! # The crossfade is the same transition with the fader for a clock
//!
//! `ExecutorFaderFunction::XFade` (`ARCHITECTURE_SPEC.md` §6) is the third of
//! `docs/MCU_MAPPING.md` §4.1's unresolved rows. A crossfade does not replace
//! the transition — the same `from`, `to` and cue traversal are used — it
//! replaces the *clock*: progress is how far the fader has travelled from where
//! it stood when the stroke began, towards the end it is heading for.
//!
//! # Two modes, and the state between two cues — S51, punch-list B36
//!
//! What S34 built was one mode and it had a hole in it, which is the entry: one
//! movement did one fade and then *the desk drove the fader back to nought* for
//! the next one. Two things replace it.
//!
//! **A stroke is the unit, and there are two kinds of it.** A [`Stroke`] is one
//! journey of the fader from where it rested to an end of its travel, and it
//! carries one transition. What that transition *is* depends on the mode and on
//! the direction, and that is the whole of the two modes:
//!
//! | Mode | Pushed up | Pulled down |
//! |---|---|---|
//! | [`CrossfadeMode::XFade`] | crossfade to the next cue | crossfade to the one after |
//! | [`CrossfadeMode::Fade`] | fade the current cue **out** | fade the next cue **in** |
//!
//! So in `XFade` each half of the travel advances the list by one cue and the
//! stage never goes dark; in `Fade` a full up-and-down advances it by one, via
//! black. Either way an operator walks a cue list by moving one fader up and
//! down and never lifts their hand.
//!
//! **Nothing ever moves the fader.** A completed stroke is *done* and holds its
//! transition at the end; the fader is then simply somewhere, and the next
//! movement — which can only be back the other way, because it is at an end —
//! arms the next stroke from there. There is no re-basing, no Go required, and
//! nothing to drive a motor to. `prismd::surface` is the other half of that
//! rule: a crossfade fader is never written (`ExecutorFaderFunction::
//! desk_may_move_it`).
//!
//! **A stroke stopped half way is the state between two cues**, and it is a
//! state this player can hold indefinitely: every entry sits at
//! `interpolate(from, to, progress)`, the clock is not consulted, and the next
//! tick computes the same numbers. That is what makes the frames a recorded
//! fader walk produces byte-identical twice over, which is how B36 is asserted.
//!
//! **Arming is lazy, and that is what keeps the cue readout honest.** A stroke
//! is armed by the first movement away from where the fader rested, not before,
//! so a playback sitting on cue 3 with an untouched fader reads *cue 3*. The
//! moment the operator starts to move, the desk is on its way to cue 4 and says
//! so — which is exactly what pressing Go does.
//!
//! **And a reversal all the way back abandons the stroke.** Pulling a fader
//! back to where it started means *not that*: the entries are already exactly
//! where they were, so the cue pointer goes back with them and the next
//! movement starts a fresh stroke in whichever direction it goes.

use prism_domain::{CrossfadeMode, CueTrigger, GoDirection, PlaybackId, SPEED_UNITY};

use crate::cue::{CuePlan, SequencePlan, interpolate};
use crate::playback::{PlaybackLayer, PlaybackSource};

/// Full travel of a crossfade fader, and the denominator its progress is
/// expressed over. The same `65535` every level in this project is measured in.
const FULL: u16 = u16::MAX;

/// How long two taps may be apart and still be one measurement.
///
/// Four seconds is fifteen beats a minute — slower than any tempo anybody taps
/// — so a tap after this is the *first* tap of a new measurement rather than an
/// absurd rate learned from an operator who walked away. In ticks, because that
/// is the only clock this module has.
pub const TAP_WINDOW: u64 = 4 * crate::tick::TICK_HZ;

/// One journey of a manual crossfade fader, and the transition it drives —
/// **S51, B36**.
///
/// Armed by the first movement away from where the fader was resting, and
/// finished when the fader reaches the end it set off for. See the module
/// documentation for the two modes and for why arming is lazy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stroke {
    /// Where the fader stood when this stroke was armed.
    origin: u16,
    /// The end it is heading for — `0` or [`FULL`], and never anything else: a
    /// stroke is a journey to an *end*, which is what makes the next one go the
    /// other way without anything having to remember a direction.
    target: u16,
    /// The cue the playback was standing on when this was armed, so a full
    /// reversal can put the pointer back where it found it.
    ///
    /// `None` when the playback was stopped, in which case a reversal leaves it
    /// stopped.
    from_cue: Option<usize>,
    /// Whether the fader has reached [`Self::target`].
    ///
    /// A finished stroke holds its transition at the end rather than handing
    /// the clock back: the entries stay where the fader put them until the
    /// fader moves again, which is what *nothing springs back* means one layer
    /// down.
    done: bool,
}

impl Stroke {
    /// How far through its transition the fader has been pushed, `0..=FULL`.
    ///
    /// Travel **towards the target only**: a fader pushed back the way it came
    /// un-does the crossfade, which is what an operator who changed their mind
    /// means by it.
    fn progress(self, position: u16) -> u16 {
        if self.done {
            return FULL;
        }
        let span = self.origin.abs_diff(self.target);
        if span == 0 {
            return FULL;
        }
        let travelled = if self.target > self.origin {
            position.saturating_sub(self.origin)
        } else {
            self.origin.saturating_sub(position)
        };
        let scaled = (u32::from(travelled) * u32::from(FULL)).div_euclid(u32::from(span));
        u16::try_from(scaled).unwrap_or(FULL)
    }
}

impl Manual {
    /// How far through its transition this fader has been pushed, or `None`
    /// when no stroke is armed.
    fn progress(self) -> Option<u16> {
        self.stroke.map(|stroke| stroke.progress(self.position))
    }

    /// Moves the fader, and says what the player has to do about it.
    ///
    /// The whole of the state machine, and it is four cases:
    ///
    /// 1. **No stroke, and the fader moved.** Arm one, from where it was, in
    ///    the direction it went — [`Arm::NextCue`] or [`Arm::FadeOut`]
    ///    depending on the mode and the direction.
    /// 2. **A stroke that has just arrived.** Mark it done; the transition
    ///    holds at its end and nothing else happens until the fader leaves.
    /// 3. **A done stroke the fader has left.** Retire it and arm the next one
    ///    from the end it was sitting at, which is why an operator never lifts
    ///    their hand.
    /// 4. **A stroke reversed all the way back to its origin.** Abandon it —
    ///    the entries are already exactly where they were.
    fn moved(&mut self, position: u16, current: Option<usize>) -> Arm {
        let previous = self.position;
        self.position = position;
        let Some(stroke) = self.stroke else {
            return self.arm(previous, position, current);
        };
        if stroke.done {
            if position == stroke.target {
                return Arm::Nothing;
            }
            // Off the end it landed on, so the next journey begins here.
            self.stroke = None;
            return self.arm(stroke.target, position, current);
        }
        if stroke.progress(position) == FULL {
            self.stroke = Some(Stroke {
                done: true,
                ..stroke
            });
            return Arm::Nothing;
        }
        // Back to where it set off from, or past it: the operator changed their
        // mind, and the transition is already exactly where it started.
        let reversed = if stroke.target > stroke.origin {
            position <= stroke.origin
        } else {
            position >= stroke.origin
        };
        if reversed {
            self.stroke = None;
            return Arm::Abandon(stroke.from_cue);
        }
        Arm::Nothing
    }

    /// Arms a stroke from `origin` towards the end `position` is heading for.
    fn arm(&mut self, origin: u16, position: u16, current: Option<usize>) -> Arm {
        if position == origin {
            return Arm::Nothing;
        }
        let target = if position > origin { FULL } else { 0 };
        self.stroke = Some(Stroke {
            origin,
            target,
            from_cue: current,
            // **A movement that goes all the way in one step arrives in one
            // step.** A pointer drag reports every few milliseconds and a
            // motorised fader slammed to an end reports once, so the stroke has
            // to be able to be born finished — otherwise the next movement off
            // that end would be read as travel *back along this one* and the
            // transition would run backwards.
            done: position == target,
        });
        // **The two modes, in one expression.** `XFade` heads for the next cue
        // whichever way the fader goes; `Fade` fades the current cue out on the
        // way up and the next one in on the way down, which is the owner's own
        // wording in B36.
        match (self.mode, target) {
            (CrossfadeMode::XFade, _) | (CrossfadeMode::Fade, 0) => Arm::NextCue,
            (CrossfadeMode::Fade, _) => Arm::FadeOut,
        }
    }
}

/// A manual crossfade fader: which of the two modes it is, where it stands, and
/// the stroke in progress if it is mid-journey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Manual {
    /// Which of the two modes — [`CrossfadeMode`].
    mode: CrossfadeMode,
    /// Where the fader is now. When no stroke is armed this is also where it is
    /// *resting*, which is what the next stroke takes as its origin.
    position: u16,
    /// The journey in progress, if the fader has moved off its rest.
    stroke: Option<Stroke>,
}

/// What a fader movement asks the player to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    /// Nothing: the fader moved inside a stroke that is already armed, or did
    /// not move at all.
    Nothing,
    /// Begin the next cue — an `XFade` stroke in either direction, and the
    /// downward half of a `Fade`.
    NextCue,
    /// Fade the current cue out without leaving it — the upward half of a
    /// `Fade`.
    FadeOut,
    /// Put the pointer back where the abandoned stroke found it.
    Abandon(Option<usize>),
}

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
    /// Whether this transition ends with the playback letting the slot go —
    /// S48.
    ///
    /// Set when an attribute leaves the tracking state: a cue-only value taken
    /// back, or a jump to a cue above the one that first asserted it. The
    /// transition runs like any other and [`Entry::live`] goes false when it is
    /// over, which is what hands the slot back to the merge.
    ///
    /// It is per **entry** and not per player on purpose. `off` releases the
    /// whole playback at once and keeps its own all-or-nothing rule, because a
    /// playback that left the merge one attribute at a time would take its LTP
    /// slots away in an order nobody chose.
    releasing: bool,
}

/// What a tick's worth of playback did to the source's place in the merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// Nothing: the source was already where it should be.
    None,
    /// The playback started, so its source goes active and is stamped with a
    /// fresh activation counter — the top of the LTP order.
    Activate,
    /// The playback finished releasing and leaves the merge.
    Deactivate,
}

/// One playback: where it is in its cue list, and what it is holding.
///
/// **It belonged to an executor until S40** and now belongs to a
/// [`PlaybackId`], which may be a cue list on no fader - see that type.
#[derive(Debug, Clone)]
pub struct CuePlayer {
    playback: PlaybackId,
    sequence: Option<SequencePlan>,
    entries: Box<[Entry]>,
    /// The cue being played, or `None` when the playback is stopped or releasing.
    current: Option<usize>,
    /// Scaled ticks since the current transition started — the player's own
    /// clock, which runs at [`Self::speed`] rather than at the tick's rate.
    elapsed: u64,
    /// The part of a scaled tick that has not added up to a whole one yet,
    /// `0..SPEED_UNITY`. Carrying it is what makes a rate exact over a long fade
    /// instead of losing a fraction every tick.
    remainder: u32,
    /// The tick index the last [`Self::advance`] was given, so a missed tick
    /// still moves the fade forward by the time it really took.
    last_tick: Option<u64>,
    /// Playback rate in units of [`SPEED_UNITY`] — the speed master.
    speed: u16,
    /// Ticks of delay before the current transition moves anything.
    delay: u64,
    /// Set by a command, consumed by the next [`Self::advance`]: commands are
    /// applied before the tick that renders them and do not know its index.
    restart: bool,
    /// Set by [`Self::tap`], consumed by the next [`Self::advance`] — for the
    /// same reason: a tap is a moment and a command does not know which one.
    tap: bool,
    /// When the previous tap landed, for the interval the next one measures.
    last_tap: Option<u64>,
    /// The manual crossfade fader, while the executor has one on it.
    manual: Option<Manual>,
    /// Whether a held `Flash` is what started this playback, so releasing it
    /// stops what it started and leaves alone what it did not.
    flashed: bool,
    /// Whether this player has told the layer its source is active.
    active: bool,
    /// Set by [`Self::unload`], so the source it was driving is cleaned up once.
    flush: bool,
}

impl CuePlayer {
    /// A player for one playback, with no sequence on it.
    #[must_use]
    fn new(playback: PlaybackId) -> Self {
        Self {
            playback,
            sequence: None,
            entries: Box::default(),
            current: None,
            elapsed: 0,
            remainder: 0,
            last_tick: None,
            speed: SPEED_UNITY,
            delay: 0,
            restart: false,
            tap: false,
            last_tap: None,
            manual: None,
            flashed: false,
            active: false,
            flush: false,
        }
    }

    /// Which playback this is.
    #[must_use]
    pub const fn playback(&self) -> PlaybackId {
        self.playback
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

    /// Puts a compiled sequence on this playback.
    ///
    /// Allocates the working memory, so this is a set-up operation and not
    /// something to do while the tick is running. The playback is left stopped;
    /// whatever it was holding is released without a fade on the next tick,
    /// because the values belonged to a cue list that is no longer there.
    pub fn load(&mut self, sequence: SequencePlan) {
        self.entries = vec![Entry::default(); sequence.slot_count()].into_boxed_slice();
        self.sequence = Some(sequence);
        self.current = None;
        self.elapsed = 0;
        self.remainder = 0;
        self.delay = 0;
        self.restart = false;
        self.flashed = false;
        // `speed`, `manual` and `last_tap` deliberately survive: they are the
        // *executor's* settings, not the sequence's, and a cue list swapped
        // under a fader that is holding a rate must not silently return to 1x.
        self.flush = false;
        // `active` deliberately survives: it records what the *layer* was last
        // told, and the next tick has to take the old source back out of the
        // merge.
    }

    /// Takes the sequence off this playback and gives the source back.
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
            manual,
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
        // **A Go takes the transition back** — S51, B36. Any stroke in progress
        // is abandoned and the new cue runs on the clock; the fader is left
        // exactly where the operator's hand left it, and its next movement arms
        // a fresh stroke from there. Nothing is re-based and nothing is driven.
        if let Some(manual) = manual.as_mut() {
            manual.stroke = None;
        }
        true
    }

    /// Jumps straight to one cue of the list, starting it if it was stopped.
    ///
    /// **S40's `Goto`**, and the command that had no message at any layer before
    /// it: there was no `Command::Goto`, no `TickCommand` for one and nothing
    /// here. It is [`Self::go`] without the stepping — the cue is entered with
    /// its own delay and fade, exactly as if the list had arrived there — so a
    /// crossfade re-bases the same way and a `Follow` after it triggers on time.
    ///
    /// Returns `false` if there is no sequence or the index is past the end of
    /// it. The **index** rather than the number, because a cue number is a
    /// string and the tick resolves nothing (`ARCHITECTURE_SPEC.md` §3.1); the
    /// daemon's core thread turns one into the other.
    pub fn goto(&mut self, index: usize) -> bool {
        let Self {
            sequence,
            entries,
            current,
            delay,
            restart,
            manual,
            ..
        } = self;
        let Some(plan) = sequence.as_ref() else {
            return false;
        };
        if index >= plan.cue_count() {
            return false;
        }
        *delay = begin(plan, entries, index);
        *current = Some(index);
        *restart = true;
        if let Some(manual) = manual.as_mut() {
            manual.stroke = None;
        }
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

    /// The playback rate, in units of [`SPEED_UNITY`].
    #[must_use]
    pub const fn speed(&self) -> u16 {
        self.speed
    }

    /// Sets the playback rate. `0` freezes the playback where it is.
    ///
    /// Takes effect on the next tick and does not disturb the transition in
    /// progress: the position already reached is kept and only the rate it goes
    /// on advancing at changes, so a speed master moved mid-fade slows the fade
    /// rather than jumping it.
    pub const fn set_speed(&mut self, speed: u16) {
        self.speed = speed;
    }

    /// Records a tap of `ExecutorButtonFunction::LearnSpeed`.
    ///
    /// The moment is taken on the next [`Self::advance`], for the same reason a
    /// Go is: a command is applied before the tick that renders it and does not
    /// know its index.
    pub const fn tap(&mut self) {
        self.tap = true;
    }

    /// Where the manual crossfade fader stands, if this executor has one.
    ///
    /// A **reading**, and nothing acts on it: since S51 nothing writes a
    /// crossfade fader's position anywhere (`ExecutorFaderFunction::
    /// desk_may_move_it`), so this exists for a test and for a readback and not
    /// for a motor.
    #[must_use]
    pub const fn crossfade(&self) -> Option<u16> {
        match self.manual {
            Some(manual) => Some(manual.position),
            None => None,
        }
    }

    /// How far through its transition the manual crossfade has been pushed,
    /// `0..=65535`, or nothing when no stroke is in progress.
    ///
    /// `None` also for a fader that is engaged and has not moved: there is no
    /// transition to be part way through until a stroke is armed, which is what
    /// keeps the cue readout honest while a hand rests on a fader.
    #[must_use]
    pub fn crossfade_progress(&self) -> Option<u16> {
        self.manual.and_then(Manual::progress)
    }

    /// Moves the manual crossfade fader — **S51, B36**.
    ///
    /// Engaging one takes the fader where it stands and arms **nothing**, so
    /// switching a fader to a crossfade never moves any light by itself and the
    /// playback goes on reading the cue it is actually on. The first movement is
    /// what arms a stroke; see [`Manual::moved`] for the four cases and the
    /// module documentation for the two modes.
    ///
    /// Changing the mode under a stroke abandons it and leaves every attribute
    /// where it stands: a fader whose job changed mid-gesture is not in the
    /// middle of anything any more.
    pub fn set_crossfade(&mut self, mode: CrossfadeMode, position: u16) {
        let Some(manual) = self.manual.as_mut() else {
            self.manual = Some(Manual {
                mode,
                position,
                stroke: None,
            });
            return;
        };
        if manual.mode != mode {
            *manual = Manual {
                mode,
                position,
                stroke: None,
            };
            // **Held where it stands, not handed back to the clock.** The
            // transition this stroke was driving is half done, and the player's
            // own clock has been running the whole time — giving it back would
            // finish the fade at whatever `elapsed` happens to say, which is a
            // jump. A fader whose job changed mid-gesture is not in the middle
            // of anything any more, so the honest answer is *stay*.
            hold(&mut self.entries);
            return;
        }
        let armed = manual.moved(position, self.current);
        let Some(plan) = self.sequence.as_ref() else {
            return;
        };
        match armed {
            Arm::Nothing => {}
            Arm::NextCue => {
                if let Some(next) = plan.step(self.current, GoDirection::Next) {
                    self.delay = begin(plan, &mut self.entries, next);
                    self.current = Some(next);
                } else {
                    // Nothing to go to, so there is no stroke to be in.
                    self.manual = self.manual.map(|manual| Manual {
                        stroke: None,
                        ..manual
                    });
                }
            }
            Arm::FadeOut => {
                // **The upward half of a `Fade`.** The cue pointer stays where
                // it is — the list has not moved, the light is going out — and
                // the entries are given the release's targets without the
                // release's dropping: an intensity heads home, everything else
                // holds, and nothing leaves the merge, because the downward half
                // has to fade the next cue in from here.
                let outgoing = self.current;
                match outgoing {
                    Some(index) => {
                        fade_out(plan, &mut self.entries, index);
                        self.delay = 0;
                    }
                    None => {
                        self.manual = self.manual.map(|manual| Manual {
                            stroke: None,
                            ..manual
                        });
                    }
                }
            }
            Arm::Abandon(from_cue) => {
                // The entries are already exactly where they were, so putting
                // the pointer back is the whole of it: `begin` from here writes
                // the values that are already out.
                self.current = from_cue;
                if let Some(index) = from_cue {
                    self.delay = begin(plan, &mut self.entries, index);
                }
            }
        }
    }

    /// Takes the manual crossfade off, so the transition runs on time again.
    pub const fn clear_crossfade(&mut self) {
        self.manual = None;
    }

    /// Whether a held flash is what started this playback.
    #[must_use]
    pub const fn is_flashed(&self) -> bool {
        self.flashed
    }

    /// Records that a flash did or did not start this playback.
    ///
    /// For an executor with **no** cue list, where starting it is the layer's
    /// business rather than the player's: the flag still lives here so there is
    /// one place that remembers, and one rule — a release stops what the flash
    /// started and leaves alone what it did not.
    pub const fn mark_flashed(&mut self, flashed: bool) {
        self.flashed = flashed;
    }

    /// Starts the playback for a held flash, remembering that the flash is what
    /// started it. Returns `false` if it was already running — in which case the
    /// release must leave it running.
    pub fn flash_on(&mut self) -> bool {
        if !self.on() {
            return false;
        }
        self.flashed = true;
        true
    }

    /// Releases a held flash: stops the playback if — and only if — the flash is
    /// what started it.
    pub fn flash_off(&mut self) -> bool {
        if !self.flashed {
            return false;
        }
        self.flashed = false;
        self.off()
    }

    /// Moves the playback on to tick `tick` and works out what it is holding.
    ///
    /// Allocation-free, lock-free and clock-free: this runs on the tick.
    fn advance(&mut self, tick: u64) -> Change {
        // The tap is resolved first, because it is a statement about *this*
        // moment and the rate it produces applies from this tick on.
        self.resolve_tap(tick);
        let Self {
            sequence,
            entries,
            current,
            elapsed: clock,
            remainder,
            last_tick,
            speed,
            delay,
            restart,
            manual,
            active,
            ..
        } = self;
        let slots = last_tick.map_or(0, |last| tick.saturating_sub(last));
        *last_tick = Some(tick);
        let Some(plan) = sequence.as_ref() else {
            return Change::None;
        };
        if *restart {
            *clock = 0;
            *remainder = 0;
            *restart = false;
        } else {
            // The player's own clock: `speed` SPEED_UNITYths of a tick per tick
            // slot, with the fraction carried rather than dropped. At unity this
            // is exactly `+1` and every fade in the project is the number it was
            // before the rate existed.
            let scaled = u64::from(*speed)
                .saturating_mul(slots)
                .saturating_add(u64::from(*remainder));
            let unity = u64::from(SPEED_UNITY);
            *clock = clock.saturating_add(scaled.div_euclid(unity));
            *remainder = u32::try_from(scaled.rem_euclid(unity)).unwrap_or(0);
        }
        let elapsed = *clock;

        // Where every held attribute has got to. Evaluated *before* the trigger
        // check below, so a cue that ends on this tick reaches its target and
        // the cue that follows it starts from there rather than from one step
        // short of it.
        //
        // A manual crossfade replaces the clock and nothing else: the same
        // `from` and `to`, driven by how far the fader has travelled. It governs
        // the *cue transition* only, so a release still fades out on time.
        // A manual crossfade replaces the clock and nothing else — and only
        // while a **stroke** is armed, since S51: a fader that is engaged and
        // has not been moved leaves the transition on the clock, which is what
        // makes switching a fader to a crossfade change nothing at all.
        let manual = match manual {
            Some(fader) if current.is_some() => fader.progress(),
            _ => None,
        };
        for entry in entries.iter_mut().filter(|entry| entry.live) {
            // Whether this entry's transition is over as well as where it has
            // got to, because an entry that is releasing leaves the merge when
            // it is — S48. One expression rather than a second pass, so the two
            // answers cannot be taken from different clocks.
            let arrived = match manual {
                Some(progress) => {
                    entry.value = interpolate(
                        entry.from,
                        entry.to,
                        u64::from(progress),
                        u64::from(u16::MAX),
                    );
                    progress == u16::MAX
                }
                None if elapsed < *delay => {
                    entry.value = entry.from;
                    false
                }
                None => {
                    let since = elapsed.saturating_sub(*delay);
                    entry.value = interpolate(entry.from, entry.to, since, entry.duration);
                    since >= entry.duration
                }
            };
            if entry.releasing && arrived {
                entry.live = false;
                entry.releasing = false;
            }
        }

        // **A held stroke suspends the automatic chain** — S51, B36. A stroke
        // stopped half way is the state *between two cues*, and `elapsed` has
        // gone on running the whole time it was held; a `Follow` fired off that
        // clock would take the list somewhere while the operator was still
        // half-way into the cue before it. A cue reached **by** the fader has
        // its follow evaluated the moment the stroke is retired, which is when
        // the next one is armed.
        //
        // Follow and Time, once per tick at most. `follow_step` and not `step`:
        // a **Go** always comes round to the first cue and an automatic chain
        // only does when the list loops, which is what `Sequence::loop` has
        // always meant. `None` is the end of the chain, so the guard the two
        // used to share is the type now.
        if manual.is_none()
            && let Some(index) = *current
            && let Some(next) = plan.follow_step(index)
            && triggers(plan, index, next, elapsed)
        {
            *delay = begin(plan, entries, next);
            *current = Some(next);
            *clock = 0;
            *remainder = 0;
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

    /// Turns a pending tap into a rate, if a second tap has landed close enough
    /// to the first for the pair to mean anything.
    ///
    /// *This transition should take that long*: the rate is the cue's own
    /// transition time over the tapped interval, so tapping at the speed a cue
    /// list already runs at leaves it exactly where it was.
    fn resolve_tap(&mut self, tick: u64) {
        if !core::mem::take(&mut self.tap) {
            return;
        }
        let previous = self.last_tap.replace(tick);
        let Some(previous) = previous else {
            return;
        };
        let interval = tick.saturating_sub(previous);
        if interval == 0 || interval > TAP_WINDOW {
            // Too fast to be two taps, or too slow to be one rhythm. Either way
            // this tap is the *first* of the next measurement, which
            // `last_tap` now holds.
            return;
        }
        // The reference is the running cue's transition. A cue with no time in
        // it has no rate to learn, and inventing one would make a tap against a
        // snap cue change every fade in the list.
        let Some(base) = self
            .sequence
            .as_ref()
            .and_then(|plan| plan.cue(self.current?))
            .map(CuePlan::transition_ticks)
            .filter(|ticks| *ticks > 0)
        else {
            return;
        };
        let learned = (u128::from(base) * u128::from(SPEED_UNITY)).div_euclid(u128::from(interval));
        self.speed = u16::try_from(learned).unwrap_or(u16::MAX);
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

/// Starts cue `index`: every held attribute re-bases from where it is now and
/// takes the target the **tracking state** at that cue gives it. Returns the
/// cue's delay.
///
/// # This is the whole of S48, and it is four lines of it
///
/// The cue's own parts are not read here at all any more. [`SequencePlan::tracked`]
/// already has them — the tracking state at a cue is everything earlier cues
/// asserted with that cue's own values over the top — so asking it once per slot
/// answers *what this cue asserts* and *what it inherits* in one question, and
/// there is no order in which the two could disagree.
///
/// Every attribute **fades from where it is**, not from where the tracking state
/// says it was. That is the deliverable about fades across the boundary, and it
/// falls out of `from = entry.value`: an attribute inherited from four cues back
/// and now asserted has been sitting at its inherited value, so that is where its
/// fade starts. The tracking state is where a cue is *going*, never where it is
/// coming from.
///
/// An attribute the state does not hold at this cue is **released** rather than
/// left: `to` is home for an intensity and where it stands for everything else,
/// and [`Entry::releasing`] is what drops it when that fade is over.
fn begin(plan: &SequencePlan, entries: &mut [Entry], index: usize) -> u64 {
    let Some(cue) = plan.cue(index) else {
        return 0;
    };
    let fade = cue.fade_in();
    for (position, entry) in entries.iter_mut().enumerate() {
        let home = plan.slot(position).map_or(0, |slot| slot.home);
        match plan.tracked(position, index) {
            Some(value) => {
                if !entry.live {
                    // An attribute this playback was not holding fades in from
                    // the value it falls back to, so a cue with a fade time
                    // fades rather than snapping on its first tick.
                    entry.live = true;
                    entry.from = home;
                    entry.value = home;
                } else {
                    entry.from = entry.value;
                }
                entry.to = value;
                entry.duration = fade;
                entry.releasing = false;
            }
            None if entry.live => {
                entry.from = entry.value;
                entry.to = match plan.slot(position) {
                    Some(slot) if slot.htp => home,
                    _ => entry.value,
                };
                entry.duration = fade;
                entry.releasing = true;
            }
            None => {}
        }
    }
    cue.delay()
}

/// Freezes every held attribute where it stands — S51, B36.
///
/// What a manual transition abandoned part way through becomes. The clock has
/// been running underneath the fader the whole time, so simply dropping the
/// stroke would let `elapsed` finish the fade in one tick; making `from`, `to`
/// and `value` the same number is *stay here* said in the only terms an entry
/// has.
fn hold(entries: &mut [Entry]) {
    for entry in entries.iter_mut().filter(|entry| entry.live) {
        entry.from = entry.value;
        entry.to = entry.value;
        entry.duration = 0;
        entry.releasing = false;
    }
}

/// Fades the current cue **out** without letting go of it — S51, B36's `Fade`.
///
/// [`release`]'s targets with none of its consequences: an intensity heads home
/// and everything else holds, but the cue pointer stays where it is and nothing
/// is dropped, because the downward half of the stroke has to fade the next cue
/// in **from here**. A playback that let its slots go at the top of the fader
/// would hand the rig to whatever is underneath it for the length of an
/// operator's pause, which is the opposite of what a two-stroke fade is for.
///
/// The duration is nought and the fader is the clock: [`Stroke::progress`] is
/// what interpolates it, so an operator who stops half way is half out.
fn fade_out(plan: &SequencePlan, entries: &mut [Entry], outgoing: usize) {
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
        entry.releasing = false;
    }
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
        // The whole playback is going, so the all-at-once rule below takes over
        // from the per-attribute one: an entry that was releasing on its own
        // stops doing so and leaves with the rest.
        entry.releasing = false;
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

/// One player per playback, kept in step with a [`PlaybackLayer`].
#[derive(Debug, Clone)]
pub struct CueLayer {
    /// Sorted by playback, like the layer's sources, so a lookup is a binary
    /// search and the two can be walked together.
    players: Box<[CuePlayer]>,
}

impl CueLayer {
    /// A player for every source in a layer, none of them loaded.
    ///
    /// Built from the layer rather than from a list of playbacks, so the two
    /// cannot end up describing different sets.
    #[must_use]
    pub fn for_layer(layer: &PlaybackLayer) -> Self {
        Self {
            players: layer
                .sources()
                .iter()
                .map(|source| CuePlayer::new(source.id()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    /// How many players this layer holds.
    #[must_use]
    pub const fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Every player, in playback order.
    #[must_use]
    pub const fn players(&self) -> &[CuePlayer] {
        &self.players
    }

    fn index_of(&self, id: PlaybackId) -> Option<usize> {
        self.players
            .binary_search_by_key(&id, CuePlayer::playback)
            .ok()
    }

    /// One player by playback.
    #[must_use]
    pub fn player(&self, id: impl Into<PlaybackId>) -> Option<&CuePlayer> {
        self.players.get(self.index_of(id.into())?)
    }

    /// One player by playback, for loading a sequence or stepping it.
    pub fn player_mut(&mut self, id: impl Into<PlaybackId>) -> Option<&mut CuePlayer> {
        let index = self.index_of(id.into())?;
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
                    if let Some(source) = layer.source_mut(player.playback) {
                        source.clear();
                    }
                    layer.deactivate(player.playback);
                }
                continue;
            }
            match player.advance(tick) {
                Change::Activate => {
                    layer.activate(player.playback);
                }
                Change::Deactivate => {
                    layer.deactivate(player.playback);
                }
                Change::None => {}
            }
            if let Some(source) = layer.source_mut(player.playback) {
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
    use crate::player::{SPEED_UNITY, TAP_WINDOW};
    use crate::testkit::{cue, cue_part, moving_head, sequence};
    use prism_domain::{
        AttributeType, CrossfadeMode, Cue, CueTrigger, FixtureId, GoDirection, PlaybackId,
        Sequence, SequenceId,
    };

    /// Three moving heads: six slots, alternating HTP dimmer and LTP pan.
    fn plan() -> MergePlan {
        let head = moving_head();
        MergePlan::build((1..=3).map(|id| (FixtureId::new(id), &head, false))).unwrap()
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
            let layer = PlaybackLayer::new(&plan, (1..=executors).map(SequenceId::new)).unwrap();
            let cues = CueLayer::for_layer(&layer);
            Self { plan, layer, cues }
        }

        fn load(&mut self, executor: u32, sequence: &Sequence) {
            let compiled = SequencePlan::build(&self.plan, sequence).unwrap();
            self.cues
                .player_mut(SequenceId::new(executor))
                .unwrap()
                .load(compiled);
        }

        fn go(&mut self, executor: u32, direction: GoDirection) -> bool {
            self.cues
                .player_mut(SequenceId::new(executor))
                .unwrap()
                .go(direction)
        }

        fn off(&mut self, executor: u32) -> bool {
            self.cues
                .player_mut(SequenceId::new(executor))
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
                .player(SequenceId::new(executor))
                .unwrap()
                .current_cue()
        }

        fn activation(&self, executor: u32) -> Option<u64> {
            self.layer.source(SequenceId::new(executor))?.activation()
        }

        fn player(&mut self, executor: u32) -> &mut crate::player::CuePlayer {
            self.cues.player_mut(SequenceId::new(executor)).unwrap()
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
            .source_mut(SequenceId::new(1))
            .unwrap()
            .set(dimmer, 40_000);
        rig.layer.activate(SequenceId::new(1));
        rig.run(0, 10);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 40_000);
        assert!(!rig.cues.player(SequenceId::new(1)).unwrap().is_loaded());
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

    /// **S40's `Goto`: the cue is entered, not stepped to.**
    ///
    /// A jump to cue 3 of a list that is stopped starts it *there* — the whole
    /// point of the command — and one to a cue past the end of the list changes
    /// nothing at all, which is the tick's answer to an impossible instruction
    /// everywhere else in this crate too.
    #[test]
    fn a_goto_enters_the_cue_it_names_and_ignores_one_that_is_not_there() {
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

        assert!(rig.player(1).goto(2), "cue 3 is in the list");
        rig.run(0, 2);
        assert_eq!(rig.current(1), Some(2));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 30_000);
        // It started the list as well as moving it: a jump from stopped is a
        // start, exactly as a Go from stopped is.
        assert_eq!(rig.activation(1), Some(0));

        // Past the end: nothing moves and nothing is said.
        assert!(!rig.player(1).goto(3));
        rig.run(3, 4);
        assert_eq!(rig.current(1), Some(2));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 30_000);
    }

    /// A `Goto` on a playback with no cue list is `false` rather than a panic.
    #[test]
    fn a_goto_on_a_playback_with_no_sequence_does_nothing() {
        let mut rig = Rig::new(1);
        assert!(!rig.player(1).goto(0));
    }

    /// A jump **backwards** is a jump, not a step: the cue is entered with its
    /// own fade, so `Goto Cue 1` from cue 3 is not the same as two Go-backs.
    #[test]
    fn a_goto_backwards_enters_the_cue_rather_than_stepping_to_it() {
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
        rig.run(0, 1);
        rig.go(1, GoDirection::Next);
        rig.run(2, 3);
        assert_eq!(rig.current(1), Some(1));

        assert!(rig.player(1).goto(0));
        rig.run(4, 5);
        assert_eq!(rig.current(1), Some(0));
        assert_eq!(rig.value(1, AttributeType::Dimmer), 10_000);
    }

    /// **A cue list on no fader is a source like any other** — S40.
    ///
    /// The merge does not know the difference: `docs/DMX_MERGE.md` §2 is written
    /// about *sources*, and this one has a master at full and no keys. What the
    /// test holds is that a sequence playback and an executor playback merge
    /// together, and that LTP orders them by activation rather than by kind.
    #[test]
    fn a_sequence_playback_merges_beside_an_executor() {
        let plan = plan();
        let layer = PlaybackLayer::new(
            &plan,
            [
                PlaybackId::of_sequence(SequenceId::new(1)),
                PlaybackId::of_sequence(SequenceId::new(7)),
            ],
        )
        .unwrap();
        let mut rig = Rig {
            cues: CueLayer::for_layer(&layer),
            plan,
            layer,
        };
        let compiled = SequencePlan::build(
            &rig.plan,
            &sequence(vec![dimmer_cue("1", 40_000, 0.0)], false),
        )
        .unwrap();
        rig.cues
            .player_mut(PlaybackId::of_sequence(SequenceId::new(7)))
            .unwrap()
            .load(compiled);
        rig.cues
            .player_mut(PlaybackId::of_sequence(SequenceId::new(7)))
            .unwrap()
            .go(GoDirection::Next);

        rig.run(0, 2);

        assert_eq!(rig.value(1, AttributeType::Dimmer), 40_000);
        assert_eq!(
            rig.layer
                .source(PlaybackId::of_sequence(SequenceId::new(7)))
                .and_then(crate::playback::PlaybackSource::activation),
            Some(0),
            "the sequence playback never went active"
        );
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
        let player = rig.cues.player_mut(SequenceId::new(1)).unwrap();
        assert!(player.on());
        rig.tick(0);
        assert_eq!(rig.current(1), Some(0));

        rig.go(1, GoDirection::Next);
        rig.tick(1);
        assert_eq!(rig.current(1), Some(1));

        let player = rig.cues.player_mut(SequenceId::new(1)).unwrap();
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
            .source_mut(SequenceId::new(2))
            .unwrap()
            .set(slot(&rig.plan, 1, AttributeType::Pan), 50_000);
        rig.layer.activate(SequenceId::new(2));
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
        rig.layer.activate(SequenceId::new(2));
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

        rig.cues.player_mut(SequenceId::new(1)).unwrap().unload();
        rig.tick(1);
        assert!(!rig.cues.player(SequenceId::new(1)).unwrap().is_loaded());
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.activation(1), None);
        // And an unloaded player leaves the source alone from then on.
        rig.layer
            .source_mut(SequenceId::new(1))
            .unwrap()
            .set(slot(&rig.plan, 1, AttributeType::Dimmer), 12_345);
        rig.layer.activate(SequenceId::new(1));
        rig.tick(2);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 12_345);
        // Unloading twice is not an error, and does not re-clear the source.
        rig.cues.player_mut(SequenceId::new(1)).unwrap().unload();
        rig.tick(3);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 12_345);
    }

    #[test]
    fn a_player_reports_the_value_it_is_holding_for_a_slot() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 50_000, 10.0)], false));
        let player = rig.cues.player(SequenceId::new(1)).unwrap();
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Dimmer)),
            None
        );
        rig.go(1, GoDirection::Next);
        rig.run(0, 220);
        let player = rig.cues.player(SequenceId::new(1)).unwrap();
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Dimmer)),
            Some(25_000)
        );
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Pan)),
            None
        );
        assert_eq!(player.provides(usize::MAX), None);
        assert_eq!(player.playback(), SequenceId::new(1).into());
        assert!(player.sequence().is_some());
    }

    #[test]
    fn an_executor_the_layer_does_not_have_has_no_player() {
        let mut rig = Rig::new(2);
        assert_eq!(rig.cues.player_count(), 2);
        assert!(rig.cues.player(SequenceId::new(9)).is_none());
        assert!(rig.cues.player_mut(SequenceId::new(9)).is_none());
    }

    #[test]
    fn a_player_with_no_sequence_answers_no_to_everything() {
        let mut rig = Rig::new(1);
        let player = rig.cues.player_mut(SequenceId::new(1)).unwrap();
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
        let player = rig.cues.player(SequenceId::new(1)).unwrap();
        assert_eq!(
            player.provides(slot(&rig.plan, 1, AttributeType::Pan)),
            None
        );

        rig.off(1);
        rig.run(1, 23);
        // Half way through the one-second fade out, the dimmer is on its way
        // down and the pan is still nobody's business.
        assert_eq!(rig.value(1, AttributeType::Dimmer), 32_768);
        let player = rig.cues.player(SequenceId::new(1)).unwrap();
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
        let mut layer = PlaybackLayer::new(&plan, [SequenceId::new(1)]).unwrap();
        let mut cues = CueLayer::for_layer(&layer);
        let player = cues.player_mut(SequenceId::new(1)).unwrap();
        assert_eq!(player.advance(7), Change::None);

        let source = layer.source_mut(SequenceId::new(1)).unwrap();
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

    // -- S34: the speed master, the tap and the manual crossfade ------------

    /// A fade at half speed reaches the same values, twice as late — and the
    /// two curves are the *same* curve sampled at half the rate rather than a
    /// second implementation that happens to end in the same place.
    #[test]
    fn a_speed_master_stretches_a_fade_and_changes_nothing_else() {
        let mut unity = Rig::new(1);
        // Two seconds is 88 ticks at unity and 176 at half, so both runs finish
        // inside the window below.
        unity.load(1, &sequence(vec![dimmer_cue("1", 60_000, 2.0)], false));
        unity.go(1, GoDirection::Next);

        let mut halved = Rig::new(1);
        halved.load(1, &sequence(vec![dimmer_cue("1", 60_000, 2.0)], false));
        halved.player(1).set_speed(SPEED_UNITY / 2);
        halved.go(1, GoDirection::Next);

        // Two hundred ticks at half speed are a hundred at unity, exactly: the
        // remainder is carried rather than dropped, so the halves add up. Note
        // that this is a *sampling* claim, not an averaging one — the value at
        // halved tick 2n is the value at unity tick n, to the number.
        let mut slow = Vec::new();
        for tick in 0..=200u64 {
            halved.tick(tick);
            slow.push(halved.value(1, AttributeType::Dimmer));
        }
        for tick in 0..=100usize {
            unity.tick(tick as u64);
            assert_eq!(
                slow[tick * 2],
                unity.value(1, AttributeType::Dimmer),
                "tick {tick}"
            );
        }
        // And it really moved, so this is not two rigs sitting still together.
        assert_eq!(unity.value(1, AttributeType::Dimmer), 60_000);
        assert!(slow.iter().collect::<std::collections::BTreeSet<_>>().len() > 50);
    }

    /// A speed of zero freezes the playback where it is, and does not stop it.
    #[test]
    fn a_speed_of_zero_holds_a_fade_and_lets_it_go_on_afterwards() {
        let mut rig = Rig::new(1);
        // Five seconds is 220 ticks, so tick 110 is exactly halfway.
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 5.0)], false));
        rig.go(1, GoDirection::Next);
        rig.run(0, 110);
        let halfway = rig.value(1, AttributeType::Dimmer);
        assert!(
            (32_000..34_000).contains(&halfway),
            "expected about half, got {halfway}"
        );

        rig.player(1).set_speed(0);
        rig.run(111, 400);
        assert_eq!(
            rig.value(1, AttributeType::Dimmer),
            halfway,
            "a frozen playback moved"
        );
        assert_eq!(rig.current(1), Some(0), "a frozen playback also stopped");

        rig.player(1).set_speed(SPEED_UNITY);
        rig.run(401, 520);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
    }

    /// A tick the scheduler missed still moves the fade by the time it took.
    ///
    /// The accumulator advances by tick *slots*, not by one per call. A cue list
    /// that fell behind wall-clock time after a stall would be a show drifting
    /// away from its own timing.
    #[test]
    fn a_missed_tick_moves_the_fade_by_the_time_it_really_took() {
        let mut steady = Rig::new(1);
        steady.load(1, &sequence(vec![dimmer_cue("1", 65_535, 4.0)], false));
        steady.go(1, GoDirection::Next);
        steady.run(0, 88);

        let mut stalled = Rig::new(1);
        stalled.load(1, &sequence(vec![dimmer_cue("1", 65_535, 4.0)], false));
        stalled.go(1, GoDirection::Next);
        stalled.tick(0);
        // Forty tick slots went by with no tick at all, and then one tick.
        stalled.tick(40);
        stalled.run(41, 88);

        assert_eq!(
            stalled.value(1, AttributeType::Dimmer),
            steady.value(1, AttributeType::Dimmer)
        );
    }

    /// Two taps mean *this transition should take that long*, and tapping at the
    /// speed a list already runs at leaves it exactly where it was.
    #[test]
    fn two_taps_set_the_rate_from_the_running_cues_own_transition() {
        let mut rig = Rig::new(1);
        // A two-second fade: 88 ticks at unity.
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 2.0)], false));
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.player(1).speed(), SPEED_UNITY);

        // Tapped at 44 ticks — one second — so the transition should run twice
        // as fast.
        rig.player(1).tap();
        rig.tick(1);
        rig.player(1).tap();
        rig.tick(45);
        assert_eq!(rig.player(1).speed(), SPEED_UNITY * 2);

        // Tapped at the rhythm it is already keeping: the *scaled* transition is
        // 44 ticks now, so tapping 44 apart asks for no change at all.
        rig.player(1).tap();
        rig.tick(46);
        rig.player(1).tap();
        rig.tick(90);
        assert_eq!(rig.player(1).speed(), SPEED_UNITY * 2);
    }

    /// One tap on its own is not a rhythm, and neither is a pair four seconds
    /// apart.
    #[test]
    fn a_single_tap_and_a_pair_too_far_apart_change_nothing() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 2.0)], false));
        rig.go(1, GoDirection::Next);
        rig.player(1).tap();
        rig.run(0, 10);
        assert_eq!(rig.player(1).speed(), SPEED_UNITY, "one tap set a rate");

        rig.player(1).tap();
        rig.tick(11 + TAP_WINDOW);
        assert_eq!(
            rig.player(1).speed(),
            SPEED_UNITY,
            "two taps a window apart set a rate"
        );
        // And that far-apart tap is the *first* of the next measurement, so a
        // pair after it does work.
        rig.player(1).tap();
        rig.tick(11 + TAP_WINDOW + 44);
        assert_eq!(rig.player(1).speed(), SPEED_UNITY * 2);
    }

    /// A tap against a cue with no time in it has no rate to learn.
    #[test]
    fn a_tap_against_a_snap_cue_leaves_the_rate_alone() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 0.0)], false));
        rig.go(1, GoDirection::Next);
        rig.player(1).tap();
        rig.tick(0);
        rig.player(1).tap();
        rig.tick(44);
        assert_eq!(rig.player(1).speed(), SPEED_UNITY);
    }

    /// **A three-cue list to walk with one fader.**
    fn walkable() -> Sequence {
        sequence(
            vec![
                dimmer_cue("1", 0, 0.0),
                // Ten seconds, so *time* could not have got anywhere.
                dimmer_cue("2", 65_535, 10.0),
                dimmer_cue("3", 20_000, 10.0),
            ],
            false,
        )
    }

    /// The crossfade drives the transition from the fader instead of from time,
    /// and engaging one moves nothing by itself.
    ///
    /// **S51 turned the second half of this round** (B36). It used to need a Go
    /// before the fader did anything, because a crossfade only replaced the
    /// clock of a transition somebody else had started. The fader starts it now:
    /// the first movement off the rest *is* the Go.
    #[test]
    fn a_manual_crossfade_is_driven_by_the_fader_and_not_by_the_clock() {
        let mut rig = Rig::new(1);
        rig.load(1, &walkable());
        rig.go(1, GoDirection::Next);
        rig.run(0, 5);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);

        // Engaging the fader where it stands moves nothing and starts nothing:
        // the playback is on cue 1 and says so.
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 0);
        rig.tick(6);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.current(1), Some(0), "engaging a fader took a cue");
        assert_eq!(rig.player(1).crossfade_progress(), None);

        // Half a fader is half a fade — and the cue pointer has moved, because
        // the operator has started the move.
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 32_768);
        rig.tick(7);
        let half = rig.value(1, AttributeType::Dimmer);
        assert!(
            (32_000..34_000).contains(&half),
            "half a fader is half a fade, got {half}"
        );
        assert_eq!(rig.current(1), Some(1));

        // **And it holds there.** Fifty ticks with nothing touched, and the
        // frame does not move: a stroke stopped half way is a state, not a fade
        // that has been paused.
        rig.run(8, 58);
        assert_eq!(rig.value(1, AttributeType::Dimmer), half);

        // Pushed back down to where it set off from: the operator changed their
        // mind, the values are back and so is the cue pointer.
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 0);
        rig.tick(59);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.current(1), Some(0), "a reversal left the pointer moved");

        rig.player(1).set_crossfade(CrossfadeMode::XFade, 65_535);
        rig.tick(60);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
        assert_eq!(rig.current(1), Some(1));
    }

    /// **`XFade`: one fader walks the list, and nothing ever moves the fader** —
    /// S51, punch-list B36.
    ///
    /// The entry in one test. Up crossfades to the next cue, down crossfades to
    /// the one after, and between the two the fader is simply left where the
    /// operator put it — **there is no Go in this test at all** after the first,
    /// which is the whole complaint: *danach fährt er zurück auf 0 für den
    /// nächsten Fade. Das ist unpraktisch.*
    #[test]
    fn a_crossfade_fader_walks_the_list_up_and_down_without_being_moved() {
        let mut rig = Rig::new(1);
        rig.load(1, &walkable());
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);

        // Up: cue 1 → cue 2.
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 0);
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 65_535);
        rig.tick(1);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
        assert_eq!(rig.current(1), Some(1));
        // The fader is at the top and **stays** at the top.
        assert_eq!(rig.player(1).crossfade(), Some(65_535));

        // Down: cue 2 → cue 3, with no Go and no re-basing.
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 32_768);
        rig.tick(2);
        let part = rig.value(1, AttributeType::Dimmer);
        assert!(
            (40_000..50_000).contains(&part),
            "half the way down is half the crossfade, got {part}"
        );
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 0);
        rig.tick(3);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 20_000);
        assert_eq!(rig.current(1), Some(2));
        assert_eq!(rig.player(1).crossfade(), Some(0));
    }

    /// **`Fade`: up takes the current cue out, down brings the next one in** —
    /// S51, B36, in the owner's own words.
    ///
    /// The difference from `XFade` is the top of the travel: the stage is dark
    /// there rather than on the next cue, and the next cue arrives on the way
    /// back down.
    #[test]
    fn a_fade_fader_takes_the_cue_out_going_up_and_the_next_one_in_coming_down() {
        let mut rig = Rig::new(1);
        rig.load(
            1,
            &sequence(
                vec![dimmer_cue("1", 65_535, 0.0), dimmer_cue("2", 30_000, 10.0)],
                false,
            ),
        );
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);

        rig.player(1).set_crossfade(CrossfadeMode::Fade, 0);
        // Half way up: half out, and **still on cue 1** — the list has not
        // moved, the light is going away.
        rig.player(1).set_crossfade(CrossfadeMode::Fade, 32_768);
        rig.tick(1);
        let half = rig.value(1, AttributeType::Dimmer);
        assert!(
            (32_000..34_000).contains(&half),
            "half a fader is half out, got {half}"
        );
        assert_eq!(rig.current(1), Some(0), "the fade-out took a cue");

        // All the way up: dark, and still on cue 1.
        rig.player(1).set_crossfade(CrossfadeMode::Fade, 65_535);
        rig.tick(2);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 0);
        assert_eq!(rig.current(1), Some(0));

        // And back down brings the **next** cue in, from the dark.
        rig.player(1).set_crossfade(CrossfadeMode::Fade, 32_768);
        rig.tick(3);
        let coming = rig.value(1, AttributeType::Dimmer);
        assert!(
            (14_000..16_000).contains(&coming),
            "half way down is half of cue 2, got {coming}"
        );
        assert_eq!(rig.current(1), Some(1));

        rig.player(1).set_crossfade(CrossfadeMode::Fade, 0);
        rig.tick(4);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 30_000);
        assert_eq!(rig.player(1).crossfade(), Some(0));
    }

    /// **A walk down a list produces the same frames twice** — B36's exit
    /// criterion, at the level the player can state it.
    ///
    /// The fader is driven through the same positions twice over, from the same
    /// starting state, and the value at every tick is compared. `crates/prismd`
    /// makes the same claim on the DMX frames themselves; this is the one that
    /// says *the playback is a function of where the fader is*, which is what
    /// makes the frames identical rather than merely similar.
    #[test]
    fn the_same_fader_walk_produces_the_same_values_twice() {
        let walk = |mode: CrossfadeMode| -> Vec<u16> {
            let mut rig = Rig::new(1);
            rig.load(1, &walkable());
            rig.go(1, GoDirection::Next);
            rig.tick(0);
            rig.player(1).set_crossfade(mode, 0);
            let mut seen = Vec::new();
            for (step, position) in [
                0_u16, 8_000, 20_000, 40_000, 65_535, 50_000, 30_000, 10_000, 0, 25_000, 65_535,
            ]
            .into_iter()
            .enumerate()
            {
                rig.player(1).set_crossfade(mode, position);
                rig.tick(step as u64 + 1);
                seen.push(rig.value(1, AttributeType::Dimmer));
            }
            seen
        };
        for mode in CrossfadeMode::ALL {
            let first = walk(mode);
            let second = walk(mode);
            assert_eq!(first, second, "{mode:?} is not a function of the fader");
            // And the walk actually moved something, or the claim is vacuous.
            assert!(
                first.iter().any(|value| *value != first[0]),
                "{mode:?} produced a flat walk"
            );
        }
    }

    /// A stroke stopped half way **holds**, and the frames say so tick after
    /// tick — the state *between two cues* B36 asks the playback to be able to
    /// hold.
    #[test]
    fn a_stroke_stopped_half_way_holds_the_mixture_indefinitely() {
        for mode in CrossfadeMode::ALL {
            let mut rig = Rig::new(1);
            rig.load(1, &walkable());
            rig.go(1, GoDirection::Next);
            rig.tick(0);
            rig.player(1).set_crossfade(mode, 0);
            rig.player(1).set_crossfade(mode, 30_000);
            rig.tick(1);
            let held = rig.value(1, AttributeType::Dimmer);
            for tick in 2..200 {
                rig.tick(tick);
                assert_eq!(
                    rig.value(1, AttributeType::Dimmer),
                    held,
                    "{mode:?} moved at tick {tick} with nobody touching the fader"
                );
            }
        }
    }

    /// Switching the mode under a stroke abandons it and moves nothing.
    ///
    /// A fader whose job changed mid-gesture is not in the middle of anything
    /// any more, and the honest answer is to stop rather than to reinterpret the
    /// half-finished journey as the other kind.
    #[test]
    fn changing_the_mode_under_a_stroke_leaves_the_light_where_it_is() {
        let mut rig = Rig::new(1);
        rig.load(1, &walkable());
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 0);
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 30_000);
        rig.tick(1);
        let held = rig.value(1, AttributeType::Dimmer);

        rig.player(1).set_crossfade(CrossfadeMode::Fade, 30_000);
        rig.tick(2);
        assert_eq!(rig.value(1, AttributeType::Dimmer), held);
        assert_eq!(rig.player(1).crossfade_progress(), None);
    }

    /// A Go abandons the stroke and leaves the fader where the hand left it.
    ///
    /// **S51 turned this round** (B36). The old rule was *arriving completes the
    /// cue and the fader is inert until the next Go re-bases it*, which is what
    /// made an operator's fader a thing the desk had opinions about. Now a Go
    /// takes the transition back on to the clock, the fader is not touched, and
    /// its next movement arms a fresh stroke from wherever it happens to be.
    #[test]
    fn a_go_takes_the_transition_back_and_leaves_the_fader_alone() {
        let mut rig = Rig::new(1);
        rig.load(1, &walkable());
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 0);
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 30_000);
        rig.tick(1);

        rig.go(1, GoDirection::Next);
        rig.tick(2);
        // The fader has not been moved by anything.
        assert_eq!(rig.player(1).crossfade(), Some(30_000));
        assert_eq!(rig.player(1).crossfade_progress(), None);
        // And the cue that the Go took is running on the clock, so time moves
        // it — which a stroke would not have.
        let before = rig.value(1, AttributeType::Dimmer);
        rig.run(3, 60);
        assert_ne!(
            rig.value(1, AttributeType::Dimmer),
            before,
            "the Go's cue did not get its clock back"
        );
    }

    /// Taking the crossfade off gives the transition its clock back.
    #[test]
    fn clearing_the_crossfade_returns_the_transition_to_time() {
        let mut rig = Rig::new(1);
        rig.load(
            1,
            &sequence(
                vec![dimmer_cue("1", 0, 0.0), dimmer_cue("2", 65_535, 1.0)],
                false,
            ),
        );
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 0);
        rig.go(1, GoDirection::Next);
        rig.tick(0);
        rig.go(1, GoDirection::Next);
        rig.run(1, 60);
        assert_eq!(
            rig.value(1, AttributeType::Dimmer),
            65_535,
            "a Go's cue runs on the clock, whatever a fader is engaged"
        );
        assert_eq!(rig.player(1).crossfade(), Some(0));

        rig.player(1).clear_crossfade();
        assert_eq!(rig.player(1).crossfade(), None);
        rig.run(61, 120);
        assert_eq!(rig.value(1, AttributeType::Dimmer), 65_535);
    }

    /// A flash starts a stopped playback and the release stops what it started —
    /// and only what it started.
    #[test]
    fn a_flash_starts_what_was_stopped_and_leaves_what_was_running() {
        let mut rig = Rig::new(2);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 0.0)], false));
        rig.load(2, &sequence(vec![dimmer_cue("1", 65_535, 0.0)], false));

        // Executor 1 is stopped: the flash starts it, and the release stops it.
        assert!(rig.player(1).flash_on());
        rig.tick(0);
        assert_eq!(rig.current(1), Some(0));
        assert!(rig.player(1).is_flashed());
        assert!(rig.player(1).flash_off());
        rig.tick(1);
        assert_eq!(rig.current(1), None);

        // Executor 2 is already running: the flash does not restart it, and the
        // release must not stop it.
        rig.go(2, GoDirection::Next);
        rig.tick(2);
        assert!(!rig.player(2).flash_on());
        assert!(!rig.player(2).is_flashed());
        assert!(!rig.player(2).flash_off());
        rig.tick(3);
        assert_eq!(
            rig.current(2),
            Some(0),
            "a flash release stopped a playback"
        );
    }

    /// A cue list swapped under a fader that is holding a rate keeps the rate.
    ///
    /// The speed and the crossfade are the *executor's* settings and not the
    /// sequence's, so a rebuild — which is how every show edit reaches the tick
    /// (S17) — must not silently return them to their defaults.
    #[test]
    fn loading_a_sequence_keeps_the_executors_own_rate_and_fader() {
        let mut rig = Rig::new(1);
        rig.load(1, &sequence(vec![dimmer_cue("1", 65_535, 1.0)], false));
        rig.player(1).set_speed(SPEED_UNITY * 3);
        rig.player(1).set_crossfade(CrossfadeMode::XFade, 12_345);
        rig.load(1, &sequence(vec![dimmer_cue("1", 30_000, 1.0)], false));
        assert_eq!(rig.player(1).speed(), SPEED_UNITY * 3);
        assert_eq!(rig.player(1).crossfade(), Some(12_345));
    }
}
