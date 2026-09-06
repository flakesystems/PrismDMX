# PrismDMX

A DMX lighting console for venues and schools. The desk and its engine are one
program: install it, start it, and the show runs — with or without a window open.

> **Closed beta.** This is a pre-release. It is complete enough to build a rig,
> program a show and run it on real hardware, and it has not yet run a real
> performance in anybody's building. That is what the beta is for. Read
> [Being a beta tester](#being-a-beta-tester) before you put it on a show that
> matters.

---

## What it is

PrismDMX is two processes that ship as one program.

- **`prismd`** — the engine. It owns the show, the DMX output, the timing and
  the control surface, and it keeps running whether or not anything is looking
  at it. Closing the window does not stop the show; nothing but an explicit
  instruction does.
- **The desk** — the interface, in a window. It draws what the engine says and
  sends it what you press. It holds no state of its own, so a second screen, a
  laptop on the other side of the room, or the console's own faders are all
  looking at the same desk.

Everything follows from that split. The engine is what must not fail, so it has
no user interface in it at all.

### What it can do today

- **A rig**: patch fixtures from the Open Fixture Library (634 fixtures, 2 157
  profiles ship with it) or from built-in generic profiles, addressed across up
  to 64 universes.
- **Output**: Art-Net (with node discovery), sACN (E1.31), and Open DMX USB /
  FTDI adapters — several at once, each carrying the universes it is actually
  wired for.
- **Programming**: a programmer with groups, presets, cue lists, cue tracking,
  fade and delay times, store modes and an undo history.
- **Playback**: executors with faders, encoders and assignable keys; speed
  masters; a cue list that lands in the same place whether you walked to a cue
  or jumped to it.
- **A command line** that is the interface — every key on the desk writes a line
  and the line is what acts, so a gesture and a typed sentence are the same
  thing.
- **A control surface**: a Behringer X-Touch over USB, with every key bindable
  in the desk itself.
- **A settings window** for the rig, the devices, the show files and the
  machine — there is nothing you have to pass on a command line.

### What it cannot do yet

Named here rather than discovered by you:

- **No 3D visualiser** (planned).
- **No Web Remote** — a phone or tablet cannot drive the desk yet (planned).
- **No timecode, OSC or PSN** (planned).
- **A profile whose channels switch on another channel's value is skipped.** The
  Open Fixture Library calls those *switching channels*; a mode with one loses
  the channels that depend on it, because a footprint that changes while the
  show runs is not one this desk can address yet. Matrix profiles — a red per
  pixel — do arrive.
- **Executor labels do not follow a show change** in every case: after loading a
  second show, a strip may keep the previous show's labels until you switch
  windows.
- **Autostart is Windows-only.** The setting exists everywhere; the start-up
  entry is written only on Windows.
- **macOS and Linux are not released.** The engine is portable and is built for
  ARM64 on every commit, but only Windows has an installer.

---

## Installing it

**Windows 10 or 11, 64-bit.** Download `PrismDMX_<version>_x64-setup.exe` from
the [latest release](https://github.com/flakesystems/PrismDMX/releases) and run
it.

The installer is **per-user**: it needs no administrator rights and installs into
your own profile. That is deliberate — a school laptop an operator cannot
administer is the ordinary case, and everything the desk does, autostart
included, works without elevation.

Nothing has to be installed alongside it: the whole program is linked against the
static C runtime, so there is no Visual C++ redistributable to chase. The one
exception is the **Edge WebView2 runtime**, which draws the window — Windows 11
has it, Windows 10 has it wherever Edge has been updated, and the installer
fetches it from Microsoft if it is missing. That is the only step that needs the
internet.

### The SmartScreen warning

**This build is not code-signed.** Windows will show *Windows protected your PC*
the first time you run the installer. That is not a judgement about the file; it
is what Windows says about every executable it has not seen many times before,
and a signing certificate is not something this project has yet.

To continue: click **More info**, then **Run anyway**. If you would rather not,
that is a completely reasonable position — check the SHA-256 of the download
against the one in the release notes first, or wait for a signed build.

### What it installs, and what it leaves alone

| | Where |
|---|---|
| The program | `%LOCALAPPDATA%\PrismDMX` (per-user) |
| Your shows, settings and rig | `%APPDATA%\PrismDMX` |

**Uninstalling removes the program and nothing else.** Your shows,
`machine.json`, your fixture corrections and your control map stay in
`%APPDATA%\PrismDMX`, and an upgrade finds them again. If you really want a
clean slate, delete that folder by hand — the uninstaller will not do it for you,
because there is no version of "I want to reinstall this" that also means "throw
away my show".

### Upgrading

Run the new installer. It replaces the program in place; it does not touch your
data, and it does not stop a desk that is running — but a desk that is running is
still the old build until you restart it.

---

## Starting it

Start **PrismDMX** from the Start menu. The first start makes a data directory,
an empty show and this desk's own identity, and puts a window up.

- **Closing the window does not stop the show.** The window hides; the engine
  keeps running and keeps sending DMX. A tray icon says so.
- **The tray menu** has three items, and they say what they do:
  - *Show the desk window* — bring it back.
  - *Close this window, leave the desk running* — quit the interface. The show
    carries on.
  - *Stop the desk* — stop the engine, in order: clients are told, the
    configured blackout-or-hold is put on stage, and the outputs are given time
    to send it before anything closes.
- **Starting it twice attaches to the desk that is already running.** There is
  never a second engine: two desks driving one rig is the worst failure this
  program can have, and it is prevented structurally.
- **Autostart** is a tick-box in *Settings → This machine*. It needs no
  administrator rights, it starts the desk into the tray at log-in, and the row
  underneath tells you what your machine actually has — not merely what you asked
  for.

### The engine on its own

`prismd.exe` sits beside `PrismDMX.exe` in the installation directory and can be
run on its own — a rack machine with no screen, a Raspberry Pi, a permanent
installation. `prismd --help` lists everything, and every operational option is
also a setting in the window.

---

## The basics

A first show, in the order it is usually done:

1. **Patch a rig.** *Settings → Outputs* to say what the building is wired with,
   then the **Patch** window to say what is on it. Search the fixture library by
   name; give each fixture a number and an address.
2. **Select and program.** Type `1 thru 6 at full` on the command line, or click
   the fixtures and turn the encoders. The programmer holds what you have
   changed until you store it or clear it.

   The encoder banks show **only the parameters the selected fixtures actually
   have**, so a four-colour PAR is four knobs and not a bank you page through.
   A fixture with **two of a parameter** — a head with two colour wheels, a tube
   with a red per pixel — numbers them: *Gobo*, *Gobo 2*, and `1 gobo 2 at 50`
   on the command line. Where there are more repeats than fit side by side the
   band grows a *Part* stepper instead.

   **Right-click an encoder to pick a step.** A gobo wheel, a colour wheel or an
   effect channel is a list of named positions in the fixture's own profile, and
   the window that opens is that list — the manufacturer's names, not numbers to
   count out.

   **Every channel of a patched fixture has a knob**, without exception. Where
   the profile describes what a channel does, the knob is that — a red, a gobo
   wheel, an iris. Where it does not, or where the meaning of the channel
   switches on another channel's value, there is still a knob for it on the
   **Control** bank, named the way the fixture's own profile names it (or
   `Ch 7`, the channel's place in the fixture). No slot of a fixture you have
   patched is out of reach.

   Each encoder is labelled with **the name the manufacturer gave that channel**
   — *Rotating Gobo*, *Color Wheel 2* — rather than the desk's generic word for
   it, and a wheel goes to the bank its **contents** say it belongs on: a wheel
   of colours is under **Colour** even when the profile calls it something else.
   Select two heads that name the same knob differently and the desk's own word
   comes back, because naming one of them would be wrong about the other.
3. **Store it.** `Store Cue 1` writes what the programmer holds into cue 1 of
   the selected sequence. `Store Group 3 "Front wash"` makes a group.
4. **Put it on a fader.** `Assign Sequence 1 Executor 1`, and executor 1's keys
   and fader drive that cue list.
5. **Run it.** `Go`, or the executor's own key, or the X-Touch.
6. **Save.** *Settings → Show files*, or `Ctrl-S`. The desk also autosaves a
   recovery copy while there is anything unsaved.

**`F11` or `Alt` + `Enter` puts the desk full screen**, and takes it out again. A
desk in a hall runs without a title bar.

**Clear lets go before it forgets.** The first press drops the fixture
*selection* and keeps everything you have set, so the next fixture joins the same
look; the second press takes the values; the third puts the encoder bank and the
page back. The key says which of the three the next press would do.

**A crossfade fader is walked, not reset.** An executor's fader can be one of two
crossfades. *XFade* crossfades to the next cue on the way up and to the one after
on the way down, so you walk a cue list with one fader without lifting your hand.
*Fade* takes the current cue out on the way up and brings the next one in on the
way down. Stop half way and the desk holds the mixture there. Nothing ever moves
the fader back for you.

**Everything is the command line.** Every key, every tile and every button writes
a line rather than acting on its own, so what a gesture means is what the line
means — and any line can be bound to a key on the X-Touch.

### Your own fixture profiles

A light the Open Fixture Library does not know is a profile you can write
yourself. It goes in **`fixtures/` inside the desk's own directory** —
`%APPDATA%\PrismDMX\fixtures` on Windows — in the Open Fixture Library's own
JSON format, and the desk reads it at start-up.

That directory and not `profiles/fixtures/`: the second one is a **download**,
and `tools/fetch-fixtures` empties it on every run, so anything of yours put
there survives until the next install and no longer. The desk's own directory is
never touched by an installer.

Two shapes, and both work:

| Where you put it | What it is for |
|---|---|
| `fixtures/my-light.json` | A light nobody has a profile for. It is filed under *Custom* and appears in the picker beside everything else |
| `fixtures/<manufacturer>/<fixture>.json` | A **correction** to a profile that came with the desk. Use the same manufacturer directory and file name the library uses and yours replaces it |

Either way the picker marks the row **yours** in its *Source* column, so a
profile you wrote is one you can find again. And once you have patched a fixture
with it, the profile is **copied into the show**: a `.prism` file is
self-contained, so it opens the same way on a desk that has never seen your
directory.

**The manual is not written yet.** It is a session of its own; until then, this
list and the hints inside each window are what there is, and the beta is a good
time to say which of them was not enough.

---

## Being a beta tester

Thank you. What is most useful is not a bug list — it is what happened when you
tried to do something real.

### What to report

Everything, including things you are not sure are faults. An entry that turns
out to be intended costs one line of explanation; a fault nobody wrote down costs
the first evening somebody else runs a show on it.

Especially:

- **Anything the desk does to the stage that you did not ask for.** Light coming
  on by itself, a level that moves when nothing moved it, an output that stops.
  These come first, always.
- **Anything you could not find.** A function that exists but you could not
  reach is the same problem as one that does not exist.
- **Anything that lost work** — a show that would not open, an edit that
  vanished, a save that did not.
- **What it was like to use** under time pressure, in a dark room, with somebody
  waiting. That is the thing no test can measure.

### How to report it

Open an issue at
[github.com/flakesystems/PrismDMX/issues](https://github.com/flakesystems/PrismDMX/issues)
and say:

1. **What you did** — the steps, in order, however small.
2. **What happened.**
3. **What you expected instead.**
4. **The version** — *Settings → This machine* names it, and so does
   `prismd --version`.
5. **The rig** — what was plugged in, and whether the fault also happens with
   nothing plugged in.

A screenshot of the whole window helps more than a crop, because the state of
the rest of the desk is usually the interesting part.

### What to send with it

- **The log.** The desk writes to `%APPDATA%\PrismDMX`. Turn the level up in
  *Settings → This machine* before reproducing something, if you can.
- **The show file**, if the fault needs it and it is not confidential. A
  `.prism` file is self-contained.
- **The machine configuration** (`%APPDATA%\PrismDMX\machine.json`) if the fault
  is about the rig, the outputs or the surface. **Read it first**: it contains
  this desk's access token if you have set one.

### What not to do with it yet

- **Do not run a performance you cannot abort on this build.** Have a way back:
  a second desk, a house lights switch somebody can reach, or a rehearsal you can
  stop.
- **Do not put it on a network you do not control** with the listener moved off
  loopback. It ships on `127.0.0.1` for a reason, and moving it off asks for a
  token because a shared school network is exactly where an unauthenticated
  lighting console does not belong.

---

## Building it yourself

You need [Rust](https://rustup.rs) (stable, 1.89 or newer), [Node](https://nodejs.org)
24, and — on Windows — the MSVC build tools and the WebView2 runtime (Windows 11
has it already).

```bash
git clone https://github.com/flakesystems/PrismDMX.git
cd PrismDMX
tools/fetch-fixtures/fetch-fixtures.sh   # or .ps1 on Windows
cd ui && npm ci && cd ..
cargo build --workspace
```

Run the engine and the interface separately while developing:

```bash
cargo run -p prismd -- --mock-output
```

```bash
npm --prefix ui run dev
```

Or build the whole thing as one program:

```bash
cargo build --release -p prismd
```

```bash
cd crates/prism-app && ../../ui/node_modules/.bin/tauri build --config tauri.bundle.conf.json
```

The engine goes first because the bundle carries it as a resource, and the Tauri
CLI has to be run from the directory holding `tauri.conf.json`. The second
configuration is what names the payload — the daemon, the fixture library and
the surface profile — and it is separate so that an ordinary
`cargo build --workspace` does not demand a release build of the daemon. The
installer lands in `target/release/bundle/nsis/`.

### The gates

All of these are green on every commit, and a pull request is not finished until
they are:

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo fmt --all --check
```

```bash
npm --prefix ui run test
```

```bash
npm --prefix ui run e2e
```

**Run the Rust and the interface suites one at a time.** Running both at once
makes browser tests fail on timeouts that pass on their own.

`CLAUDE.md` has the standards this project holds itself to — including the one
that matters most: **no test may need a device**, so the whole suite runs on a
laptop with nothing plugged in.

---

## Where things are

| | |
|---|---|
| `crates/prism-domain` | The types everything speaks |
| `crates/prism-engine` | The tick, the merge, the DMX encoding |
| `crates/prism-core` | The show, the programmer, the command line |
| `crates/prism-protocols` | Art-Net, sACN, Open DMX USB |
| `crates/prism-surface`, `crates/prism-midi` | The X-Touch, and the port it is on |
| `crates/prism-ipc`, `crates/prismd` | The protocol, and the daemon |
| `crates/prism-app` | This shell |
| `ui/` | The interface |
| `ARCHITECTURE_SPEC.md` | Why it is built this way |
| `docs/IPC_PROTOCOL.md` | What travels between the two halves |

## Licence

MIT — see [LICENSE](LICENSE).
