# PrismDMX 0.9.0 — Closed Beta 1

The first release anybody outside this repository can install. It is a
**pre-release**: complete enough to build a rig, program a show and run it on
real hardware, and it has not yet run a performance in anybody's building. That
is what the beta is for.

## What is in it

**One program.** The engine and the desk install together and start together.
Closing the window hides it; the show keeps running, and a tray icon says so.
Starting it a second time attaches to the desk that is already there rather than
starting a second engine — two desks driving one rig is the worst failure this
program can have, and it is prevented structurally rather than by convention.

**A rig you can build.** 634 fixtures and 2 157 profiles from the Open Fixture
Library ship with it, searchable by name, alongside the generic profiles. Up to
64 universes, addressed and patched in the desk.

**Output for a real venue.** Art-Net with node discovery, sACN (E1.31), and Open
DMX USB / FTDI adapters — several at once, each carrying only the universes it is
wired for, reconfigurable while the show runs. An Art-Net node that never answers
reads *never answered* rather than *OK*.

**Programming and playback.** Groups, presets, cue lists with tracking, fade and
delay times, three store modes, an undo history, executors with assignable
faders, encoders and keys, and speed masters. A cue list lands in the same place
whether you walked to a cue or jumped to it.

**The command line is the interface.** Every key, tile and button writes a line
rather than acting on its own, and the parser lives in the engine — so a key
bound on the X-Touch runs its line with no window open at all.

**A Behringer X-Touch**, over USB, with all 73 controls bindable in the desk
itself, and *Learn* to name the key you pressed.

**Settings rather than flags.** The rig, the MIDI port, the show files, the
network exposure, the log level, the universe count, the exit action and the
fixture library are all edited in the window. Nothing has to be passed on a
command line.

### New in this release

- The desktop shell itself: one installable program, a tray, an explicit stop,
  and *start it twice and it attaches*.
- **The operating system's own file dialogues** for every path — Open, Save as,
  New, Export, Import, the fixture library and the surface profile. Typing a path
  still works everywhere.
- **Opt-in autostart** that actually writes a start-up entry, needs no
  administrator rights, and tells you what your machine has rather than only what
  you asked for.
- A per-user installer, and a build that comes off CI rather than off one
  machine.

## What is missing

Named here rather than left to be discovered:

- **No 3D visualiser**, **no Web Remote** (a phone cannot drive the desk yet),
  **no timecode, OSC or PSN**. All planned.
- **Crossfade is not finished.** A crossfade fader completes the fade and returns
  to zero instead of holding where you left it. Known, and being reworked into
  two modes.
- **Executor labels can lag a show change**: after loading a second show, a strip
  may keep the previous show's labels until you switch windows.
- **Autostart is Windows-only.** The setting exists everywhere; only Windows
  writes the entry.
- **No manual yet.** The hints inside each window are what there is.
- **Windows only.** The engine is portable and is cross-compiled for ARM64 on
  every commit, but only Windows has an installer.

## Installing it

Windows 10 or 11, 64-bit. Run the installer below.

It is **per-user**: no administrator rights, and it installs into your own
profile. Your shows and settings live in `%APPDATA%\PrismDMX` and an uninstall
does not touch them.

Nothing has to be installed alongside it — no Visual C++ redistributable, no
runtime library. The one exception is the **Edge WebView2 runtime**, which draws
the window: Windows 11 has it, Windows 10 has it wherever Edge has been updated,
and the installer fetches it from Microsoft if it is missing. That is the only
step that needs the internet, and only on a machine that does not have it
already.

### It is not signed, and Windows will say so

**This build has no code-signing certificate.** SmartScreen will show *Windows
protected your PC* the first time you run the installer. That is what Windows
says about any executable it has not seen often, not a judgement about this file.

Click **More info** → **Run anyway**. If you would rather not, check the SHA-256
below against your download first — or wait for a signed build, which is a
completely reasonable thing to do.

## What to report, and how

Everything, including what you are not sure is a fault. Especially:

- **Anything the desk does to the stage that you did not ask for** — light coming
  up by itself, a level that moves when nothing moved it, an output that stops.
  These come first.
- **Anything you could not find.** A function you could not reach is the same
  problem as one that does not exist.
- **Anything that lost work.**
- **What it was like to use** with somebody waiting — the thing no test measures.

Open an issue at
<https://github.com/flakesystems/PrismDMX/issues> with: what you did, what
happened, what you expected, the version (*Settings → This machine*), and what
was plugged in. A screenshot of the whole window helps more than a crop.

Send the log from `%APPDATA%\PrismDMX` if you can, and the `.prism` file if the
fault needs it. **Read `machine.json` before attaching it** — it carries this
desk's access token if you have set one.

## Please do not, yet

- **Run a performance you cannot abort on this build.** Keep a way back: a second
  desk, a house-lights switch somebody can reach, or a rehearsal you can stop.
- **Put it on a network you do not control** with the listener moved off
  loopback. It ships on `127.0.0.1`, and moving it off asks for a token, because
  a shared school network is exactly where an unauthenticated lighting console
  does not belong.

Thank you for taking the first one.
