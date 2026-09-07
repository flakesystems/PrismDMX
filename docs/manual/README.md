# The manuals — who each one is for, and what language it is in

Three manuals, three audiences, and since the owner's decision of 2026-09-07
**every one of them in both languages**, with English as the default.

| Manual | Audience | Source |
|---|---|---|
| Operator's manual | the person running a show | [`operator.de.md`](operator.de.md), `operator.en.md` |
| Installer's manual | the person who wires the building and sets the desk up | [`installer.de.md`](installer.de.md), `installer.en.md` |
| Developer's manual | the person who changes the code | [`developer.en.md`](developer.en.md), `developer.de.md` |

The language is in the **filename**, always, even where only one exists yet.
Half the files carrying a suffix and half not is how a translation ends up
overwriting an original.

## The decision this replaces, and why it is not simply reversed

S41 decided the opposite — one language per manual, no translation — and the
argument was not that translating is hard. It was this:

> **Two languages per document is two documents.** They diverge on the first fix
> that only lands in one of them, and the reader who gets the stale one has no
> way of knowing.

That is still true, and it is worth saying plainly that **the decision was
overruled rather than refuted**: the owner wants the documentation readable by
people who do not read German, which is a reason about readers and beats a
reason about maintenance. What does not go away is the failure mode.

The old text named the condition under which it would change its mind, and this
is the whole of why the reversal is safe:

> translation is a decision to be taken deliberately later — **with a mechanism
> that makes a stale translation fail a build** — rather than a habit started
> now.

**That mechanism now exists**, and it is three things rather than good
intentions:

1. **Both languages of a document are held against each other by shape.**
   `both_languages_of_a_document_have_the_same_chapters` in `web/tests/site.rs`
   compares the chapter count of the two versions. It cannot tell you a
   paragraph is out of date — no test can — but it catches what actually
   happens, which is a chapter added to one language and not the other.
2. **A page that has no translation yet says so, on the page, before the text.**
   It is not silently served in the other language and it does not vanish from
   the navigation: `prism_web::Source::Borrowed` puts a line at the top saying
   which language the reader is about to get.
3. **The number of untranslated pages is written into a test.**
   `the_number_of_untranslated_pages_is_written_down` lists them by name, so
   adding one is a deliberate edit to a test and translating one is a deliberate
   deletion. A gap can exist; it cannot be quiet.

## Which documents are *not* on the site

The four specifications — [`ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md),
[`IPC_PROTOCOL.md`](../IPC_PROTOCOL.md), [`MCU_MAPPING.md`](../MCU_MAPPING.md)
and [`DMX_MERGE.md`](../DMX_MERGE.md) — and the fault register
[`ISSUES.md`](../ISSUES.md) are **working documents**, and the owner's decision
of 2026-09-07 keeps them off the website. They change with the source, every
identifier in them is English, and a German copy would be a translation layer
between a reader and the thing they are about to implement.

Two of them have a **public counterpart written for the site instead**, in
`docs/site/`: `command-line.{en,de}.md`, which explains the twenty-nine words
rather than listing them, and `known-faults.{en,de}.md`, which carries only the
faults that are still open. Neither is a copy — both are held to the truth by a
test, the first against `CONSOLE_WORDS` and the second against the open entries
in `ISSUES.md`.

## What is held to the code, and how

The operator's manual carries two lists that are **generated**, not written:
the window types and the words of the console line. Both are in the code, both
change without anybody remembering to open a manual, and a manual that has
quietly fallen behind is worse than none.

`crates/prism-core/tests/documentation.rs` is the mechanism. Each list lives
between markers:

```markdown
<!-- generated:window-types -->
…
<!-- /generated -->
```

The test rebuilds the block from `prism_domain::WindowType::ALL` and
`prism_core::console::CONSOLE_WORDS`, **keeps the prose already written against
each row**, and inserts `TODO` for a row that is new. If the result differs from
the file it *writes the file* and fails, so the next `cargo test` shows a diff
in `git status` rather than a sentence in a log. It then fails a second time if
any `TODO` is left, because a row nobody has described is not a documented one.

The same file asserts that every window type has a section of its own, and that
every workspace member has a `README.md` with the three headings a crate README
is supposed to have — what it may not contain, how to test it, and which sessions
built it. The fourth thing a README says, *what the crate is for*, is its opening
paragraph and is not something a test can check.

## What is *not* here

These are manuals. The references are older, are maintained beside the code they
describe, and are linked from the manuals rather than folded into them:

| | |
|---|---|
| [`../COMMAND_LINE.md`](../COMMAND_LINE.md) | the console line, word by word, for whoever extends it |
| [`../DMX_MERGE.md`](../DMX_MERGE.md) | how values become one frame |
| [`../IPC_PROTOCOL.md`](../IPC_PROTOCOL.md) | what travels between the daemon and a client |
| [`../MCU_MAPPING.md`](../MCU_MAPPING.md) | every note, CC and offset of the X-Touch |
| [`../ISSUES.md`](../ISSUES.md) | what is known to be wrong |
| [`../../ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) | why it is built this way |
