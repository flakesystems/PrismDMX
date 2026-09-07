# COMMAND_LINE.md — the console line

**Status:** reference for operators and for the people who extend it.
**Parent documents:** [`ARCHITECTURE_SPEC.md`](../ARCHITECTURE_SPEC.md) §4.5 (the
decision), [`docs/IPC_PROTOCOL.md`](IPC_PROTOCOL.md) §5 (the commands).
**Implemented by:** `crates/prism-core/src/console.rs` (the parser),
`crates/prism-core/src/file.rs::run_command_line` (running one),
`ui/src/desk/shell.tsx` (the keys), `ui/src/desk/commandline.tsx` (the screen).

> **The parser lives in the daemon since S49.** It was
> `ui/src/desk/console.ts` from S40 until then, because that is where the
> operator types — and the consequence was quiet and then loud: **a key on the
> X-Touch cannot run a line** if the only parser is in a browser. S43 shipped a
> stop-gap and wrote it down as one; S49 moved the grammar rather than
> rewriting it, so every word below means exactly what it meant. What changed is
> **who reads it**: a line is sent as `Command::CommandLineInput { run: true }`
> and the daemon applies what it means, and what a line *would* do is asked as
> `Query::CommandLineReading` and drawn under the box. §5 has both.

---

## 1. A key writes a word into the line. It does not act.

This is the whole design and it is worth reading before the table of words.
`Session::commandLine` is session state (`ARCHITECTURE_SPEC.md` §4.1), so what
you are part-way through typing is already on every screen attached to this
desk — the browser, a second browser, the X-Touch's display. The buttons on the
screen do not send commands of their own: they **write into that line**, and the
line is what is sent.

Three shapes, and every control on the screen is one of them:

| Shape | Words | What pressing it does |
|---|---|---|
| a whole command with no argument | `Clear` `Full` `Oops` `Update` | writes the word and runs it at once |
| a command that needs arguments | `Store` `Edit` `Goto` `Move` `Copy` `Delete` `Label` `Assign` | writes the word and **waits** for you to finish the line |
| an argument keyword | `Fixture` `Group` `Sequence` `Cue` `Preset` `View` `Executor` | appends the word to the line as it stands |
| a **chooser** in a window | the `Executors` window's fader, encoder and key rows | writes the whole line and sends it, because the pointer has supplied every argument (S45) |

So `Fixture` `1` `Enter` is three presses that build `Fixture 1`, and it is the
same line you could have typed. Picking an item out of a **list** — a group in
the pool, a cue in the sheet, a fixture in the fixture sheet — writes the line
that names it *and* submits it, because the pointer has supplied the argument
the line was waiting for.

**What is not a line**, deliberately and in full:

- the **executor keys and faders** on the bar above the command line: a Go is a
  gesture with timing in it (§4.3) and a fader is a stream of positions;
- the **encoders**, the five bank keys and the two parameter arrows: the
  vocabulary has no word for a bank, and the encoders are what one selects;
- **dragging and resizing a window** on the canvas (§4.2);
- **Add window**, which names a window *type* rather than a number;
- the three **times** and the trigger of a cue, and the fields of the patch form:
  they carry a value rather than naming a place, and inventing `fade 2.5` would
  be a second grammar for something no console types.

---

## 2. The words

Case does not matter. Spacing mostly does not either — `1thru4` is a range,
because a console's keypad has no space bar worth reaching for. A name may be
quoted and need not be.

### 2.1 Selecting

| Line | What it does |
|---|---|
| `1` · `12` | selects that fixture |
| `1 thru 4` · `1thru4` | selects a range, in the direction it is written |
| `1 + 3` · `1, 3` | selects both. `+` and `,` mean the same thing |
| `Fixture 12 thru 16` | the same, with the keyword an operator may type out of habit |
| `Group 3` | selects the fixtures of group 3 |
| `Preset 4` | applies preset 4 to the selection |
| `Sequence 5` | makes 5 the cue list a store goes into |
| `View 2` | switches the canvas to view 2 |
| `Executor 3` | selects the executor the transport acts on |

A **cue on its own is not a line**: `Cue 5` could be a Goto or an Edit, and the
desk says so rather than guessing.

### 2.2 Levels

| Line | What it does |
|---|---|
| `at 50` | the selection to 50 % |
| `1 thru 4 at 50` | selects and sets, which is two commands |
| `5 pan at 25` · `5 at pan 25` | an attribute other than the dimmer, on either side of `at` |
| `5 gobo 2 at 50` | **which one of that kind** — a head with two gobo wheels, S52 |
| `at full` · `at out` · `at zero` | the words for the two ends |
| `Full` | the selection to full, with nothing else on the line |

An attribute word is the attribute's own name, lower-cased: `dimmer`, `pan`,
`red`, `colorwheel`, `warmwhite`, `bladerotation`. There are forty-one of them
and `prism_domain::AttributeType` is the list.

**`raw` is the odd one and the useful one** — S54. It is not a kind of
parameter; it is *a channel this desk has no word for*, and every DMX slot of a
patched fixture that reaches no other attribute reaches this one. `1 raw 3 at 50`
drives the third such channel of fixture 1 to half. Which channel that is has a
name on the encoder — the manufacturer's own, or `Ch 7` for a slot nobody named
— because a number alone would be a channel an operator has to count out.

**A number straight after an attribute word says which channel of that kind you
mean** — S52, and the line counts from one where the model counts from nought,
so `gobo 1` and `gobo` are the same wheel. The rule is narrow on purpose,
because the shortest thing an occurrence could be is also a fixture number: it
is read only when the attribute word is **not the first word** of the line and
the number stands **immediately before `at`**. So `pan 5 at 25` still selects
fixture 5, and `5 at pan 25` — where the attribute stands after `at` and the
next word is the level — is unchanged.

### 2.3 Storing

| Line | What it does |
|---|---|
| `Store Cue 5` | stores the programmer into cue 5 of the **selected** sequence |
| `Store Sequence 5 Cue 3` | into a cue of a named sequence |
| `Store Sequence 4` | stores into cue list 4, and makes it if the number is free |
| `Store Preset 1` | stores into preset 1, in the bank the encoders are showing |
| `Store Preset 1 Color` | into a **named** pool, whatever the encoders are on |
| `Store Preset 1 Multi "The look"` | into the `Multi` pool, which takes every value the programmer holds across the banks — S43 |
| `Store Group 3` | stores the **selection** as group 3 |
| `Store View 2 "Programmer"` | stores the canvas as view 2 |

**When the destination already holds something, the line asks** — *merge,
override or cancel*, in the command line itself. The show carries on while the
question stands: no other client is blocked, and a Go from the X-Touch does not
wait on it. Escape cancels, and a cancelled question changes nothing at all.

A cue list is asked with **append, override or merge** instead, because a
sequence store is about *cues* where a cue store is about *values*.

**A cue list made on a free number becomes the selected one.** `Store Cue 1`
names no cue list and means the selected one, so a desk that made list 4 and
went on pointing at list 1 would send the next store into the wrong place. A
store into a cue list that already exists leaves the selection alone: choosing
what to edit is `Sequence 4`'s job, and an operator storing into a second list
has not said they want to move there.

### 2.4 Editing

| Line | What it does |
|---|---|
| `Edit Cue 3` | loads cue 3 of the selected sequence into the programmer |
| `Edit Sequence 5 Cue 3` | the same, naming the cue list |
| `Update` | stores the programmer back into the cue it came from, in Override mode |
| `Label View 1 "Programmer"` | names a view. The same for cues, sequences, groups, presets and executors |
| `Delete Cue 3` | takes it out. The same for the other five |
| `Move Cue 3 Cue 8` | renumbers. Asks when 8 is taken |
| `Move Executor 1 Executor 5` | moves the content, and **swaps** when 5 holds something |
| `Move View 1 View 3` | swaps the two layouts; the numbers stay where they are |
| `Copy Sequence 2 Sequence 6` | copies. The same for cues, groups, presets and views |
| `Assign Sequence 5 Executor 1` | puts a cue list on a fader |
| `Assign Executor 1 Fader Master` | says what that executor's fader does — `Empty`, `Master`, `Speed`, `XFade`, `Fade` |
| `Assign Executor 1 Encoder Speed` | the same for its encoder — `Empty`, `Master`, `Speed` |
| `Assign Executor 1 Button 2 Go+` | the same for one of its four keys, **numbered from one** |
| `Assign Executor 1 Button 4 Command "Go+ Sequence 3"` | the custom row: that key sends this line |
| `Oops` | takes the last edit back |

**Update blinks** when there is an edit to put back. That is
`Session::editingCue`, which is session state — so every screen blinks together.

**One verb, two sentences** — S45. *Assign this list to that fader* and *assign
this function to that control* are the same act on a desk and an operator says
"assign" for both, so the parser tells them apart by the first noun: a
**sequence** is being put on a fader, an **executor** is being given a function.
The keys are numbered from **one** on the line, because that is how an operator
counts the four keys under a fader; `ExecutorChange::Button` counts from zero the
way the hardware does, and the one word between them is here.

The function words are the enum spellings, and the parser does not read the
*desk* any more than it reads the show (§4): every word it takes is one
`prism_domain` declares, so a function added in Rust is a word here without this
table being edited. **Quote a command line that has punctuation in it** —
`go+`, `+` and `,` are rewritten by the tokeniser so that `1 + 2` and
`go+ executor 0` mean what they say, and a quoted chunk is the one thing it keeps
exactly as typed. So `Command clear` is a line and `Command "Go+ Sequence 3"` is
a line, and `Command Go+ Sequence 3` is the first two words of one.

**These are the lines the control editor writes.** The `Executors` window's
choosers are keys like any other (§1, and `ARCHITECTURE_SPEC.md` §4.5): they
write one of the lines above and send it, rather than sending a command of their
own — so the window, the line and a bound X-Touch key are one path and cannot
drift apart.

### 2.5 Playback

| Line | What it does |
|---|---|
| `Go+` · `Go-` · `On` · `Off` | on the **selected** cue list |
| `Go+ Executor 1` | on one executor |
| `Go+ Sequence 2` | on one cue list, wherever it is playing |
| `Goto Cue 5` | jumps the selected cue list straight to cue 5 |
| `Goto Executor 1 Cue 5` · `Goto Sequence 2 Cue 5` | the same, named |
| `Page 2` | pages the fader bank |

**A cue list that is on no fader still plays**, and it is the *same* playback the
fader would drive. `On Sequence 1` and a Go on the executor somebody later puts
it on are one playback with one cue pointer, because since S45 a playback is a
cue list's and an executor is a handle on it — see `prism_domain::PlaybackId`.
Two executors on one list are therefore two handles, not two players: punch-list
entry **B18**.

### 2.6 S26's lines, kept and unbroken

`go 3`, `off 3`, `page 2`, `clear` and every form of `at` mean exactly what they
meant before S40. A bare number after `go`, `go-`, `on` or `off` is an
**executor**, which is what it meant when there was nothing else it could be.

---

## 3. Completion and history

Under the input is what is legal **at this point in the line** — the words, never
the numbers. Tab takes the first, or click one. It is a *grammar* answer and not
a *show* answer: it offers the word `sequence`, never the sequences there are,
for the same reason the parser does not read the show (§4). A list of what
exists is what the pools on the canvas are for.

The words come **with the reading** since S49 — `Answer::CommandLineReading`
carries them — so the completion table is the grammar's and lives beside it.

The **up and down arrows** walk back through the lines you have typed. Both are
client-local (`ARCHITECTURE_SPEC.md` §4.2): the line you are typing is shared,
and what you typed *before* is not — two operators on two screens each have
their own train of thought.

---

## 3.1 A pool is a key — S43

`ARCHITECTURE_SPEC.md` §4.5 has said since S40 that every key on the desk writes
a word into the line. S43 finished the thought: the tiles in the Sequence Sheet,
the Fixture List, the Cue Viewer, the Group Pool, the Preset Pool and the
executor strip are keys too.

**With a verb standing in the line, a click appends its words instead of
selecting.** `Store` and a click on sequence 2 gives `Store Sequence 2`, and
sends it, because the pointer has supplied the argument the line was waiting for.
With nothing typed, the same click is a list pick and does what the box has
always done.

`ui/src/desk/consoleshell.ts::pickOnto` is the whole rule, and since **S49** it
is three lines over one thing: the daemon's reading of the line the click *would*
produce (`Query::CommandLineReading`). Appending a noun never changes a line's
first word, so the candidate's own answer says everything.

- **not a verb line** → the row does its own thing. A line that does not begin
  with a verb is a fixture selection being built, so `1 thru` plus a click on a
  group tile is a range in progress rather than a group being named; and *with
  nothing typed* falls out of the same test rather than being a case of its own,
  because `Group 3` on an empty line is not a verb line either.
- **a clearing verb** → appended and left standing. `Label` and `Color` are valid
  commands without a last word and both *take something away* — a name, a colour.
  A click that deletes a name is the worst kind of shortcut.
- **not yet a command** → appended and left standing, with the daemon's own
  complaint under it. `Store Fixture 5` reads *"fixture" is not something to
  name*, rather than the typed `Store` being quietly discarded and a fixture
  selected. Discarding what the operator typed is the behaviour this rule exists
  to remove.

Anything else is finished, so it is sent. **Which nouns a verb takes is not a
table the interface holds**: it never was, and since S49 neither is the list of
verbs — `verb` and `clearing` are two fields of the answer, out of
`prism_core::console::VERB_WORDS` and `CLEARING_VERBS`.
`the_verb_table_partitions_every_word_the_console_knows` is the test that stops a
verb entering the grammar without entering the table.

---

## 4. Two rules for whoever extends this

**The parser does not read the show.** `Copy Sequence 2 Sequence 6` means the
same thing whether or not sequence 2 exists. What a line *means* depends only on
the line; what there *is* is the daemon's, and a refusal is a message rather
than an exception. A parser that consulted the patch would be a parser that can
be wrong about the daemon's state — which is decision **D3**, and S26 wrote it
down first.

The one thing that *is* read is whether a destination is already occupied, and
only to decide whether to **ask**. Until S49 an interface read that out of its
own mirror (`ui/src/desk/exists.ts`); it is `ShowFile::holds` now, answered
beside the reading, so a client one delta behind can no longer ask a question
about a cue somebody has just deleted. What may **not** be answered there is what
a store would *cost* — that is `Query::StorePreview`, a different question with a
different cadence.

**The parser never fails.** Every string there is answers with commands or with a
sentence somebody can be shown. `crates/prism-core/tests/console.rs` runs ten
thousand generated lines through it; that is an exit criterion rather than a
precaution, and it is worth more since S49 than it was before, because the thing
that would fall over is now the **daemon** rather than one browser tab.

---

## 5. What travels, and when

Two messages, and neither of them is a parser in a client.

| | Message | When |
|---|---|---|
| running a line | `Command::CommandLineInput { text, run: true, mode }` | Enter, a key that writes-and-runs, an item picked out of a list, a bound X-Touch key with *send*, an executor key carrying a line |
| typing a line | `Command::CommandLineInput { text, run: false }` | every keystroke, paced at S25's cadence, because `Session::commandLine` is shared |
| reading a line | `Query::CommandLineReading { text }` | as it is typed, and again before Enter if the answer in hand is about an older line |

**What comes back from running one is the deltas of what the daemon did**, and a
line can fall into more than one command: `1 thru 3 at 50` is a `SelectFixtures`
and a `SetAttribute`. A line that is **not** a command is written into
`Session::commandLine` and left standing — the reading already says what is wrong
with it — and a line whose *first* command is refused stops there, because a line
is one sentence and carrying out the second half of one whose first half was
refused is doing something nobody asked for.

`mode` is the operator's answer to the question in §2.3, when they were asked.
Absent means *the mode that cannot lose anything*, which is what the parse
already carries — and that is what makes a bound key on a desk with no client
attached safe to press.
