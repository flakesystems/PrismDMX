# PrismDMX 0.9.1 — Closed Beta 2

The second beta, and the first one built out of what the first one found. Every
change below started as an entry somebody wrote down after trying to do
something real with `0.9.0`; five of the six are faults, and the sixth is a
sentence the documentation was missing.

It is still a **pre-release**: complete enough to build a rig, program a show and
run it on real hardware, and it has not yet run a performance in anybody's
building. That is what the beta is for.

## What changed since 0.9.0

**The crossfade fader works the way a crossfade fader works.** It used to
complete its fade and then get driven back to zero for the next one — so every
cue needed a Go before the fader did anything, and the desk kept taking the
handle out of your hand. Now **nothing ever moves it**. There are two modes,
chosen per executor:

- **XFade** — push up to crossfade to the next cue, pull down to crossfade to
  the one after. Each half of the travel is a cue and the stage never goes dark.
- **Fade** — push up to take the current cue out, pull down to bring the next one
  in. One up-and-down is a cue, through black.

Stop half way and the desk **holds the mixture there** for as long as you like.
Walk a whole cue list with one fader without lifting your hand.

**Clear lets go before it forgets.** The first press used to take everything you
had programmed and leave the fixtures selected, which is the wrong way round for
the thing people actually do: select, set, let go, select the next, set. So the
order is reversed — **first press drops the selection and keeps the look**,
second press takes the values, third puts the encoder bank and the page back. The
key still says which of the three the next press would do. Emptying the
programmer by hand now takes two presses; that is the trade.

**A third of the fixture library was arriving with channels missing.** Of the
15 150 channels in the installed library, **5 037 reached nothing at all** and
were quietly dropped: all CMY colour mixing, every colour wheel, every built-in
effect, frost, fog, the framing shutters, gobo and prism rotation. The desk
understands **34 parameters** now instead of 15, one for every kind of channel
the Open Fixture Library has, and **no channel in the library is left
unassigned**. The encoder banks are still the same seven and the first page of
each is unchanged, so nothing you already knew has moved.

**And the channels that have named positions now say so.** A gobo wheel, a
colour wheel, a strobe channel — the encoder tells you which slot you are
standing in (*Gobo 3*, *Strobe slow*, *Open*) and offers the list to pick from,
instead of leaving you to know that gobo 3 lives at 27 %.

**Your own fixture profiles have a home.** `fixtures/` inside the desk's own
directory — `%APPDATA%\PrismDMX\fixtures` — which no install ever touches. Drop
a `.json` in and it appears in the picker marked **yours**; put one in a
manufacturer directory with the same name as a profile that shipped with the desk
and yours **replaces** it. Once patched, the profile travels inside the `.prism`
file, so a show opens the same way on a desk that has never seen your directory.

**`F11` and `Alt` + `Enter` put the desk full screen**, and take it out again. A
desk in a hall runs without a title bar.

**A desk killed from Task Manager no longer leaves a dead tray icon behind.** The
shell notices within a second, says so, and closes rather than sitting there with
a *Stop the desk* that cannot find anything to stop — and a second start no
longer puts a second icon beside the first.

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

### Since the first beta

The desktop shell, the operating system's own file dialogues for every path,
opt-in autostart that needs no administrator rights, a per-user installer and a
build that comes off CI rather than off one machine — all of that arrived in
`0.9.0` and is unchanged here.

## What is missing

Named here rather than left to be discovered:

- **No 3D visualiser**, **no Web Remote** (a phone cannot drive the desk yet),
  **no timecode, OSC or PSN**. All planned.
- **A fixture with two of one parameter loses the second.** A head with two
  colour wheels, or an LED tube whose profile writes out a red per pixel, keeps
  the lower channel and drops the higher — this desk gives one fixture one of
  each parameter. Every *kind* of channel now arrives; what is left is the
  repeats, and it is the next thing on the list for the fixture library.
- **A named range is a label, not a slot.** Picking *Gobo 3* writes the middle of
  that range as an ordinary number, and that is what a cue stores. Nothing keeps
  track of the fact that it was a slot, so a fixture whose profile changes
  underneath a stored cue keeps the number rather than the name.
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
