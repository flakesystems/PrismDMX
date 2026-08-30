//! Executors — the eight faders of the current page (D7).
//!
//! Each executor binds a sequence to a fader, four buttons and an encoder. The
//! function assignments are data, not code paths, because the same executor
//! model is driven from the X-Touch and from the UI.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ExecutorId, SequenceId};

/// Executors per page (decision **D7**).
pub const EXECUTORS_PER_PAGE: u32 = 8;

/// Buttons per executor: Rec, Solo, Mute, Select (`docs/MCU_MAPPING.md` §2.1).
///
/// Stated once, and this is the once: the control editor draws four rows,
/// `prism_core::Show::configure_executor` refuses a fifth, and the desk's own
/// strip has exactly this many keys under each fader.
pub const EXECUTOR_BUTTONS: u8 = 4;

/// [`crate::Sequence::speed`] at which a list plays at the times its cues carry.
///
/// A fixed-point rate rather than a float: it crosses into the tick, where
/// `prism_engine` multiplies it into an elapsed-tick accumulator, and a power of
/// two keeps that exact. `0` freezes a playback, [`SPEED_UNITY`] is 1x, and
/// `u16::MAX` is just under 64x — which is the range a *speed master* covers
/// (`docs/DMX_MERGE.md` section 4 item 3), the thing S34 built and S22 recorded
/// as having no domain type at all.
pub const SPEED_UNITY: u16 = 1_024;

/// What one of an executor's four buttons does.
///
/// **No longer `Copy` since S45**, because [`Self::CommandLine`] carries a line
/// an operator wrote. That is `prism_domain::SurfaceAction`'s history repeating
/// (S43) and `prism_core::Effect`'s before it (S37): the moment a vocabulary can
/// name something a person typed, it stops fitting in a register. It costs a
/// `clone` at the handful of places that matched on one by value.
///
/// The eight fixed functions keep their bare-string encoding, so a `.prism` file
/// written before S45 reads unchanged; the ninth is externally tagged, which is
/// serde's default for a variant with a field.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
pub enum ExecutorButtonFunction {
    /// Nothing assigned.
    #[default]
    Empty,
    /// Advance to the next cue.
    #[serde(rename = "Go+")]
    GoForward,
    /// Step back to the previous cue.
    #[serde(rename = "Go-")]
    GoBack,
    /// Learn the sequence speed from the tap rhythm.
    LearnSpeed,
    /// Stop the sequence.
    Off,
    /// Start the sequence.
    On,
    /// Momentary full level while held.
    Flash,
    /// Latch on and off.
    Toggle,
    /// Send a line the operator wrote — the **custom row** of S45's control
    /// editor, and punch-list entry B15's third round.
    ///
    /// It is a variant here rather than eight more variants somebody has to
    /// invent, because the owner's answer was that every desk needs a different
    /// number of them: *fire cue 3 of list 7*, *blackout the house*, *page to
    /// 4*. `SurfaceAction::WriteCommandLine` is the same answer for a key on the
    /// desk, and this is it for a key on an executor.
    ///
    /// **It writes and runs**, unlike the surface's, which offers the choice. An
    /// executor key is not a keyboard: there is nothing to correct with before
    /// the next press, and a Go key that needed Enter afterwards is not a Go
    /// key. Who runs it is the **daemon**, since S49: the press becomes
    /// `Command::CommandLineInput { run: true }` and `prism_core::console` reads
    /// the line. Before S49 the daemon could only write it down and hope a
    /// client with the keyboard focus was watching.
    CommandLine {
        /// The line, exactly as it would be typed.
        line: String,
    },
}

/// Which of an executor's buttons a press names.
///
/// **The executor decides what a press means, not whoever sent it.** A strip
/// button is a *position* on the hardware — the operator pressed the third key
/// of that strip — and what that key does is [`Executor::button_functions`],
/// which is show data. A client that read `is_active` and sent a Go or an Off
/// because the show said `Toggle` would be deciding for the show: two clients
/// would race, and the daemon would be told to do something nobody pressed.
/// `prism_core::Show::apply` is the one place that resolution happens.
///
/// [`Self::Function`] is the other half of the same rule rather than a hole in
/// it. `docs/MCU_MAPPING.md` section 4.1 binds the transport keys to `On`, `Off`,
/// `Go+` and `Go-` **in the profile**, which is the desk's own configuration
/// written by a person, not an inference a client made at run time. A binding
/// that names `Toggle` is still resolved by the daemon against `is_active`; what
/// the profile chose is *which function*, never *what it comes out as*.
///
/// **Not `Copy` since S45**, because [`ExecutorButtonFunction`] stopped being
/// one — a profile row may name the custom row and carry the line with it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum ExecutorButtonRef {
    /// The executor's own button at a hardware position: Rec, Solo, Mute,
    /// Select = 0..4 (`docs/MCU_MAPPING.md` section 2.1).
    ///
    /// A position the executor has no function for is not an error — it is a key
    /// with nothing on it, and pressing it does nothing.
    Slot {
        /// Which of the four, from zero.
        index: u8,
    },
    /// A function bound directly by the desk's configuration — a profile row, a
    /// transport key, an interface's own key.
    Function {
        /// What was bound.
        function: ExecutorButtonFunction,
    },
}

/// What an executor's fader does.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ExecutorFaderFunction {
    /// Nothing assigned.
    #[default]
    Empty,
    /// Master level of the sequence.
    Master,
    /// Playback speed.
    Speed,
    /// Manual crossfade between cues.
    XFade,
}

impl ExecutorFaderFunction {
    /// Every function a fader can be given, so a test — and S45's control
    /// editor — walks the whole set rather than the ones somebody remembered.
    ///
    /// **No custom row here, and that is deliberate.** A button is a moment and
    /// a line is a moment, so `ExecutorButtonFunction::CommandLine` is one key
    /// press spelled out; a fader is a stream of positions at S25's cadence and
    /// a line has nowhere to put one. `ARCHITECTURE_SPEC.md` §4.5 names the
    /// executor faders as the exception a line cannot express, and this is that
    /// sentence read the other way round.
    pub const ALL: [Self; 4] = [Self::Empty, Self::Master, Self::Speed, Self::XFade];
}

/// What an executor's encoder does.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ExecutorEncoderFunction {
    /// Nothing assigned.
    #[default]
    Empty,
    /// Master level of the sequence.
    Master,
    /// Playback speed.
    Speed,
}

impl ExecutorEncoderFunction {
    /// Every function an encoder can be given — [`ExecutorFaderFunction::ALL`]'s
    /// reason, and it has no custom row for the same one.
    pub const ALL: [Self; 3] = [Self::Empty, Self::Master, Self::Speed];
}

/// One control of an executor, and what it is to do — **S45**.
///
/// # One control at a time
///
/// `OutputChange`'s rule and `CueProperty`'s and `MachineChange`'s: a command
/// carrying the whole executor would make a client read it, change one member
/// and send the rest back, and two operators with the editor open would each
/// undo the other. What a *slot* holds — which cue list stands on it — is
/// `Command::AssignExecutor` and stays its own command, because that is a
/// different act: putting a show on a fader rather than saying what the fader
/// does.
///
/// # Why this is a `Command` and not a `MachineChange`
///
/// The one decision S45 exists to make, and both precedents were real.
/// `docs/IPC_PROTOCOL.md` §5 carries the argument in full; in short, an executor
/// is one of the six numbered things `ObjectRef` names — it is copied, moved,
/// labelled and coloured by show verbs, it lives in the `.prism` file, and a Web
/// Remote with no X-Touch plugged in still has eight of them with four keys
/// each. S38's F-keys are *hardware controls of this building*; these are not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum ExecutorChange {
    /// What the fader does.
    Fader {
        /// The function.
        function: ExecutorFaderFunction,
    },
    /// What the encoder does.
    Encoder {
        /// The function.
        function: ExecutorEncoderFunction,
    },
    /// What one of the four buttons does.
    ///
    /// The index is a *hardware position* — Rec, Solo, Mute, Select = 0..4,
    /// `docs/MCU_MAPPING.md` §2.1 — exactly as [`ExecutorButtonRef::Slot`]'s is.
    /// An index past the four is refused rather than clamped: a desk that
    /// quietly assigned a fifth key would be answering a question nobody asked.
    Button {
        /// Which of the four, from zero.
        index: u8,
        /// The function.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        function: ExecutorButtonFunction,
    },
}

/// One executor: a **handle** on a cue list's playback — S45.
///
/// It says which list, what its fader does, what each of its four keys does and
/// what its encoder does. What it deliberately no longer says is what the list
/// is *doing*: the master level, the rate, whether it is running and which cue
/// it stands on are the playback's, and a playback is the cue list's
/// ([`crate::PlaybackId`]). Two executors carrying one list are therefore two
/// handles on one number rather than two opinions about it, which is punch-list
/// entry B18.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Executor {
    /// Executor number: `page * 8 + slot`.
    pub id: ExecutorId,
    /// The sequence this executor plays, if one is assigned.
    pub sequence_id: Option<SequenceId>,
    /// Fader assignment.
    pub fader_function: ExecutorFaderFunction,
    /// Button assignments, in hardware order: Rec, Solo, Mute, Select.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub button_functions: Vec<ExecutorButtonFunction>,
    /// Encoder assignment.
    pub encoder_function: ExecutorEncoderFunction,
}

impl ExecutorId {
    /// The executor page this executor belongs to.
    #[must_use]
    pub const fn page(self) -> u32 {
        self.get() / EXECUTORS_PER_PAGE
    }

    /// The slot within its page, `0..8`.
    #[must_use]
    pub const fn slot(self) -> u32 {
        self.get() % EXECUTORS_PER_PAGE
    }

    /// Builds an executor number from a page and a slot.
    ///
    /// Saturates rather than wrapping. `SetExecutorPage` carries an arbitrary
    /// `u32` from any client, and in a release build `page * 8` would wrap
    /// silently for a large one — addressing the wrong executor during a show.
    /// Saturating keeps an out-of-range page out of range instead.
    #[must_use]
    pub const fn from_page_and_slot(page: u32, slot: u32) -> Self {
        Self::new(page.saturating_mul(EXECUTORS_PER_PAGE).saturating_add(slot))
    }
}

impl ExecutorButtonFunction {
    /// Every **fixed** function a key can be given, so a test — and S45's
    /// control editor — walks the whole set rather than the ones somebody
    /// remembered.
    ///
    /// [`Self::CommandLine`] is deliberately not in it: it is not a choice, it
    /// is a choice plus a line somebody typed, so the editor draws it as a row
    /// with a box rather than as a ninth entry in a list.
    ///
    /// Added in S38.
    pub const ALL: [Self; 8] = [
        Self::Empty,
        Self::GoForward,
        Self::GoBack,
        Self::LearnSpeed,
        Self::Off,
        Self::On,
        Self::Flash,
        Self::Toggle,
    ];

    /// The line this key sends, if it is a custom row.
    #[must_use]
    pub fn command_line(&self) -> Option<&str> {
        match self {
            Self::CommandLine { line } => Some(line),
            _ => None,
        }
    }
}

/// [`crate::arb::an_action`]'s trick, for [`crate::arb::arbitrary_from_list`]'s
/// reason: eight unit variants derived as an eight-way union cost eight slots of
/// 560 bytes each, and the ninth carries the only `String` on the enum, so a
/// fixed line would leave it untested through the codec.
#[cfg(any(test, feature = "proptest"))]
impl proptest::arbitrary::Arbitrary for ExecutorButtonFunction {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;
        prop_oneof![
            8 => (0..Self::ALL.len()).prop_map(|index| Self::ALL[index].clone()),
            1 => ".{0,24}".prop_map(|line| Self::CommandLine { line }),
        ]
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        EXECUTOR_BUTTONS, Executor, ExecutorButtonFunction, ExecutorButtonRef, ExecutorChange,
        ExecutorEncoderFunction, ExecutorFaderFunction, ExecutorId, SequenceId,
    };
    use ts_rs::{Config, TS};

    fn executor() -> Executor {
        Executor {
            id: ExecutorId::new(9),
            sequence_id: Some(SequenceId::new(1)),
            fader_function: ExecutorFaderFunction::Master,
            button_functions: vec![
                ExecutorButtonFunction::GoForward,
                ExecutorButtonFunction::GoBack,
            ],
            encoder_function: ExecutorEncoderFunction::Speed,
        }
    }

    #[test]
    fn executor_matches_the_wire_shape() {
        // S45 took four fields off: the master, the rate, whether it runs and
        // which cue it stands on are the *playback's*, and a playback is the cue
        // list's. What is left is the assignment, which is what an executor is.
        assert_eq!(
            serde_json::to_string(&executor()).unwrap(),
            r#"{"id":9,"sequenceId":1,"faderFunction":"Master","buttonFunctions":["Go+","Go-"],"encoderFunction":"Speed"}"#
        );
    }

    #[test]
    fn go_buttons_keep_their_console_labels() {
        assert_eq!(
            serde_json::to_string(&ExecutorButtonFunction::GoForward).unwrap(),
            "\"Go+\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutorButtonFunction::GoBack).unwrap(),
            "\"Go-\""
        );
        let back: ExecutorButtonFunction = serde_json::from_str("\"Go-\"").unwrap();
        assert_eq!(back, ExecutorButtonFunction::GoBack);
    }

    #[test]
    fn the_custom_row_carries_its_line_and_the_eight_fixed_ones_still_read_as_strings() {
        // The whole of S45's backwards compatibility for `buttonFunctions`: the
        // eight that existed keep their bare-string encoding, so an executor
        // written before this session reads unchanged, and the ninth is
        // externally tagged because it has a field.
        let custom = ExecutorButtonFunction::CommandLine {
            line: "Go+ Sequence 3".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&custom).unwrap(),
            r#"{"CommandLine":{"line":"Go+ Sequence 3"}}"#
        );
        let back: ExecutorButtonFunction =
            serde_json::from_str(r#"{"CommandLine":{"line":"Go+ Sequence 3"}}"#).unwrap();
        assert_eq!(back, custom);
        assert_eq!(custom.command_line(), Some("Go+ Sequence 3"));
        assert_eq!(ExecutorButtonFunction::Flash.command_line(), None);
        let old: Vec<ExecutorButtonFunction> =
            serde_json::from_str(r#"["Go+","Go-","Off","Empty"]"#).expect("an older list reads");
        assert_eq!(old[0], ExecutorButtonFunction::GoForward);
        assert_eq!(old[3], ExecutorButtonFunction::Empty);
    }

    #[test]
    fn an_executor_written_before_s45_loses_only_what_moved() {
        // A `.prism` file keeps each executor as an opaque document (S15). The
        // four fields S45 moved are ignored on the way in — `prism_core::store`
        // reads the *level* out of the raw row and puts it on the sequence, which
        // is the half a `serde(default)` cannot do — and everything the executor
        // still is survives.
        let json = r#"{"id":9,"sequenceId":1,"faderFunction":"Master",
            "buttonFunctions":["Go+"],"encoderFunction":"Empty","masterLevel":40000,
            "speed":2048,"isActive":true,"currentCueIndex":3}"#;
        let executor: Executor = serde_json::from_str(json).expect("an older document still reads");
        assert_eq!(executor.id, ExecutorId::new(9));
        assert_eq!(executor.sequence_id, Some(SequenceId::new(1)));
        assert_eq!(executor.fader_function, ExecutorFaderFunction::Master);
        assert_eq!(
            executor.button_functions,
            vec![ExecutorButtonFunction::GoForward]
        );
    }

    #[test]
    fn an_unassigned_slot_is_empty_not_absent() {
        assert_eq!(
            ExecutorFaderFunction::default(),
            ExecutorFaderFunction::Empty
        );
        assert_eq!(
            ExecutorEncoderFunction::default(),
            ExecutorEncoderFunction::Empty
        );
        assert_eq!(
            ExecutorButtonFunction::default(),
            ExecutorButtonFunction::Empty
        );
    }

    #[test]
    fn every_function_the_editor_offers_is_in_its_type_s_own_list() {
        // The lists the control editor draws from, and the reason they are
        // consts rather than three arrays in TypeScript: a function added in
        // Rust appears in the chooser without a second list being edited.
        assert_eq!(ExecutorButtonFunction::ALL.len(), 8);
        assert!(
            !ExecutorButtonFunction::ALL
                .iter()
                .any(|function| matches!(function, ExecutorButtonFunction::CommandLine { .. }))
        );
        assert_eq!(ExecutorFaderFunction::ALL.len(), 4);
        assert_eq!(ExecutorEncoderFunction::ALL.len(), 3);
        assert_eq!(EXECUTOR_BUTTONS, 4);
    }

    #[test]
    fn a_change_names_one_control_and_says_which() {
        // `OutputChange`'s shape and `CueProperty`'s: one field per command, so
        // two operators with the editor open cannot undo each other.
        assert_eq!(
            serde_json::to_string(&ExecutorChange::Fader {
                function: ExecutorFaderFunction::XFade
            })
            .unwrap(),
            r#"{"t":"Fader","function":"XFade"}"#
        );
        assert_eq!(
            serde_json::to_string(&ExecutorChange::Encoder {
                function: ExecutorEncoderFunction::Speed
            })
            .unwrap(),
            r#"{"t":"Encoder","function":"Speed"}"#
        );
        assert_eq!(
            serde_json::to_string(&ExecutorChange::Button {
                index: 2,
                function: ExecutorButtonFunction::Toggle
            })
            .unwrap(),
            r#"{"t":"Button","index":2,"function":"Toggle"}"#
        );
    }

    #[test]
    fn executor_ids_decompose_into_page_and_slot() {
        // D7: one page is eight executors, so id = page * 8 + slot.
        assert_eq!(ExecutorId::new(0).page(), 0);
        assert_eq!(ExecutorId::new(0).slot(), 0);
        assert_eq!(ExecutorId::new(9).page(), 1);
        assert_eq!(ExecutorId::new(9).slot(), 1);
        assert_eq!(ExecutorId::from_page_and_slot(1, 1), ExecutorId::new(9));
    }

    #[test]
    fn an_absurd_page_number_saturates_instead_of_wrapping() {
        // SetExecutorPage carries an arbitrary u32 from any client. Wrapping
        // would silently address a low executor - a live show hazard.
        assert_eq!(
            ExecutorId::from_page_and_slot(u32::MAX, 7),
            ExecutorId::new(u32::MAX)
        );
        assert_eq!(
            ExecutorId::from_page_and_slot(u32::MAX / 8, 0).get(),
            (u32::MAX / 8) * 8
        );
    }

    #[test]
    fn a_button_press_names_a_position_or_a_function_and_the_two_are_distinguishable() {
        // A strip press is a *position*: what it does is the executor's own
        // `buttonFunctions`, resolved by `prism_core::Show::apply` and nowhere
        // else. A profile row names a function outright, which is the desk's
        // configuration rather than a client's inference. The wire has to keep
        // the two apart, so the tag is not optional.
        assert_eq!(
            serde_json::to_string(&ExecutorButtonRef::Slot { index: 2 }).unwrap(),
            r#"{"t":"Slot","index":2}"#
        );
        assert_eq!(
            serde_json::to_string(&ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::Toggle
            })
            .unwrap(),
            r#"{"t":"Function","function":"Toggle"}"#
        );
        for reference in [
            ExecutorButtonRef::Slot { index: 0 },
            ExecutorButtonRef::Slot { index: u8::MAX },
            ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::GoForward,
            },
        ] {
            let json = serde_json::to_string(&reference).unwrap();
            let back: ExecutorButtonRef = serde_json::from_str(&json).unwrap();
            assert_eq!(back, reference);
        }
    }

    #[test]
    fn speed_unity_is_the_rate_a_sequence_plays_its_own_times_at() {
        // A power of two, because the tick multiplies it into an accumulator and
        // divides by it again: anything else quantises a fade differently at
        // unity than at rest, which is a fade that changes shape when nobody
        // touched it.
        assert_eq!(crate::SPEED_UNITY, 1_024);
        assert!(crate::SPEED_UNITY.is_power_of_two());
        // The whole speed range in one line: frozen, unity, and just under 64x.
        assert_eq!(u32::from(u16::MAX) / u32::from(crate::SPEED_UNITY), 63);
    }

    #[test]
    fn typescript_unions_match_the_specification() {
        let cfg = Config::new();
        // The eight fixed functions are a union of string literals and stay
        // one, so `buttonFunctions` reads in TypeScript the way it reads in a
        // `.prism` file. The custom row is the ninth arm; its doc comment
        // travels with it, so the assertion is on the shape rather than on the
        // prose.
        let button = ExecutorButtonFunction::inline(&cfg);
        assert!(
            button.starts_with(
                "\"Empty\" | \"Go+\" | \"Go-\" | \"LearnSpeed\" | \"Off\" | \"On\" | \"Flash\" \
                 | \"Toggle\" | { \"CommandLine\": {"
            ),
            "{button}"
        );
        assert!(button.contains("line: string,"), "{button}");
        assert_eq!(
            ExecutorFaderFunction::inline(&cfg),
            "\"Empty\" | \"Master\" | \"Speed\" | \"XFade\""
        );
        assert_eq!(
            ExecutorEncoderFunction::inline(&cfg),
            "\"Empty\" | \"Master\" | \"Speed\""
        );
    }
}
