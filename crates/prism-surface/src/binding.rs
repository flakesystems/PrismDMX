//! Layer 3: what a control *does*.
//!
//! `docs/MCU_MAPPING.md` §4. Layers 1 and 2 turn bytes into
//! [`SurfaceEvent`]s and back; this layer is the one that knows PrismDMX, and
//! its whole job is a table lookup followed by filling in the arguments a
//! binding cannot know:
//!
//! ```text
//!   SurfaceEvent ──▶ [`Bindings`] ──▶ [`SurfaceAction`] ──▶ `Command`
//!    what the         the table,      what it means,       with the executor,
//!    operator did     from JSON       device-free          the level, the view
//!                                                          filled in from
//!                                                          [`SurfaceContext`]
//! ```
//!
//! # There is no arithmetic here, and that is the point
//!
//! A fader arrives as a level of 0…65535, already scaled against what this
//! surface reports at the top of travel; an encoder and the jog wheel arrive as
//! parameter steps, already through their two different acceleration curves
//! (§2.7). Everything that needed a measurement to get right was done one layer
//! down, so a binding is a `match` and nothing else — which is what makes this
//! layer safe to let a user edit.
//!
//! # The context is what the session knows and this crate must not
//!
//! Half of `docs/IPC_PROTOCOL.md` §5's commands carry an argument the surface
//! has not got: `SetExecutorMaster` needs an executor number, and the strip only
//! knows it is the fourth strip; `SelectView` carries a view number, and
//! `Channel ▶` only means *the next one*. [`SurfaceContext`] is those answers,
//! as plain `Copy` data, resolved by whoever holds the session — `prismd`. This
//! crate holds no session, no view library and no show, and asking for the
//! answers rather than the models is what keeps it that way.
//!
//! # Loading never fails
//!
//! [`Bindings::load`] answers with a table whatever it is given: the profile
//! when it parses, the built-in defaults and a [`ProfileError`] when it does
//! not. `IMPLEMENTATION_PLAN.md` S22 asks that a malformed profile *never*
//! blocks startup, and a function that cannot fail is a stronger way to say so
//! than a caller that remembers to handle the error. **No file is read here**:
//! the text arrives as a `&str` and which file it came from is `prismd`'s
//! business, which is also what lets the fallback be tested without a
//! filesystem.
//!
//! # One button is refused rather than defaulted away from
//!
//! SMPTE/Beats (note 53) switches the surface between the sound console and
//! PrismDMX in the combined Xctl+MC mode (§4.3). Layer 2 already drops its
//! presses and counts them, so a binding on it could never fire — but a profile
//! that names it is a person who believes they have bound it, so the loader
//! refuses the profile and says why. Belt and braces, and the braces are the
//! sentence the person reads.

use core::fmt;

use prism_domain::{
    AttributeType, Command, ExecutorButtonFunction, ExecutorButtonRef, ExecutorId, FeatureGroup,
    GoDirection, ParamDirection, PlaybackTarget, ViewId, WindowType,
};
use serde::{Deserialize, Serialize};

use crate::control::ButtonId;
use crate::model::{STRIP_BUTTONS, SurfaceEvent};
use crate::profile::{Fader, GlobalButton, McuProfile, StripButton};

/// The profile format this crate reads — `profileVersion` in the JSON.
pub const PROFILE_VERSION: u32 = 1;

/// Panel buttons a table has one slot for each of.
const GLOBAL_SLOTS: usize = GlobalButton::ALL.len();

/// Which executor an action acts on.
///
/// Two answers, because `docs/MCU_MAPPING.md` §4.1 has two kinds of row: a
/// strip's own controls act on the executor under that strip, and the transport
/// section, the Flip button and the main fader act on the one that is
/// *selected*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExecutorTarget {
    /// The executor on the strip the event came from: `executorPage * 8 + index`
    /// (**D7**). Nothing at all when the control is not a strip's.
    Strip,
    /// The executor the session has selected. Nothing when none is.
    Selected,
}

/// Which way a relative move goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t")]
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
    /// Which view that *is* comes from [`SurfaceContext`]: the view library
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
    /// Write the show to disk.
    SaveShow,
    /// Undo.
    Oops,
    /// Redo.
    Redo,
}

/// What the session knows and this crate does not.
///
/// Every field is an answer rather than a model: the caller resolves them once
/// per batch of events from the session it holds, and this crate never sees a
/// view library, a show or an executor. `Copy`, and small enough to rebuild per
/// event without anybody caring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SurfaceContext {
    /// The executor page the fader bank is showing (**D7**).
    pub executor_page: u32,
    /// The executor the transport section and the main fader act on.
    pub selected_executor: Option<ExecutorId>,
    /// The stored view before the active one, if there is one.
    pub previous_view: Option<ViewId>,
    /// The stored view after the active one, if there is one.
    pub next_view: Option<ViewId>,
    /// The programmer page.
    pub programmer_page: u32,
    /// The attribute the jog wheel turns — the programmer parameter the session
    /// has selected. Nothing when nothing is selected, and then a turn of the
    /// wheel produces no command at all rather than a guess.
    pub parameter: Option<AttributeType>,
}

/// What a control carried with it, for the actions that need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
    /// A button went down (`true`) or came up (`false`).
    Button(bool),
    /// A fader was moved, as a level.
    Level(u16),
    /// A relative control was turned, in parameter steps.
    Steps(i32),
}

impl Input {
    /// The level, for an action that sets one.
    const fn level(self) -> Option<u16> {
        match self {
            Self::Level(level) => Some(level),
            Self::Button(_) | Self::Steps(_) => None,
        }
    }

    /// The steps, for an action that moves by them.
    const fn steps(self) -> Option<i32> {
        match self {
            Self::Steps(steps) => Some(steps),
            Self::Button(_) | Self::Level(_) => None,
        }
    }

    /// Which edge of a button this was, for the one action that cares.
    const fn pressed(self) -> Option<bool> {
        match self {
            Self::Button(pressed) => Some(pressed),
            Self::Level(_) | Self::Steps(_) => None,
        }
    }
}

/// Which strip the event came from, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// A channel strip's control.
    Strip(u8),
    /// Something on the panel, which belongs to no strip.
    Panel,
}

impl ExecutorTarget {
    /// The executor this target names, given where the event came from.
    fn resolve(self, origin: Origin, context: &SurfaceContext) -> Option<ExecutorId> {
        match (self, origin) {
            (Self::Strip, Origin::Strip(strip)) => Some(ExecutorId::from_page_and_slot(
                context.executor_page,
                u32::from(strip),
            )),
            (Self::Strip, Origin::Panel) => None,
            (Self::Selected, _) => context.selected_executor,
        }
    }
}

/// A page number moved by a signed step, saturating at zero.
///
/// Saturating rather than wrapping for the reason `ExecutorId::from_page_and_slot`
/// saturates: paging left at page 0 must stay at page 0, where wrapping would
/// address the last page of a `u32` and put eight executors nobody has ever
/// heard of onto the faders.
const fn step_page(page: u32, delta: i32) -> u32 {
    if delta < 0 {
        page.saturating_sub(delta.unsigned_abs())
    } else {
        page.saturating_add(delta.unsigned_abs())
    }
}

impl SurfaceAction {
    /// Whether this action wants the release of a button as well as the press.
    ///
    /// Only [`Self::ExecutorButton`], and it always does: layer 3 cannot know
    /// whether the executor has a `Flash` on that key, so it forwards both
    /// edges and lets the executor decide. Everything else is an instruction
    /// rather than a gesture, and an instruction has one edge.
    #[must_use]
    pub const fn is_momentary(self) -> bool {
        matches!(self, Self::ExecutorButton { .. })
    }

    /// The command this action means, or nothing.
    ///
    /// Nothing when the action needs an answer the context has not got — no
    /// executor is selected, no parameter is on the jog wheel, no view lies
    /// beyond the active one — and nothing when it is bound to a control that
    /// cannot drive it, such as a master on a button. Both are ordinary: an
    /// operator turning the jog wheel with nothing selected has done nothing,
    /// and a profile that binds a level to a button is a mistake that should
    /// cost nothing at run time.
    fn resolve(self, origin: Origin, input: Input, context: &SurfaceContext) -> Option<Command> {
        Some(match self {
            Self::ExecutorMaster { target } => Command::SetExecutorMaster {
                executor_id: target.resolve(origin, context)?,
                level: input.level()?,
            },
            Self::ExecutorGo { target, direction } => Command::ExecutorGo {
                target: PlaybackTarget::of_executor(target.resolve(origin, context)?),
                direction,
            },
            Self::ExecutorOff { target } => Command::ExecutorOff {
                target: PlaybackTarget::of_executor(target.resolve(origin, context)?),
            },
            Self::ExecutorButton { target, button } => Command::ExecutorButton {
                executor_id: target.resolve(origin, context)?,
                button,
                pressed: input.pressed()?,
            },
            Self::SelectExecutor { target } => Command::SelectExecutor {
                executor_id: target.resolve(origin, context)?,
            },
            Self::ClearProgrammer => Command::ClearProgrammer,
            Self::ExecutorPage { delta } => Command::SetExecutorPage {
                page: step_page(context.executor_page, delta),
            },
            Self::SelectView { view } => Command::SelectView { view_id: view },
            Self::StepView { direction } => Command::SelectView {
                view_id: match direction {
                    Step::Prev => context.previous_view?,
                    Step::Next => context.next_view?,
                },
            },
            Self::ProgrammerPage { delta } => Command::SetProgrammerPage {
                page: step_page(context.programmer_page, delta),
            },
            Self::SelectProgrammerParam { direction } => {
                Command::SelectProgrammerParam { direction }
            }
            Self::AdjustParameter => Command::SetAttribute {
                attribute: context.parameter?,
                value: input.steps()?,
                relative: true,
            },
            Self::SetEncoderBank { group } => Command::SetEncoderBank { group },
            Self::OpenWindow { window } => Command::OpenWindow {
                window,
                params: None,
            },
            Self::SaveShow => Command::SaveShow,
            Self::Oops => Command::Oops,
            Self::Redo => Command::Redo,
        })
    }
}

/// A control a binding can name.
///
/// The `Strip[*]` of §4.2 is one entry rather than eight: a strip's binding is
/// the same on every strip and the strip index is what fills in the executor.
/// A profile that wanted strip 3 to differ would be describing a desk whose
/// faders are not a bank, which **D7** says they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundControl {
    /// `Strip[*].Fader`.
    StripFader,
    /// `Strip[*].Encoder`.
    StripEncoder,
    /// `Strip[*].Button.<name>`.
    StripButton(StripButton),
    /// `Main.Fader`.
    MainFader,
    /// `Global.<name>`.
    Global(GlobalButton),
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
                    StripButton::from_name(button).map(Self::StripButton)
                } else {
                    GlobalButton::from_name(name.strip_prefix("Global.")?).map(Self::Global)
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
            Self::StripButton(button) => write!(f, "Strip[*].Button.{button}"),
            Self::MainFader => f.write_str("Main.Fader"),
            Self::Global(button) => write!(f, "Global.{button}"),
            Self::Jog => f.write_str("Global.Jog"),
        }
    }
}

/// Why a binding profile was refused.
///
/// Every variant is something a person wrote, so every message names what they
/// wrote and what was expected of it. The profile is refused whole rather than
/// row by row: a table half of which was understood is a desk whose buttons do
/// some of what its author intended, which is worse to operate than one that
/// does what the defaults say and complains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// It is not JSON, or not JSON of this shape.
    Malformed(String),
    /// A `profileVersion` this build does not read.
    UnsupportedVersion {
        /// What the file said.
        found: u32,
        /// What this build reads.
        supported: u32,
    },
    /// A profile written for another surface.
    WrongDevice {
        /// The `device` the file names.
        found: String,
        /// The surface the profile was loaded for.
        expected: &'static str,
    },
    /// A `control` this crate does not know.
    UnknownControl(String),
    /// The same control bound twice.
    DuplicateControl(BoundControl),
    /// A binding on a button PrismDMX must never take — SMPTE/Beats.
    ReservedControl(GlobalButton),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(reason) => write!(f, "the surface profile could not be read: {reason}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "the surface profile is version {found} and this build reads version {supported}"
            ),
            Self::WrongDevice { found, expected } => write!(
                f,
                "the surface profile is for {found:?} and this surface is {expected:?}"
            ),
            Self::UnknownControl(name) => {
                write!(f, "{name:?} is not a control a binding can name")
            }
            Self::DuplicateControl(control) => {
                write!(f, "{control} is bound twice; a control has one binding")
            }
            Self::ReservedControl(button) => write!(
                f,
                "Global.{button} must not be bound: on a surface shared with a sound console \
                 (Xctl+MC) it is the button that switches the desk between the two hosts, so a \
                 binding on it strands the operator away from their sound desk. Leave it unbound",
            ),
        }
    }
}

impl core::error::Error for ProfileError {}

/// The binding table: one action per control, or none.
///
/// Fixed-size and [`Copy`], like everything else this crate holds, so a
/// controller that reloads a profile mid-show swaps one value for another and
/// allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bindings {
    strip_fader: Option<SurfaceAction>,
    strip_encoder: Option<SurfaceAction>,
    strip_buttons: [Option<SurfaceAction>; STRIP_BUTTONS],
    main_fader: Option<SurfaceAction>,
    global: [Option<SurfaceAction>; GLOBAL_SLOTS],
    jog: Option<SurfaceAction>,
}

/// The panel's default bindings, as `docs/MCU_MAPPING.md` §4.1 gives them.
///
/// A list of pairs rather than sixty-four array slots in declaration order: the
/// array is indexed by [`GlobalButton::index`], and a table written out by
/// position is one where inserting a button silently moves everything after it.
const DEFAULT_GLOBAL: [(GlobalButton, SurfaceAction); 25] = [
    // Encoder Assign — five feature groups on the first five buttons. The sixth
    // (Instrument) is left alone rather than doubled up, because a button that
    // repeats its neighbour teaches an operator that it is broken.
    (
        GlobalButton::AssignTrack,
        SurfaceAction::SetEncoderBank {
            group: FeatureGroup::Dimmer,
        },
    ),
    (
        GlobalButton::AssignSend,
        SurfaceAction::SetEncoderBank {
            group: FeatureGroup::Position,
        },
    ),
    (
        GlobalButton::AssignPan,
        SurfaceAction::SetEncoderBank {
            group: FeatureGroup::Color,
        },
    ),
    (
        GlobalButton::AssignPlugin,
        SurfaceAction::SetEncoderBank {
            group: FeatureGroup::Beam,
        },
    ),
    (
        GlobalButton::AssignEq,
        SurfaceAction::SetEncoderBank {
            group: FeatureGroup::Focus,
        },
    ),
    // Faderbank ◀▶ — executor page down and up (D7).
    (
        GlobalButton::BankLeft,
        SurfaceAction::ExecutorPage { delta: -1 },
    ),
    (
        GlobalButton::BankRight,
        SurfaceAction::ExecutorPage { delta: 1 },
    ),
    // Channel ◀▶ — the UI view (D8).
    (
        GlobalButton::ChannelLeft,
        SurfaceAction::StepView {
            direction: Step::Prev,
        },
    ),
    (
        GlobalButton::ChannelRight,
        SurfaceAction::StepView {
            direction: Step::Next,
        },
    ),
    // Flip — Go+ on the selected executor.
    (
        GlobalButton::Flip,
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Selected,
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::GoForward,
            },
        },
    ),
    // F1–F8: free. Four windows and four spare, which is what "free" means in a
    // file somebody is expected to edit.
    (
        GlobalButton::F1,
        SurfaceAction::OpenWindow {
            window: WindowType::FixtureSheet,
        },
    ),
    (
        GlobalButton::F2,
        SurfaceAction::OpenWindow {
            window: WindowType::SequenceSheet,
        },
    ),
    (
        GlobalButton::F3,
        SurfaceAction::OpenWindow {
            window: WindowType::Patch,
        },
    ),
    (
        GlobalButton::F4,
        SurfaceAction::OpenWindow {
            window: WindowType::Settings,
        },
    ),
    // Utility.
    (GlobalButton::Save, SurfaceAction::SaveShow),
    (GlobalButton::Undo, SurfaceAction::Oops),
    (GlobalButton::Enter, SurfaceAction::Redo),
    // Transport — the part of the panel that stays PrismDMX's in shared
    // operation (§4.3), and therefore the part to spend on live-show work.
    // §4.1 gives these four `Go-`, `Go+`, `Off` and `On` on the selected
    // executor. Until S34 the protocol had no `On` and Play resolved to a Go,
    // which is the second row of the deviation table §4.2.1 carried; now the
    // row is bound as it is written.
    (
        GlobalButton::Rewind,
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Selected,
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::GoBack,
            },
        },
    ),
    (
        GlobalButton::FastForward,
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Selected,
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::GoForward,
            },
        },
    ),
    (
        GlobalButton::Stop,
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Selected,
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::Off,
            },
        },
    ),
    (
        GlobalButton::Play,
        SurfaceAction::ExecutorButton {
            target: ExecutorTarget::Selected,
            button: ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::On,
            },
        },
    ),
    (GlobalButton::Record, SurfaceAction::ClearProgrammer),
    // The cursor cluster is XTouch.txt's "Zoom ▲▼ / ◀▶".
    (
        GlobalButton::CursorUp,
        SurfaceAction::ProgrammerPage { delta: -1 },
    ),
    (
        GlobalButton::CursorDown,
        SurfaceAction::ProgrammerPage { delta: 1 },
    ),
    (
        GlobalButton::CursorLeft,
        SurfaceAction::SelectProgrammerParam {
            direction: ParamDirection::Prev,
        },
    ),
];

/// The last of the panel's defaults, which the array above has no room for
/// without becoming a table nobody can read the end of.
const DEFAULT_CURSOR_RIGHT: SurfaceAction = SurfaceAction::SelectProgrammerParam {
    direction: ParamDirection::Next,
};

impl Bindings {
    /// A table with nothing bound.
    ///
    /// The starting point [`parse`](Self::parse) fills in, and what a profile
    /// with an empty `bindings` array means: a surface that does nothing is a
    /// legitimate thing to ask for and a silent one is not the same as a
    /// defaulted one.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            strip_fader: None,
            strip_encoder: None,
            strip_buttons: [None; STRIP_BUTTONS],
            main_fader: None,
            global: [None; GLOBAL_SLOTS],
            jog: None,
        }
    }

    /// The built-in defaults — every row of `docs/MCU_MAPPING.md` §4.1.
    ///
    /// What a desk does with no profile file, with a profile that will not
    /// parse, and what the shipped `profiles/surface/xtouch.json` reproduces. A
    /// test asserts the third of those, and a *different* test asserts the rows
    /// against expectations transcribed from the document by hand — because a
    /// test that read the table it is checking would pass for any table at all.
    #[must_use]
    pub fn defaults() -> Self {
        let mut table = Self::empty();
        // Strip fader: the master of the executor on that strip.
        table.strip_fader = Some(SurfaceAction::ExecutorMaster {
            target: ExecutorTarget::Strip,
        });
        // Strip encoder: Empty, per §4.1. Left as `None`.
        // Rec / Solo / Mute / Select: **that strip executor's own** first,
        // second, third and fourth button. §4.1 calls this row configurable and
        // lists the eight `ExecutorButtonFunction`s; that list is the executor's
        // and not this table's, so what the table binds is the *position* and
        // the show says what it does (S34). `prism_core::default_executor` is
        // where the desk's own answer to that lives — Go+, Go-, Off, Empty.
        for button in [
            StripButton::Rec,
            StripButton::Solo,
            StripButton::Mute,
            StripButton::Select,
        ] {
            table.set_strip_button(
                button,
                Some(SurfaceAction::ExecutorButton {
                    target: ExecutorTarget::Strip,
                    button: ExecutorButtonRef::Slot {
                        index: u8::try_from(button.index()).unwrap_or(u8::MAX),
                    },
                }),
            );
        }
        // Pushing the V-Pot selects that strip's executor, which is what the
        // transport section and the main fader then act on. Not a §4.1 row —
        // that table has no line for the push — but the selection has to be
        // reachable from the surface or half of §4.1 has no subject.
        table.set_strip_button(
            StripButton::VPotPush,
            Some(SurfaceAction::SelectExecutor {
                target: ExecutorTarget::Strip,
            }),
        );
        // Main fader: the selected executor's fader. §4.1 calls it `XFade`,
        // which is an `ExecutorFaderFunction` — what the executor does with its
        // fader is show data (`ARCHITECTURE_SPEC.md` §6) and there is one fader
        // command for all four functions. Since S34 the daemon routes that one
        // command through the executor's own `fader_function`, so a fader set to
        // `XFade` crossfades and this row means what §4.1 says it means.
        table.main_fader = Some(SurfaceAction::ExecutorMaster {
            target: ExecutorTarget::Selected,
        });
        // The jog wheel turns the selected programmer parameter.
        table.jog = Some(SurfaceAction::AdjustParameter);
        for (button, action) in DEFAULT_GLOBAL {
            table.set_global(button, Some(action));
        }
        table.set_global(GlobalButton::CursorRight, Some(DEFAULT_CURSOR_RIGHT));
        table
    }

    /// Reads a profile, refusing anything it does not understand.
    ///
    /// `profile` is the surface the table is for: it decides which `device` the
    /// file must name and which buttons are reserved.
    ///
    /// # Errors
    ///
    /// [`ProfileError`], naming what was wrong in words the profile's author can
    /// act on. Callers that must start anyway want [`load`](Self::load).
    pub fn parse(text: &str, profile: &McuProfile) -> Result<Self, ProfileError> {
        let document: ProfileDocument = serde_json::from_str(text)
            .map_err(|error| ProfileError::Malformed(error.to_string()))?;
        if document.profile_version != PROFILE_VERSION {
            return Err(ProfileError::UnsupportedVersion {
                found: document.profile_version,
                supported: PROFILE_VERSION,
            });
        }
        if document.device != profile.key {
            return Err(ProfileError::WrongDevice {
                found: document.device,
                expected: profile.key,
            });
        }
        let mut table = Self::empty();
        let mut seen: Vec<BoundControl> = Vec::new();
        for entry in document.bindings {
            let control = BoundControl::from_name(&entry.control)
                .ok_or(ProfileError::UnknownControl(entry.control))?;
            if seen.contains(&control) {
                return Err(ProfileError::DuplicateControl(control));
            }
            seen.push(control);
            if let (BoundControl::Global(button), Some(_)) = (control, entry.action)
                && profile.is_reserved(button)
            {
                return Err(ProfileError::ReservedControl(button));
            }
            table.set(control, entry.action);
        }
        Ok(table)
    }

    /// Reads a profile, falling back to the defaults, and **cannot fail**.
    ///
    /// The exit criterion of `IMPLEMENTATION_PLAN.md` S22 as a type: a malformed
    /// profile answers with the built-in table and the reason, so a caller has
    /// nothing to decide and no way to turn a bad file into a desk that will not
    /// start. The reason is a warning to log, not an error to handle.
    #[must_use]
    pub fn load(text: &str, profile: &McuProfile) -> (Self, Option<ProfileError>) {
        match Self::parse(text, profile) {
            Ok(table) => (table, None),
            Err(error) => (Self::defaults(), Some(error)),
        }
    }

    /// Puts an action on a control.
    pub fn set(&mut self, control: BoundControl, action: Option<SurfaceAction>) {
        match control {
            BoundControl::StripFader => self.strip_fader = action,
            BoundControl::StripEncoder => self.strip_encoder = action,
            BoundControl::StripButton(button) => self.set_strip_button(button, action),
            BoundControl::MainFader => self.main_fader = action,
            BoundControl::Global(button) => self.set_global(button, action),
            BoundControl::Jog => self.jog = action,
        }
    }

    /// What a control does, if anything.
    #[must_use]
    pub fn action(&self, control: BoundControl) -> Option<SurfaceAction> {
        match control {
            BoundControl::StripFader => self.strip_fader,
            BoundControl::StripEncoder => self.strip_encoder,
            BoundControl::StripButton(button) => {
                self.strip_buttons.get(button.index()).copied().flatten()
            }
            BoundControl::MainFader => self.main_fader,
            BoundControl::Global(button) => self.global.get(button.index()).copied().flatten(),
            BoundControl::Jog => self.jog,
        }
    }

    /// How many controls are bound.
    #[must_use]
    pub fn bound(&self) -> usize {
        self.strip_fader.is_some() as usize
            + self.strip_encoder.is_some() as usize
            + self.main_fader.is_some() as usize
            + self.jog.is_some() as usize
            + self
                .strip_buttons
                .iter()
                .filter(|slot| slot.is_some())
                .count()
            + self.global.iter().filter(|slot| slot.is_some()).count()
    }

    /// The command an event means, or nothing at all.
    ///
    /// Nothing for a control with no binding, for a button coming back up, for
    /// a touch — a hand arriving on a fader is layer 2's business and no
    /// command of its own — and for an action the context cannot complete.
    #[must_use]
    pub fn command(&self, event: SurfaceEvent, context: &SurfaceContext) -> Option<Command> {
        match event {
            SurfaceEvent::Button { button, pressed } => {
                let (action, origin) = match button {
                    ButtonId::Strip { strip, button } => (
                        self.action(BoundControl::StripButton(button))?,
                        Origin::Strip(strip),
                    ),
                    ButtonId::Global(button) => {
                        (self.action(BoundControl::Global(button))?, Origin::Panel)
                    }
                };
                // A press is the event and the release ends it, so a release
                // says nothing — **except** for the one action that is a
                // gesture rather than an instruction. `Flash` is an
                // `ExecutorButtonFunction`, and its release is what puts the
                // master back.
                if !pressed && !action.is_momentary() {
                    return None;
                }
                action.resolve(origin, Input::Button(pressed), context)
            }
            SurfaceEvent::Touch { .. } => None,
            SurfaceEvent::Moved { fader, level } => match fader {
                Fader::Strip(strip) => {
                    self.strip_fader?
                        .resolve(Origin::Strip(strip), Input::Level(level), context)
                }
                Fader::Main => {
                    self.main_fader?
                        .resolve(Origin::Panel, Input::Level(level), context)
                }
            },
            SurfaceEvent::Encoder { strip, steps } => {
                self.strip_encoder?
                    .resolve(Origin::Strip(strip), Input::Steps(steps), context)
            }
            SurfaceEvent::Jog { steps } => {
                self.jog?
                    .resolve(Origin::Panel, Input::Steps(steps), context)
            }
        }
    }

    /// Puts an action on a strip button.
    fn set_strip_button(&mut self, button: StripButton, action: Option<SurfaceAction>) {
        if let Some(slot) = self.strip_buttons.get_mut(button.index()) {
            *slot = action;
        }
    }

    /// Puts an action on a panel button.
    fn set_global(&mut self, button: GlobalButton, action: Option<SurfaceAction>) {
        if let Some(slot) = self.global.get_mut(button.index()) {
            *slot = action;
        }
    }
}

/// The file, as `docs/MCU_MAPPING.md` §4.2 shapes it.
///
/// Unknown keys are **allowed here and nowhere else**: the shipped profile
/// carries the verification record, the shared-mode description and the reserved
/// list beside its bindings, and a document format that refused prose would
/// force that documentation into a second file nobody would open.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileDocument {
    profile_version: u32,
    device: String,
    #[serde(default)]
    bindings: Vec<BindingEntry>,
}

/// One row of the table.
///
/// `deny_unknown_fields`, unlike the document: a misspelled key *here* is an
/// argument that silently did not arrive.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingEntry {
    control: String,
    /// `null` is a control deliberately left alone, which is not the same as one
    /// the file forgot: the shipped profile writes out F5–F8 so that a person
    /// looking for them finds them.
    action: Option<SurfaceAction>,
}

#[cfg(test)]
mod tests {
    use super::{
        Bindings, BoundControl, ExecutorTarget, ProfileError, Step, SurfaceAction, SurfaceContext,
    };
    use crate::control::ButtonId;
    use crate::model::SurfaceEvent;
    use crate::profile::{Fader, GlobalButton, McuProfile, StripButton, X_TOUCH};
    use prism_domain::{
        AttributeType, Command, ExecutorButtonFunction, ExecutorButtonRef, ExecutorId,
        FeatureGroup, GoDirection, ParamDirection, PlaybackTarget, ViewId, WindowType,
    };

    fn context() -> SurfaceContext {
        SurfaceContext {
            executor_page: 2,
            selected_executor: Some(ExecutorId::new(19)),
            previous_view: Some(ViewId::new(1)),
            next_view: Some(ViewId::new(7)),
            programmer_page: 3,
            parameter: Some(AttributeType::Tilt),
        }
    }

    fn press(button: GlobalButton) -> SurfaceEvent {
        SurfaceEvent::Button {
            button: ButtonId::Global(button),
            pressed: true,
        }
    }

    fn document(bindings: &str) -> String {
        format!(r#"{{"profileVersion":1,"device":"behringer-x-touch","bindings":[{bindings}]}}"#)
    }

    #[test]
    fn a_strip_fader_addresses_the_executor_under_it_on_the_current_page() {
        // D7: page * 8 + index, and the level is the one layer 2 scaled.
        let table = Bindings::defaults();
        assert_eq!(
            table.command(
                SurfaceEvent::Moved {
                    fader: Fader::Strip(3),
                    level: 40_000,
                },
                &context()
            ),
            Some(Command::SetExecutorMaster {
                executor_id: ExecutorId::new(19),
                level: 40_000,
            })
        );
    }

    #[test]
    fn the_main_fader_and_the_transport_act_on_the_selected_executor() {
        let table = Bindings::defaults();
        let context = context();
        assert_eq!(
            table.command(
                SurfaceEvent::Moved {
                    fader: Fader::Main,
                    level: 100,
                },
                &context
            ),
            Some(Command::SetExecutorMaster {
                executor_id: ExecutorId::new(19),
                level: 100,
            })
        );
        // §4.1 gives Stop the executor's `Off` **function**, which since S34
        // is a command the protocol can express — so the transport row is bound
        // as it is written rather than translated into the nearest thing that
        // existed.
        assert_eq!(
            table.command(press(GlobalButton::Stop), &context),
            Some(Command::ExecutorButton {
                executor_id: ExecutorId::new(19),
                button: ExecutorButtonRef::Function {
                    function: ExecutorButtonFunction::Off,
                },
                pressed: true,
            })
        );
        assert_eq!(
            table.command(press(GlobalButton::Play), &context),
            Some(Command::ExecutorButton {
                executor_id: ExecutorId::new(19),
                button: ExecutorButtonRef::Function {
                    function: ExecutorButtonFunction::On,
                },
                pressed: true,
            })
        );
    }

    #[test]
    fn nothing_happens_when_the_context_cannot_answer() {
        // Every one of these is an ordinary moment rather than a fault: no
        // executor selected, nothing on the jog wheel, no view beyond this one.
        let table = Bindings::defaults();
        let empty = SurfaceContext::default();
        assert_eq!(table.command(press(GlobalButton::Stop), &empty), None);
        assert_eq!(
            table.command(SurfaceEvent::Jog { steps: 4 }, &empty),
            None,
            "a wheel with no parameter under it"
        );
        assert_eq!(
            table.command(press(GlobalButton::ChannelRight), &empty),
            None
        );
        assert_eq!(
            table.command(
                SurfaceEvent::Moved {
                    fader: Fader::Main,
                    level: 7,
                },
                &empty
            ),
            None
        );
    }

    #[test]
    fn a_release_and_a_touch_say_nothing() {
        let table = Bindings::defaults();
        let context = context();
        // Save is an instruction rather than a gesture, so its release has
        // nothing to say. **`ExecutorButton` is the exception and the next test
        // is about it**: a `Flash` release is half the gesture, and layer 3
        // cannot know which function the executor has on that key.
        assert_eq!(
            table.command(
                SurfaceEvent::Button {
                    button: ButtonId::Global(GlobalButton::Save),
                    pressed: false,
                },
                &context
            ),
            None
        );
        assert_eq!(
            table.command(
                SurfaceEvent::Touch {
                    fader: Fader::Strip(0),
                    touched: true,
                },
                &context
            ),
            None
        );
    }

    #[test]
    fn paging_saturates_at_zero_rather_than_wrapping() {
        // `ExecutorId::from_page_and_slot` saturates for the same reason: a page
        // that wrapped would put eight executors nobody has heard of onto the
        // faders, during a show.
        let table = Bindings::defaults();
        let first = SurfaceContext {
            executor_page: 0,
            programmer_page: 0,
            ..context()
        };
        assert_eq!(
            table.command(press(GlobalButton::BankLeft), &first),
            Some(Command::SetExecutorPage { page: 0 })
        );
        assert_eq!(
            table.command(press(GlobalButton::CursorUp), &first),
            Some(Command::SetProgrammerPage { page: 0 })
        );
        assert_eq!(
            table.command(press(GlobalButton::BankRight), &first),
            Some(Command::SetExecutorPage { page: 1 })
        );
    }

    #[test]
    fn an_action_bound_to_a_control_that_cannot_drive_it_does_nothing() {
        // A level on a button and a master on the panel: two ways of writing a
        // profile wrong that must cost nothing at run time.
        let mut table = Bindings::empty();
        table.set(
            BoundControl::Global(GlobalButton::F5),
            Some(SurfaceAction::ExecutorMaster {
                target: ExecutorTarget::Strip,
            }),
        );
        table.set(
            BoundControl::Global(GlobalButton::F6),
            Some(SurfaceAction::AdjustParameter),
        );
        assert_eq!(table.command(press(GlobalButton::F5), &context()), None);
        assert_eq!(table.command(press(GlobalButton::F6), &context()), None);
    }

    #[test]
    fn an_unbound_control_produces_nothing_and_the_empty_table_binds_nothing() {
        let table = Bindings::empty();
        assert_eq!(table.bound(), 0);
        assert_eq!(table.command(press(GlobalButton::Play), &context()), None);
        assert_eq!(
            table.command(
                SurfaceEvent::Moved {
                    fader: Fader::Strip(0),
                    level: 1,
                },
                &context()
            ),
            None
        );
        assert_eq!(
            table.command(SurfaceEvent::Encoder { strip: 0, steps: 1 }, &context()),
            None
        );
        assert_eq!(
            table.command(SurfaceEvent::Jog { steps: 1 }, &context()),
            None
        );
        assert_eq!(
            table.command(
                SurfaceEvent::Button {
                    button: ButtonId::Strip {
                        strip: 0,
                        button: StripButton::Rec
                    },
                    pressed: true,
                },
                &context()
            ),
            None
        );
    }

    #[test]
    fn every_kind_of_control_can_be_read_back_out_of_the_table() {
        // `action` is what a settings screen and the tests next door ask with,
        // and it has one arm per kind of control. Walked exhaustively so an arm
        // that returned the wrong field would have somewhere to show up.
        let table = Bindings::defaults();
        assert_eq!(
            table.action(BoundControl::StripFader),
            Some(SurfaceAction::ExecutorMaster {
                target: ExecutorTarget::Strip
            })
        );
        assert_eq!(
            table.action(BoundControl::MainFader),
            Some(SurfaceAction::ExecutorMaster {
                target: ExecutorTarget::Selected
            })
        );
        assert_eq!(
            table.action(BoundControl::Jog),
            Some(SurfaceAction::AdjustParameter)
        );
        assert_eq!(table.action(BoundControl::StripEncoder), None);
        assert_eq!(
            table.action(BoundControl::StripButton(StripButton::Select)),
            Some(SurfaceAction::ExecutorButton {
                target: ExecutorTarget::Strip,
                button: ExecutorButtonRef::Slot { index: 3 },
            })
        );
        assert_eq!(
            table.action(BoundControl::Global(GlobalButton::Save)),
            Some(SurfaceAction::SaveShow)
        );
        // And every one of them can be replaced, which is what "user-editable"
        // means for the controls that are not buttons.
        let mut edited = table;
        for control in [
            BoundControl::StripFader,
            BoundControl::MainFader,
            BoundControl::Jog,
            BoundControl::StripEncoder,
        ] {
            edited.set(control, None);
            assert_eq!(edited.action(control), None, "{control}");
        }
        assert_eq!(edited.bound(), table.bound() - 3);
    }

    #[test]
    fn every_control_name_round_trips_through_its_spelling() {
        let mut controls = vec![
            BoundControl::StripFader,
            BoundControl::StripEncoder,
            BoundControl::MainFader,
            BoundControl::Jog,
        ];
        controls.extend(StripButton::ALL.map(BoundControl::StripButton));
        controls.extend(GlobalButton::ALL.map(BoundControl::Global));
        for control in controls {
            let name = control.to_string();
            assert_eq!(
                BoundControl::from_name(&name),
                Some(control),
                "{name} did not read back"
            );
        }
        assert_eq!(BoundControl::from_name("Strip[*].Touch"), None);
        assert_eq!(BoundControl::from_name("Global.Nonesuch"), None);
        assert_eq!(BoundControl::from_name("Strip[*].Button.Wibble"), None);
        assert_eq!(BoundControl::from_name("Fader"), None);
    }

    #[test]
    fn a_profile_binds_what_it_names_and_leaves_the_rest_alone() {
        let text = document(
            r#"{"control":"Global.F5","action":{"t":"SelectView","view":4}},
               {"control":"Global.F6","action":null},
               {"control":"Strip[*].Encoder","action":{"t":"ExecutorMaster","target":"Strip"}}"#,
        );
        let table = Bindings::parse(&text, &X_TOUCH).expect("a profile this crate understands");
        assert_eq!(table.bound(), 2);
        assert_eq!(
            table.command(press(GlobalButton::F5), &context()),
            Some(Command::SelectView {
                view_id: ViewId::new(4)
            })
        );
        assert_eq!(table.command(press(GlobalButton::F6), &context()), None);
        assert_eq!(
            table.command(SurfaceEvent::Encoder { strip: 1, steps: 3 }, &context()),
            None,
            "a master needs a level and an encoder reports steps"
        );
    }

    #[test]
    fn a_profile_that_binds_smpte_beats_is_refused_and_the_reason_says_why() {
        let text = document(r#"{"control":"Global.SmpteBeats","action":{"t":"SaveShow"}}"#);
        let error = Bindings::parse(&text, &X_TOUCH).expect_err("the reserved button");
        assert_eq!(
            error,
            ProfileError::ReservedControl(GlobalButton::SmpteBeats)
        );
        let message = error.to_string();
        assert!(
            message.contains("switches the desk between the two hosts"),
            "{message}"
        );
        assert!(message.contains("Leave it unbound"), "{message}");
    }

    #[test]
    fn naming_the_reserved_button_without_binding_it_is_allowed() {
        // `null` is how a profile says *deliberately nothing here*, and the
        // shipped file does exactly that for SMPTE/Beats.
        let text = document(r#"{"control":"Global.SmpteBeats","action":null}"#);
        let table = Bindings::parse(&text, &X_TOUCH).expect("an unbound reservation");
        assert_eq!(table.bound(), 0);
    }

    #[test]
    fn every_way_a_profile_can_be_wrong_names_what_was_wrong() {
        let cases: [(String, ProfileError); 5] = [
            (
                "not json at all".to_owned(),
                ProfileError::Malformed(String::new()),
            ),
            (
                r#"{"profileVersion":9,"device":"behringer-x-touch","bindings":[]}"#.to_owned(),
                ProfileError::UnsupportedVersion {
                    found: 9,
                    supported: 1,
                },
            ),
            (
                r#"{"profileVersion":1,"device":"some-other-desk","bindings":[]}"#.to_owned(),
                ProfileError::WrongDevice {
                    found: "some-other-desk".to_owned(),
                    expected: "behringer-x-touch",
                },
            ),
            (
                document(r#"{"control":"Global.Wibble","action":null}"#),
                ProfileError::UnknownControl("Global.Wibble".to_owned()),
            ),
            (
                document(
                    r#"{"control":"Global.F1","action":{"t":"SaveShow"}},
                       {"control":"Global.F1","action":{"t":"Oops"}}"#,
                ),
                ProfileError::DuplicateControl(BoundControl::Global(GlobalButton::F1)),
            ),
        ];
        for (text, want) in cases {
            let error = Bindings::parse(&text, &X_TOUCH).expect_err(&text);
            match (&error, &want) {
                // The parser's own words are serde's and are not this crate's to
                // assert; that it refused, and said something, is.
                (ProfileError::Malformed(reason), ProfileError::Malformed(_)) => {
                    assert!(!reason.is_empty());
                }
                _ => assert_eq!(error, want, "{text}"),
            }
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn a_misspelled_key_inside_a_binding_is_refused_rather_than_dropped() {
        // The document may carry prose; a row may not carry a typo, because the
        // typo is an argument that silently did not arrive.
        let text = document(r#"{"control":"Global.F1","actoin":{"t":"SaveShow"}}"#);
        assert!(matches!(
            Bindings::parse(&text, &X_TOUCH),
            Err(ProfileError::Malformed(_))
        ));
        let with_prose = r#"{"profileVersion":1,"device":"behringer-x-touch","documentation":"why",
             "bindings":[{"control":"Global.F1","action":{"t":"SaveShow"}}]}"#;
        assert!(Bindings::parse(with_prose, &X_TOUCH).is_ok());
    }

    #[test]
    fn a_bad_profile_falls_back_to_the_defaults_and_load_cannot_fail() {
        let (table, error) = Bindings::load("{", &X_TOUCH);
        assert_eq!(table, Bindings::defaults());
        assert!(error.is_some());
        let (table, error) = Bindings::load(
            &document(r#"{"control":"Global.F1","action":{"t":"SaveShow"}}"#),
            &X_TOUCH,
        );
        assert_eq!(error, None);
        assert_eq!(table.bound(), 1);
    }

    #[test]
    fn a_surface_with_no_reserved_button_accepts_what_the_x_touch_refuses() {
        // The reservation is the profile's, not this module's: `is_reserved` is
        // what decides, so a device without that button is not second-guessed.
        let open = McuProfile {
            reserved_buttons: &[],
            ..X_TOUCH
        };
        let text = document(r#"{"control":"Global.SmpteBeats","action":{"t":"Oops"}}"#);
        assert!(Bindings::parse(&text, &open).is_ok());
        assert!(Bindings::parse(&text, &X_TOUCH).is_err());
    }

    #[test]
    fn every_action_the_table_can_hold_resolves_to_the_command_it_names() {
        // One case per variant, so a variant added without a resolution is a
        // test that stops compiling rather than a binding that does nothing.
        let context = context();
        let cases = [
            (
                SurfaceAction::ExecutorGo {
                    target: ExecutorTarget::Selected,
                    direction: GoDirection::Prev,
                },
                Command::ExecutorGo {
                    target: PlaybackTarget::of_executor(ExecutorId::new(19)),
                    direction: GoDirection::Prev,
                },
            ),
            (
                SurfaceAction::SelectExecutor {
                    target: ExecutorTarget::Selected,
                },
                Command::SelectExecutor {
                    executor_id: ExecutorId::new(19),
                },
            ),
            (SurfaceAction::ClearProgrammer, Command::ClearProgrammer),
            (
                SurfaceAction::ExecutorPage { delta: 2 },
                Command::SetExecutorPage { page: 4 },
            ),
            (
                SurfaceAction::SelectView {
                    view: ViewId::new(5),
                },
                Command::SelectView {
                    view_id: ViewId::new(5),
                },
            ),
            (
                SurfaceAction::StepView {
                    direction: Step::Prev,
                },
                Command::SelectView {
                    view_id: ViewId::new(1),
                },
            ),
            (
                SurfaceAction::StepView {
                    direction: Step::Next,
                },
                Command::SelectView {
                    view_id: ViewId::new(7),
                },
            ),
            (
                SurfaceAction::ProgrammerPage { delta: 1 },
                Command::SetProgrammerPage { page: 4 },
            ),
            (
                SurfaceAction::SelectProgrammerParam {
                    direction: ParamDirection::Next,
                },
                Command::SelectProgrammerParam {
                    direction: ParamDirection::Next,
                },
            ),
            (
                SurfaceAction::SetEncoderBank {
                    group: FeatureGroup::Beam,
                },
                Command::SetEncoderBank {
                    group: FeatureGroup::Beam,
                },
            ),
            (
                SurfaceAction::OpenWindow {
                    window: WindowType::Groups,
                },
                Command::OpenWindow {
                    window: WindowType::Groups,
                    params: None,
                },
            ),
            (SurfaceAction::SaveShow, Command::SaveShow),
            (SurfaceAction::Oops, Command::Oops),
            (SurfaceAction::Redo, Command::Redo),
        ];
        for (action, want) in cases {
            let mut table = Bindings::empty();
            table.set(BoundControl::Global(GlobalButton::F7), Some(action));
            assert_eq!(
                table.command(press(GlobalButton::F7), &context),
                Some(want),
                "{action:?}"
            );
        }
        // The two that need a value rather than a press, on the controls that
        // carry one.
        let mut table = Bindings::empty();
        table.set(BoundControl::Jog, Some(SurfaceAction::AdjustParameter));
        assert_eq!(
            table.command(SurfaceEvent::Jog { steps: -3 }, &context),
            Some(Command::SetAttribute {
                attribute: AttributeType::Tilt,
                value: -3,
                relative: true,
            })
        );
        table.set(
            BoundControl::StripEncoder,
            Some(SurfaceAction::ExecutorMaster {
                target: ExecutorTarget::Strip,
            }),
        );
        assert_eq!(
            table.command(
                SurfaceEvent::Moved {
                    fader: Fader::Strip(1),
                    level: 5,
                },
                &context
            ),
            None,
            "the strip fader is unbound in this table"
        );
    }

    #[test]
    fn an_action_reads_back_out_of_the_json_it_was_written_as() {
        // The wire form is what a person edits, so it is asserted as text
        // rather than by a round trip through this crate's own serialiser.
        let text = document(
            r#"{"control":"Global.F1","action":{"t":"OpenWindow","window":"PresetPool"}},
               {"control":"Global.F2","action":{"t":"ExecutorGo","target":"Strip","direction":"Prev"}},
               {"control":"Global.F3","action":{"t":"ExecutorPage","delta":-1}},
               {"control":"Global.F4","action":{"t":"StepView","direction":"Next"}}"#,
        );
        let table = Bindings::parse(&text, &X_TOUCH).expect("the documented shape");
        assert_eq!(
            table.action(BoundControl::Global(GlobalButton::F1)),
            Some(SurfaceAction::OpenWindow {
                window: WindowType::PresetPool
            })
        );
        assert_eq!(
            table.action(BoundControl::Global(GlobalButton::F2)),
            Some(SurfaceAction::ExecutorGo {
                target: ExecutorTarget::Strip,
                direction: GoDirection::Prev
            })
        );
        assert_eq!(
            table.action(BoundControl::Global(GlobalButton::F3)),
            Some(SurfaceAction::ExecutorPage { delta: -1 })
        );
        assert_eq!(
            table.action(BoundControl::Global(GlobalButton::F4)),
            Some(SurfaceAction::StepView {
                direction: Step::Next
            })
        );
    }
}
