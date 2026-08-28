//! The control surface's binding vocabulary — `docs/MCU_MAPPING.md` §4.
//!
//! # Why layer 3's vocabulary is domain vocabulary, and not `prism-surface`'s
//!
//! Until S38 every type below lived in `prism-surface`, which was right while
//! the only thing that read a binding table was the thing that decoded MIDI. It
//! stopped being right the moment the table became something an **operator
//! edits from a window**: a `BoundControl` now travels in a
//! [`crate::Command`], comes back in a [`crate::Query`]'s answer and is drawn
//! by an interface, and a type on the wire is this crate's by definition — the
//! interface's copy of it is generated from here.
//!
//! What did **not** move is the part that needs a device. `docs/MCU_MAPPING.md`
//! §2.1's note and CC numbers are still in `prism_surface::profile::X_TOUCH`
//! and nowhere else, which is the promise §2.1 makes in as many words; so are
//! the shadow model, the codec and the resolution of an action into a
//! [`crate::Command`], which needs an event and a session. What moved is the
//! **names**: which controls a surface has and what a binding may say about
//! one. `IMPLEMENTATION_PLAN.md` S32 predicted the split from the other side —
//! an OSC control that fires a command belongs in the same editor rather than
//! in a second mapping system — and a vocabulary that is not MCU-specific
//! cannot live in the MCU crate.
//!
//! # One statement of the reserved control
//!
//! [`RESERVED_BUTTONS`] is the whole of §4.3's rule and it is stated **once**:
//! `prism_surface::X_TOUCH.reserved_buttons` points at this array rather than
//! carrying a copy, so there is no pair to keep in step. It is here rather than
//! there because the refusal has to happen where a *command* is validated —
//! `prism_core::MachineConfig::apply` — and that is a crate which must never
//! depend on a MIDI codec.

use core::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ExecutorButtonRef, FeatureGroup, GoDirection, ParamDirection, ViewId, WindowType};

/// Which button on a channel strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub enum StripButton {
    /// Rec / Arm, the top button of the strip.
    Rec,
    /// Solo.
    Solo,
    /// Mute.
    Mute,
    /// Select.
    Select,
    /// Pushing the V-Pot in. A button, not a rotation — see
    /// [`ControlEvent::VPot`](crate::ControlEvent::VPot) for the turn.
    VPotPush,
}

impl StripButton {
    /// Every strip button, so a test can walk the whole set rather than the
    /// ones somebody remembered.
    pub const ALL: [Self; 5] = [
        Self::Rec,
        Self::Solo,
        Self::Mute,
        Self::Select,
        Self::VPotPush,
    ];

    /// This button's position in [`ALL`](Self::ALL), which is how layer 2's
    /// shadow model indexes a strip's LEDs.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// What a binding profile calls this button, as `docs/MCU_MAPPING.md` §3
    /// spells it: `Strip[*].Button.Select`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rec => "Rec",
            Self::Solo => "Solo",
            Self::Mute => "Mute",
            Self::Select => "Select",
            Self::VPotPush => "VPotPush",
        }
    }

    /// The button a control name refers to — the inverse of
    /// [`name`](Self::name).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|button| button.name() == name)
    }
}

impl fmt::Display for StripButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Every button on the panel that does not belong to a channel strip.
///
/// The names are the ones printed on the surface, which is deliberate: this
/// layer knows MIDI and topology, and nothing about what PrismDMX does when one
/// is pressed. `Play` is a note number here; that it starts an executor is
/// layer 3's opinion (`docs/MCU_MAPPING.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub enum GlobalButton {
    /// Encoder Assign: Track.
    AssignTrack,
    /// Encoder Assign: Send.
    AssignSend,
    /// Encoder Assign: Pan/Surround.
    AssignPan,
    /// Encoder Assign: Plug-in.
    AssignPlugin,
    /// Encoder Assign: EQ.
    AssignEq,
    /// Encoder Assign: Instrument.
    AssignInstrument,
    /// Fader bank left — executor page down (D7).
    BankLeft,
    /// Fader bank right — executor page up (D7).
    BankRight,
    /// Channel left — previous UI view (D8).
    ChannelLeft,
    /// Channel right — next UI view (D8).
    ChannelRight,
    /// Flip.
    Flip,
    /// Global View.
    GlobalView,
    /// Display: Name/Value.
    NameValue,
    /// Display: SMPTE/Beats.
    SmpteBeats,
    /// Function key 1.
    F1,
    /// Function key 2.
    F2,
    /// Function key 3.
    F3,
    /// Function key 4.
    F4,
    /// Function key 5.
    F5,
    /// Function key 6.
    F6,
    /// Function key 7.
    F7,
    /// Function key 8.
    F8,
    /// Global View group: MIDI Tracks.
    ViewMidiTracks,
    /// Global View group: Inputs.
    ViewInputs,
    /// Global View group: Audio Tracks.
    ViewAudioTracks,
    /// Global View group: Audio Instruments.
    ViewAudioInstruments,
    /// Global View group: Aux.
    ViewAux,
    /// Global View group: Busses.
    ViewBusses,
    /// Global View group: Outputs.
    ViewOutputs,
    /// Global View group: User.
    ViewUser,
    /// Modifier: Shift. Held, not latched.
    ModShift,
    /// Modifier: Option. Held, not latched.
    ModOption,
    /// Modifier: Control. Held, not latched.
    ModControl,
    /// Modifier: Alt. Held, not latched.
    ModAlt,
    /// Automation: Read/Off.
    AutoRead,
    /// Automation: Write.
    AutoWrite,
    /// Automation: Trim.
    AutoTrim,
    /// Automation: Touch. Not the fader touch sensor — that is
    /// [`ControlEvent::Touch`](crate::ControlEvent::Touch).
    AutoTouch,
    /// Automation: Latch.
    AutoLatch,
    /// Automation: Group.
    AutoGroup,
    /// Utility: Save.
    Save,
    /// Utility: Undo.
    Undo,
    /// Utility: Cancel.
    Cancel,
    /// Utility: Enter.
    Enter,
    /// Upper transport row: Markers.
    Markers,
    /// Upper transport row: Nudge.
    Nudge,
    /// Upper transport row: Cycle.
    Cycle,
    /// Upper transport row: Drop.
    Drop,
    /// Upper transport row: Replace.
    Replace,
    /// Upper transport row: Click.
    Click,
    /// Upper transport row: Solo. The global one that clears solos, not a
    /// strip's — see [`StripButton::Solo`].
    SoloClear,
    /// Transport: Rewind.
    Rewind,
    /// Transport: Fast forward.
    FastForward,
    /// Transport: Stop.
    Stop,
    /// Transport: Play.
    Play,
    /// Transport: Record.
    Record,
    /// Cursor: up.
    CursorUp,
    /// Cursor: down.
    CursorDown,
    /// Cursor: left.
    CursorLeft,
    /// Cursor: right.
    CursorRight,
    /// Cursor cluster: Zoom.
    Zoom,
    /// Cursor cluster: Scrub.
    Scrub,
    /// Foot switch 1.
    FootSwitch1,
    /// Foot switch 2.
    FootSwitch2,
}

impl GlobalButton {
    /// Every global button.
    ///
    /// Kept beside `prism_surface::X_TOUCH`'s note table rather than derived from it, so
    /// that the two have to agree — and a test asserts they do, in both
    /// directions. A button added here without a note, or a note added there
    /// without a button, turns that test red.
    pub const ALL: [Self; 64] = [
        Self::AssignTrack,
        Self::AssignSend,
        Self::AssignPan,
        Self::AssignPlugin,
        Self::AssignEq,
        Self::AssignInstrument,
        Self::BankLeft,
        Self::BankRight,
        Self::ChannelLeft,
        Self::ChannelRight,
        Self::Flip,
        Self::GlobalView,
        Self::NameValue,
        Self::SmpteBeats,
        Self::F1,
        Self::F2,
        Self::F3,
        Self::F4,
        Self::F5,
        Self::F6,
        Self::F7,
        Self::F8,
        Self::ViewMidiTracks,
        Self::ViewInputs,
        Self::ViewAudioTracks,
        Self::ViewAudioInstruments,
        Self::ViewAux,
        Self::ViewBusses,
        Self::ViewOutputs,
        Self::ViewUser,
        Self::ModShift,
        Self::ModOption,
        Self::ModControl,
        Self::ModAlt,
        Self::AutoRead,
        Self::AutoWrite,
        Self::AutoTrim,
        Self::AutoTouch,
        Self::AutoLatch,
        Self::AutoGroup,
        Self::Save,
        Self::Undo,
        Self::Cancel,
        Self::Enter,
        Self::Markers,
        Self::Nudge,
        Self::Cycle,
        Self::Drop,
        Self::Replace,
        Self::Click,
        Self::SoloClear,
        Self::Rewind,
        Self::FastForward,
        Self::Stop,
        Self::Play,
        Self::Record,
        Self::CursorUp,
        Self::CursorDown,
        Self::CursorLeft,
        Self::CursorRight,
        Self::Zoom,
        Self::Scrub,
        Self::FootSwitch1,
        Self::FootSwitch2,
    ];

    /// This button's position in [`ALL`](Self::ALL), which is how layer 2's
    /// shadow model indexes the panel's LEDs.
    ///
    /// The note number would be the obvious index and is the wrong one: it is a
    /// property of *this* surface, and the shadow model is layer 2's, where a
    /// device with a different note map still has these sixty-four buttons.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// What a binding profile calls this button, after the `Global.` prefix
    /// (`docs/MCU_MAPPING.md` §4.2).
    ///
    /// The variant's own spelling, deliberately: a profile is written by a
    /// person against this list, and a second vocabulary — the legend printed on
    /// the panel, say, with its slashes and spaces — would be a second thing to
    /// keep in step. A test asserts the sixty-four names are distinct and that
    /// [`from_name`](Self::from_name) reverses every one.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AssignTrack => "AssignTrack",
            Self::AssignSend => "AssignSend",
            Self::AssignPan => "AssignPan",
            Self::AssignPlugin => "AssignPlugin",
            Self::AssignEq => "AssignEq",
            Self::AssignInstrument => "AssignInstrument",
            Self::BankLeft => "BankLeft",
            Self::BankRight => "BankRight",
            Self::ChannelLeft => "ChannelLeft",
            Self::ChannelRight => "ChannelRight",
            Self::Flip => "Flip",
            Self::GlobalView => "GlobalView",
            Self::NameValue => "NameValue",
            Self::SmpteBeats => "SmpteBeats",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::ViewMidiTracks => "ViewMidiTracks",
            Self::ViewInputs => "ViewInputs",
            Self::ViewAudioTracks => "ViewAudioTracks",
            Self::ViewAudioInstruments => "ViewAudioInstruments",
            Self::ViewAux => "ViewAux",
            Self::ViewBusses => "ViewBusses",
            Self::ViewOutputs => "ViewOutputs",
            Self::ViewUser => "ViewUser",
            Self::ModShift => "ModShift",
            Self::ModOption => "ModOption",
            Self::ModControl => "ModControl",
            Self::ModAlt => "ModAlt",
            Self::AutoRead => "AutoRead",
            Self::AutoWrite => "AutoWrite",
            Self::AutoTrim => "AutoTrim",
            Self::AutoTouch => "AutoTouch",
            Self::AutoLatch => "AutoLatch",
            Self::AutoGroup => "AutoGroup",
            Self::Save => "Save",
            Self::Undo => "Undo",
            Self::Cancel => "Cancel",
            Self::Enter => "Enter",
            Self::Markers => "Markers",
            Self::Nudge => "Nudge",
            Self::Cycle => "Cycle",
            Self::Drop => "Drop",
            Self::Replace => "Replace",
            Self::Click => "Click",
            Self::SoloClear => "SoloClear",
            Self::Rewind => "Rewind",
            Self::FastForward => "FastForward",
            Self::Stop => "Stop",
            Self::Play => "Play",
            Self::Record => "Record",
            Self::CursorUp => "CursorUp",
            Self::CursorDown => "CursorDown",
            Self::CursorLeft => "CursorLeft",
            Self::CursorRight => "CursorRight",
            Self::Zoom => "Zoom",
            Self::Scrub => "Scrub",
            Self::FootSwitch1 => "FootSwitch1",
            Self::FootSwitch2 => "FootSwitch2",
        }
    }

    /// The button a control name refers to — the inverse of
    /// [`name`](Self::name).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|button| button.name() == name)
    }
}

impl fmt::Display for GlobalButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// `GlobalButton` is the enum that made this necessary: sixty-four buttons
// carrying nothing, and a derived value tree of 36 064 bytes — larger than the
// whole of `Command`. See `crate::arb::arbitrary_from_list`.
#[cfg(any(test, feature = "proptest"))]
crate::arb::arbitrary_from_list!(GlobalButton);
#[cfg(any(test, feature = "proptest"))]
crate::arb::arbitrary_from_list!(StripButton);

/// Which executor an action acts on.
///
/// Two answers, because `docs/MCU_MAPPING.md` §4.1 has two kinds of row: a
/// strip's own controls act on the executor under that strip, and the transport
/// section, the Flip button and the main fader act on the one that is
/// *selected*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ExecutorTarget {
    /// The executor on the strip the event came from: `executorPage * 8 + index`
    /// (**D7**). Nothing at all when the control is not a strip's.
    Strip,
    /// The executor the session has selected. Nothing when none is.
    Selected,
}

/// Which way a relative move goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum Step {
    /// Backwards — `Channel ◀`.
    Prev,
    /// Forwards — `Channel ▶`.
    Next,
}

/// What a control does.
///
/// A vocabulary of its own rather than a `Command` with holes in it: a binding
/// is written before there is an event, and half of `Command`'s fields are
/// answers only an event and a session have. The variants that need nothing are
/// still their own variant rather than a `Command`, so the table has one shape.
///
/// **Every one of these resolves to a command that already exists.** S22 wrote
/// that down as a constraint and recorded the three rows of §4.1 it could not
/// then satisfy; S34 gave the protocol the command they were waiting for, and
/// [`Self::ExecutorButton`] is how they are satisfied now. The constraint is
/// unchanged: a name in a user-editable file that nothing answers is worse than
/// a row this table cannot express.
/// **No longer `Copy` since S43**, because [`Self::WriteCommandLine`] carries a
/// line. That is `prism_core::Effect`'s history repeating (S37): the moment a
/// vocabulary can name something an operator wrote, it stops fitting in a
/// register. It costs a `clone` at the handful of places that matched on one by
/// value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum SurfaceAction {
    /// Move an executor's master — a fader, or an encoder.
    ExecutorMaster {
        /// Whose master.
        target: ExecutorTarget,
    },
    /// Step an executor's sequence.
    ExecutorGo {
        /// Which executor.
        target: ExecutorTarget,
        /// Which way.
        direction: GoDirection,
    },
    /// Stop an executor.
    ExecutorOff {
        /// Which executor.
        target: ExecutorTarget,
    },
    /// Press one of an executor's buttons, and let the executor decide what that
    /// means — `docs/MCU_MAPPING.md` §4.1's strip-button and transport rows.
    ///
    /// The **only** action here that forwards a release as well as a press: a
    /// `Flash` is momentary and its release is half the gesture. Layer 3 does
    /// not know which function it is sending to, which is the point — the
    /// executor knows, and `prism_core::Show::apply` is where a release that
    /// means nothing is dropped.
    ExecutorButton {
        /// Which executor.
        target: ExecutorTarget,
        /// Which of its buttons — a hardware position for a strip key, a named
        /// function for a panel key this profile has assigned outright.
        button: ExecutorButtonRef,
    },
    /// Make an executor the selected one, which is what the transport section
    /// and the main fader then act on.
    SelectExecutor {
        /// Which executor.
        target: ExecutorTarget,
    },
    /// Advance the three-stage Clear.
    ClearProgrammer,
    /// Page the fader bank — **D7**, eight executors to a page.
    ExecutorPage {
        /// How many pages, signed. Saturates at page 0 rather than wrapping.
        delta: i32,
    },
    /// Jump to a stored view by number.
    SelectView {
        /// Which view.
        view: ViewId,
    },
    /// Move to the neighbouring stored view — **D8**, `Channel ◀▶`.
    ///
    /// Which view that *is* comes from `prism_surface::SurfaceContext`: the view library
    /// belongs to the session, and this crate does not hold one.
    StepView {
        /// Which way.
        direction: Step,
    },
    /// Page the programmer.
    ProgrammerPage {
        /// How many pages, signed. Saturates at page 0.
        delta: i32,
    },
    /// Move the programmer parameter the jog wheel turns.
    SelectProgrammerParam {
        /// Which way.
        direction: ParamDirection,
    },
    /// Change the selected programmer parameter by the steps the control
    /// reported — the jog wheel's row in §4.1.
    AdjustParameter,
    /// Switch the encoder bank.
    SetEncoderBank {
        /// Which feature group.
        group: FeatureGroup,
    },
    /// Open a window on the canvas.
    OpenWindow {
        /// Which window.
        window: WindowType,
    },
    /// Open the window chooser, so the operator picks which.
    ///
    /// **S43**, and the counterpart to [`Self::OpenWindow`] rather than a
    /// replacement for it: a key bound to a *particular* window is what an
    /// operator sets up for a show, and a key that opens the chooser is what
    /// they reach for when they have not. The punch list asks for both (B9).
    ///
    /// Resolves to `Command::SetWindowPicker { open: true }`. That the chooser
    /// is a *panel* and this is a *daemon* is the whole reason
    /// `Session::window_picker` is session state — see its documentation.
    OpenWindowPicker,
    /// Put a line into the command line, ready to be run.
    ///
    /// **S43, punch-list entry B4** — the one action an operator writes the
    /// contents of, which is why the control editor calls these *custom* rows
    /// and lets a desk have as many as it needs.
    ///
    /// # It writes, and it runs only if the operator said so
    ///
    /// `docs/COMMAND_LINE.md` §1 is the design this starts from: *a key writes a
    /// word into the line, it does not act*. Every key on the screen behaves
    /// this way and so does every one on the desk that names a word. A bound
    /// line is the same gesture with more than one word in it, so an operator
    /// sees what is about to happen and presses Enter — or corrects it, which a
    /// key that fired straight away would not allow.
    ///
    /// **S43's rebuild adds the other half**, because the owner asked for it and
    /// the reason is good: *bei Send Command soll es eine Option geben, den Text
    /// nur in die Konsole zu schreiben, oder zu schreiben und direkt
    /// abzusenden*. A key bound to `Go Executor 1` that needs Enter afterwards
    /// is not a Go key. So [`Self::submit`] is the operator's answer, per
    /// binding, and the default is the old behaviour.
    ///
    /// # Who runs it, since the daemon cannot
    ///
    /// **The parser lives in the interface** (`ui/src/desk/console.ts`), not in
    /// `prismd`, so a daemon handed a line has nothing to turn it into a command
    /// with. Running one is therefore not something this action can do by
    /// itself: it sets `Session::command_line` and bumps
    /// `Session::command_line_run`, and the client that has the **keyboard
    /// focus** parses the line and sends what it means. That is the same client
    /// that would have run it if the operator had pressed Enter, and picking it
    /// by focus is what stops two screens each sending the commands once — a
    /// doubled Go being the failure worth designing against.
    ///
    /// It is a stop-gap and it is written down as one: moving the parser into
    /// the daemon is `IMPLEMENTATION_PLAN.md` S49, after which the daemon runs
    /// the line itself and no client is involved. Two focused screens on two
    /// machines is the case this does not cover.
    ///
    /// Resolves to `Command::CommandLineInput`.
    WriteCommandLine {
        /// The line to write, exactly as it would be typed.
        line: String,
        /// Run it as well, rather than leaving it for Enter.
        ///
        /// `#[serde(default)]` so a profile written before S43 reads as *write
        /// only*, which is what those bindings did.
        #[serde(default)]
        submit: bool,
    },
    /// Write the show to disk.
    SaveShow,
    /// Undo.
    Oops,
    /// Redo.
    Redo,
}

/// A control a binding can name.
///
/// The `Strip[*]` of §4.2 is one entry rather than eight: a strip's binding is
/// the same on every strip and the strip index is what fills in the executor.
/// A profile that wanted strip 3 to differ would be describing a desk whose
/// faders are not a bank, which **D7** says they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum BoundControl {
    /// `Strip[*].Fader`.
    StripFader,
    /// `Strip[*].Encoder`.
    StripEncoder,
    /// `Strip[*].Button.<name>`.
    StripButton {
        /// Which of the strip's five buttons.
        button: StripButton,
    },
    /// `Main.Fader`.
    MainFader,
    /// `Global.<name>`.
    Global {
        /// Which of the panel's sixty-four buttons.
        button: GlobalButton,
    },
    /// `Global.Jog`.
    Jog,
}

impl BoundControl {
    /// The control a profile's `control` string names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Strip[*].Fader" => Some(Self::StripFader),
            "Strip[*].Encoder" => Some(Self::StripEncoder),
            "Main.Fader" => Some(Self::MainFader),
            "Global.Jog" => Some(Self::Jog),
            _ => {
                if let Some(button) = name.strip_prefix("Strip[*].Button.") {
                    StripButton::from_name(button).map(|button| Self::StripButton { button })
                } else {
                    GlobalButton::from_name(name.strip_prefix("Global.")?)
                        .map(|button| Self::Global { button })
                }
            }
        }
    }
}

impl fmt::Display for BoundControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StripFader => f.write_str("Strip[*].Fader"),
            Self::StripEncoder => f.write_str("Strip[*].Encoder"),
            Self::StripButton { button } => write!(f, "Strip[*].Button.{button}"),
            Self::MainFader => f.write_str("Main.Fader"),
            Self::Global { button } => write!(f, "Global.{button}"),
            Self::Jog => f.write_str("Global.Jog"),
        }
    }
}

impl SurfaceAction {
    /// Whether this action wants the release of a button as well as the press.
    ///
    /// Only [`Self::ExecutorButton`], and it always does: layer 3 cannot know
    /// whether the executor has a `Flash` on that key, so it forwards both
    /// edges and lets the executor decide. Everything else is an instruction
    /// rather than a gesture, and an instruction has one edge.
    ///
    /// Takes `&self` since S43: [`Self::WriteCommandLine`] made this enum
    /// non-`Copy`, and a `const fn` may not drop one.
    #[must_use]
    pub const fn is_momentary(&self) -> bool {
        matches!(self, Self::ExecutorButton { .. })
    }
}

/// Buttons PrismDMX must never bind, drive or claim — `docs/MCU_MAPPING.md` §4.3.
///
/// **Stated once, and this is the once.** `prism_surface::X_TOUCH.reserved_buttons`
/// points at this array rather than carrying a copy, so there is no pair to keep
/// in step. It is here rather than there because the refusal has to happen where
/// a *command* is validated — `prism_core::MachineConfig::configure` — and that
/// is a crate which must never depend on a MIDI codec.
///
/// One of them, and §4.3 is emphatic about why: **SMPTE/Beats is the button that
/// switches the surface between the two hosts** in the combined Xctl+MC mode. It
/// is the operator's way back to the sound desk, and a console that steals it is
/// a console somebody has to power-cycle to get out of. Reserved in **every**
/// mode rather than only in the shared one, deliberately, so that one table is
/// safe on a desk whose mode nobody has checked.
pub const RESERVED_BUTTONS: [GlobalButton; 1] = [GlobalButton::SmpteBeats];

/// Why a reserved control may not be bound, in the words an operator reads.
///
/// A `const` rather than a sentence written twice: `prism_surface::ProfileError`
/// refuses a *file* that names one and `prism_core::MachineError` refuses a
/// *command* that does, and the two must say the same thing — an operator who
/// met one wording in a log and another in a window would reasonably conclude
/// they were two different rules.
pub const RESERVED_REASON: &str = "on a surface shared with a sound console (Xctl+MC) it is the \
     button that switches the desk between the two hosts, so a binding on it strands the operator \
     away from their sound desk. Leave it unbound";

impl BoundControl {
    /// The reserved button this control is, if it is one.
    ///
    /// [`RESERVED_BUTTONS`] as a question, so that the two places which have to
    /// refuse a binding — a profile file and a `MachineChange::SurfaceBinding` —
    /// ask rather than each carrying a list.
    #[must_use]
    pub fn reserved(self) -> Option<GlobalButton> {
        match self {
            Self::Global { button } if RESERVED_BUTTONS.contains(&button) => Some(button),
            _ => None,
        }
    }

    /// Every control a binding table has a row for, in the order an editor
    /// draws them: the strip's own controls, the main fader, the jog wheel and
    /// then the panel.
    ///
    /// Here rather than in `prism-surface` because it is the *table's* shape
    /// rather than a device's wiring — a row exists for a control whether or
    /// not this surface has ever sent a byte for it, which is what lets an
    /// editor draw the whole desk and an operator find the key they mean.
    #[must_use]
    pub fn all() -> Vec<Self> {
        let mut controls = vec![Self::StripFader, Self::StripEncoder];
        controls.extend(StripButton::ALL.map(|button| Self::StripButton { button }));
        controls.push(Self::MainFader);
        controls.push(Self::Jog);
        controls.extend(GlobalButton::ALL.map(|button| Self::Global { button }));
        controls
    }
}

/// One row of the binding table, as it is stored and as it travels — S38.
///
/// The same pair `docs/MCU_MAPPING.md` §4.2's file format writes as
/// `{ "control": …, "action": … }`, in the shape a command and a machine
/// configuration carry it. `None` is a control deliberately left alone, which
/// the file format has always distinguished from one nobody wrote down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct SurfaceBinding {
    /// Which control.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::boxed()")
    )]
    pub control: BoundControl,
    /// What it does, or nothing.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::boxed()")
    )]
    pub action: Option<SurfaceAction>,
}

/// One control of the surface, as the control editor draws it — S38.
///
/// [`SurfaceBinding`] plus the two facts a client may not work out for itself,
/// because both are properties of the **device profile** and a client holds
/// none:
///
/// - [`permanent`](Self::permanent) is `docs/MCU_MAPPING.md` §4.3's ownership —
///   whether this control keeps reaching PrismDMX while the surface is also
///   driving a sound console, or whether it follows the operator's switch. An
///   interface that guessed would tell an operator that a key is always in
///   reach when it is not, which is precisely the mistake §4.3 exists to
///   prevent.
/// - [`reserved`](Self::reserved) is the one control that may never be bound.
///   The daemon refuses it either way ([`RESERVED_BUTTONS`]); saying so here is
///   what lets the editor grey the row rather than let an operator find out by
///   being told no.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct SurfaceControl {
    /// Which control.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::boxed()")
    )]
    pub control: BoundControl,
    /// What a binding profile calls it — `docs/MCU_MAPPING.md` §4.2's `control`
    /// string, so what an operator reads in the window is what they would write
    /// in the file.
    ///
    /// Carried rather than rendered by the client for one reason: the
    /// alternative is a sixty-four entry table in TypeScript that says what
    /// `GlobalButton::name` already says.
    pub name: String,
    /// What it does now, or nothing.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::boxed()")
    )]
    pub action: Option<SurfaceAction>,
    /// Whether it reaches PrismDMX **permanently** in the combined Xctl+MC mode
    /// (§4.3), rather than following the operator's switch between the hosts.
    pub permanent: bool,
    /// Whether it may never be bound (§4.3's SMPTE/Beats).
    pub reserved: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        BoundControl, ExecutorTarget, GlobalButton, RESERVED_BUTTONS, RESERVED_REASON, Step,
        StripButton, SurfaceAction, SurfaceBinding, SurfaceControl,
    };
    use crate::{ExecutorButtonRef, FeatureGroup, GoDirection, ParamDirection, ViewId, WindowType};

    /// The names are what a person types into a profile file, so the round trip
    /// is the test: every control this crate can name reads back as itself.
    #[test]
    fn every_control_round_trips_through_the_name_a_profile_spells_it_with() {
        for control in BoundControl::all() {
            let name = control.to_string();
            assert_eq!(BoundControl::from_name(&name), Some(control), "{name}");
        }
    }

    /// And the shape of those names is `docs/MCU_MAPPING.md` §4.2's, written out
    /// here rather than derived — a test that built its expectations out of
    /// `Display` would pass for any spelling at all.
    #[test]
    fn the_names_are_the_ones_section_4_2_gives() {
        assert_eq!(BoundControl::StripFader.to_string(), "Strip[*].Fader");
        assert_eq!(BoundControl::StripEncoder.to_string(), "Strip[*].Encoder");
        assert_eq!(BoundControl::MainFader.to_string(), "Main.Fader");
        assert_eq!(BoundControl::Jog.to_string(), "Global.Jog");
        assert_eq!(
            BoundControl::StripButton {
                button: StripButton::VPotPush
            }
            .to_string(),
            "Strip[*].Button.VPotPush"
        );
        assert_eq!(
            BoundControl::Global {
                button: GlobalButton::AssignPlugin
            }
            .to_string(),
            "Global.AssignPlugin"
        );
    }

    /// A name this crate does not know is **nothing**, rather than a control it
    /// guessed at: a profile that misspelled one has to be told.
    #[test]
    fn a_name_that_is_not_a_control_is_refused() {
        for name in [
            "",
            "Strip[*].Touch",
            "Strip[*].Button.Wibble",
            "Global.Nonesuch",
            "Global.",
            "Main.Encoder",
            "Strip[0].Fader",
        ] {
            assert_eq!(BoundControl::from_name(name), None, "{name:?}");
        }
    }

    /// The whole desk, counted by hand out of §2.1 rather than out of the
    /// constants the code uses.
    #[test]
    fn the_list_is_the_whole_desk_and_has_no_repeats() {
        let controls = BoundControl::all();
        assert_eq!(controls.len(), 1 + 1 + 5 + 1 + 1 + 64);
        let mut sorted = controls.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), controls.len());
        // The order is the one an editor draws: the strip first, then the main
        // fader and the wheel, then the panel.
        assert_eq!(controls.first(), Some(&BoundControl::StripFader));
        assert_eq!(
            controls.last(),
            Some(&BoundControl::Global {
                button: GlobalButton::FootSwitch2
            })
        );
    }

    /// §4.3's one reserved button, and nothing else is.
    #[test]
    fn only_smpte_beats_is_reserved() {
        assert_eq!(RESERVED_BUTTONS, [GlobalButton::SmpteBeats]);
        for control in BoundControl::all() {
            let expected = control
                == BoundControl::Global {
                    button: GlobalButton::SmpteBeats,
                };
            assert_eq!(control.reserved().is_some(), expected, "{control}");
        }
        // The sentence names the button's job rather than the rule's number,
        // because it is read by whoever tried to bind it.
        assert!(RESERVED_REASON.contains("Xctl+MC"));
        assert!(RESERVED_REASON.contains("switches the desk between the two hosts"));
        assert!(RESERVED_REASON.contains("Leave it unbound"));
    }

    /// Only an executor-button action wants both edges — everything else is an
    /// instruction, and an instruction has one.
    #[test]
    fn an_executor_button_is_the_only_momentary_action() {
        assert!(
            SurfaceAction::ExecutorButton {
                target: ExecutorTarget::Strip,
                button: ExecutorButtonRef::Slot { index: 0 },
            }
            .is_momentary()
        );
        for action in [
            SurfaceAction::ExecutorMaster {
                target: ExecutorTarget::Strip,
            },
            SurfaceAction::ExecutorGo {
                target: ExecutorTarget::Selected,
                direction: GoDirection::Next,
            },
            SurfaceAction::ExecutorOff {
                target: ExecutorTarget::Strip,
            },
            SurfaceAction::SelectExecutor {
                target: ExecutorTarget::Strip,
            },
            SurfaceAction::ClearProgrammer,
            SurfaceAction::ExecutorPage { delta: -1 },
            SurfaceAction::SelectView {
                view: ViewId::new(1),
            },
            SurfaceAction::StepView {
                direction: Step::Next,
            },
            SurfaceAction::ProgrammerPage { delta: 1 },
            SurfaceAction::SelectProgrammerParam {
                direction: ParamDirection::Prev,
            },
            SurfaceAction::AdjustParameter,
            SurfaceAction::SetEncoderBank {
                group: FeatureGroup::Color,
            },
            SurfaceAction::OpenWindow {
                window: WindowType::Patch,
            },
            SurfaceAction::SaveShow,
            SurfaceAction::Oops,
            SurfaceAction::Redo,
        ] {
            assert!(!action.is_momentary(), "{action:?}");
        }
    }

    /// The panel's sixty-four names are distinct and reverse, and so are the
    /// strip's five — the property a table lookup by name depends on.
    #[test]
    fn every_button_name_is_distinct_and_reverses() {
        let mut names: Vec<&str> = GlobalButton::ALL
            .iter()
            .map(|button| button.name())
            .collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two panel buttons share a name");
        for (index, button) in GlobalButton::ALL.into_iter().enumerate() {
            assert_eq!(GlobalButton::from_name(button.name()), Some(button));
            assert_eq!(button.to_string(), button.name());
            // The index is the position in `ALL`, which is how layer 2 addresses
            // a shadow model's LEDs.
            assert_eq!(button.index(), index);
        }
        for button in StripButton::ALL {
            assert_eq!(StripButton::from_name(button.name()), Some(button));
            assert_eq!(button.to_string(), button.name());
            assert_eq!(StripButton::ALL[button.index()], button);
        }
        assert_eq!(GlobalButton::from_name("Nonesuch"), None);
        assert_eq!(StripButton::from_name("Nonesuch"), None);
    }

    /// The two wire rows carry what they say they carry, in `camelCase`.
    #[test]
    fn the_wire_rows_are_camel_case_and_carry_the_tagged_union() {
        let row = SurfaceBinding {
            control: BoundControl::Global {
                button: GlobalButton::F1,
            },
            action: Some(SurfaceAction::OpenWindow {
                window: WindowType::Patch,
            }),
        };
        assert_eq!(
            serde_json::to_string(&row).unwrap(),
            r#"{"control":{"t":"Global","button":"F1"},"action":{"t":"OpenWindow","window":"Patch"}}"#
        );
        let control = SurfaceControl {
            control: BoundControl::Jog,
            name: "Global.Jog".to_owned(),
            action: None,
            permanent: true,
            reserved: false,
        };
        assert_eq!(
            serde_json::to_string(&control).unwrap(),
            r#"{"control":{"t":"Jog"},"name":"Global.Jog","action":null,"permanent":true,"reserved":false}"#
        );
    }
}
