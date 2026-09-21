//! The console keypad: the words a key writes, and what writing one means.
//!
//! `ARCHITECTURE_SPEC.md` §4.5 is the design and it is older than this module —
//! *a key on the desk writes a word into the command line. It does not act.*
//! What is new here is only **where the list lives**.
//!
//! # Why this is in Rust and not where it was
//!
//! It was `ui/src/desk/keys.ts`, a hand-written table beside the window that
//! draws it, and that was right while the keypad was the only thing that had
//! one. S59 gives the **surface** the same vocabulary: a key on an X-Touch may
//! be bound to a word, and pressing it must do what pressing the same word on
//! screen does. Two hand-written tables would make that a promise rather than a
//! fact, and the failure would be quiet — a word added to the screen and not to
//! the desk is not a compile error anywhere, it is an operator finding that the
//! key they bound last week does nothing.
//!
//! So the table is here, and [`crate::export_bindings`] writes it into
//! `ui/src/bindings/variants.ts` beside `FEATURE_GROUP_ATTRIBUTES` — which is
//! this project's rule about what to generate, recorded in `PROGRESS.md` §7:
//! *what is generated is the thing that would actually drift*. `keys.ts` keeps
//! the English titles, because a title is prose about an interface and not a
//! fact about the grammar; it reads the words and the shapes from here.
//!
//! # What this module deliberately does not know
//!
//! Whether a word is a word the line can read. That is
//! `prism_core::console`'s, this crate has no grammar in it, and the two are
//! held together by a test over there rather than by a dependency that would
//! point the wrong way — `prism-core` reads `prism-domain`, never the reverse.
//! `a_console_key_writes_a_word_the_line_knows` is the assertion, and it is the
//! reason a word cannot be added here that nothing can parse.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which of `ARCHITECTURE_SPEC.md` §4.5's shapes a key is.
///
/// The spellings are lower case on purpose: they are what `keys.ts` has called
/// them since S43, and a generated table that renamed them would be a rewrite of
/// working interface code to no end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum KeyShape {
    /// A whole command with no argument — `Clear`, `Update`, `Full`. Writes the
    /// word and **runs it at once**.
    Run,
    /// The Oops key, and the only shape that writes nothing at all (B58): while
    /// a line stands it takes the line's last word, and only an empty line lets
    /// it undo an edit.
    ///
    /// **The daemon makes that choice**, not the key and not the client, which
    /// is what lets the X-Touch's Undo key behave the same way with no screen
    /// attached to ask.
    Oops,
    /// A command that needs arguments — `Store`, `Edit`, `Label`. Writes the
    /// word and **waits**; the line is finished on the screen, by Enter or by a
    /// click on a list that supplies the argument and submits with it.
    Write,
    /// An argument keyword — `Fixture`, `Cue`, `Thru`. **Appends** the word to
    /// the line as it stands rather than starting a new one.
    Append,
}

/// Every [`KeyShape`], in the order this module declares them.
///
/// `ts-rs` writes the union in declaration order, so a spelling is found by
/// position — the same correspondence `AttributeType::ALL` carries for the
/// encoder banks, and the reason a `rename_all` cannot make the generated table
/// disagree with this one.
pub(crate) const KEY_SHAPES: [KeyShape; 4] = [
    KeyShape::Run,
    KeyShape::Oops,
    KeyShape::Write,
    KeyShape::Append,
];

/// One key of the keypad: the word it writes, and which shape it is.
///
/// Neither `Deserialize` nor `Arbitrary`, and both for the same reason: the word
/// is a `&'static str` because this table is a constant and never arrives from
/// anywhere. A `ConsoleKey` is looked up by name ([`ConsoleKey::named`]), not
/// decoded — what travels on the wire is the word, inside
/// [`crate::SurfaceAction::ConsoleWord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleKey {
    /// The word, spelled as an operator reads it on the key.
    ///
    /// Written with a capital because that is what is printed on the key; the
    /// line itself does not care, `prism_core::console` lower-cases what it
    /// tokenises.
    pub word: &'static str,
    /// Which shape — see [`KeyShape`].
    pub shape: KeyShape,
}

/// The keypad, in the order a console has it.
///
/// `Clear` is first and is also drawn in the header, because it is the one key
/// an operator reaches for without looking and the `CommandKeys` window can be
/// closed. It is the same word and the same gesture in both places — and, since
/// S59, on the desk as well.
///
/// # The four that S59 added
///
/// `Color` was in §4.5's table from the beginning and never on the keypad,
/// which is a gap rather than a decision and is closed here. `New`, `At` and
/// `Thru` are the owner's, asked for so that the desk can reach the whole
/// grammar: the first writes and waits like the other verbs, the other two are
/// argument keywords and append.
pub const CONSOLE_KEYS: [ConsoleKey; 23] = [
    ConsoleKey {
        word: "Clear",
        shape: KeyShape::Run,
    },
    ConsoleKey {
        word: "Full",
        shape: KeyShape::Run,
    },
    ConsoleKey {
        word: "Update",
        shape: KeyShape::Run,
    },
    ConsoleKey {
        word: "Oops",
        shape: KeyShape::Oops,
    },
    ConsoleKey {
        word: "Store",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Edit",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Goto",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Move",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Copy",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Delete",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Label",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Color",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Assign",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "New",
        shape: KeyShape::Write,
    },
    ConsoleKey {
        word: "Fixture",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "Group",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "Sequence",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "Cue",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "Preset",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "View",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "Executor",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "At",
        shape: KeyShape::Append,
    },
    ConsoleKey {
        word: "Thru",
        shape: KeyShape::Append,
    },
];

/// A word added to the line as it stands.
///
/// One space between words and one at the end, so the operator's next keystroke
/// is a number rather than a correction. An empty line does not gain a leading
/// space, because `Fixture 1` and ` Fixture 1` are the same line and only one of
/// them reads well.
///
/// **The twin of `ui/src/desk/consoleshell.ts`'s `appended`**, and deliberately
/// a copy of three lines rather than a round trip: a key on the screen appends
/// without asking anybody, and making it ask would put a network hop inside a
/// keystroke to save nothing. What must not drift is the *vocabulary*, and that
/// is why [`CONSOLE_KEYS`] is generated and this is not.
#[must_use]
pub fn appended(line: &str, word: &str) -> String {
    let head = line.trim_end();
    if head.is_empty() {
        format!("{word} ")
    } else {
        format!("{head} {word} ")
    }
}

impl ConsoleKey {
    /// The line this key makes of the line as it stands, and whether to run it.
    ///
    /// §4.5's three writing shapes in one place, so the desk and the screen
    /// cannot disagree about what a key does:
    ///
    /// - **run** replaces the line with the word and runs it — `Clear`, `Full`,
    ///   `Update` are whole commands and there is nothing to wait for;
    /// - **write** replaces the line with the word and a space, and waits;
    /// - **append** adds the word to what is standing, which is the one shape
    ///   that reads the line it is given.
    ///
    /// `None` for [`KeyShape::Oops`], which writes no line at all: it sends
    /// `Command::Oops` and the daemon decides whether that takes a word off the
    /// line or an edit off the journal (B58).
    #[must_use]
    pub fn line_for(&self, standing: &str) -> Option<(String, bool)> {
        match self.shape {
            KeyShape::Run => Some((self.word.to_owned(), true)),
            KeyShape::Write => Some((format!("{} ", self.word), false)),
            KeyShape::Append => Some((appended(standing, self.word), false)),
            KeyShape::Oops => None,
        }
    }

    /// The key that writes this word, matched without regard to case.
    ///
    /// The door a binding comes through: a profile file names a word as text,
    /// and a word no key writes is refused rather than bound to nothing. Case
    /// is ignored because a profile is written by hand and `store` is the same
    /// key as `Store`.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        CONSOLE_KEYS
            .into_iter()
            .find(|key| key.word.eq_ignore_ascii_case(word))
    }
}

#[cfg(test)]
mod tests {
    use super::{CONSOLE_KEYS, ConsoleKey, KeyShape};

    #[test]
    fn every_word_appears_once() {
        for key in CONSOLE_KEYS {
            let same: Vec<_> = CONSOLE_KEYS
                .iter()
                .filter(|other| other.word.eq_ignore_ascii_case(key.word))
                .collect();
            assert_eq!(same.len(), 1, "{} is on the keypad twice", key.word);
        }
    }

    #[test]
    fn a_word_is_found_whatever_its_case() {
        assert_eq!(
            ConsoleKey::named("STORE").map(|key| key.shape),
            Some(KeyShape::Write)
        );
        assert_eq!(
            ConsoleKey::named("thru").map(|key| key.shape),
            Some(KeyShape::Append)
        );
        assert_eq!(ConsoleKey::named("enter"), None);
    }

    /// The table the generator walks by position has to *be* the declaration
    /// order, or every generated shape would be the wrong one.
    #[test]
    fn the_shapes_are_listed_in_declaration_order() {
        assert_eq!(
            super::KEY_SHAPES,
            [
                KeyShape::Run,
                KeyShape::Oops,
                KeyShape::Write,
                KeyShape::Append
            ]
        );
        for key in CONSOLE_KEYS {
            assert!(super::KEY_SHAPES.contains(&key.shape));
        }
    }

    #[test]
    fn a_word_joins_the_line_with_one_space_and_leaves_one() {
        assert_eq!(super::appended("", "Fixture"), "Fixture ");
        assert_eq!(super::appended("Store ", "Cue"), "Store Cue ");
        assert_eq!(super::appended("1 thru 5", "At"), "1 thru 5 At ");
    }

    /// The three writing shapes, against a line that is already standing — which
    /// is the case that tells them apart. A *run* and a *write* key throw the
    /// standing line away because both are starting a command; only *append*
    /// builds on it.
    #[test]
    fn each_shape_makes_its_own_line() {
        let standing = "Store ";
        let line = |word: &str| ConsoleKey::named(word).and_then(|key| key.line_for(standing));
        assert_eq!(line("Clear"), Some(("Clear".to_owned(), true)));
        assert_eq!(line("Store"), Some(("Store ".to_owned(), false)));
        assert_eq!(line("Cue"), Some(("Store Cue ".to_owned(), false)));
        assert_eq!(line("Oops"), None);
    }

    /// The one key of its shape, and the reason to say so out loud: [`KeyShape::Oops`]
    /// exists because *one* key writes nothing, and a second one would mean the
    /// daemon's choice between *take a word* and *undo an edit* had two callers
    /// that could disagree about it.
    #[test]
    fn oops_is_the_only_key_that_writes_nothing() {
        let writing_nothing: Vec<_> = CONSOLE_KEYS
            .iter()
            .filter(|key| key.shape == KeyShape::Oops)
            .map(|key| key.word)
            .collect();
        assert_eq!(writing_nothing, vec!["Oops"]);
    }
}
