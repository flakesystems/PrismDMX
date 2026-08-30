//! Cues and sequences.
//!
//! Cue numbers are strings on purpose (`ARCHITECTURE_SPEC.md` §6): an operator
//! inserting a cue between 1 and 2 types `1.5`, and `1.5` must sort between them
//! without a float ever entering the show file.

use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AttributeType, FixtureId, PresetId, RgbColor, SequenceId};

/// What starts a cue.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum CueTrigger {
    /// Waits for a Go from the operator.
    #[default]
    Go,
    /// Starts when the previous cue has finished fading.
    Follow,
    /// Starts after `triggerTime` seconds.
    Time,
    /// Starts on an audio trigger. Deferred past V1 — see `IMPLEMENTATION_PLAN.md` S5.
    Sound,
}

/// What a cue says about one attribute it names — **S48**.
///
/// # Three states, and only two of them are written down
///
/// A cue is *an edit, not a state* (`docs/DMX_MERGE.md` section 3): it carries
/// what was in the programmer when it was stored and nothing else, and
/// everything it does not name keeps whatever the cue before it left. Until S48
/// that gave an attribute two states — the cue names it, or it does not — and
/// the second one is *inherits*, said by an absence rather than by a value.
///
/// The third is here. A cue may name an attribute and **take it back when the
/// list leaves the cue**, which is what an operator means by a one-off: a
/// blinder on cue 12 that is not still on at cue 13, without cue 13 having to
/// know it happened. So: [`Self::Track`] asserts and carries forward,
/// [`Self::CueOnly`] asserts and hands the attribute back to whatever was
/// underneath it, and *inherits* stays what it always was — no part at all.
///
/// This is the other half of punch-list **B21** (S43). What the programmer marks
/// as *overriding* is exactly what a cue has to assert for a jump into it to
/// produce the same light twice, and a value that is taken back at the end is
/// still an assertion **while the cue is current**.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum CueTracking {
    /// The value stands until a later cue says otherwise.
    ///
    /// **The default, and the meaning every cue written before S48 already
    /// had.** `#[serde(default)]` on [`CuePart::tracking`] is what makes an
    /// older `.prism` file open, and this is why the default is the right one
    /// rather than merely a compiling one: tracking is what a cue has always
    /// done here.
    #[default]
    Track,
    /// The value is taken back when the list leaves this cue.
    ///
    /// Back to *what was underneath*, which is whatever the tracking state held
    /// for that attribute before this cue asserted it — and to nothing at all
    /// when no earlier cue held it, in which case the playback stops providing
    /// it and the merge falls through to whatever is below (`docs/DMX_MERGE.md`
    /// section 1).
    CueOnly,
}

/// One attribute value inside a cue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct CuePart {
    /// The fixture this part applies to.
    pub fixture: FixtureId,
    /// The attribute being set.
    pub attribute: AttributeType,
    /// The value, `0..=65535`.
    pub value: u16,
    /// Link to the preset this value came from, which keeps the cue
    /// live-updatable when the preset is edited later.
    pub preset_ref: Option<PresetId>,
    /// Whether the value carries forward or is taken back at the end of the cue
    /// - **S48**.
    ///
    /// `#[serde(default)]`, and the default is [`CueTracking::Track`] because
    /// that is what a cue stored before this field existed did. A `.prism` file
    /// keeps each sequence as its own MessagePack document (S15), so an older
    /// file simply has no such key and reads as the tracking cue it was.
    #[serde(default)]
    pub tracking: CueTracking,
}

/// Which cue the programmer is editing, and whether it has moved since (S39).
///
/// **The update state.** `Command::EditCue` loads a cue into the programmer and
/// puts one of these in `Session::editing_cue`; `Command::Update` stores it back
/// and clears [`Self::modified`]. What it is *for* is the Update key: a desk
/// blinks it when there is an edit to put back, which is exactly
/// `editing_cue.is_some() && modified`.
///
/// It is session state rather than programmer state because it is **operating**
/// state in `ARCHITECTURE_SPEC.md` §4.1's sense: every attached client has to
/// blink the same key, and a second screen that worked it out from its own
/// mirror of the programmer would have to know which cue that programmer came
/// from — which is precisely the fact this carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct CueEdit {
    /// The sequence the cue is in.
    pub sequence_id: SequenceId,
    /// The cue, by its number.
    ///
    /// The number and not an index, for `Command::DeleteCue`'s reason: a number
    /// is what an operator wrote on a running order, and an index moves when a
    /// cue is inserted above it. A renumber of the cue being edited **carries
    /// this with it** — see `prism_core::ShowFile`.
    pub cue_number: String,
    /// Whether the programmer has changed since the cue was loaded.
    ///
    /// False the moment `EditCue` lands and false again after an `Update`. Any
    /// programmer edit in between sets it, which is what makes the key blink.
    pub modified: bool,
}

impl CueEdit {
    /// Whether this edit is of that cue of that sequence.
    ///
    /// The number is **trimmed on both sides**, because an operator typed one of
    /// them: `" 2 "` and `"2"` are the same cue everywhere else in the desk
    /// (`prism_core::Show::cue`), and an update state that did not agree would
    /// survive a `DeleteCue` of the very cue it names.
    #[must_use]
    pub fn is_of(&self, sequence: SequenceId, number: &str) -> bool {
        self.sequence_id == sequence && self.cue_number.trim() == number.trim()
    }
}

/// One step of a sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    /// Cue number as typed, e.g. `1`, `1.5`, `2`. A string, so decimal ordering
    /// survives the show file unchanged.
    pub number: String,
    /// Operator-facing name.
    pub name: String,
    /// Fade-in time in seconds.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub fade_in: f64,
    /// Fade-out time in seconds.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub fade_out: f64,
    /// Delay before the fade starts, in seconds.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub delay: f64,
    /// What starts this cue.
    pub trigger: CueTrigger,
    /// Trigger time in seconds, for [`CueTrigger::Time`].
    #[serde(with = "crate::finite::option")]
    #[ts(as = "Option<f64>")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "proptest::option::of(crate::arb::seconds())")
    )]
    pub trigger_time: Option<f64>,
    /// The values this cue sets.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub parts: Vec<CuePart>,
}

impl Cue {
    /// Which attributes this cue asserts, whether they track or are taken back.
    ///
    /// The set a cue sheet draws in S43's *overriding* language, and the one
    /// [`CueTrack::inherited`] is the complement of.
    #[must_use]
    pub fn asserts(&self) -> BTreeSet<CueKey> {
        self.parts
            .iter()
            .map(|part| (part.fixture, part.attribute))
            .collect()
    }

    /// Orders two cue numbers the way an operator reads them: `1`, `1.5`, `2`,
    /// `10` — not the lexical order `1`, `1.5`, `10`, `2`.
    ///
    /// A number that does not parse sorts after every number that does, and ties
    /// break on the original text, so the order is total and stable.
    #[must_use]
    pub fn compare_numbers(left: &str, right: &str) -> Ordering {
        match (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
            (Ok(left_value), Ok(right_value)) => left_value.total_cmp(&right_value),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => left.cmp(right),
        }
    }
}

/// What one attribute of one fixture is held at, as a tracking state names it.
///
/// A pair rather than a struct because it is a **key**: [`CueTrack`] holds its
/// state in a map ordered by it, and the order is the one every attribute table
/// in the desk uses — fixture number, then `AttributeType`'s own order.
pub type CueKey = (FixtureId, AttributeType);

/// One attribute whose held value moved when a cue was entered.
///
/// `value` is `None` for an attribute the list has stopped holding — a
/// [`CueTracking::CueOnly`] value taken back with nothing underneath it. That is
/// a different fact from *held at zero*, and the merge treats it differently: an
/// attribute a playback does not provide falls through to whatever is below it,
/// and one held at zero wins its slot at zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CueChange {
    /// The fixture.
    pub fixture: FixtureId,
    /// The attribute.
    pub attribute: AttributeType,
    /// What it is held at from this cue on, or `None` for no longer held.
    pub value: Option<u16>,
}

/// The tracking rule, written once — **S48**.
///
/// # What this exists to decide
///
/// A cue list walked from the top produces, at every cue, a full set of
/// attribute values: the ones that cue asserts, plus everything an earlier cue
/// asserted and nothing has overwritten since. That set is the **tracking
/// state**, and until S48 nothing in this project computed it — so a cue reached
/// by a `Goto` kept whatever the operator happened to be looking at, and cue 3
/// produced one output when walked into and another when jumped to.
///
/// It is **derived and never stored**. A `.prism` file keeps the *edits*,
/// because a file that stored the resolved state would be a file that could not
/// be corrected by editing cue 2 - which is `Query::DarkUniverses` (S37) and
/// `Query::ArtNetNodes` (S46) arriving at the same rule from two other
/// directions.
///
/// # The fold, and why it needs two states rather than an undo
///
/// The obvious way to take a [`CueTracking::CueOnly`] value back is to remember
/// what it covered and put that back. That is a stack, it is per attribute, and
/// it is wrong the first time two cue-only cues touch one attribute in a row.
///
/// So there is no undo. Two states are carried instead:
///
/// - `tracked` — the state built from [`CueTracking::Track`] parts **only**. A
///   cue-only part never enters it, so it never covered anything and there is
///   nothing to restore.
/// - `visible` — `tracked` with the current cue's cue-only parts laid over it.
///   This is what the playback holds while that cue is the one being played.
///
/// Leaving a cue is then not an operation at all: the next cue's overlay
/// replaces this one's, and every attribute that was only ever in the overlay
/// falls back to `tracked` by construction.
///
/// # Who uses it
///
/// `prism_engine::SequencePlan` walks it once per compile to build the table its
/// player reads on the tick; `prism_core` walks it to answer
/// `crate::Query::CueTracking` and to write a blocking cue. **One rule, three
/// readers** — a second implementation in TypeScript is exactly what the query
/// exists to prevent.
#[derive(Debug, Clone, Default)]
pub struct CueTrack {
    /// The state built from tracking parts only.
    tracked: BTreeMap<CueKey, u16>,
    /// `tracked` with the current cue's cue-only parts over it.
    visible: BTreeMap<CueKey, u16>,
    /// What the current cue holds cue-only.
    overlay: BTreeMap<CueKey, u16>,
    /// Reused between cues, so a long list is not a long list of allocations.
    touched: BTreeSet<CueKey>,
    /// The current cue's parts, last-wins, for the same reason.
    here: BTreeMap<CueKey, (u16, CueTracking)>,
}

impl CueTrack {
    /// A walk that has not entered a cue yet: nothing is held.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enters `cue` and reports every attribute whose held value moved.
    ///
    /// `changes` is cleared first and left holding the difference, in [`CueKey`]
    /// order. It is a **difference** rather than the whole state because that is
    /// what makes compiling a long list linear in the number of cue parts rather
    /// than quadratic in the number of cues: only the attributes this cue names
    /// and the ones the *previous* cue held cue-only can have moved.
    ///
    /// A cue that names one attribute twice says the second thing, exactly as a
    /// second keystroke would — and the second part's [`CuePart::tracking`] is
    /// what governs, because it is the one that was written last.
    pub fn enter(&mut self, cue: &Cue, changes: &mut Vec<CueChange>) {
        changes.clear();
        let Self {
            tracked,
            visible,
            overlay,
            touched,
            here,
        } = self;

        touched.clear();
        here.clear();
        touched.extend(overlay.keys().copied());
        for part in &cue.parts {
            let key = (part.fixture, part.attribute);
            touched.insert(key);
            here.insert(key, (part.value, part.tracking));
        }

        overlay.clear();
        for (key, (value, tracking)) in here.iter() {
            match tracking {
                CueTracking::Track => tracked.insert(*key, *value),
                CueTracking::CueOnly => overlay.insert(*key, *value),
            };
        }

        for key in touched.iter() {
            let now = overlay.get(key).or_else(|| tracked.get(key)).copied();
            if visible.get(key).copied() == now {
                continue;
            }
            match now {
                Some(value) => visible.insert(*key, value),
                None => visible.remove(key),
            };
            changes.push(CueChange {
                fixture: key.0,
                attribute: key.1,
                value: now,
            });
        }
    }

    /// What the list holds while the cue just entered is being played.
    ///
    /// The cue's own values, everything an earlier cue asserted and nothing has
    /// overwritten, and the current cue's cue-only values over the top.
    #[must_use]
    pub const fn visible(&self) -> &BTreeMap<CueKey, u16> {
        &self.visible
    }

    /// What the list would hand on to the **next** cue.
    ///
    /// [`Self::visible`] without the current cue's cue-only overlay, which is
    /// the same thing said the other way round: a cue-only value is not handed
    /// on, and that is the whole of what makes it cue-only.
    #[must_use]
    pub const fn tracked(&self) -> &BTreeMap<CueKey, u16> {
        &self.tracked
    }

    /// Which of the attributes now held this cue does **not** name — what it
    /// inherits.
    ///
    /// The list a cue sheet draws in the resting style beside the ones the cue
    /// asserts, and the list a blocking cue writes into itself.
    #[must_use]
    pub fn inherited(&self, cue: &Cue) -> Vec<(CueKey, u16)> {
        let named = cue.asserts();
        self.visible
            .iter()
            .filter(|(key, _)| !named.contains(*key))
            .map(|(key, value)| (*key, *value))
            .collect()
    }
}

/// What an operator is saying about a whole cue — **S48**.
///
/// The command argument behind `crate::Command::SetCueTracking`, and
/// deliberately **not** [`CueTracking`]: that type is what one part of a cue
/// carries, and this is one of three things a person does to a cue. Two of them
/// set every part's [`CuePart::tracking`]; the third writes values.
///
/// # Why blocking is here and not a flag on [`Cue`]
///
/// A blocking cue is one that **asserts everything** — nothing reaches past it,
/// so a list can be cut into sections an operator can rehearse from. There are
/// two ways to build one and only one of them survives contact with this
/// session's own rule.
///
/// A `block: bool` on the cue would be a *mode*, and the engine would have to
/// honour it by treating the inherited values as if the cue had named them. But
/// those values are derived from the cues before it, so editing cue 2 would
/// still change what a blocking cue 5 puts out — which is precisely what
/// blocking is asked for to stop.
///
/// So blocking is an **edit**: [`Self::Block`] writes the inherited values into
/// the cue as ordinary parts. After it, the cue names everything, nothing before
/// it reaches past it, and the file is self-contained. What the desk *shows* as
/// a blocking cue is then derived rather than stored — a cue that inherits
/// nothing — so a later edit that gives cue 2 a new attribute correctly stops
/// the mark, because the cue no longer asserts everything.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum CueTrackingMode {
    /// Every value this cue holds carries forward — the default a cue is stored
    /// in.
    #[default]
    Track,
    /// Every value this cue holds is taken back when the list leaves it.
    CueOnly,
    /// The cue asserts everything: what it inherits is written into it, and
    /// every value it holds carries forward.
    ///
    /// A cue-only value **becomes a tracking one**, because a value that is
    /// taken back at the end is not an assertion and a cue that asserts
    /// everything cannot have one.
    Block,
}

/// One field of a cue, changed on its own.
///
/// # Why one field rather than a whole cue
///
/// A cue sheet edits a cell: a name, a fade time, a trigger. The obvious
/// alternative — a command carrying every editable field at once — makes a
/// client read the cue, change one member and send the rest back, which is a
/// read-modify-write over state the daemon owns. Two operators editing two
/// different columns of one cue would then each undo the other's edit, and
/// neither would have done anything wrong. So the command names the field.
///
/// `Parts` is deliberately **not** here. What a cue *does* comes from the
/// programmer through `Command::StoreCue`; a client that sent values would be
/// authoring show content, which is the same rule that keeps channels out of
/// `Command::PatchFixture` and profiles out of `Command::EmbedFixtureType`.
///
/// # The number and the name went out in S40, and that is not a loss
///
/// This enum had a `Number` and a `Name` until S40, and both are now said by a
/// verb of their own: renumbering a cue is `Command::Move` and naming one is
/// `Command::Label`, because renumbering and naming are the same act on a cue
/// as on a sequence, a group, a preset, a view and an executor — and S40's
/// command line is where that stops being an observation and becomes the
/// grammar (`ARCHITECTURE_SPEC.md` §4.5). Two ways to rename a cue would have
/// been two grammars for one line.
///
/// What is left is what a *cue* has and nothing else does: three times and a
/// trigger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum CueProperty {
    /// Fade-in time in seconds.
    FadeIn {
        /// Seconds, never negative.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        seconds: f64,
    },
    /// Fade-out time in seconds.
    FadeOut {
        /// Seconds, never negative.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        seconds: f64,
    },
    /// Delay before the fade starts, in seconds.
    Delay {
        /// Seconds, never negative.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        seconds: f64,
    },
    /// What starts the cue, and after how long.
    ///
    /// The two travel together because [`CueTrigger::Time`] is the only trigger
    /// `trigger_time` means anything for: setting them separately leaves a cue
    /// that is triggered by a time nobody has given yet, which is a state an
    /// operator would meet halfway through an edit.
    Trigger {
        /// The trigger.
        trigger: CueTrigger,
        /// Seconds, for [`CueTrigger::Time`]. `None` for the others.
        #[serde(with = "crate::finite::option")]
        #[ts(as = "Option<f64>")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "proptest::option::of(crate::arb::seconds())")
        )]
        trigger_time: Option<f64>,
    },
}

/// A cue list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Sequence {
    /// Sequence number.
    pub id: SequenceId,
    /// Operator-facing name.
    pub name: String,
    /// Colour for the scribble strip and the executor bar, if one was chosen.
    ///
    /// **The sequence carries it, not the executor**, for [`Self::name`]'s
    /// reason: what a strip shows is the cue list that is on it, so a list moved
    /// from fader 3 to fader 6 takes its colour with it and a fader that is
    /// given a different list shows that one's. An executor is a place, and a
    /// colour is a property of the thing standing in it.
    ///
    /// `None` is *no colour chosen* rather than black, and the two are different
    /// things: a strip with no colour is lit white and readable, and
    /// `prism_surface::color` refuses to quantise anything but exact black to an
    /// unlit strip. `Command::Color` with `None` is how an operator takes a
    /// colour back off.
    ///
    /// `#[serde(default)]` because a `.prism` file keeps each sequence as a
    /// MessagePack document (S15) and one written before this does not carry it.
    #[serde(default)]
    pub color: Option<RgbColor>,
    /// The cues, in playback order.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub cues: Vec<Cue>,
    /// Whether an **automatic** chain of follows runs off the end and round
    /// again.
    ///
    /// **Not whether a Go wraps.** A Go always does — Go+ on the last cue enters
    /// the first and Go− on the first enters the last, on every list — because
    /// holding is the wrong answer to a key somebody pressed. What this flag
    /// buys is a list that never stops on its own: `Follow` and `Time` cues
    /// carry on past the end instead of resting there, which is a chase. The two
    /// are different questions and `prism_engine::SequencePlan` answers them
    /// with two functions, `step` and `follow_step`.
    ///
    /// Named `looping` in Rust because `loop` is a keyword; the wire name stays
    /// `loop`, as specified.
    #[serde(rename = "loop")]
    pub looping: bool,
    /// The master level of this list's playback, `0..=65535` — **S45**.
    ///
    /// It was `Executor::master_level` until then, one per executor, and
    /// punch-list entry B18 is what that cost: two faders on one cue list moved
    /// independently, and one of them was always lying about the light. There is
    /// one of these per list now, and every executor whose fader is
    /// [`crate::ExecutorFaderFunction::Master`] is a handle on it.
    ///
    /// **Full is the default and the safe answer.** A cue list nobody has faded
    /// has to produce light when it is switched on — the same rule
    /// `prism_engine::PlaybackSource` has followed since S2 — so a list written
    /// before S45 comes up at full, and `prism_core::store` overwrites that with
    /// the level the executor carried when the file has one.
    #[serde(default = "full_master")]
    #[cfg_attr(any(test, feature = "proptest"), proptest(strategy = "0..=u16::MAX"))]
    pub master_level: u16,
    /// The rate this list plays at, in units of [`crate::SPEED_UNITY`] — S45.
    ///
    /// The *speed master* of `docs/DMX_MERGE.md` §4 item 3, moved off the
    /// executor for [`Self::master_level`]'s reason. `0` freezes a playback,
    /// [`crate::SPEED_UNITY`] is 1x, and `u16::MAX` is just under 64x.
    ///
    /// **Unity is the default**, because a list written before this field
    /// existed was playing at the times its own cues carry, which is what unity
    /// means.
    #[serde(default = "unity_speed")]
    #[cfg_attr(any(test, feature = "proptest"), proptest(strategy = "0..=u16::MAX"))]
    pub speed: u16,
    /// Whether this cue list is running (S40, and every playback of it since
    /// S45).
    ///
    /// Written only by the tick's readback — `prism_engine::PlaybackReport`
    /// through `prismd::Core::poll_playback`. A command that starts a playback
    /// does not set it on the way past: the command has only been *queued* when
    /// it is acknowledged, and two authors for one field means the loser is
    /// whichever arrives second.
    ///
    /// `#[serde(default)]` because a `.prism` file keeps each sequence as a
    /// MessagePack document (S15) and one written before S40 does not carry it.
    /// It is not really persisted state either — a show reopens with nothing
    /// running — which is why `false` is the right default rather than a
    /// migration.
    #[serde(default)]
    pub is_active: bool,
    /// Which cue this list's playback is standing on, as an index into
    /// [`Self::cues`].
    ///
    /// The same author and the same reason for existing, and it is the half
    /// nothing could know before S34: what cue a playback is on lives on the
    /// tick thread.
    #[serde(default)]
    pub current_cue_index: Option<u32>,
}

impl Sequence {
    /// The cues in **playback order**, which is by number and not by position in
    /// the file.
    ///
    /// `1`, `1.5`, `2`, `10` is the order an operator reads and the order
    /// `prism_engine::SequencePlan` compiles, and inserting a cue between two
    /// others is the entire reason cue numbers are decimal strings. Stable, so
    /// two cues with the same number keep their file order.
    #[must_use]
    pub fn ordered_cues(&self) -> Vec<&Cue> {
        let mut order: Vec<&Cue> = self.cues.iter().collect();
        order.sort_by(|left, right| Cue::compare_numbers(&left.number, &right.number));
        order
    }

    /// What a walk from the first cue down to `index` holds, cue by cue.
    ///
    /// Answers one [`CueTrack`] per cue of [`Self::ordered_cues`], each carrying
    /// the state **while that cue is being played**. Walked once rather than
    /// re-folded per cue, so a list of a thousand cues costs one pass.
    #[must_use]
    pub fn tracking(&self) -> Vec<CueTrack> {
        let mut track = CueTrack::new();
        let mut changes = Vec::new();
        self.ordered_cues()
            .into_iter()
            .map(|cue| {
                track.enter(cue, &mut changes);
                track.clone()
            })
            .collect()
    }
}

/// The level a cue list written before [`Sequence::master_level`] existed was
/// playing at — full, because a playback nobody has faded has to make light.
const fn full_master() -> u16 {
    u16::MAX
}

/// The rate a cue list written before [`Sequence::speed`] existed was playing
/// at, which is the only rate it could have been playing at.
const fn unity_speed() -> u16 {
    crate::SPEED_UNITY
}

#[cfg(test)]
mod tests {
    use crate::{
        AttributeType, Cue, CuePart, CueProperty, CueTrack, CueTracking, CueTrackingMode,
        CueTrigger, FixtureId, RgbColor, Sequence, SequenceId,
    };

    /// **The number is trimmed on both sides**, because an operator typed one
    /// of them: `" 2 "` and `"2"` are the same cue everywhere else in the desk,
    /// and an update state that did not agree would survive a `DeleteCue` of
    /// the very cue it names.
    #[test]
    fn a_cue_edit_is_of_the_cue_whatever_spacing_the_operator_typed() {
        let edit = crate::CueEdit {
            sequence_id: SequenceId::new(1),
            cue_number: " 1.5 ".to_owned(),
            modified: false,
        };
        assert!(edit.is_of(SequenceId::new(1), "1.5"));
        assert!(edit.is_of(SequenceId::new(1), "  1.5"));
        assert!(!edit.is_of(SequenceId::new(1), "1.50"));
        assert!(!edit.is_of(SequenceId::new(2), "1.5"));
    }

    fn cue(number: &str) -> Cue {
        Cue {
            number: number.to_owned(),
            name: "Look".to_owned(),
            fade_in: 3.0,
            fade_out: 3.0,
            delay: 0.0,
            trigger: CueTrigger::Go,
            trigger_time: None,
            parts: vec![CuePart {
                fixture: FixtureId::new(1),
                attribute: AttributeType::Dimmer,
                value: 65535,
                preset_ref: None,
                tracking: CueTracking::Track,
            }],
        }
    }

    #[test]
    fn cue_matches_the_wire_shape() {
        let json = serde_json::to_value(cue("1.5")).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "delay",
                "fadeIn",
                "fadeOut",
                "name",
                "number",
                "parts",
                "trigger",
                "triggerTime",
            ]
        );
        assert_eq!(json["number"], "1.5");
        assert_eq!(json["trigger"], "Go");
    }

    #[test]
    fn all_four_trigger_types_exist() {
        for (trigger, text) in [
            (CueTrigger::Go, "\"Go\""),
            (CueTrigger::Follow, "\"Follow\""),
            (CueTrigger::Time, "\"Time\""),
            (CueTrigger::Sound, "\"Sound\""),
        ] {
            assert_eq!(serde_json::to_string(&trigger).unwrap(), text);
        }
    }

    #[test]
    fn cue_numbers_sort_by_decimal_value_not_lexically() {
        let mut numbers = ["10", "2", "1.5", "1"];
        numbers.sort_by(|a, b| Cue::compare_numbers(a, b));
        assert_eq!(numbers, ["1", "1.5", "2", "10"]);
    }

    #[test]
    fn an_unparsable_cue_number_sorts_last_instead_of_panicking() {
        let mut numbers = ["2", "oops", "1", "later"];
        numbers.sort_by(|a, b| Cue::compare_numbers(a, b));
        assert_eq!(numbers, ["1", "2", "later", "oops"]);
    }

    #[test]
    fn a_sequence_keeps_its_cues_in_order_and_may_loop() {
        let sequence = Sequence {
            id: SequenceId::new(1),
            name: "Main".to_owned(),
            color: None,
            cues: vec![cue("1"), cue("2")],
            looping: true,
            master_level: u16::MAX,
            speed: crate::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
        };
        let json = serde_json::to_value(&sequence).unwrap();
        assert_eq!(json["loop"], true);
        assert_eq!(json["cues"].as_array().unwrap().len(), 2);
        assert_eq!(json["color"], serde_json::Value::Null);
    }

    /// A colour is not required, and a sequence written before there was one
    /// opens without a colour rather than failing to open.
    ///
    /// The same claim as the playback state below it, for the same reason: a
    /// `.prism` file keeps each sequence as its own MessagePack document (S15),
    /// so every field added to this type after S15 is one an older file does not
    /// carry.
    #[test]
    fn a_sequence_written_before_colours_opens_without_one() {
        let sequence: Sequence =
            serde_json::from_str(r#"{"id":1,"name":"Main","cues":[],"loop":false}"#).unwrap();
        assert_eq!(sequence.color, None);

        // And a colour that is there survives the round trip it was written for.
        let blue = RgbColor { r: 0, g: 0, b: 255 };
        let colored = Sequence {
            color: Some(blue),
            ..sequence
        };
        let text = serde_json::to_string(&colored).unwrap();
        assert!(text.contains(r#""color":{"r":0,"g":0,"b":255}"#), "{text}");
        assert_eq!(
            serde_json::from_str::<Sequence>(&text).unwrap().color,
            Some(blue)
        );
    }

    /// **A sequence written before S40 does not carry its playback state**, and
    /// a `.prism` file keeps each sequence as a MessagePack document (S15), so
    /// the two fields are `#[serde(default)]` and a file from S39 opens with
    /// nothing running - which is also the right answer for a show that has just
    /// been loaded.
    #[test]
    fn a_sequence_written_before_the_playback_state_reads_as_stopped() {
        let sequence: Sequence =
            serde_json::from_str(r#"{"id":1,"name":"Main","cues":[],"loop":false}"#).unwrap();
        assert!(!sequence.is_active);
        assert_eq!(sequence.current_cue_index, None);
    }

    /// **A cue written before S48 tracks**, which is what it did when it was
    /// written. The field is `#[serde(default)]` and the default carries the
    /// meaning the old file already had, so a `.prism` from S47 opens and means
    /// the same thing.
    #[test]
    fn a_cue_part_written_before_tracking_reads_as_a_tracking_one() {
        let part: CuePart = serde_json::from_str(
            r#"{"fixture":1,"attribute":"Dimmer","value":65535,"presetRef":null}"#,
        )
        .unwrap();
        assert_eq!(part.tracking, CueTracking::Track);

        // A whole cue out of an older file, through MessagePack, which is what a
        // `.prism` actually keeps (S15).
        let packed = rmp_serde::to_vec_named(&serde_json::json!({
            "number": "1",
            "name": "Look",
            "fadeIn": 3.0,
            "fadeOut": 3.0,
            "delay": 0.0,
            "trigger": "Go",
            "triggerTime": null,
            "parts": [{
                "fixture": 1,
                "attribute": "Dimmer",
                "value": 65535,
                "presetRef": null,
            }],
        }))
        .unwrap();
        let older: Cue = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(older.parts[0].tracking, CueTracking::Track);

        // And a cue-only part survives the round trip it was added for.
        let one_off = CuePart {
            tracking: CueTracking::CueOnly,
            ..older.parts[0].clone()
        };
        let text = serde_json::to_string(&one_off).unwrap();
        assert!(text.contains(r#""tracking":"CueOnly""#), "{text}");
        assert_eq!(serde_json::from_str::<CuePart>(&text).unwrap(), one_off);
    }

    /// The three words an operator says about a cue, on the wire.
    #[test]
    fn the_three_tracking_modes_are_named_on_the_wire() {
        for (mode, text) in [
            (CueTrackingMode::Track, "\"Track\""),
            (CueTrackingMode::CueOnly, "\"CueOnly\""),
            (CueTrackingMode::Block, "\"Block\""),
        ] {
            assert_eq!(serde_json::to_string(&mode).unwrap(), text);
        }
        assert_eq!(CueTrackingMode::default(), CueTrackingMode::Track);
    }

    fn valued(fixture: u32, value: u16, tracking: CueTracking) -> CuePart {
        CuePart {
            fixture: FixtureId::new(fixture),
            attribute: AttributeType::Dimmer,
            value,
            preset_ref: None,
            tracking,
        }
    }

    fn look(number: &str, parts: Vec<CuePart>) -> Cue {
        Cue {
            parts,
            number: number.to_owned(),
            ..cue(number)
        }
    }

    fn held(track: &CueTrack, fixture: u32) -> Option<u16> {
        track
            .visible()
            .get(&(FixtureId::new(fixture), AttributeType::Dimmer))
            .copied()
    }

    /// The rule itself: what a cue does not name keeps what an earlier cue left.
    #[test]
    fn a_walk_down_a_list_carries_every_value_an_earlier_cue_asserted() {
        let mut track = CueTrack::new();
        let mut changes = Vec::new();

        track.enter(
            &look("1", vec![valued(1, 32_768, CueTracking::Track)]),
            &mut changes,
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].value, Some(32_768));
        assert_eq!(held(&track, 1), Some(32_768));

        // A cue that names something else leaves fixture 1 exactly where it was
        // - and says so by reporting no change for it.
        track.enter(
            &look("2", vec![valued(2, 10_000, CueTracking::Track)]),
            &mut changes,
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].fixture, FixtureId::new(2));
        assert_eq!(held(&track, 1), Some(32_768));
        assert_eq!(held(&track, 2), Some(10_000));

        // And a cue that names nothing at all changes nothing at all.
        track.enter(&look("3", Vec::new()), &mut changes);
        assert!(changes.is_empty());
        assert_eq!(held(&track, 1), Some(32_768));
    }

    /// A cue-only value is handed back to **what was underneath it**, which is
    /// what makes it an undo of one edit rather than a blackout.
    #[test]
    fn a_cue_only_value_is_handed_back_to_whatever_was_under_it() {
        let mut track = CueTrack::new();
        let mut changes = Vec::new();
        track.enter(
            &look("1", vec![valued(1, 32_768, CueTracking::Track)]),
            &mut changes,
        );
        track.enter(
            &look("2", vec![valued(1, 65_535, CueTracking::CueOnly)]),
            &mut changes,
        );
        assert_eq!(held(&track, 1), Some(65_535));
        // It is **visible** and not **tracked**: that difference is the whole
        // type, and it is why leaving the cue needs no undo.
        assert_eq!(
            track
                .tracked()
                .get(&(FixtureId::new(1), AttributeType::Dimmer))
                .copied(),
            Some(32_768)
        );

        track.enter(&look("3", Vec::new()), &mut changes);
        assert_eq!(held(&track, 1), Some(32_768));
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].value, Some(32_768));
    }

    /// Two cue-only cues in a row over one attribute, which is the case an undo
    /// stack gets wrong: the second one covers the first, and leaving the second
    /// goes back to what neither of them wrote.
    #[test]
    fn two_cue_only_cues_in_a_row_both_hand_back_to_the_tracked_value() {
        let mut track = CueTrack::new();
        let mut changes = Vec::new();
        track.enter(
            &look("1", vec![valued(1, 100, CueTracking::Track)]),
            &mut changes,
        );
        track.enter(
            &look("2", vec![valued(1, 200, CueTracking::CueOnly)]),
            &mut changes,
        );
        track.enter(
            &look("3", vec![valued(1, 300, CueTracking::CueOnly)]),
            &mut changes,
        );
        assert_eq!(held(&track, 1), Some(300));
        track.enter(&look("4", Vec::new()), &mut changes);
        assert_eq!(held(&track, 1), Some(100));
    }

    /// A cue-only value with nothing under it stops being held at all, which is
    /// a different fact from *held at zero*: the merge falls through to whatever
    /// is below the playback rather than to the playback writing a nought.
    #[test]
    fn a_cue_only_value_with_nothing_under_it_stops_being_held() {
        let mut track = CueTrack::new();
        let mut changes = Vec::new();
        track.enter(
            &look("1", vec![valued(1, 65_535, CueTracking::CueOnly)]),
            &mut changes,
        );
        assert_eq!(held(&track, 1), Some(65_535));
        track.enter(&look("2", Vec::new()), &mut changes);
        assert_eq!(held(&track, 1), None);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].value, None, "a release was reported as a value");
    }

    /// A cue that asserts a value the list already holds moves nothing, and the
    /// walk says so by reporting **no change** — which is what keeps compiling a
    /// long list linear in the edits rather than in the cues.
    #[test]
    fn a_cue_that_asserts_what_is_already_held_reports_no_change() {
        let mut track = CueTrack::new();
        let mut changes = Vec::new();
        track.enter(
            &look("1", vec![valued(1, 100, CueTracking::Track)]),
            &mut changes,
        );
        assert_eq!(changes.len(), 1);
        // The same value again, said by a different cue: the attribute is in
        // `touched` because the cue names it, and it is not in `changes`
        // because nothing about the output moved.
        track.enter(
            &look("2", vec![valued(1, 100, CueTracking::Track)]),
            &mut changes,
        );
        assert!(changes.is_empty(), "{changes:?}");
        assert_eq!(held(&track, 1), Some(100));
        // And what the cue asserts is what it names, whatever the value is.
        assert_eq!(
            look("2", vec![valued(1, 100, CueTracking::Track)]).asserts(),
            [(FixtureId::new(1), AttributeType::Dimmer)]
                .into_iter()
                .collect()
        );
    }

    /// What a cue **inherits** is the complement of what it asserts, and it is
    /// the list a blocking cue writes into itself.
    #[test]
    fn what_a_cue_inherits_is_everything_held_that_it_does_not_name() {
        let sequence = Sequence {
            id: SequenceId::new(1),
            name: "Main".to_owned(),
            color: None,
            cues: vec![
                look("1", vec![valued(1, 100, CueTracking::Track)]),
                look("2", vec![valued(2, 200, CueTracking::Track)]),
                look("3", vec![valued(3, 300, CueTracking::Track)]),
            ],
            looping: false,
            master_level: u16::MAX,
            speed: crate::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
        };
        let states = sequence.tracking();
        assert_eq!(states.len(), 3);

        // **The first cue of a list inherits nothing** — there is nothing above
        // it — so it blocks by construction, and that is worth drawing rather
        // than hiding.
        assert!(states[0].inherited(&sequence.cues[0]).is_empty());
        assert_eq!(
            states[2].inherited(&sequence.cues[2]),
            vec![
                ((FixtureId::new(1), AttributeType::Dimmer), 100),
                ((FixtureId::new(2), AttributeType::Dimmer), 200),
            ]
        );
    }

    /// The playback order is the number order, and a cue is found by the number
    /// an operator typed — trimmed, because they typed it.
    #[test]
    fn cues_are_walked_in_number_order_whatever_order_the_file_holds_them() {
        let sequence = Sequence {
            id: SequenceId::new(1),
            name: "Main".to_owned(),
            color: None,
            cues: vec![look("10", Vec::new()), look("2", Vec::new()), cue("1.5")],
            looping: false,
            master_level: u16::MAX,
            speed: crate::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
        };
        let numbers: Vec<&str> = sequence
            .ordered_cues()
            .iter()
            .map(|cue| cue.number.as_str())
            .collect();
        assert_eq!(numbers, ["1.5", "2", "10"]);
    }

    /// A cue property is one field, tagged like every other message.
    #[test]
    fn a_cue_property_names_the_field_it_changes() {
        assert_eq!(
            serde_json::to_string(&CueProperty::FadeIn { seconds: 2.5 }).unwrap(),
            r#"{"t":"FadeIn","seconds":2.5}"#
        );
        assert_eq!(
            serde_json::to_string(&CueProperty::Trigger {
                trigger: CueTrigger::Time,
                trigger_time: Some(4.0),
            })
            .unwrap(),
            r#"{"t":"Trigger","trigger":"Time","triggerTime":4.0}"#
        );
    }

    /// The trigger and its time travel together, and a trigger that needs no
    /// time says so with a `null` rather than by leaving the field out.
    #[test]
    fn a_trigger_carries_its_time_or_a_null() {
        let json = serde_json::to_value(CueProperty::Trigger {
            trigger: CueTrigger::Go,
            trigger_time: None,
        })
        .unwrap();
        assert!(json["triggerTime"].is_null(), "{json}");
    }

    /// A fade time that is not a number never reaches a cue: `crate::finite`
    /// guards this field on the way in as well as on the way out, because
    /// MessagePack can carry a NaN faithfully and a cue with one would never
    /// finish fading.
    #[test]
    fn a_fade_time_that_is_not_a_number_is_refused_in_both_directions() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                serde_json::to_string(&CueProperty::FadeIn { seconds: value }).is_err(),
                "{value} was written"
            );
        }
        let packed = rmp_serde::to_vec_named(&serde_json::json!({
            "t": "FadeOut",
            "seconds": f64::INFINITY,
        }))
        .unwrap();
        assert!(rmp_serde::from_slice::<CueProperty>(&packed).is_err());
    }
}
