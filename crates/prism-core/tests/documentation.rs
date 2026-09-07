//! The manuals, held to the code — S41.
//!
//! # Why a documentation test lives in `prism-core`
//!
//! Because this is the crate that owns both lists a manual has to match: the
//! console grammar directly (`prism_core::console::CONSOLE_WORDS`) and the
//! window types through `prism-domain`, which it already depends on. It is also
//! compiled by **every** CI job that runs tests, where `prism-app` — the other
//! candidate, which already carries the workspace-shaped `version.rs` — is
//! compiled on Windows alone.
//!
//! # What it does, and why it writes rather than only complains
//!
//! S41 had a choice: *generate the chapter*, or *check that every variant
//! appears somewhere*. The second is cheaper and weaker — it passes for a
//! manual that names a window in a footnote — so this is the first, in the shape
//! that keeps the manual a **reviewable file in the repository** rather than
//! build output.
//!
//! Each list lives between markers in `docs/manual/operator.de.md`:
//!
//! ```text
//! <!-- generated:window-types -->
//! | Fenster | Wofür |
//! |---|---|
//! | `FixtureSheet` | … |
//! <!-- /generated -->
//! ```
//!
//! The test rebuilds the block from the code, **keeping the prose already
//! written against each row** and inserting `TODO` for a row that is new. If the
//! result differs from the file it *writes the file* and fails — so the next
//! `cargo test` shows a diff in `git status` rather than a sentence in a log —
//! and it fails again while any `TODO` is left, because a row nobody has
//! described is not a documented one.
//!
//! It is the same idea as `prism_domain::export_bindings`: the generated thing
//! is written by the test suite, so it cannot quietly go stale. The difference
//! is that these blocks are committed, because a manual is read by people who do
//! not run `cargo test`.
//!
//! # And the READMEs
//!
//! `ARCHITECTURE_SPEC.md` §10.1's rules are **per crate** and lived only in the
//! specification. Every workspace member now says which side of them it is on in
//! its own `README.md` — and says it in one sentence with a link, rather than
//! copying the rule, because a second copy of a rule is the copy that goes out
//! of date. What is checked here is that the file exists and has the four
//! headings a crate README is supposed to have.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use prism_core::console::{CONSOLE_WORDS, VERB_WORDS};
use prism_domain::WindowType;

/* -------------------------------------------------------------------------- */
/* Where things are                                                           */
/* -------------------------------------------------------------------------- */

/// The repository root, from this crate's directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate is two levels under the repository root")
        .to_path_buf()
}

/// The operator's manual, which is the one holding both generated blocks.
fn operator_manual() -> PathBuf {
    root().join("docs/manual/operator.de.md")
}

/// Reads a file that the repository is supposed to contain.
///
/// **Line endings are normalised**, and that is not tidiness: a Windows runner
/// checks this tree out with CRLF unless somebody has set `core.autocrlf`, and a
/// generated block compared byte for byte against one built with `\n` would
/// differ on every line. The test would then rewrite the manual and fail on
/// Windows and nowhere else — a red build that says *the manual is stale* about
/// a manual that is not.
fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} could not be read: {error}", path.display()))
        .replace("\r\n", "\n")
}

/* -------------------------------------------------------------------------- */
/* The generated blocks                                                       */
/* -------------------------------------------------------------------------- */

/// One row of a generated table: the key it is filed under, and the prose.
struct Row {
    key: String,
    prose: String,
}

/// The prose a new row gets, and the word this test refuses to leave in a
/// manual.
const TODO: &str = "TODO";

/// Where a block starts and ends, given its name.
fn markers(name: &str) -> (String, String) {
    (
        format!("<!-- generated:{name} -->"),
        "<!-- /generated -->".to_owned(),
    )
}

/// The rows currently written in a block, by key.
///
/// A row is `| `key` | prose |`. Anything that is not one — the header and the
/// separator — is skipped, so the header is the generator's and never the
/// file's.
fn rows_in(block: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let Some((key, prose)) = rest.split_once("` | ") else {
            continue;
        };
        rows.push(Row {
            key: key.to_owned(),
            prose: prose.trim_end_matches('|').trim().to_owned(),
        });
    }
    rows
}

/// The block between the markers, or a panic naming the marker that is missing.
fn block_of<'a>(text: &'a str, name: &str) -> &'a str {
    let (open, close) = markers(name);
    let start = text
        .find(&open)
        .unwrap_or_else(|| panic!("docs/manual/operator.de.md has no {open}"))
        + open.len();
    let end = text[start..]
        .find(&close)
        .unwrap_or_else(|| panic!("the {open} block is never closed"))
        + start;
    &text[start..end]
}

/// Rebuilds a block from `keys`, keeping whatever prose the file already has.
fn rebuilt(existing: &[Row], header: (&str, &str), keys: &[String]) -> String {
    let mut out = String::from("\n");
    let _ = writeln!(out, "| {} | {} |", header.0, header.1);
    out.push_str("|---|---|\n");
    for key in keys {
        let prose = existing
            .iter()
            .find(|row| &row.key == key)
            .map_or(TODO, |row| row.prose.as_str());
        let _ = writeln!(out, "| `{key}` | {prose} |");
    }
    out
}

/// Replaces a block, writes the file back, and answers whether anything moved.
fn regenerate(path: &Path, name: &str, header: (&str, &str), keys: &[String]) -> bool {
    let text = read(path);
    let block = block_of(&text, name);
    let wanted = rebuilt(&rows_in(block), header, keys);
    if block == wanted {
        return false;
    }
    let (open, close) = markers(name);
    let start = text.find(&open).expect("the marker was just found") + open.len();
    let end = text[start..].find(&close).expect("the block closes") + start;
    let mut updated = String::with_capacity(text.len() + wanted.len());
    updated.push_str(&text[..start]);
    updated.push_str(&wanted);
    updated.push_str(&text[end..]);
    std::fs::write(path, updated).expect("the manual is writable");
    true
}

/* -------------------------------------------------------------------------- */
/* The window types                                                           */
/* -------------------------------------------------------------------------- */

/// The manual's window table is `WindowType::ALL`, in that order.
///
/// The order is the enum's rather than alphabetical, so a variant appended in
/// Rust is appended here — which is what makes the diff this test produces one
/// line rather than fourteen.
#[test]
fn the_window_chapter_is_every_window_type_there_is() {
    let keys: Vec<String> = WindowType::ALL
        .iter()
        .map(|window| format!("{window:?}"))
        .collect();
    let manual = operator_manual();
    let rewritten = regenerate(&manual, "window-types", ("Fenster", "Wofür"), &keys);
    assert!(
        !rewritten,
        "docs/manual/operator.de.md's window table did not match WindowType::ALL and has been \
         rewritten — read the diff, fill in any TODO, and commit it"
    );
    let text = read(&manual);
    let block = block_of(&text, "window-types");
    for row in rows_in(block) {
        assert_ne!(
            row.prose, TODO,
            "the window type {} has no description in docs/manual/operator.de.md",
            row.key
        );
    }
}

/// Every window type has a section of its own, not only a table row.
///
/// The exit criterion is that the manual **covers** every `WindowType`, and one
/// cell of a table is a mention rather than a chapter. The heading carries the
/// variant name in backticks so this can be checked without knowing how the
/// German title was written.
#[test]
fn every_window_type_has_a_section_in_the_operators_manual() {
    let text = read(&operator_manual());
    for window in WindowType::ALL {
        let needle = format!("`{window:?}`");
        let sections = text
            .lines()
            .filter(|line| line.starts_with("### ") && line.contains(&needle))
            .count();
        assert_eq!(
            sections, 1,
            "docs/manual/operator.de.md should have exactly one `### …{needle}` section, it has \
             {sections}"
        );
    }
}

/* -------------------------------------------------------------------------- */
/* The console words                                                          */
/* -------------------------------------------------------------------------- */

/// The manual's word list is `CONSOLE_WORDS`, sorted.
///
/// Sorted rather than in the constant's order, because the constant is a
/// completion table and a reader looking a word up wants it where the alphabet
/// puts it. Sorting is the generator's, so it is deterministic either way.
#[test]
fn the_console_chapter_is_every_word_the_line_knows() {
    let mut keys: Vec<String> = CONSOLE_WORDS
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
    keys.sort_unstable();
    let manual = operator_manual();
    let rewritten = regenerate(&manual, "console-words", ("Wort", "Was es tut"), &keys);
    assert!(
        !rewritten,
        "docs/manual/operator.de.md's word table did not match prism_core::console::CONSOLE_WORDS \
         and has been rewritten — read the diff, fill in any TODO, and commit it"
    );
    let text = read(&manual);
    let block = block_of(&text, "console-words");
    for row in rows_in(block) {
        assert_ne!(
            row.prose, TODO,
            "the console word {} has no description in docs/manual/operator.de.md",
            row.key
        );
    }
}

/// Every **verb** is a word the manual documents, and `goback` is the one
/// exception, in writing.
///
/// `VERB_WORDS` and `CONSOLE_WORDS` are two lists and the second is the one an
/// operator types. They differ by exactly one entry: `goback` is what the
/// tokeniser rewrites `go-` to, so that `+` and `-` can be separators
/// elsewhere on the line. It is not a word anybody types, and the manual
/// documents `go-`.
///
/// Stated here rather than assumed, because a verb added to the grammar and not
/// to the completion table would otherwise reach neither the manual nor the
/// suggestions under the input.
#[test]
fn the_only_verb_the_manual_does_not_document_is_the_tokenisers_own() {
    let documented: Vec<&str> = CONSOLE_WORDS.to_vec();
    let missing: Vec<&str> = VERB_WORDS
        .iter()
        .copied()
        .filter(|word| !documented.contains(word))
        .collect();
    assert_eq!(
        missing,
        vec!["goback"],
        "a verb the operator's manual cannot document has appeared, or `goback` has gone"
    );
    assert!(
        documented.contains(&"go-"),
        "`go-` is the word `goback` stands for and the manual documents it"
    );
}

/* -------------------------------------------------------------------------- */
/* A README in every crate                                                    */
/* -------------------------------------------------------------------------- */

/// Every workspace member, read off the root manifest rather than listed here.
///
/// `members` is a list of paths and globs — `crates/*` and `web` today — so the
/// members are whatever that expands to. Reading the manifest is what makes a
/// crate added tomorrow a crate this test knows about; a hand-written list here
/// would be the second list, and the one that is wrong.
fn workspace_members() -> Vec<PathBuf> {
    let manifest = read(&root().join("Cargo.toml"));
    let list = manifest
        .split_once("members = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(list, _)| list)
        .expect("the workspace manifest has a members list");
    let mut members: Vec<PathBuf> = Vec::new();
    for entry in list.split(',') {
        let entry = entry.trim().trim_matches('"');
        if entry.is_empty() {
            continue;
        }
        if let Some(parent) = entry.strip_suffix("/*") {
            let directory = root().join(parent);
            let mut expanded: Vec<PathBuf> = std::fs::read_dir(&directory)
                .unwrap_or_else(|error| {
                    panic!("{} could not be read: {error}", directory.display())
                })
                .filter_map(Result::ok)
                .map(|found| found.path())
                .filter(|path| path.join("Cargo.toml").is_file())
                .collect();
            expanded.sort();
            members.append(&mut expanded);
        } else {
            let path = root().join(entry);
            assert!(
                path.join("Cargo.toml").is_file(),
                "the workspace names {entry}, which has no Cargo.toml"
            );
            members.push(path);
        }
    }
    assert!(
        members.len() >= 10,
        "only {} workspace members were found, which is fewer than there are",
        members.len()
    );
    members
}

/// A crate README says four things, and three of them are headings.
///
/// The fourth is *what the crate is for*, which is the opening paragraph and has
/// no heading to check — so what is asserted is the three that do: what it may
/// not contain, how to test it, and which sessions built it.
///
/// Deliberately **not** the §10.1 rules themselves: those live in
/// `ARCHITECTURE_SPEC.md`, and a README that copied them would be a second
/// source that can go out of date. What a README carries is the *consequence*
/// for its own crate, in a sentence, with the link — and this checks that the
/// section exists rather than what it says, because a test that checked the
/// prose would be the eleventh copy of the rule.
const REQUIRED_HEADINGS: [&str; 3] = ["## What it may not contain", "## Testing it", "## Sessions"];

#[test]
fn every_workspace_member_has_a_readme() {
    for member in workspace_members() {
        let readme = member.join("README.md");
        assert!(
            readme.is_file(),
            "{} has no README.md — ARCHITECTURE_SPEC.md §10.1's rules are per crate and a crate \
             that does not say which side of them it is on is one somebody has to grep for",
            member.display()
        );
        let text = read(&readme);
        assert!(
            text.len() > 400,
            "{}'s README.md is too short to say anything",
            member.display()
        );
        for heading in REQUIRED_HEADINGS {
            assert!(
                text.contains(heading),
                "{}'s README.md has no `{heading}` section",
                member.display()
            );
        }
    }
}

/// The README names its own crate, so a copied one cannot pass.
///
/// The cheapest way to satisfy the test above is to copy a neighbour's file.
/// This is what stops that being enough.
#[test]
fn every_readme_names_the_crate_it_belongs_to() {
    for member in workspace_members() {
        let name = member
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a crate directory has a name")
            .to_owned();
        let text = read(&member.join("README.md"));
        assert!(
            text.contains(&name),
            "{}'s README.md never names `{name}`",
            member.display()
        );
    }
}

/* -------------------------------------------------------------------------- */
/* The manuals exist and say which version they describe                      */
/* -------------------------------------------------------------------------- */

/// Every page of documentation says which version it describes.
///
/// It is S42's exit criterion for the website, and it is met at the **source**
/// rather than at the generator: a manual that carries the version is one that
/// still carries it when somebody reads the Markdown on GitHub. The number is
/// the workspace's, so this goes red on a release that forgot a manual.
#[test]
fn every_manual_says_which_version_it_describes() {
    let version = env!("CARGO_PKG_VERSION");
    for manual in ["operator.de.md", "installer.de.md", "developer.en.md"] {
        let path = root().join("docs/manual").join(manual);
        let text = read(&path);
        assert!(
            text.contains(version),
            "docs/manual/{manual} does not say that it describes {version}"
        );
    }
}

/// Every version `docs/RELEASE_NOTES.md` describes has a changelog entry.
///
/// The two documents are deliberately different shapes and different languages —
/// the release notes are **one** version at length, in English, because that
/// text is the body of the GitHub release beside the English `README.md`; the
/// changelog is **every** version in a few lines, in German, because that is the
/// page the site serves beside the two German manuals. What holds them together
/// is the set of versions, and this is where it is held: a release written up in
/// one and forgotten in the other goes red here.
#[test]
fn every_version_in_the_release_notes_is_in_the_changelog() {
    let notes = read(&root().join("docs/RELEASE_NOTES.md"));
    let changelog = read(&root().join("CHANGELOG.md"));
    let versions: Vec<&str> = notes
        .lines()
        .filter_map(|line| line.strip_prefix("# PrismDMX "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();
    assert!(
        !versions.is_empty(),
        "docs/RELEASE_NOTES.md has no `# PrismDMX <version>` heading, so this test is looking at \
         the wrong thing"
    );
    for version in versions {
        let heading = format!("## {version}");
        assert!(
            changelog.contains(&heading),
            "docs/RELEASE_NOTES.md describes {version} and CHANGELOG.md has no `{heading}`"
        );
    }
}

/// The changelog has an entry for the version that is about to be built.
///
/// S41 decided the changelog is **written** and not generated — `PROGRESS.md`
/// §2 is a verification protocol for developers, and prose generated from it
/// would be exactly the document a user cannot use. What *is* mechanical is the
/// set of versions, and this is it: a release whose changelog nobody wrote goes
/// red here rather than shipping with a page that stops at the version before.
#[test]
fn the_changelog_has_an_entry_for_this_version() {
    let version = env!("CARGO_PKG_VERSION");
    let text = read(&root().join("CHANGELOG.md"));
    let heading = format!("## {version}");
    assert!(
        text.contains(&heading),
        "CHANGELOG.md has no `{heading}` section"
    );
}

/// The site's command-line page names every word the desk understands.
///
/// `docs/site/command-line.{en,de}.md` are written **for the website** rather
/// than generated: a reference somebody reaches for while a show is loading
/// should explain the words, not list them in the order an array happens to
/// have. What that buys in readability it risks in currency, so the currency is
/// checked here — beside `CONSOLE_WORDS`, which is the truth — rather than in
/// the site generator, which would need a dependency on this crate to ask.
///
/// A word the desk gains and the page never mentions is a reference that is
/// quietly incomplete, and quietly is the worst way for a reference to be wrong.
#[test]
fn the_sites_command_line_page_names_every_word_the_desk_knows() {
    for page in [
        "docs/site/command-line.en.md",
        "docs/site/command-line.de.md",
    ] {
        let text = read(&root().join(page));
        let missing: Vec<&str> = CONSOLE_WORDS
            .iter()
            .copied()
            .filter(|word| !text.contains(&format!("`{word}`")))
            .collect();
        assert!(
            missing.is_empty(),
            "{page} does not mention {missing:?} — the desk knows {} words and a reference for \
             the website has to name all of them",
            CONSOLE_WORDS.len()
        );
    }
}
