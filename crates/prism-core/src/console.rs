//! The command line: what an operator types, and the commands it means — S49.
//!
//! # It is the desk's parser, and there is one of it
//!
//! `1 thru 3 at 50` is not a command. It is two of them — a `SelectFixtures`
//! and a `SetAttribute` — and turning one into the other is what this module
//! does. **D3 is untouched**: nothing here decides what the desk *is*, it
//! decides what was *asked for*, and every command it produces goes through the
//! ordinary appliers to be validated, applied or refused. A line naming a
//! fixture the show has not got parses perfectly and is refused by the show.
//!
//! Until S49 this lived in `ui/src/desk/console.ts`, because that is where the
//! operator types. The consequence was quiet and then loud: **a key on the
//! X-Touch cannot run a line** if the only parser is in a browser, and S43's
//! stop-gap — bump a counter and let whichever client holds the keyboard focus
//! parse it — runs the line twice when two screens are focused on two machines.
//! A doubled Go is the fault this module exists to remove.
//!
//! # It never fails
//!
//! [`parse_command_line`] answers with commands or with a sentence, for every
//! string there is. There is no error type and no panic path: a console that
//! could fail on a typo in the middle of a show is a console that stops
//! answering. `tests/console.rs` runs ten thousand generated lines through it.
//!
//! # It does not read the show, and that is why the modes are questions
//!
//! S26 wrote the rule and S40 kept it: a parser that consulted the patch could
//! be wrong about state it does not own. So `Copy Sequence 2 Sequence 6` means
//! the same thing whether or not sequence 2 exists, and a line whose
//! destination might already hold something carries a [`ModeQuestion`] rather
//! than a decision. Whoever *has* the show — `prismd`, answering
//! `prism_domain::Query::CommandLineReading` — looks at it to decide whether to
//! ask, and the operator's answer travels in the command
//! (`Command::CommandLineInput::mode`, [`apply_mode`]).
//!
//! # The grammar, in full
//!
//! ```text
//!   line     := select | level | verb | playback | word
//!
//!   select   := ["+"] ["fixture"] fixtures  -- 1 · 1 thru 4 · 1 + 3 · fixture 12 · + 5
//!   level    := [fixtures] [attribute] "at" percent
//!
//!   word     := "clear" | "full" | "oops" | "update"
//!   verb     := "store"  target
//!             | "new"    "view" n [name]     -- an empty view, and switch to it
//!             | "edit"   cue
//!             | "goto"   [playback] cue      -- goto cue 5
//!             | "delete" object
//!             | "move"   object object
//!             | "copy"   object object
//!             | "label"  object [name]
//!             | "color"  object [colour]
//!             | "assign" "sequence" n "executor" n
//!             | "assign" "executor" n control function
//!             | "page"   n
//!   playback := ("on" | "off" | "go" | "go+" | "go-") [target]
//!
//!   object   := ("sequence" | "cue" | "group" | "preset" | "view" | "executor") n
//!   colour   := "red" | "green" | "yellow" | "blue" | "magenta" | "cyan"
//!             | "white" | "none" | hex        -- #f80 · #ff8800 · ff8800
//!   target   := object | n                  -- `go 3` is S26's, and is an executor
//!   fixtures := range (("+" | ",") range)*
//!   range    := number ["thru" number]
//! ```
//!
//! A bare `Group 3`, `Sequence 5`, `View 2`, `Executor 4` or `Preset 1` is the
//! **selecting** form of that word: a group selects its fixtures, a sequence
//! becomes the one a store goes into, a view is switched to, an executor is the
//! one the transport acts on, and a preset is applied to the selection.
//!
//! Case does not matter and neither does spacing: `1thru4` is a range, because
//! a console's keypad has no space bar worth reaching for. A name may be quoted
//! — `Label View 1 "House lights"` — and need not be.

use prism_domain::{
    AttributeType, Command, CommandLineMode, CommandLineQuestion, CommandLineReadingKind,
    ExecutorButtonFunction, ExecutorChange, ExecutorEncoderFunction, ExecutorFaderFunction,
    ExecutorId, FixtureId, GoDirection, GroupId, ObjectRef, OverwriteMode, PlaybackTarget,
    PresetId, PresetPool, RgbColor, SelectionMode, SequenceId, SequenceStoreMode, StoreMode,
    ViewId,
};

/// The largest level the protocol carries — `u16::MAX`.
const FULL_LEVEL: f64 = 65_535.0;

/// The most fixtures one range may name.
///
/// A guard rather than a limit anybody will meet: `1 thru 999999999` is a typo,
/// and building that vector is how a console stops answering. The daemon would
/// refuse the command anyway; the point is to say so before a thread has spent
/// a second on it.
pub const MAX_RANGE: u32 = 4096;

/// The word that makes a range.
const THRU: &str = "thru";

/// The words that name one of the six numbered things a desk has.
const OBJECT_WORDS: [&str; 6] = ["sequence", "cue", "group", "preset", "view", "executor"];

/// Every word that begins a **command** rather than a selection.
///
/// The head words of [`parse_command_line`]'s own `match`, and the line between
/// the two halves of this grammar: a line that starts with one of these is
/// *doing* something to a named object, and a line that does not is a **fixture
/// selection**.
///
/// S43's rebuild is what needs the distinction written down. A click on a pool
/// appends its object to the line when the line is a command waiting for one,
/// and does the pool's own thing when it is not — and *is not* means, in so many
/// words, *this is a selection, and a selection takes fixtures*. Typing
/// `1 thru` and clicking a group is not a group being named, it is a range being
/// built. `prism_domain::Answer::CommandLineReading::verb` is how a client is
/// told which it is holding, because since S49 the client has no such table.
pub const VERB_WORDS: [&str; 21] = [
    "clear", "full", "oops", "update", "store", "new", "edit", "goto", "delete", "move", "copy",
    "label", "color", "assign", "on", "off", "go", "go+", "go-", "goback", "page",
];

/// The verbs whose **absent** trailing argument is itself an instruction.
///
/// `Label View 1` clears the name and `Color Sequence 4` takes the colour off —
/// both deliberate, and both parse to a perfectly good command with nothing
/// after the object. That makes them the two verbs a **pointer** must never
/// finish: a click that silently wiped a name is the worst kind of shortcut, so
/// they are appended and left for Enter.
pub const CLEARING_VERBS: [&str; 2] = ["label", "color"];

/// Every word the line knows, for completion.
pub const CONSOLE_WORDS: [&str; 29] = [
    "at", "assign", "clear", "color", "copy", "cue", "delete", "edit", "executor", "fixture",
    "full", "go", "go+", "go-", "goto", "group", "label", "move", "new", "off", "on", "oops",
    "page", "preset", "sequence", "store", "thru", "update", "view",
];

/// The words that take a colour off rather than putting one on.
const NO_COLOR_WORDS: [&str; 2] = ["none", "off"];

/// The colours a scribble strip can light, and what each one is as twenty-four
/// bits.
///
/// **Seven words and not seventy.** A strip's backlight is three lamps
/// (`docs/MCU_MAPPING.md` §2.3), so these are the colours it can actually show —
/// an operator who types one of them knows exactly what the desk will look like.
/// Anything else is a hex triplet, which the daemon keeps in full and the
/// surface quantises hue-first.
const COLOR_NAMES: [(&str, (u8, u8, u8)); 7] = [
    ("red", (255, 0, 0)),
    ("green", (0, 255, 0)),
    ("yellow", (255, 255, 0)),
    ("blue", (0, 0, 255)),
    ("magenta", (255, 0, 255)),
    ("cyan", (0, 255, 255)),
    ("white", (255, 255, 255)),
];

/// Every word a colour may be written as, for completion.
fn color_words() -> Vec<&'static str> {
    COLOR_NAMES
        .iter()
        .map(|(word, _)| *word)
        .chain(NO_COLOR_WORDS)
        .collect()
}

/// Which set of words a [`ModeQuestion`] offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeQuestionKind {
    /// A cue or a preset: `StoreMode`, whose Remove takes values back out.
    Store,
    /// A whole cue list: `SequenceStoreMode`, whose Append cannot lose a cue.
    Sequence,
    /// A copy, a move or a group store: `OverwriteMode`, which has two.
    Overwrite,
}

impl ModeQuestionKind {
    /// The words this question offers, in the order it offers them.
    #[must_use]
    pub fn modes(self) -> Vec<CommandLineMode> {
        match self {
            Self::Store => vec![
                CommandLineMode::Merge,
                CommandLineMode::Override,
                CommandLineMode::Remove,
            ],
            Self::Sequence => vec![
                CommandLineMode::Append,
                CommandLineMode::Override,
                CommandLineMode::Merge,
            ],
            Self::Overwrite => vec![CommandLineMode::Merge, CommandLineMode::Override],
        }
    }
}

/// A line whose destination may already hold something.
///
/// The parser cannot know whether it does — it does not read the show — so it
/// says *what would be written and which words the question takes*, and whoever
/// holds the show asks when something is there. [`apply_mode`] puts the answer
/// into the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeQuestion {
    /// Which set of words the prompt offers.
    pub kind: ModeQuestionKind,
    /// What the line would write to, in the words an operator typed.
    pub what: String,
    /// Where to look to see whether anything is already there.
    pub at: ObjectRef,
}

impl ModeQuestion {
    /// The wire form, for `prism_domain::Answer::CommandLineReading`.
    ///
    /// [`Self::at`] is deliberately **not** in it: whether something is already
    /// there is the daemon's to know, and a client that was handed the reference
    /// and looked in its own mirror would be the second opinion S49 exists to
    /// remove.
    #[must_use]
    pub fn question(&self) -> CommandLineQuestion {
        CommandLineQuestion {
            what: self.what.clone(),
            modes: self.kind.modes(),
        }
    }
}

/// What a line turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleReading {
    /// A line with nothing in it. Enter on an empty console does nothing.
    Empty,
    /// Commands, in the order they have to be sent.
    Commands {
        /// What to apply.
        commands: Vec<Command>,
        /// Set **only** when the line carries a mode somebody may have to
        /// choose, which is most lines' `None`.
        question: Option<ModeQuestion>,
    },
    /// A line that is not one, and what to tell the operator about it.
    Refused(String),
}

impl ConsoleReading {
    /// The word `prism_domain::Answer::CommandLineReading` carries for this
    /// shape.
    #[must_use]
    pub const fn kind(&self) -> CommandLineReadingKind {
        match self {
            Self::Empty => CommandLineReadingKind::Empty,
            Self::Commands { .. } => CommandLineReadingKind::Commands,
            Self::Refused(_) => CommandLineReadingKind::Error,
        }
    }

    /// The commands, or nothing at all for an empty or refused line.
    #[must_use]
    pub fn commands(&self) -> &[Command] {
        match self {
            Self::Commands { commands, .. } => commands,
            Self::Empty | Self::Refused(_) => &[],
        }
    }

    /// The question the line carries, if it carries one.
    #[must_use]
    pub const fn question(&self) -> Option<&ModeQuestion> {
        match self {
            Self::Commands { question, .. } => question.as_ref(),
            Self::Empty | Self::Refused(_) => None,
        }
    }
}

/// One word of a line, as typed and as matched.
#[derive(Debug, Clone)]
struct Token {
    /// Lower-cased, for matching.
    text: String,
    /// Exactly as the operator typed it, for names.
    raw: String,
}

/// Which playback word a line began with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Playback {
    On,
    Off,
    Go,
    GoBack,
}

/* -------------------------------------------------------------------------- */
/* The entry point                                                            */
/* -------------------------------------------------------------------------- */

/// Reads a line.
///
/// Never fails — see the module documentation.
#[must_use]
pub fn parse_command_line(line: &str) -> ConsoleReading {
    let words = tokenise(line);
    let Some(head) = words.first() else {
        return ConsoleReading::Empty;
    };
    match head.text.as_str() {
        "clear" => only(&words, "clear", vec![Command::ClearProgrammer]),
        // A whole command with no argument (§4.5): dimmer to full on whatever is
        // selected, which is what the key does on every console there is.
        "full" => only(
            &words,
            "full",
            vec![Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: level_from_percent(100.0),
                relative: false,
            }],
        ),
        "oops" => only(&words, "oops", vec![Command::Oops]),
        "update" => only(&words, "update", vec![Command::Update]),
        "store" => store_line(&words),
        "new" => new_line(&words),
        "edit" => edit_line(&words),
        "goto" => goto_line(&words),
        "delete" => delete_line(&words),
        "move" => pair_line(&words, "move"),
        "copy" => pair_line(&words, "copy"),
        "label" => label_line(&words),
        "color" => color_line(&words),
        "assign" => assign_line(&words),
        "on" => playback_line(&words, Playback::On),
        "off" => playback_line(&words, Playback::Off),
        "go" => playback_line(&words, Playback::Go),
        "goback" => playback_line(&words, Playback::GoBack),
        "page" => page_line(&words),
        _ => selection_line(&words),
    }
}

/// Whether a line begins with a **verb** — see [`VERB_WORDS`].
#[must_use]
pub fn is_verb_line(line: &str) -> bool {
    VERB_WORDS.contains(&head_word(line).as_str())
}

/// Whether a line begins with one of the two [`CLEARING_VERBS`].
#[must_use]
pub fn is_clearing_line(line: &str) -> bool {
    CLEARING_VERBS.contains(&head_word(line).as_str())
}

/// The first word of a line, lower-cased, as a pointer reads it.
///
/// Deliberately the raw split rather than [`tokenise`]: what a click needs to
/// know is which verb the operator typed, and `go+` is a verb whose tokenised
/// form is a different word.
fn head_word(line: &str) -> String {
    line.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

/// A word that takes nothing after it.
fn only(words: &[Token], keyword: &str, commands: Vec<Command>) -> ConsoleReading {
    if words.len() == 1 {
        just(commands)
    } else {
        too_much(keyword, words)
    }
}

/// Commands with no question attached, which is most lines.
fn just(commands: Vec<Command>) -> ConsoleReading {
    ConsoleReading::Commands {
        commands,
        question: None,
    }
}

/// One command and the question it carries.
fn asking(command: Command, kind: ModeQuestionKind, at: ObjectRef) -> ConsoleReading {
    ConsoleReading::Commands {
        commands: vec![command],
        question: Some(ModeQuestion {
            kind,
            what: object_text(&at),
            at,
        }),
    }
}

/// A line that is not one.
fn refused(message: impl Into<String>) -> ConsoleReading {
    ConsoleReading::Refused(message.into())
}

/// The complaint for a line that says more than its first word allows.
fn too_much(keyword: &str, words: &[Token]) -> ConsoleReading {
    let said = words
        .iter()
        .map(|word| spoken(&word.raw))
        .collect::<Vec<_>>()
        .join(" ");
    refused(format!("\"{said}\" says more than {keyword} takes."))
}

/// A token as the operator typed it.
///
/// `go-` becomes the token `goback` in [`tokenise`] so that the `+` and `-` of
/// `go+` and `go-` are not read as separators; a message about it has to say the
/// word that was typed.
fn spoken(word: &str) -> String {
    if word == "goback" {
        "go-".to_owned()
    } else {
        word.to_owned()
    }
}

/// The raw words of a run, joined for a complaint.
fn said(words: &[Token]) -> String {
    words
        .iter()
        .map(|word| word.raw.clone())
        .collect::<Vec<_>>()
        .join(" ")
}

/* -------------------------------------------------------------------------- */
/* Object references                                                          */
/* -------------------------------------------------------------------------- */

/// An object reference read off the front of the words, and what is left.
struct ObjectRead<'a> {
    /// What was named.
    target: ObjectRef,
    /// The words after it.
    rest: &'a [Token],
}

/// `sequence 4`, `cue 1.5`, `group 3`, `view 2`, `executor 1`, `preset 6`.
///
/// A **cue** may name its sequence — `sequence 5 cue 3` — and when it does not,
/// `sequence_id` is `None` and the daemon resolves it against
/// `Session::selected_sequence`. A client that filled it in would be sending a
/// command whose meaning had already moved on a second screen.
fn read_object(words: &[Token]) -> Result<ObjectRead<'_>, String> {
    let Some(head) = words.first() else {
        return Err(
            "which one? Try a word like sequence, cue, group, preset, view or executor.".to_owned(),
        );
    };
    if !OBJECT_WORDS.contains(&head.text.as_str()) {
        return Err(format!(
            "\"{}\" is not something to name. Try sequence, cue, group, preset, view or executor.",
            head.raw
        ));
    }
    let Some(number_word) = words.get(1) else {
        let word = &head.text;
        return Err(format!("{word} which one? Try \"{word} 1\"."));
    };
    // `sequence 5 cue 3` — a cue that names the list it is in.
    if head.text == "sequence" && words.get(2).is_some_and(|word| word.text == "cue") {
        let Some(sequence_id) = whole_number(&number_word.text) else {
            return Err(format!("\"{}\" is not a sequence number.", number_word.raw));
        };
        let Some(cue_word) = words.get(3) else {
            return Err("cue which one? Try \"cue 1\".".to_owned());
        };
        if !is_cue_number(&cue_word.text) {
            return Err(format!("\"{}\" is not a cue number.", cue_word.raw));
        }
        return Ok(ObjectRead {
            target: ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(sequence_id)),
                cue_number: cue_word.raw.clone(),
            },
            rest: &words[4..],
        });
    }
    if head.text == "cue" {
        if !is_cue_number(&number_word.text) {
            return Err(format!("\"{}\" is not a cue number.", number_word.raw));
        }
        return Ok(ObjectRead {
            target: ObjectRef::Cue {
                sequence_id: None,
                cue_number: number_word.raw.clone(),
            },
            rest: &words[2..],
        });
    }
    let Some(id) = whole_number(&number_word.text) else {
        return Err(format!(
            "\"{}\" is not a {} number.",
            number_word.raw, head.text
        ));
    };
    let rest = &words[2..];
    let target = match head.text.as_str() {
        "sequence" => ObjectRef::Sequence {
            sequence_id: SequenceId::new(id),
        },
        "group" => ObjectRef::Group {
            group_id: GroupId::new(id),
        },
        "preset" => ObjectRef::Preset {
            preset_id: PresetId::new(id),
        },
        "view" => ObjectRef::View {
            view_id: ViewId::new(id),
        },
        _ => ObjectRef::Executor {
            executor_id: ExecutorId::new(id),
        },
    };
    Ok(ObjectRead { target, rest })
}

/// One object reference in the words an operator typed.
///
/// Deliberately not `ObjectRef`'s own `Display`, which writes a cue's list
/// **before** it (`sequence 5 cue 3`, the order the line is typed in). A reading
/// is a sentence rather than a line, and *cue 3 of sequence 5* is how one is
/// read aloud at a desk.
fn object_text(target: &ObjectRef) -> String {
    match target {
        ObjectRef::Sequence { sequence_id } => format!("sequence {sequence_id}"),
        ObjectRef::Cue {
            sequence_id,
            cue_number,
        } => format!("cue {cue_number}{}", sequence_suffix(*sequence_id)),
        ObjectRef::Group { group_id } => format!("group {group_id}"),
        ObjectRef::Preset { preset_id } => format!("preset {preset_id}"),
        ObjectRef::View { view_id } => format!("view {view_id}"),
        ObjectRef::Executor { executor_id } => format!("executor {executor_id}"),
    }
}

/// ` of sequence 4`, or nothing at all for the selected one.
fn sequence_suffix(sequence_id: Option<SequenceId>) -> String {
    sequence_id.map_or_else(String::new, |id| format!(" of sequence {id}"))
}

/* -------------------------------------------------------------------------- */
/* The verbs                                                                  */
/* -------------------------------------------------------------------------- */

/// `store cue 5`, `store sequence 4`, `store preset 1`, `store group 3`,
/// `store view 2`.
///
/// Each carries the mode that **cannot lose anything** and a [`ModeQuestion`]
/// beside it, so whoever finds something already there can ask before running
/// it. A `store view` is the odd one out and has no mode at all: `StoreView`
/// has always overwritten the layout, because that is what storing a view *is*.
fn store_line(words: &[Token]) -> ConsoleReading {
    let read = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    let name = join_name(read.rest);
    match read.target {
        ObjectRef::Cue {
            sequence_id,
            ref cue_number,
        } => asking(
            Command::StoreCue {
                sequence_id,
                cue_number: cue_number.clone(),
                mode: StoreMode::Merge,
            },
            ModeQuestionKind::Store,
            read.target.clone(),
        ),
        ObjectRef::Sequence { sequence_id } => asking(
            Command::StoreSequence {
                sequence_id,
                name: if name.is_empty() {
                    format!("Sequence {sequence_id}")
                } else {
                    name
                },
                mode: SequenceStoreMode::Append,
            },
            ModeQuestionKind::Sequence,
            read.target.clone(),
        ),
        // `Store Preset 1 Color "Deep blue"` — the pool may be named, and when
        // it is not the desk's answer is `Session::encoder_bank`, resolved by
        // the daemon. A name that happens to *be* one of the bank words is
        // quoted, which is what the quotes are for.
        ObjectRef::Preset { preset_id } => {
            let pool = pool_in(read.rest);
            asking(
                Command::StorePreset {
                    preset_id,
                    pool,
                    name: if pool.is_none() {
                        name
                    } else {
                        join_name(&read.rest[1..])
                    },
                    // **A colour the line does not carry is one the daemon
                    // keeps.** There is no word for a colour and there should
                    // not be: a store that dropped `Preset::color` would throw
                    // away something an operator chose with a picker and
                    // nothing here can put back.
                    color: None,
                    mode: StoreMode::Merge,
                },
                ModeQuestionKind::Store,
                read.target.clone(),
            )
        }
        ObjectRef::Group { group_id } => asking(
            Command::StoreGroup {
                group_id,
                name,
                mode: OverwriteMode::Merge,
            },
            ModeQuestionKind::Overwrite,
            read.target.clone(),
        ),
        ObjectRef::View { view_id } => just(vec![Command::StoreView {
            view_id,
            name: if name.is_empty() {
                format!("View {view_id}")
            } else {
                name
            },
        }]),
        ObjectRef::Executor { .. } => refused(
            "an executor is assigned rather than stored. Try \"assign sequence 5 executor 1\".",
        ),
    }
}

/// `new view 5` · `new view 5 "Busking"` — S43, punch-list B11.
///
/// The opposite of `store view 5`, and both are wanted: storing means *keep what
/// is on the canvas*, and this means *give me an empty one*.
///
/// **A view is the only thing this verb takes**, and that is not an oversight
/// waiting to be filled in. `New Sequence 5` would be `Store Sequence 5` on a
/// free number, which S40 already decided; a view is different because storing
/// one is the *only* session command that reads the canvas, so it is the only
/// one whose empty form means something else.
fn new_line(words: &[Token]) -> ConsoleReading {
    let read = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    let ObjectRef::View { view_id } = read.target else {
        return refused("new takes a view: try \"new view 5\". Anything else is stored into.");
    };
    let name = join_name(read.rest);
    just(vec![Command::NewView {
        view_id,
        name: if name.is_empty() {
            format!("View {view_id}")
        } else {
            name
        },
    }])
}

/// `edit cue 3` · `edit sequence 5 cue 3`.
fn edit_line(words: &[Token]) -> ConsoleReading {
    let read = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    let ObjectRef::Cue {
        sequence_id,
        ref cue_number,
    } = read.target
    else {
        return refused("edit takes a cue: try \"edit cue 3\", or \"edit sequence 5 cue 3\".");
    };
    if !read.rest.is_empty() {
        return too_much("edit", words);
    }
    just(vec![Command::EditCue {
        sequence_id,
        cue_number: cue_number.clone(),
    }])
}

/// `goto cue 5` · `goto sequence 2 cue 5` · `goto executor 1 cue 5`.
///
/// A cue with no cue list named is the **selected** sequence's, which is
/// `PlaybackTarget::Selected` — the daemon resolves it, because the selection is
/// session state and a client that read it would be sending a command whose
/// meaning had already moved.
fn goto_line(words: &[Token]) -> ConsoleReading {
    let rest = &words[1..];
    // `goto executor 1 cue 5` — an executor and then the cue.
    if rest.first().is_some_and(|word| word.text == "executor") {
        let Some(executor_id) = rest.get(1).and_then(|word| whole_number(&word.text)) else {
            return refused("executor which one? Try \"goto executor 1 cue 5\".");
        };
        let cue = match read_object(&rest[2.min(rest.len())..]) {
            Ok(read) => read,
            Err(message) => return refused(message),
        };
        let ObjectRef::Cue { ref cue_number, .. } = cue.target else {
            return refused("goto takes a cue. Try \"goto executor 1 cue 5\".");
        };
        return just(vec![Command::Goto {
            target: PlaybackTarget::of_executor(ExecutorId::new(executor_id)),
            cue_number: cue_number.clone(),
        }]);
    }
    let read = match read_object(rest) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    let ObjectRef::Cue {
        sequence_id,
        ref cue_number,
    } = read.target
    else {
        return refused("goto takes a cue. Try \"goto cue 5\".");
    };
    just(vec![Command::Goto {
        target: sequence_id.map_or(PlaybackTarget::Selected, PlaybackTarget::of_sequence),
        cue_number: cue_number.clone(),
    }])
}

/// `delete sequence 4` and its five siblings.
fn delete_line(words: &[Token]) -> ConsoleReading {
    let read = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    if !read.rest.is_empty() {
        return too_much("delete", words);
    }
    just(vec![Command::Delete {
        target: read.target,
    }])
}

/// `move executor 1 executor 5` · `copy sequence 2 sequence 6`.
fn pair_line(words: &[Token], keyword: &str) -> ConsoleReading {
    let first = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    let Ok(second) = read_object(first.rest) else {
        return refused(format!(
            "{keyword} {} where? Try \"{keyword} sequence 2 sequence 6\".",
            object_text(&first.target)
        ));
    };
    if !second.rest.is_empty() {
        return too_much(keyword, words);
    }
    if !first.target.same_kind_as(&second.target) {
        return refused(format!(
            "{} and {} are not the same kind of thing.",
            object_text(&first.target),
            object_text(&second.target)
        ));
    }
    let command = if keyword == "move" {
        Command::Move {
            from: first.target.clone(),
            to: second.target.clone(),
            mode: OverwriteMode::Merge,
        }
    } else {
        Command::Copy {
            from: first.target.clone(),
            to: second.target.clone(),
            mode: OverwriteMode::Merge,
        }
    };
    // **An executor and a view swap rather than overwriting**, so there is
    // nothing to ask: nothing is lost either way. Both are places on the desk
    // whose *number* is the position rather than a name for the thing in it —
    // see `crate::objects` and `SessionState::move_view`.
    if keyword == "move"
        && matches!(
            first.target,
            ObjectRef::Executor { .. } | ObjectRef::View { .. }
        )
    {
        return just(vec![command]);
    }
    asking(command, ModeQuestionKind::Overwrite, second.target)
}

/// `label view 1 "Programmer"`, and the same for the other five.
fn label_line(words: &[Token]) -> ConsoleReading {
    let read = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    let name = join_name(read.rest);
    just(vec![Command::Label {
        target: read.target,
        name,
    }])
}

/// `color sequence 4 red`, `color executor 1 #ff8800`, `color sequence 4 none`.
///
/// **A colour with no word after it takes the colour off**, which is `Label`'s
/// rule one verb along: `label view 1` clears the name, and clearing is not a
/// second command. `none` and `off` say the same thing out loud, for an operator
/// who would rather see the word than trust the absence of one.
fn color_line(words: &[Token]) -> ConsoleReading {
    let read = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    if read.rest.len() > 1 {
        return too_much("color", words);
    }
    let color = match color_of(read.rest.first()) {
        Ok(color) => color,
        Err(message) => return refused(message),
    };
    just(vec![Command::Color {
        target: read.target,
        color,
    }])
}

/// One colour word, or the complaint for a word that is not one.
fn color_of(word: Option<&Token>) -> Result<Option<RgbColor>, String> {
    let Some(word) = word else {
        return Ok(None);
    };
    if NO_COLOR_WORDS.contains(&word.text.as_str()) {
        return Ok(None);
    }
    if let Some((_, (r, g, b))) = COLOR_NAMES.iter().find(|(name, _)| *name == word.text) {
        return Ok(Some(RgbColor {
            r: *r,
            g: *g,
            b: *b,
        }));
    }
    if let Some(color) = hex_color(&word.text) {
        return Ok(Some(color));
    }
    Err(format!(
        "\"{}\" is not a colour. Try {} or a hex triplet like #ff8800.",
        spoken(&word.raw),
        color_words().join(", ")
    ))
}

/// `#f80`, `#ff8800`, `ff8800` — or `None` for a word that is not one.
fn hex_color(text: &str) -> Option<RgbColor> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    // The three-digit form doubles each digit, as CSS does: `#f80` is `#ff8800`
    // rather than `#0f0800`, which is what an operator who typed it means.
    let full = if digits.len() == 3 {
        digits.chars().flat_map(|digit| [digit, digit]).collect()
    } else {
        digits.to_owned()
    };
    if full.len() != 6 {
        return None;
    }
    let byte = |from: usize| u8::from_str_radix(&full[from..from + 2], 16).ok();
    Some(RgbColor {
        r: byte(0)?,
        g: byte(2)?,
        b: byte(4)?,
    })
}

/// A colour as `#ff8800`, for the readout.
fn hex_of(color: RgbColor) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// `assign sequence 5 executor 1`, and since S45 `assign executor 1 fader master`.
///
/// **Two sentences under one verb, told apart by their first noun**, which is
/// what makes them one word to learn rather than two: *assign this list to that
/// fader* and *assign this function to that control* are the same act on a desk,
/// and an operator says "assign" for both. `docs/COMMAND_LINE.md` §2.4 has the
/// table.
fn assign_line(words: &[Token]) -> ConsoleReading {
    let sequence = match read_object(&words[1..]) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };
    // S45: `assign executor 1 <control> <function>` — punch-list entry B15.
    if let ObjectRef::Executor { executor_id } = sequence.target {
        return assign_control_line(executor_id, sequence.rest);
    }
    let ObjectRef::Sequence { sequence_id } = sequence.target else {
        return refused(
            "assign takes a sequence or an executor. Try \"assign sequence 5 executor 1\" or \"assign executor 1 fader master\".",
        );
    };
    let Ok(executor) = read_object(sequence.rest) else {
        return refused("assign it where? Try \"assign sequence 5 executor 1\".");
    };
    let ObjectRef::Executor { executor_id } = executor.target else {
        return refused(
            "a sequence is assigned to an executor. Try \"assign sequence 5 executor 1\".",
        );
    };
    if !executor.rest.is_empty() {
        return too_much("assign", words);
    }
    just(vec![Command::AssignExecutor {
        executor_id,
        sequence_id: Some(sequence_id),
    }])
}

/// How many keys an executor has, numbered from one on the line.
const EXECUTOR_BUTTONS: u32 = 4;

/// `assign executor 1 fader master`, `… button 2 go+`, `… encoder speed`, and
/// `… button 3 command "Go+ Sequence 3"` — **S45**, punch-list entry B15.
///
/// The control words are `fader`, `encoder` and `button <n>`; a button is
/// numbered **from one**, because that is how an operator counts the keys under
/// a fader, and `ExecutorChange::Button` counts from zero the way the hardware
/// does. One word between the two, in one place.
///
/// The function words are the enum spellings, lower-cased — the parser does not
/// read the show, so it does not read the *desk* either, and every word it
/// accepts is one `prism_domain` declares. `command` is the ninth and takes a
/// line. **Quote it when it has punctuation in it**: the tokeniser rewrites
/// `go+`, `+` and `,` so that `1 + 2` and `go+ executor 0` mean what they say,
/// and a quoted chunk is the one thing it keeps exactly as typed.
fn assign_control_line(executor_id: ExecutorId, rest: &[Token]) -> ConsoleReading {
    let Some(control) = rest.first() else {
        return refused(
            "assign what on it? Try \"assign executor 1 fader master\", or button, or encoder.",
        );
    };
    match control.text.as_str() {
        "fader" => {
            let names = fader_words();
            let Some(function) = ExecutorFaderFunction::ALL
                .into_iter()
                .find(|choice| matches_word(fader_word(*choice), rest.get(1)))
            else {
                return no_such_function(rest.get(1), &names, "a fader");
            };
            if rest.len() > 2 {
                return too_much("assign", rest);
            }
            change_command(executor_id, ExecutorChange::Fader { function })
        }
        "encoder" => {
            let names = encoder_words();
            let Some(function) = ExecutorEncoderFunction::ALL
                .into_iter()
                .find(|choice| matches_word(encoder_word(*choice), rest.get(1)))
            else {
                return no_such_function(rest.get(1), &names, "an encoder");
            };
            if rest.len() > 2 {
                return too_much("assign", rest);
            }
            change_command(executor_id, ExecutorChange::Encoder { function })
        }
        "button" => {
            let numbered = rest.get(1).and_then(|word| whole_number(&word.text));
            let Some(numbered) = numbered.filter(|n| (1..=EXECUTOR_BUTTONS).contains(n)) else {
                return refused(format!(
                    "which button? An executor has {EXECUTOR_BUTTONS}, numbered from one."
                ));
            };
            #[expect(
                clippy::cast_possible_truncation,
                reason = "numbered is 1..=4, so the index is 0..=3"
            )]
            let index = (numbered - 1) as u8;
            // The custom row: everything after `command` is the line, joined
            // back together as it was typed.
            if matches_word(
                &button_word(&ExecutorButtonFunction::CommandLine {
                    line: String::new(),
                }),
                rest.get(2),
            ) {
                let line = join_name(&rest[3.min(rest.len())..]);
                if line.is_empty() {
                    return refused(
                        "send which line? Try `assign executor 1 button 4 command \"Go+ Sequence 3\"`.",
                    );
                }
                return change_command(
                    executor_id,
                    ExecutorChange::Button {
                        index,
                        function: ExecutorButtonFunction::CommandLine { line },
                    },
                );
            }
            let names = button_words();
            let Some(function) = fixed_button_functions()
                .into_iter()
                .find(|choice| matches_word(&button_word(choice), rest.get(2)))
            else {
                return no_such_function(rest.get(2), &names, "a button");
            };
            if rest.len() > 3 {
                return too_much("assign", rest);
            }
            change_command(executor_id, ExecutorChange::Button { index, function })
        }
        _ => refused(format!(
            "\"{}\" is not one of an executor's controls. Try fader, encoder or button.",
            control.raw
        )),
    }
}

/// One `ConfigureExecutor`, which is what every arm above ends in.
fn change_command(executor_id: ExecutorId, change: ExecutorChange) -> ConsoleReading {
    just(vec![Command::ConfigureExecutor {
        executor_id,
        change,
    }])
}

/// Whether a token is a function's spelling **as a token**.
///
/// `Go+` and `Go-` are the two the tokeniser rewrites — `go+` becomes `go` and
/// `go-` becomes `goback`, so that `1 + 2` can mean what it says — and this puts
/// an enum name through the same rewrite rather than keeping a second table that
/// could disagree with it.
fn matches_word(name: &str, word: Option<&Token>) -> bool {
    let tokenised = tokenise(name)
        .first()
        .map_or_else(|| name.to_lowercase(), |token| token.text.clone());
    word.is_some_and(|word| word.text == tokenised)
}

/// What an operator is told when a control was given a word it does not know.
fn no_such_function(word: Option<&Token>, choices: &[String], what: &str) -> ConsoleReading {
    // The **typed** spellings, not the tokenised ones: an operator types `go+`.
    let list = choices
        .iter()
        .map(|choice| choice.to_lowercase())
        .collect::<Vec<_>>()
        .join(", ");
    refused(word.map_or_else(
        || format!("{what} does what? Try {list}."),
        |word| {
            format!(
                "\"{}\" is not something {what} does. Try {list}.",
                spoken(&word.raw)
            )
        },
    ))
}

/// `on`, `off`, `go`, `go+`, `go-`, each with an executor, a sequence or
/// nothing.
///
/// **Nothing means the selected sequence** — `PlaybackTarget::Selected`, which
/// is what makes a bare `Go+` on a desk with a cue list chosen mean something.
/// S26's `go 3` is kept: a bare number after one of these words is an executor,
/// which is what it meant before there was anything else it could be.
fn playback_line(words: &[Token], keyword: Playback) -> ConsoleReading {
    let target = match read_playback(&words[1..]) {
        Ok(target) => target,
        Err(message) => return refused(message),
    };
    just(vec![match keyword {
        Playback::On => Command::ExecutorOn { target },
        Playback::Off => Command::ExecutorOff { target },
        Playback::Go => Command::ExecutorGo {
            target,
            direction: GoDirection::Next,
        },
        Playback::GoBack => Command::ExecutorGo {
            target,
            direction: GoDirection::Prev,
        },
    }])
}

/// The target of a playback word: an executor, a sequence, or the selection.
fn read_playback(words: &[Token]) -> Result<PlaybackTarget, String> {
    let Some(head) = words.first() else {
        return Ok(PlaybackTarget::Selected);
    };
    // S26's form, kept and unbroken: a bare number is an executor.
    if let Some(bare) = whole_number(&head.text) {
        return if words.len() > 1 {
            Err(format!(
                "\"{}\" says more than a playback takes.",
                said(words)
            ))
        } else {
            Ok(PlaybackTarget::of_executor(ExecutorId::new(bare)))
        };
    }
    let read = read_object(words)?;
    if !read.rest.is_empty() {
        return Err(format!(
            "\"{}\" says more than a playback takes.",
            said(read.rest)
        ));
    }
    match read.target {
        ObjectRef::Executor { executor_id } => Ok(PlaybackTarget::of_executor(executor_id)),
        ObjectRef::Sequence { sequence_id } => Ok(PlaybackTarget::of_sequence(sequence_id)),
        other => Err(format!(
            "{} is not something that plays back. Try an executor or a sequence.",
            object_text(&other)
        )),
    }
}

/// `page 2` — the fader bank.
fn page_line(words: &[Token]) -> ConsoleReading {
    let rest = &words[1..];
    let Some(first) = rest.first() else {
        return refused("page which page? Try \"page 1\".");
    };
    if rest.len() > 1 {
        return too_much("page", words);
    }
    let Some(page) = whole_number(&first.text) else {
        return refused(format!("\"{}\" is not a page number.", first.raw));
    };
    just(vec![Command::SetExecutorPage { page }])
}

/* -------------------------------------------------------------------------- */
/* Selections and levels                                                      */
/* -------------------------------------------------------------------------- */

/// Everything else: a keyword on its own, a selection, a level, or both.
///
/// `fixture` at the front is optional noise an operator may type out of habit,
/// and is accepted for exactly that reason.
fn selection_line(words: &[Token]) -> ConsoleReading {
    // A bare keyword and a number is the *selecting* form of that word. A
    // leading `+` reaches it too, so `+ Group 3` adds a group's fixtures to what
    // is selected rather than replacing it (S43, B17).
    let plus = words.first().is_some_and(|word| word.text == "+");
    let after_plus = if plus { &words[1..] } else { words };
    if let Some(keyword) = keyword_selection(after_plus, plus) {
        return keyword;
    }
    // **A leading `+` adds to the selection instead of replacing it** — S43,
    // punch-list B17. `+ 5` and `+ fixture 5` are *and this one too*; `5` on its
    // own is still *the selection is 5*.
    //
    // The mode is `Toggle` rather than `Add`, which is what makes the same
    // gesture take a fixture back out again — the sheet's rows write this line.
    let rest = if after_plus
        .first()
        .is_some_and(|word| word.text == "fixture" || word.text == "fixtures")
    {
        &after_plus[1..]
    } else {
        after_plus
    };
    let mut commands = Vec::new();

    // The attribute may sit on either side of `at` — `5 pan at 25` is how a
    // console reads aloud and `5 at pan 25` is how one is often typed — so it is
    // taken out of the line before either half is read.
    let named = attribute_in(rest);
    let without: Vec<Token> = match named {
        Some((ref word, _)) => rest
            .iter()
            .filter(|token| token.text != *word)
            .cloned()
            .collect(),
        None => rest.to_vec(),
    };
    let at = without.iter().position(|word| word.text == "at");

    let selection_words = at.map_or(&without[..], |index| &without[..index]);
    if !selection_words.is_empty() {
        let ids = match read_fixtures(selection_words) {
            Ok(ids) => ids,
            Err(message) => return refused(message),
        };
        commands.push(Command::SelectFixtures {
            ids,
            mode: if plus {
                SelectionMode::Toggle
            } else {
                SelectionMode::Set
            },
        });
    }

    let Some(at) = at else {
        if commands.is_empty() {
            return refused(format!("\"{}\" is not a command.", said(words)));
        }
        return just(commands);
    };

    let attribute = named.map_or(AttributeType::Dimmer, |(_, attribute)| attribute);
    match read_level(&without[at + 1..], attribute) {
        Ok(level) => {
            commands.push(level);
            just(commands)
        }
        Err(message) => refused(message),
    }
}

/// `Group 3`, `Sequence 5`, `View 2`, `Executor 4`, `Preset 1` — the selecting
/// form of an argument keyword.
///
/// Answers `None` when the line is not one of these, which is when it is a
/// fixture selection or a level. A **cue** on its own is deliberately not here:
/// `Cue 5` could be a Goto or an Edit and the desk must not guess, so it says so.
fn keyword_selection(words: &[Token], adding: bool) -> Option<ConsoleReading> {
    let head = words.first()?;
    if head.text == "fixture" || head.text == "fixtures" {
        return None;
    }
    if !OBJECT_WORDS.contains(&head.text.as_str()) {
        return None;
    }
    // `+` means *and this one too*, which only a **selection** can mean. A view,
    // a sequence, an executor or a preset is one thing at a time, so the word is
    // refused there rather than quietly ignored.
    if adding && head.text != "group" {
        return Some(refused(format!(
            "\"+\" adds to a selection, and {} is not one. Try \"{} …\" on its own.",
            head.text, head.raw
        )));
    }
    let read = match read_object(words) {
        Ok(read) => read,
        Err(message) => return Some(refused(message)),
    };
    if !read.rest.is_empty() {
        return Some(too_much(&head.text, words));
    }
    Some(match read.target {
        ObjectRef::Group { group_id } => just(vec![Command::SelectGroup {
            group_id,
            mode: if adding {
                SelectionMode::Toggle
            } else {
                SelectionMode::Set
            },
        }]),
        ObjectRef::Sequence { sequence_id } => just(vec![Command::SelectSequence { sequence_id }]),
        ObjectRef::View { view_id } => just(vec![Command::SelectView { view_id }]),
        ObjectRef::Executor { executor_id } => just(vec![Command::SelectExecutor { executor_id }]),
        ObjectRef::Preset { preset_id } => just(vec![Command::ApplyPreset { preset_id }]),
        // **A cue on its own is not a line.** `Cue 5` could be a Goto or an
        // Edit, and the desk says so rather than guessing — and so does
        // `Sequence 5 Cue 3`, which reads as the same half-finished thought.
        ObjectRef::Cue { .. } => {
            refused("a cue on its own is ambiguous. Try \"goto cue 5\" or \"edit cue 5\".")
        }
    })
}

/// The `at` half of a line: a percentage, for the attribute the line named.
///
/// The attribute defaults to `Dimmer`, because `1 at 50` means intensity on
/// every lighting desk there has ever been.
fn read_level(words: &[Token], attribute: AttributeType) -> Result<Command, String> {
    let Some(first) = words.first() else {
        return Err("at what? A level is a percentage: 0 to 100.".to_owned());
    };
    if words.len() > 1 {
        return Err(format!(
            "\"{}\" is more than a level. A level is a percentage: 0 to 100.",
            said(words)
        ));
    }
    if first.text == "full" {
        return Ok(Command::SetAttribute {
            attribute,
            value: level_from_percent(100.0),
            relative: false,
        });
    }
    if first.text == "out" || first.text == "zero" {
        return Ok(Command::SetAttribute {
            attribute,
            value: 0,
            relative: false,
        });
    }
    let Ok(percent) = first.text.parse::<f64>() else {
        return Err(format!(
            "\"{}\" is not a percentage. A level is 0 to 100, or \"full\".",
            first.raw
        ));
    };
    if !percent.is_finite() {
        return Err(format!(
            "\"{}\" is not a percentage. A level is 0 to 100, or \"full\".",
            first.raw
        ));
    }
    if !(0.0..=100.0).contains(&percent) {
        return Err(format!("{} % is outside 0 to 100.", first.raw));
    }
    Ok(Command::SetAttribute {
        attribute,
        value: level_from_percent(percent),
        relative: false,
    })
}

/// The level a percentage names, **truncated**.
///
/// `at 50` is 32 767 and not 32 768. A percentage names the level at or below
/// it, so a value never rounds up past what was asked for; `at 0` is off and
/// `at 100` is exactly full, which are the two an operator will check. The same
/// arithmetic `ui/src/desk/level.ts` does, and the recording is what says the
/// two agree.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the product is clamped to 0..=65535 before it is truncated"
)]
fn level_from_percent(percent: f64) -> i32 {
    let clamped = percent.clamp(0.0, 100.0);
    (clamped * FULL_LEVEL / 100.0).floor() as i32
}

/// The pool the first of these words names, if it names one.
///
/// Only the **first**: `Store Preset 1 Color Deep blue` names the Colour pool
/// and is called *Deep blue*, and a name with a pool word later in it is a name.
///
/// The table is every pool and not the banks, so **`Multi` is a word an operator
/// can type** — S43. It is the one pool that is not an encoder bank, which is
/// exactly why a line that names no pool still cannot mean it: the fallback is
/// `Session::encoder_bank`, and there is no bank an operator could be standing
/// on that would say *store everything*.
fn pool_in(words: &[Token]) -> Option<PresetPool> {
    let head = words.first()?;
    PresetPool::ALL
        .into_iter()
        .find(|pool| pool_word(*pool) == head.text)
}

/// The attribute one of these words names, if any of them does.
fn attribute_in(words: &[Token]) -> Option<(String, AttributeType)> {
    words.iter().find_map(|word| {
        AttributeType::ALL
            .into_iter()
            .find(|attribute| attribute_word(*attribute) == word.text)
            .map(|attribute| (word.text.clone(), attribute))
    })
}

/// The fixture numbers a selection names.
///
/// Answers a message rather than failing on anything that is not one. A range
/// runs in the direction it is written — `4 thru 1` is 4, 3, 2, 1 — because
/// selection order is what an operator sees when they fan a value across it.
fn read_fixtures(words: &[Token]) -> Result<Vec<FixtureId>, String> {
    let mut ids: Vec<FixtureId> = Vec::new();
    let mut seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut expecting_range = true;
    let mut index = 0usize;
    while index < words.len() {
        let word = &words[index];
        if !expecting_range {
            if word.text != "+" {
                return Err(format!(
                    "\"{}\" is not a fixture. Separate fixtures with + and ranges with thru.",
                    word.raw
                ));
            }
            expecting_range = true;
            index += 1;
            continue;
        }
        let Some(from) = whole_number(&word.text) else {
            return Err(format!("\"{}\" is not a fixture number.", word.raw));
        };
        index += 1;
        if words.get(index).is_some_and(|word| word.text == THRU) {
            let Some(to_word) = words.get(index + 1) else {
                return Err("thru what? A range is two numbers, as in 1 thru 4.".to_owned());
            };
            let Some(to) = whole_number(&to_word.text) else {
                return Err(format!("\"{}\" is not a fixture number.", to_word.raw));
            };
            if from.abs_diff(to) > MAX_RANGE {
                return Err(format!(
                    "{from} thru {to} is more than {MAX_RANGE} fixtures."
                ));
            }
            if to >= from {
                for id in from..=to {
                    push_fixture(&mut ids, &mut seen, id);
                }
            } else {
                for id in (to..=from).rev() {
                    push_fixture(&mut ids, &mut seen, id);
                }
            }
            index += 2;
        } else {
            push_fixture(&mut ids, &mut seen, from);
        }
        expecting_range = false;
    }
    if expecting_range {
        return Err("the line ends with a +, so something is missing after it.".to_owned());
    }
    Ok(ids)
}

/// Appends a fixture number once. A selection holds no fixture twice.
fn push_fixture(ids: &mut Vec<FixtureId>, seen: &mut std::collections::BTreeSet<u32>, id: u32) {
    if seen.insert(id) {
        ids.push(FixtureId::new(id));
    }
}

/// A whole number that is not negative, or `None`.
fn whole_number(word: &str) -> Option<u32> {
    if word.is_empty() || !word.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    word.parse::<u32>().ok()
}

/// Whether a word is a cue number.
///
/// Cue numbers are **strings** on the wire (`ARCHITECTURE_SPEC.md` §6) so that
/// `1.5` sorts between `1` and `2` without a float entering the show file, and
/// this is the same shape read back: digits, with at most one point in them.
fn is_cue_number(word: &str) -> bool {
    let mut halves = word.splitn(2, '.');
    let whole = halves.next().unwrap_or_default();
    if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match halves.next() {
        None => true,
        Some(fraction) => {
            !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
        }
    }
}

/// The name at the end of a line, as it was typed.
///
/// The quotes are already off: [`tokenise`] takes them from a quoted run, and no
/// other token can begin with one — a `"` that is not the first character of a
/// chunk is an ordinary character in it. The TypeScript this was ported from
/// stripped them a second time here, and that second strip could not fire.
fn join_name(words: &[Token]) -> String {
    words
        .iter()
        .map(|word| word.raw.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

/* -------------------------------------------------------------------------- */
/* The tokeniser                                                              */
/* -------------------------------------------------------------------------- */

/// A line as words, each with the spelling the operator used beside it.
///
/// `+`, `,` and `thru` are separators as well as words, so `1+2thru4` reads the
/// same as `1 + 2 thru 4` — a console keypad has digits and a few keys, and an
/// operator should not have to hunt for a space bar. A comma reads as a `+`,
/// because both mean *and also*.
///
/// **A quoted run is one token and keeps its spaces**, which is what makes
/// `Label View 1 "House lights"` a two-word name rather than two words.
fn tokenise(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for chunk in chunks(line) {
        if chunk.starts_with('"') {
            let raw = chunk.strip_prefix('"').unwrap_or(&chunk).to_owned();
            let raw = raw.strip_suffix('"').unwrap_or(&raw).to_owned();
            out.push(Token {
                text: raw.to_lowercase(),
                raw,
            });
            continue;
        }
        // `go+` and `go-` first, or the `+` below would split the first of them
        // into a keyword and a separator. `goback` is the token the second
        // becomes, so the two directions are words rather than punctuation.
        let spaced = replace_ignoring_case(&chunk, "go+", " go ");
        let spaced = replace_ignoring_case(&spaced, "go-", " goback ");
        let spaced = spaced.replace([',', '+'], " + ");
        let spaced = replace_ignoring_case(&spaced, THRU, " thru ");
        for piece in spaced.split_whitespace() {
            out.push(Token {
                text: piece.to_lowercase(),
                raw: piece.to_owned(),
            });
        }
    }
    out
}

/// The chunks a line falls into: a quoted run, or a run of non-space characters.
///
/// A quote only opens a run when it is the **first** character of the run, which
/// is what the regular expression this replaces did: `abc"def"` is one ordinary
/// chunk, and `"abc def"` is one quoted one. An unterminated quote runs to the
/// end of the line, so a name being typed reads as a name.
fn chunks(line: &str) -> Vec<String> {
    let characters: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < characters.len() {
        if characters[index].is_whitespace() {
            index += 1;
            continue;
        }
        let start = index;
        if characters[index] == '"' {
            index += 1;
            while index < characters.len() && characters[index] != '"' {
                index += 1;
            }
            if index < characters.len() {
                index += 1;
            }
        } else {
            while index < characters.len() && !characters[index].is_whitespace() {
                index += 1;
            }
        }
        out.push(characters[start..index].iter().collect());
    }
    out
}

/// `str::replace`, matched without regard to case.
///
/// The tokeniser's rewrites are case-insensitive because the words they rewrite
/// are — `GO+` is a Go — and the replacement is lower case, which is what the
/// token would have been anyway.
///
/// **The fold is ASCII and that is load-bearing**, not a shortcut: every needle
/// here is ASCII, and `str::to_lowercase` can change a string's *length* (`İ` is
/// two bytes and lower-cases to three), so an offset found in a fully folded
/// copy would not be an offset in the original. `to_ascii_lowercase` is
/// length-preserving by construction, which makes every index below exact.
fn replace_ignoring_case(text: &str, needle: &str, replacement: &str) -> String {
    let lowered = text.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut rest = 0usize;
    while let Some(found) = lowered[rest..].find(&needle) {
        let at = rest + found;
        out.push_str(&text[rest..at]);
        out.push_str(replacement);
        rest = at + needle.len();
    }
    out.push_str(&text[rest..]);
    out
}

/* -------------------------------------------------------------------------- */
/* The reading                                                                */
/* -------------------------------------------------------------------------- */

/// What to show under the input for a line as it stands.
///
/// An empty line says nothing, a bad one says what is wrong with it, and a good
/// one says **what it will do** — so an operator can see before Enter that
/// `1 thru 3 at 50` is two commands and which two.
#[must_use]
pub fn reading_text(reading: &ConsoleReading) -> String {
    match reading {
        ConsoleReading::Empty => String::new(),
        ConsoleReading::Refused(message) => message.clone(),
        ConsoleReading::Commands { commands, .. } => commands
            .iter()
            .map(describe)
            .collect::<Vec<_>>()
            .join(" · "),
    }
}

/// One command in words.
///
/// Deliberately not the wire form: `{"t":"SetAttribute"}` is a thing to debug
/// with, not a thing to read at a desk in the dark.
fn describe(command: &Command) -> String {
    match command {
        Command::SelectFixtures { ids, mode } => {
            let list = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" + ");
            if *mode == SelectionMode::Set {
                format!("select {list}")
            } else {
                format!("add {list} to the selection")
            }
        }
        Command::SelectGroup { group_id, mode } => {
            if *mode == SelectionMode::Set {
                format!("select group {group_id}")
            } else {
                format!("add group {group_id} to the selection")
            }
        }
        Command::ApplyPreset { preset_id } => format!("apply preset {preset_id}"),
        Command::SetAttribute {
            attribute, value, ..
        } => format!(
            "{} → {}%",
            attribute_word(*attribute),
            round_to_tenth(f64::from(*value) / FULL_LEVEL * 1000.0)
        ),
        Command::ClearProgrammer => "clear".to_owned(),
        Command::Update => "update the cue being edited".to_owned(),
        Command::Oops => "oops".to_owned(),
        Command::StoreCue {
            sequence_id,
            cue_number,
            ..
        } => format!("store cue {cue_number}{}", sequence_suffix(*sequence_id)),
        Command::StoreSequence { sequence_id, .. } => format!("store sequence {sequence_id}"),
        Command::StoreGroup { group_id, .. } => format!("store group {group_id}"),
        Command::StorePreset { preset_id, .. } => format!("store preset {preset_id}"),
        Command::StoreView { view_id, .. } => format!("store view {view_id}"),
        Command::NewView { view_id, .. } => format!("new, empty view {view_id}"),
        Command::EditCue {
            sequence_id,
            cue_number,
        } => format!("edit cue {cue_number}{}", sequence_suffix(*sequence_id)),
        Command::Goto { target, cue_number } => format!("goto cue {cue_number} on {target}"),
        Command::Delete { target } => format!("delete {}", object_text(target)),
        Command::Copy { from, to, .. } => {
            format!("copy {} to {}", object_text(from), object_text(to))
        }
        Command::Move { from, to, .. } => {
            format!("move {} to {}", object_text(from), object_text(to))
        }
        Command::Label { target, name } => {
            format!("label {} {}", object_text(target), quoted(name))
        }
        Command::Color { target, color } => color.map_or_else(
            || format!("take the colour off {}", object_text(target)),
            |color| format!("colour {} {}", object_text(target), hex_of(color)),
        ),
        Command::AssignExecutor {
            executor_id,
            sequence_id,
        } => format!(
            "assign {} to executor {executor_id}",
            sequence_id.map_or_else(
                || "nothing".to_owned(),
                |sequence_id| format!("sequence {sequence_id}")
            )
        ),
        Command::ConfigureExecutor {
            executor_id,
            change,
        } => format!("set the {} of executor {executor_id}", control_text(change)),
        Command::ExecutorGo { target, direction } => format!(
            "{} on {target}",
            if *direction == GoDirection::Next {
                "go"
            } else {
                "back"
            }
        ),
        Command::ExecutorOff { target } => format!("off on {target}"),
        Command::ExecutorOn { target } => format!("on {target}"),
        Command::SelectSequence { sequence_id } => format!("select sequence {sequence_id}"),
        Command::SelectView { view_id } => format!("view {view_id}"),
        Command::SelectExecutor { executor_id } => format!("select executor {executor_id}"),
        Command::SetExecutorPage { page } => format!("page {page}"),
        // Every command this parser can produce is named above. The arm exists
        // because `Command` is the whole protocol, and a readout is not where a
        // command added to it should become a compile error.
        other => tag_of(other),
    }
}

/// One control of an executor and what it is being given, in words — S45.
fn control_text(change: &ExecutorChange) -> String {
    match change {
        ExecutorChange::Fader { function } => format!("fader to {}", fader_name(*function)),
        ExecutorChange::Encoder { function } => format!("encoder to {}", encoder_name(*function)),
        ExecutorChange::Button { index, function } => {
            // Counted from one, which is how an operator counts the keys under a
            // fader; the command counts from zero the way the hardware does.
            let key = format!("button {}", u16::from(*index) + 1);
            match function {
                ExecutorButtonFunction::CommandLine { line } => {
                    format!("{key} to send {}", quoted(line))
                }
                fixed => format!("{key} to {}", button_name(fixed)),
            }
        }
    }
}

/// A string in double quotes, escaped the way JSON escapes one.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| format!("\"{text}\""))
}

/// A number rounded to one decimal place, printed the shortest way that reads.
fn round_to_tenth(value: f64) -> f64 {
    value.round() / 10.0
}

/// The tag a command carries on the wire, for the readout's last arm.
fn tag_of(command: &Command) -> String {
    serde_json::to_value(command)
        .ok()
        .and_then(|value| {
            value
                .get("t")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default()
}

/* -------------------------------------------------------------------------- */
/* The words of the generated enums                                           */
/* -------------------------------------------------------------------------- */

/// An attribute as the line spells it, which is its own name lower-cased.
fn attribute_word(attribute: AttributeType) -> String {
    attribute_name(attribute).to_lowercase()
}

/// An attribute's name, as `prism_domain` declares it.
const fn attribute_name(attribute: AttributeType) -> &'static str {
    match attribute {
        AttributeType::Dimmer => "Dimmer",
        AttributeType::Pan => "Pan",
        AttributeType::Tilt => "Tilt",
        AttributeType::Red => "Red",
        AttributeType::Green => "Green",
        AttributeType::Blue => "Blue",
        AttributeType::White => "White",
        AttributeType::Amber => "Amber",
        AttributeType::Iris => "Iris",
        AttributeType::Zoom => "Zoom",
        AttributeType::Focus => "Focus",
        AttributeType::Gobo => "Gobo",
        AttributeType::Prism => "Prism",
        AttributeType::Shutter => "Shutter",
        AttributeType::Control => "Control",
    }
}

/// A preset pool as the line spells it.
fn pool_word(pool: PresetPool) -> String {
    pool_name(pool).to_lowercase()
}

/// A preset pool's name, as `prism_domain` declares it.
const fn pool_name(pool: PresetPool) -> &'static str {
    match pool {
        PresetPool::Dimmer => "Dimmer",
        PresetPool::Position => "Position",
        PresetPool::Gobo => "Gobo",
        PresetPool::Color => "Color",
        PresetPool::Beam => "Beam",
        PresetPool::Focus => "Focus",
        PresetPool::Control => "Control",
        PresetPool::Multi => "Multi",
    }
}

/// A fader function's spelling, as `prism_domain` declares it.
const fn fader_word(function: ExecutorFaderFunction) -> &'static str {
    match function {
        ExecutorFaderFunction::Empty => "Empty",
        ExecutorFaderFunction::Master => "Master",
        ExecutorFaderFunction::Speed => "Speed",
        ExecutorFaderFunction::XFade => "XFade",
    }
}

/// An encoder function's spelling.
const fn encoder_word(function: ExecutorEncoderFunction) -> &'static str {
    match function {
        ExecutorEncoderFunction::Empty => "Empty",
        ExecutorEncoderFunction::Master => "Master",
        ExecutorEncoderFunction::Speed => "Speed",
    }
}

/// A button function's spelling — the eight fixed ones.
fn button_word(function: &ExecutorButtonFunction) -> String {
    match function {
        ExecutorButtonFunction::Empty => "Empty",
        ExecutorButtonFunction::GoForward => "Go+",
        ExecutorButtonFunction::GoBack => "Go-",
        ExecutorButtonFunction::LearnSpeed => "LearnSpeed",
        ExecutorButtonFunction::Off => "Off",
        ExecutorButtonFunction::On => "On",
        ExecutorButtonFunction::Flash => "Flash",
        ExecutorButtonFunction::Toggle => "Toggle",
        ExecutorButtonFunction::CommandLine { .. } => "Command",
    }
    .to_owned()
}

/// The eight fixed button functions, in the order a chooser offers them.
fn fixed_button_functions() -> Vec<ExecutorButtonFunction> {
    vec![
        ExecutorButtonFunction::Empty,
        ExecutorButtonFunction::GoForward,
        ExecutorButtonFunction::GoBack,
        ExecutorButtonFunction::LearnSpeed,
        ExecutorButtonFunction::Off,
        ExecutorButtonFunction::On,
        ExecutorButtonFunction::Flash,
        ExecutorButtonFunction::Toggle,
    ]
}

/// The spellings a fader takes, for a complaint that lists them.
fn fader_words() -> Vec<String> {
    ExecutorFaderFunction::ALL
        .into_iter()
        .map(|function| fader_word(function).to_owned())
        .collect()
}

/// The spellings an encoder takes.
fn encoder_words() -> Vec<String> {
    ExecutorEncoderFunction::ALL
        .into_iter()
        .map(|function| encoder_word(function).to_owned())
        .collect()
}

/// The spellings a button takes.
fn button_words() -> Vec<String> {
    fixed_button_functions().iter().map(button_word).collect()
}

/// What a fader function is called in a sentence an operator reads.
const fn fader_name(function: ExecutorFaderFunction) -> &'static str {
    match function {
        ExecutorFaderFunction::Empty => "Nothing",
        ExecutorFaderFunction::Master => "Master",
        ExecutorFaderFunction::Speed => "Speed",
        ExecutorFaderFunction::XFade => "Crossfade",
    }
}

/// What an encoder function is called.
const fn encoder_name(function: ExecutorEncoderFunction) -> &'static str {
    match function {
        ExecutorEncoderFunction::Empty => "Nothing",
        ExecutorEncoderFunction::Master => "Master",
        ExecutorEncoderFunction::Speed => "Speed",
    }
}

/// What a button function is called.
const fn button_name(function: &ExecutorButtonFunction) -> &'static str {
    match function {
        ExecutorButtonFunction::Empty => "Nothing",
        ExecutorButtonFunction::GoForward => "Go forward",
        ExecutorButtonFunction::GoBack => "Go back",
        ExecutorButtonFunction::LearnSpeed => "Learn speed",
        ExecutorButtonFunction::Off => "Off",
        ExecutorButtonFunction::On => "On",
        ExecutorButtonFunction::Flash => "Flash",
        ExecutorButtonFunction::Toggle => "Toggle",
        ExecutorButtonFunction::CommandLine { .. } => "Command line",
    }
}

/* -------------------------------------------------------------------------- */
/* The mode an operator chose                                                 */
/* -------------------------------------------------------------------------- */

/// Puts the operator's chosen mode into the command that was waiting for one.
///
/// **The choice travels in the command** — S28's rule, S39's implementation, and
/// the reason this exists rather than the daemon guessing an outcome the
/// operator did not ask for. Exactly one command of a line carries a mode, and a
/// mode that does not belong to the command it meets is **left alone** rather
/// than forced in: the question is built from the command, so the case cannot
/// arise from an interface, and this is what stops it arising from anywhere else.
#[must_use]
pub fn apply_mode(commands: Vec<Command>, mode: CommandLineMode) -> Vec<Command> {
    commands
        .into_iter()
        .map(|command| match command {
            Command::StoreCue {
                sequence_id,
                cue_number,
                mode: held,
            } => Command::StoreCue {
                sequence_id,
                cue_number,
                mode: store_mode(mode).unwrap_or(held),
            },
            Command::StorePreset {
                preset_id,
                pool,
                name,
                color,
                mode: held,
            } => Command::StorePreset {
                preset_id,
                pool,
                name,
                color,
                mode: store_mode(mode).unwrap_or(held),
            },
            Command::StoreSequence {
                sequence_id,
                name,
                mode: held,
            } => Command::StoreSequence {
                sequence_id,
                name,
                mode: sequence_store_mode(mode).unwrap_or(held),
            },
            Command::StoreGroup {
                group_id,
                name,
                mode: held,
            } => Command::StoreGroup {
                group_id,
                name,
                mode: overwrite_mode(mode).unwrap_or(held),
            },
            Command::Copy {
                from,
                to,
                mode: held,
            } => Command::Copy {
                from,
                to,
                mode: overwrite_mode(mode).unwrap_or(held),
            },
            Command::Move {
                from,
                to,
                mode: held,
            } => Command::Move {
                from,
                to,
                mode: overwrite_mode(mode).unwrap_or(held),
            },
            other => other,
        })
        .collect()
}

/// The cue-and-preset store mode a chosen word names, if it names one.
const fn store_mode(mode: CommandLineMode) -> Option<StoreMode> {
    match mode {
        CommandLineMode::Merge => Some(StoreMode::Merge),
        CommandLineMode::Override => Some(StoreMode::Override),
        CommandLineMode::Remove => Some(StoreMode::Remove),
        CommandLineMode::Append => None,
    }
}

/// The sequence store mode a chosen word names, if it names one.
const fn sequence_store_mode(mode: CommandLineMode) -> Option<SequenceStoreMode> {
    match mode {
        CommandLineMode::Append => Some(SequenceStoreMode::Append),
        CommandLineMode::Override => Some(SequenceStoreMode::Override),
        CommandLineMode::Merge => Some(SequenceStoreMode::Merge),
        CommandLineMode::Remove => None,
    }
}

/// The copy-and-move mode a chosen word names, if it names one.
const fn overwrite_mode(mode: CommandLineMode) -> Option<OverwriteMode> {
    match mode {
        CommandLineMode::Merge => Some(OverwriteMode::Merge),
        CommandLineMode::Override => Some(OverwriteMode::Override),
        CommandLineMode::Remove | CommandLineMode::Append => None,
    }
}

/* -------------------------------------------------------------------------- */
/* Completion                                                                 */
/* -------------------------------------------------------------------------- */

/// The words that are legal **at this point in the line**, longest prefix first.
///
/// It is deliberately a *grammar* answer and not a *show* answer — it offers the
/// word `sequence`, never the sequences there are, for the same reason the
/// parser does not read the show. A list of what exists is what the pools on the
/// canvas are for.
///
/// **They come back capitalised** — S43, punch-list B14. `Fixture`, not
/// `fixture`. The grammar's own table is lower case because the parser
/// lower-cases everything it reads; what a *person* is shown is a word, and
/// every other word this desk shows an operator starts with a capital.
#[must_use]
pub fn completions(line: &str) -> Vec<String> {
    let words = tokenise(line);
    let trailing = line.chars().next_back().is_some_and(char::is_whitespace);
    let typed = if trailing {
        String::new()
    } else {
        words
            .last()
            .map(|word| word.text.clone())
            .unwrap_or_default()
    };
    let before = if trailing {
        &words[..]
    } else {
        &words[..words.len().saturating_sub(1)]
    };
    legal_words(before)
        .into_iter()
        .filter(|word| word.starts_with(&typed) && *word != typed)
        .map(capitalised)
        .collect()
}

/// A word as an operator is shown it: first letter upper case, rest untouched.
fn capitalised(word: &str) -> String {
    let mut characters = word.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// Which words may come next, given what is already there.
fn legal_words(before: &[Token]) -> Vec<&'static str> {
    let Some(head) = before.first() else {
        return CONSOLE_WORDS.to_vec();
    };
    match head.text.as_str() {
        // The one verb whose **last** argument is a word rather than a thing. A
        // reference is two tokens, so anything from the fourth word on is the
        // colour — `color sequence 4 ‹here›`.
        "color" => {
            if before.len() >= 3 {
                color_words()
            } else {
                OBJECT_WORDS.to_vec()
            }
        }
        // The one verb that takes a *view* and nothing else — see `new_line`.
        "new" => vec!["view"],
        // After a verb, and after each of its arguments, the words that name a
        // thing. A number is not offered: there is nothing to complete about one.
        "store" | "delete" | "move" | "copy" | "label" | "goto" | "edit" | "assign" | "on"
        | "off" | "go" | "goback" => OBJECT_WORDS.to_vec(),
        text => {
            if before.len() == 1 && (text == "at" || whole_number(text).is_some()) {
                vec!["at", "thru", "full"]
            } else {
                Vec::new()
            }
        }
    }
}
