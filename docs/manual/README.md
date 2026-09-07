# The manuals — who each one is for, and why it is in the language it is in

Three manuals, three audiences, and **one language each**. That is a decision
S41 had to make and is the first thing recorded here, because it is the decision
a later session is most likely to undo by accident.

| Manual | Audience | Language |
|---|---|---|
| [`operator.md`](operator.md) | the person running a show | **German** |
| [`installer.md`](installer.md) | the person who wires the building and sets the desk up | **German** |
| [`developer.md`](developer.md) | the person who changes the code | **English** |

## Why not one language, and why not two of each

**Two languages per document is two documents.** They diverge on the first fix
that only lands in one of them, and the reader who gets the stale one has no way
of knowing. Every manual here therefore exists exactly once, and translation is
a decision to be taken deliberately later — with a mechanism that makes a stale
translation fail a build — rather than a habit started now.

**So each manual takes the language of the people who read it.** PrismDMX was
written for venues and schools in Germany; the owner writes German,
`docs/ISSUES.md` is German, and the first building this desk runs a show in is a
German one. An operator reading under time pressure in a dark room should not be
reading a second language. The installer is standing in the same building, often
the same person, and the venue's network, its Art-Net node's front panel and its
IT department are all German too.

**The developer's manual is English** for the opposite reason and it is just as
strong: every identifier, every commit message, every line of
`ARCHITECTURE_SPEC.md`, `PROGRESS.md` and the crate documentation is English. A
German manual over an English code base would be a translation layer between a
reader and the thing they are about to edit, and every proper noun in it would
be in the other language anyway.

**What this excludes, said out loud:** somebody who wants to run a show and
reads neither German nor code. The door for them today is the English
[`README.md`](../../README.md) and the crate documentation, and the honest
statement is that the operator's manual is not yet for them. When there is a
second venue that needs it, the manual gets a translation *and* a test that
holds the two together.

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
