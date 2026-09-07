# PrismDMX — Developer's manual

**For:** whoever is about to change the code.
**Applies to:** `0.9.2`.
**Language:** English, because every identifier, commit message, specification
and crate document in this repository is English and a German manual over them
would be a translation layer between a reader and the thing they are editing.
[`README.md`](README.md) has that decision in full.

This is not the specification. [`ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md)
is what the system *is*; this is how to work on it — the shape it actually
ended up with, the rules that came out of getting things wrong, and four
worked recipes for the four things people most often need to add.

---

## Contents

1. [Getting a build](#1-getting-a-build)
2. [The shape it ended up with](#2-the-shape-it-ended-up-with)
3. [The rules that came out of mistakes](#3-the-rules-that-came-out-of-mistakes)
4. [Adding a command](#4-adding-a-command)
5. [Adding a window type](#5-adding-a-window-type)
6. [Adding a fixture type — or teaching the library reader](#6-adding-a-fixture-type--or-teaching-the-library-reader)
7. [Adding an output kind](#7-adding-an-output-kind)
8. [The gates, and what each one is for](#8-the-gates-and-what-each-one-is-for)
9. [Testing rules that are not negotiable](#9-testing-rules-that-are-not-negotiable)
10. [How work is recorded](#10-how-work-is-recorded)

---

## 1. Getting a build

You need [Rust](https://rustup.rs) stable (1.89 or newer),
[Node](https://nodejs.org) 24 and — on Windows — the MSVC build tools. On Linux,
building the shell additionally needs a webview toolkit (`libgtk-3-dev`,
`libwebkit2gtk-4.1-dev`); nothing else in the workspace does.

```bash
git clone https://github.com/flakesystems/PrismDMX.git
cd PrismDMX
tools/fetch-fixtures/fetch-fixtures.sh   # or .ps1 on Windows
cd ui && npm ci && cd ..
cargo build --workspace
```

Two of those steps are worth a sentence each.

**The fixture library is fetched, not committed.** It has an upstream with its
own release cadence, and a copy in this tree would be the one that is out of
date. The script pins a revision, so two machines that install on different days
get the same profiles. Corpus tests **skip themselves** when it is absent, which
is why every CI job that runs them fetches it first.

**`ui/src/bindings/` is generated, not committed.** `cargo test -p prism-domain`
writes it from the Rust types, so a stale binding cannot survive a green build,
and nothing in `ui/` typechecks before that has run once.

Running the two halves separately while developing:

```bash
cargo run -p prismd -- --mock-output
npm --prefix ui run dev
```

---

## 2. The shape it ended up with

### Two processes, and the boundary is the point

`prismd` owns the show, the session, the engine and every output. The shell and
the interface own a window. **If the interface dies the lights keep working** —
the operator carries on from the X-Touch, and when the interface comes back it
finds the state the console established. That is decision **D2**, and everything
else follows from it: the engine has no user interface *in* it at all.

### Nine crates, in dependency order

```
prism-domain ── the words
   ├── prism-engine ──── the tick, the merge, the encoding   (no I/O at all)
   ├── prism-core ────── the show, the programmer, the console line
   ├── prism-protocols ─ Art-Net, sACN, Open DMX USB
   ├── prism-surface ─── the X-Touch, as three layers        (no port)
   ├── prism-midi ────── the port                            (no protocol)
   └── prism-ipc ─────── framing and transports
            └── prismd ──── the daemon: all of the above, wired together
                    └── prism-app ── the shell and the installer
```

Each crate has a `README.md` saying what it is for, what it may not contain, and
how to test it. Read that before the source.

### Where state lives, and the three appliers

There are exactly three kinds of state, and every command belongs to exactly one
of them — `crates/prism-core/tests/command_application.rs` is what asserts that,
by counting.

| State | Lives in | Persisted in | Example |
|---|---|---|---|
| **The show** | `prism_core::Show` | the `.prism` file | fixtures, cues, groups, presets, executors |
| **The session** | `prism_core::SessionState` | the `.prism` file, beside the show | the active view, the open windows, the page, the selection, the command line |
| **The machine** | `prism_core::MachineConfig` | `machine.json` | the output patch, the desk identity, the surface port, the network exposure |

A fourth kind exists and has **no** applier: `Shutdown` acts on the *process*.
It is counted in that test anyway, because a kind whose membership was implicit
would be a hole exactly the size of the check.

Two consequences that catch people out:

- **The session is in the show file.** So a layout travels with a show. What is
  deliberately *not* there — monitor assignment, scroll position, camera, which
  settings tab is showing — is client-local, per `ARCHITECTURE_SPEC.md` §4.2.
- **The rig is not in the show file.** A show carried to another hall on a stick
  must not bring the first hall's cabling. That is §7.0's decision, and it is
  the same argument as the sACN CID's.

### D11: the console operates the interface

The active view, the open windows, the executor page and the selection are
**session state in the daemon**. That is what lets an X-Touch switch a view
while no client is running, and what makes a client restart an ordinary
reconnect rather than a recovery. There is a CI-run gate for it: a mock surface
presses two keys with **no client connected**, and a client that connects
afterwards finds the view and the window in its snapshot.

### The command line is the interface

`ARCHITECTURE_SPEC.md` §4.5. Every key, tile and button in the interface
**writes a line** rather than acting, so a gesture and a typed sentence are the
same thing, and any line can be bound to an X-Touch key. The parser is
`prism_core::console` — in the daemon since S49, because a key on a control
surface cannot run a line whose only parser is in a browser.

Two rules hold it up, and both are load-bearing:

- **The parser does not read the show.** What a line *means* depends only on the
  line. What there *is* is the daemon's, and a refusal is a message rather than
  an exception. (The one thing that *is* read is whether a destination is
  occupied — and only to decide whether to ask a question.)
- **The parser never fails.** Every string there is answers with commands or
  with a sentence somebody can be shown. A console that could throw on a typo
  mid-show is a console that stops answering.

### The tick

44 Hz, its own thread, its own priority. It takes no lock, waits for nothing and
**makes no allocator call** — measured on ten paths, with an eleventh test that
allocates on purpose so the probe cannot break silently. Everything that needs a
`Vec` happens on the core thread and reaches the tick behind one atomic.

Frames leave through a triple buffer, one subscriber per output driver. Adding
or removing an output while the show runs costs the outputs that did not change
**no tick and no frame**.

---

## 3. The rules that came out of mistakes

Every one of these is in `PROGRESS.md` §6 with the session that learned it and
the test that caught it. They are here because they generalise, and because each
of them has already been re-learned in a second shape at least once.

### A rule asked in two layers is a function, not a `match`

*Nothing ever moves a crossfade fader* is a statement about the surface **and**
about the browser. Written twice, the two drift the first time somebody adds a
fader function: one `match` gets a new arm and the other falls through to the
wrong default. `ExecutorFaderFunction::desk_may_move_it` is the shape — one
predicate in `prism-domain`, asked by both sides, and a test that walks `ALL`
rather than the variants somebody remembered.

The same shape appears as `AttributeType::is_additive_emitter` and as
`crossfade_mode`. When you find yourself writing the second `match` on an enum
that already has one somewhere else, that is the signal.

### A rule stated over a category changes meaning when the category grows

*A colour rests open* was implemented as *the colour bank rests at full*, and
those were the same statement while every colour in the model was an additive
emitter. Adding CMY — which are filters, where full is opaque — made every CMY
rig come up **black** at home, and nothing went red until a corpus test asked
about a fixture nobody was looking at.

Ask the **attribute**, not the bank; ask the thing itself, not the group it
happens to be in today.

### A new constructor beside an old one is a new way to break every old rule

A resolved switching alias built from the raw-channel constructor rested at
nought; a red must rest open. The rules the old constructor enforced became a
list nobody was holding the new one to. When a second way of building a type
appears, go and find what the first one was quietly guaranteeing.

### An invariant over what a thing *produces* beats any number of counters over what it discards

Four counters read nought while 707 DMX slots of the library had no knob on
them, because a slot the reader did not understand was simply never written
down — the absence had no name, so nothing counted it. The assertion that found
it asks the operator's question: *does every slot of every profile have exactly
one `AttributeDef`?*

Counters are diagnostics. They cannot be the guarantee.

### A floor, not a list of fixes

Fixing exactly the four causes of an unreachable slot would have made the corpus
green and left the fifth — a capability type the format grows next year — to
fall through the same hole in silence, which is how the four got there one
session at a time. `AttributeType::Raw` is a **default**: anything that reaches
no other attribute reaches it.

### A hand-written diff is a list nobody is holding to the struct

`SessionState::commit` compares field by field. A field added to the struct, the
command and the applier — but not to the diff — moves the session and emits **no
delta**, so a second screen sits on stale state and the desk it was typed on
looks right. The only thing that catches it is
`session_deltas_reproduce_the_session_they_came_from`: a property over arbitrary
commands that replays every emitted delta into a mirror and compares.

If you add a field to a persisted or mirrored type, the checklist is: the
struct, the command, the applier, **the diff**, the classification predicates,
and the TypeScript.

### An absent field is a better migration than a migration

`occurrence` had to reach seven types — the key every value in a show is filed
under. It went in as `#[serde(default, skip_serializing_if = …)]` and
**nought-based**, so *absent* and *the first one* are the same statement: a show
with no repeats serialises byte for byte as it did, an older file opens with
every value where it was, and there is no migration row at all.

### A converter's own counter cannot tell you its output is accepted

The library reader numbered repeats correctly and its duplicate counter read
nought — and one profile in 2 871 was then refused by the *show*, whose
validator still asked the old question. Run the whole corpus through the door
the real caller uses.

### A gesture that moves no value must not take a state away

Reordering the Clear stages made the first press one that takes no values, and a
rule justified by *the values are gone* went on being true after its reason
stopped applying. When you change what a gesture does, go and read the rules
that were justified by what it used to do.

### D3 applies to the test suite

A gesture is a command out and a delta back. A test that clicks and then reads
is a test that reads too early. Three end-to-end flakes in this repository were
all this, in three shapes — and the sharpest is that `count()` does not wait, so
a guard like `if (count > 0) click()` turns a missing element into a **silently
skipped step** and a broken interface into a green run.

Wait for the state, never for a duration.

### A measurement taken with a different rule than the code uses is not a measurement of the code

Two figures reported from a throwaway script were both wrong, and the corpus
test caught its own author. If you quote a number, quote one the code produced.

---

## 4. Adding a command

A command is the only way anything changes. The path, in order:

1. **Decide which of the three it acts on** — the show, the session, or the
   machine. If the honest answer is *none of them*, stop and read the `Shutdown`
   entry in `PROGRESS.md` §7 before inventing a fourth.
2. **Declare the variant** in `prism_domain::Command`, with a doc comment saying
   what it does and *why it is that shape*. Field names are `camelCase` on the
   wire; the enum is internally tagged with `t`.
3. **Classify it.** `Command::is_session_command`, `is_machine_command` and
   `is_undoable` are three separate lists, and a command missing from one routes
   to the wrong applier or vanishes from the Oops journal.
4. **Implement it in the one applier that owns it** —
   `prism_core::Show::apply`, `SessionState::apply` or `MachineConfig::apply` —
   and emit the deltas it causes. If it changes a mirrored field, **check the
   hand-written diff** (§3).
5. **Give it a word on the console line** if an operator should be able to type
   it: `prism_core::console`, `VERB_WORDS` and `CONSOLE_WORDS`. Adding a word to
   the grammar without adding it to the table is what
   `the_verb_table_partitions_every_word_the_console_knows` exists to stop, and
   the manual's word list is regenerated by
   `crates/prism-core/tests/documentation.rs`.
6. **Write the tests before the code**, per `CLAUDE.md`. At minimum: applied to a
   populated show, refused by the two appliers that do not own it (with the state
   asserted byte-identical afterwards), and its place in the count in
   `command_application.rs`.
7. **Document the wire form** in [`docs/IPC_PROTOCOL.md`](../IPC_PROTOCOL.md) §5.
8. `cargo test -p prism-domain` to regenerate the TypeScript.

---

## 5. Adding a window type

1. **Add the variant** to `prism_domain::WindowType` **and to
   `WindowType::ALL`**, which is what the control editor's chooser, the proptest
   strategy and three tests walk.
2. **Add the case** to `ui/src/canvas/content.tsx`. A window this build cannot
   draw is deliberately *left out of the canvas* rather than drawn as an empty
   frame, so a missing case is silent — the switch is what makes it not.
3. **Write the component.** It scrolls **inside its own body** and never outside
   the canvas (`CLAUDE.md`). It reads through `ui/src/mirror/` and holds nothing:
   what it draws must survive being closed and reopened, and must be the same on
   a second screen.
4. **If it is not built yet, say so in the window.** Two of the fourteen do
   exactly that, on purpose: an operator who opens one from an X-Touch F-key
   should find an answer rather than an empty rectangle they will file a bug
   about.
5. **Write its chapter** in [`operator.md`](operator.md) — a section headed with
   the variant name, and a row in the generated table. The test will rewrite the
   table with a `TODO` and fail until both exist.
6. `cargo test -p prism-domain` for the bindings, then `npm --prefix ui run test`.

---

## 6. Adding a fixture type — or teaching the library reader

Most of the time the answer is **not** a new `AttributeType`. Read this order:

**Is it a label an operator reads, or a key a preset is filed under?** A label
comes out of the fixture file — `AttributeDef::label` carries the
manufacturer's own channel name onto the encoder. A key must **not**, because
`1 gobo at 50` has to reach the head whose file says *Gobo* and the one whose
file says *Gobo Wheel* alike. The Open Fixture Library's capability types are a
**closed set**, so there is no open-ended name to lift anyway.

**Does the format state a distinction the model does not have?** Then the enum
grows, and it grows by **appending** — the first fifteen rows are asserted to be
where they are, because that is what keeps *Red, Green, Blue, White* the first
page of the colour bank. `AttributeType::ALL` is 41 rows today.

**Is it a channel the reader does not understand?** Then it is already handled:
every DMX slot that reaches no other attribute reaches `AttributeType::Raw`, on
the Control bank, under the manufacturer's own name or `Ch 7`. Do not add a
special case; if `Raw` is producing a bad answer, the fix is in the reader.

When the enum does grow:

1. Append the variant and put it on a `FeatureGroup`.
2. Decide its **resting value**: `is_additive_emitter` decides open vs shut, and
   a wheel is neither (its value is a slot number). Getting this wrong makes a
   rig come up lit — see §3.
3. Add it to `prism_core::library::ofl`'s mapping.
4. **Run the corpus.** `every_channel_in_the_installed_library_maps_to_an_attribute`,
   `no_slot_of_any_profile_is_out_of_reach`,
   `no_profile_in_the_installed_library_rests_a_colour_shut` and
   `every_profile_in_the_installed_library_is_one_a_show_accepts` are the four
   that will find what a hand-written test cannot. They need the library
   installed.
5. Check the tick still allocates nothing, and whether the new shape deserves an
   eleventh path in `tick_allocations.rs`.

---

## 7. Adding an output kind

The seam is `prism_protocols::DmxOutput` — three methods — and it was chosen so
that the **tests** can implement it.

1. **Write the packet by hand, against the specification, field by field.** Both
   network protocols here are: the seam an external crate would need is larger
   than the packet, and the requirement is a byte-for-byte assertion against a
   published standard, which is easiest to trust when the bytes are written once
   beside the field names.
2. **Put the socket behind a trait.** `UdpSender` and `FtdiBackend` are the two
   that exist. This is what lets the packet be asserted with no network present
   *and* asserted again on a datagram received over loopback.
3. **Add the variant** to `prism_domain::OutputKind`, with its parameters, and
   the settings panel row that edits it.
4. **Decide the three things every driver has to answer:**
   - what its `health` means, and whether *the socket accepted it* is honest
     (for a network protocol it is not — that is punch-list B6);
   - what a **refresh** costs, and whether it goes out one cadence early so the
     maximum gap is a maximum;
   - what happens on shutdown.
5. **Reconnect, do not fail.** A cable pulled mid-show is the expected case:
   `catch_unwind`, exponential backoff, the light goes red, the engine keeps
   running.
6. **Never broadcast, and never send multicast from a test.** A suite that put
   lighting data on the network it runs on is doing to a build server what a
   broadcast does to a school.
7. Record what only real hardware can answer as a row in
   `ARCHITECTURE_SPEC.md` §14, **with a recipe somebody without this repository
   could follow** — and hold the device-specific facts as *data* so verifying
   them is a data update and one test, not a refactor.

---

## 8. The gates, and what each one is for

All of these are green on every commit, and a pull request is not finished until
they are. **Run the Rust and the interface suites one at a time** — running both
at once makes browser tests fail on timeouts that pass on their own.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo doc --workspace --no-deps          # with RUSTDOCFLAGS="-D warnings"
npx --prefix ui tsc -b --force
npm --prefix ui run lint
npm --prefix ui run test
npm --prefix ui run build
npm --prefix ui run e2e
```

CI runs seven jobs, and each is there for a reason worth knowing:

| Job | What it is for |
|---|---|
| **Windows — full build and test** | The release target. Everything, including the corpus |
| **Windows — the shell and its installer** | An installer that only ever builds on one person's machine is a file, not a release. It also *inspects* the produced `.exe`, because a bundler given a configuration with no payload exits zero happily |
| **Linux — platform-neutral crates** | The crates that may contain no `#[cfg(target_os)]`, run on a second platform. That is what proves the claim |
| **Linux ARM64 — cross-compile check** | Catches platform code leaking into a crate that is supposed to be neutral, so a portability break fails on the commit that caused it rather than a year later |
| **UI — typecheck, lint, test, build** | With the bindings regenerated from Rust first, so a stale binding cannot pass |
| **UI — end-to-end against a daemon** | A real `prismd`, a real Chromium, and a daemon the spec *kills* under the browser |
| **Web — the documentation site** | A site that only builds on one machine is the same problem as an installer that does |

The doc gate is worth a note: `cargo doc` is run with warnings denied, so a
broken intra-doc link fails the build. The crate documentation is a large part
of what this project knows, and a link that silently stopped resolving is a
paragraph nobody will read.

---

## 9. Testing rules that are not negotiable

From `CLAUDE.md`, plus what practice added:

- **No test may need a device.** The whole suite runs on a laptop with nothing
  plugged in. A tool that needs hardware lives *outside* the workspace, and what
  it produced comes back as a recorded fixture the ordinary suite replays.
- **No test may need a window.** The shell's decisions are functions a test calls
  with no Tauri anywhere. What is genuinely left to the platform is a row in
  §14 with a recipe.
- **No test sends multicast, and none broadcasts.**
- **Test-driven for core logic.** Anything touching the programmer, the cue
  engine, the merge or the surface translation arrives with its tests.
- **Coverage:** ≥ 85 % global, > 95 % on the engine, the programmer and the
  protocols. A coverage measurement without `cargo llvm-cov clean --workspace`
  in front of it — and between two crates — is not a measurement.
- **A performance number measured on a busy machine is not a number.** Run the
  bare-tick control beside the full one; the pair is the reading, never the
  first figure alone.
- **Wait for state, not for a duration** (§3).

---

## 10. How work is recorded

Three files, and they are not interchangeable:

| | |
|---|---|
| [`IMPLEMENTATION_PLAN.md`](../../IMPLEMENTATION_PLAN.md) | What each session is, with deliverables and **exit criteria**. Session numbers are identity, never order; the running order is a table at the end |
| [`PROGRESS.md`](../../PROGRESS.md) | What was **measured**, not what was intended. §2 is one verification record per session, §5 the open verifications, §6 the decision log, §7 the lessons carried forward, §8 the prompt that starts the next session |
| [`docs/ISSUES.md`](../ISSUES.md) | What is known to be wrong, in German, numbered `Bnn`. Numbers are identity and are never reused; an entry is closed with ✅, ⛔ or ➡️ and **never by deleting it** |

Commits are Conventional Commits: `feat(engine):`, `fix(mcu):`,
`test(programmer):`, `docs(arch):`.

**Markdown is updated in the same pass as the code**, not in a follow-up commit.
That is an explicit instruction from the owner, and it is why a manual chapter
and the code it describes cannot be one commit apart.

---

## Where to go next

| | |
|---|---|
| [`../../ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) | Why it is built this way |
| `cargo doc --workspace --no-deps --open` | The crate documentation, which is unusually complete and is the specification of each type |
| [`../DMX_MERGE.md`](../DMX_MERGE.md) · [`../IPC_PROTOCOL.md`](../IPC_PROTOCOL.md) · [`../MCU_MAPPING.md`](../MCU_MAPPING.md) · [`../COMMAND_LINE.md`](../COMMAND_LINE.md) | The four references |
| [`../../CLAUDE.md`](../../CLAUDE.md) | The standards this project holds itself to |
