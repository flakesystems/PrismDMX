# Changelog

What changed for somebody who uses the program — every version, briefly.

**This is the English version; the German one is [`CHANGELOG.md`](../CHANGELOG.md)
in the repository root**, which is the file GitHub shows and the one the release
tooling reads. They are the same list in two languages, and a test holds both to
the same set of versions.

Not to be confused with `docs/RELEASE_NOTES.md`, which is a different document
rather than a third copy: **every** version gets a few lines here, the
**current** one gets its full length there, and that is the text which appears on
the GitHub release page.

**It is written, not generated.** `PROGRESS.md` §2 would be the obvious source
and is the wrong one: it is a verification log for developers, full of sentences
about tests and measurements. Prose generated from it would be exactly the
document a user can do nothing with. What is mechanical is the **set of
versions**, and that is checked.

Every version so far is a **pre-release**.

---

## Not yet released

**These changes ship as 0.9.3.** The version is not on the program yet — the
owner held the release until the Controls round (S59) and the 3D viewer (S30)
were in it, and both now are.

### The 3D viewer (S30)

**The *Viewer 3D* window shows the rig as it hangs**, and a beam out of every
fixture that is lit, in the colour it is putting out. The beams come **off the
cable** — the same levels as the DMX Sheet — so what you see is what the rig is
actually being told: pan, tilt, zoom, dimmer, colour, a closed shutter.

- **Placing fixtures.** A panel beside the picture works on the selection:
  *Set* gives them a position and a rotation, *Spread* lays the selection out
  across the stage — a truss of eight in one go. Each is **one Oops**, and
  placing costs the DMX output nothing.
- **Look around** with the mouse, plus *Front*, *Top*, *Side*, *3D* and *Frame
  all*. The camera is the screen's own; a second screen may show the rig from
  somewhere else.
- **A click on a fixture** selects it, as a click in the Fixture Sheet does.
- A profile from a **GDTF** file is drawn at its size, with its beam where the
  manufacturer says it leaves the body; any other as a small box. The devices'
  own 3D models and the gobo pictures come later.
- It draws on an ordinary 2D surface, so it runs on every machine the desk runs
  on — graphics card or not.

### The fixture library is GDTF now (S61)

**The desk reads [GDTF](https://gdtf.eu)** — the format manufacturers publish
their devices in, and the one a rig is exchanged between programs in. A `.gdtf`
file carries what a channel list cannot: **the pictures of its gobos**, the
device's **3D model**, its size, and **where its beam comes out** and which way
it points. That is what the 3D viewer needs, which is why this comes before it.

- **The installed library is GDTF.** `tools/fetch-fixtures/fetch-fixtures`
  installs it, from a folder of `.gdtf` files or from a free account on
  [gdtf-share.com](https://gdtf-share.com) — that service has no anonymous bulk
  download, which is why the script asks for one and says so when it is given
  neither.
- **Your own fixtures in the Open Fixture Library's format keep working.** A
  lamp nobody has published a GDTF for is still written as JSON in `fixtures/`
  in the data directory; by hand that is far easier than a ZIP full of XML. Both
  stand side by side in the list, and what is in the data directory still wins.
- **A `.gdtf` in your own folder replaces the library's copy of that fixture
  whatever it is called** — the key comes out of the file, not out of the file
  name.
- **The patch window says where a profile came from.** A new *Format* column
  (`GDTF` or `OFL`), and a line under the chosen fixture's name saying what it
  carries — *3D model · 1 beam*.
- **Gobos have names and pictures.** Where a GDTF describes a wheel, a channel's
  steps are named after the wheel's slots, and the picture's name travels with
  the profile into the show.
- The desk says at start-up how many profiles it offers and **how many of them
  are GDTF** — and says so separately when the installed library has none yet.

### Four ways to fill the library (S62)

**A GDTF no longer has to be put into a folder by hand.** The Patch window has
two new keys, and the settings have a section:

- ***Import profile (GDTF)*** takes one `.gdtf` through the file dialogue,
  reads it, puts it in your own fixture folder and offers it in the picker at
  once. Anything that is not a readable fixture file never reaches the folder.
- ***Import rig (MVR)*** takes your planner's rig file **into the show**: every
  profile in it and every planned fixture with its number and address, **in one
  step that one Oops takes back**. Nothing already patched is touched — a
  planned fixture whose number your show is already using gets the next free
  one. The desk then says how many arrived and what it skipped. An `.mvr` simply
  left in the fixture folder still fills the library without the key.
- ***Settings → This machine → GDTF Share*** downloads the whole published
  library **with your own account** at [gdtf-share.com](https://gdtf-share.com)
  into your fixture folder. A line counts them, the desk keeps running lights
  while it works, and there is one sentence at the end. If you ask it to,
  Windows keeps your credentials in its **credential manager** — never in a
  settings file; *Forget this account* takes them back out.

**Nobody needs an account.** The Open Fixture Library is still there, your own
file in the folder still beats the installed one, and an `.mvr` solves the real
case with no internet at all. Why the desk does not simply ship the GDTF library
is in `docs/FIXTURE_LIBRARY.md`.

What the open beta reported in its first two weeks — ten reports, all ten
fixed (S56 and S57), and the register's last open entry with them (B52, S58).
**All ten GitHub issues are closed** (#9, #21, #23–#30):

- **Controls:** binding a key to *choose a window* or *write a command*
  disconnected the client every time the menu was opened. Fixed (B55).
- **`fixtures/`** is made on the first start (B54).
- **Number fields** in the patch and output forms can be emptied while typing;
  they are checked when applied (B53).
- **Command line:** changing view leaves the line where it is (B56); Oops takes
  the last word first, on the X-Touch too (B58); the feedback no longer moves
  anything (B61).
- **Windows:** a click into a window that was not focused selects at once (B57).
- **Crossfade:** every handle shows the same position, and the motor fader no
  longer goes back when it is let go of (B59).
- **Cue Viewer:** no store bar — cues are stored from the command line; the
  Update key blinks in the *CommandKeys* window (B62).
- **Patch window**, rebuilt around the library (B60, S57): *Add fixture* opens
  the library and the settings side by side; every fixture is **one** row, its
  mode chosen beside it, and the whole row picks; the list loads more as it is
  scrolled; a new fixture starts at the **next free address**, which an overlap
  names too; a fixture with no name is named after its type; and **several of
  one type** are patched in one step that one Oops takes back.
- **Mode channels** (B52): a knob whose channel another one switches — the
  neighbour of *Mode Select* on an ADJ Flat Par QA12 — is now called what it is
  right now (*Strobe*, *Program Speed*, *Sound Sensitivity*) and offers that
  channel's steps. What counts is what is on the cable, so it follows a cue as
  well.

**And the Controls round (S59)** — the desk gains the command line's vocabulary,
and its lamps start saying something:

- **Every console key can be put on the desk.** `Store`, `Edit`, `Label`,
  `Color`, `New`, `Fixture`, `Cue`, `At`, `Thru` and the rest are the same list
  the *Command Keys* window shows — one table for both devices, so a word cannot
  reach one of them and not the other. `Color`, `New`, `At` and `Thru` are new,
  and they have been added to the screen's keypad as well.
- **A bound key lights up when pressing it would do something.** There were two
  lamps on the whole desk until now and both hung on where the key sits; the
  lamp follows **what the key is bound to** now: `Store` is dark until the
  programmer holds something, `Clear` stays lit until the three-stage clear has
  nothing left to take, an argument keyword like `Cue` lights only once a verb
  is waiting for its object.
- **The jog wheel is much faster**, in the unit that decides whether anything is
  visible: a slow click now moves **exactly one DMX step** instead of a
  thirteenth of one, and a fast one four. There is a slider for it in *This
  machine → Jog wheel*, 10 to 400 %.
- **The Controls page is tidier.** The strips, the main fader and the jog wheel
  belong to the Executors window and to the programmer, and have moved into a
  collapsible *Advanced* section; the *Strip / Selected* column is gone, because
  the rows that remain all mean the selected executor.
- **The Controls page has a picture of the desk.** Beside the list, a drawing
  of the surface to scale with every key where it really is — filled in means
  bound, hollow means free, and whatever is lit on the real desk glows here too.
  It is how you see which keys are still free while you are laying one out.
- **The shipped key table is complete** — every key on the panel does something
  except SMPTE/Beats (reserved) and Name/Value (no lamp). An update **replaces**
  a desk's own table; export it first if you want to keep it.

---

## 0.9.2 — the fixture library, completely

*6 September 2026 ·
[Release](https://github.com/flakesystems/PrismDMX/releases/tag/v0.9.2)*

All of this started with the owner wanting to do something with `0.9.1` and not
being able to. It is one subject: **what becomes of a fixture profile when you
patch it.**

- **A fixture with two channels of the same kind keeps both.** A head with two
  colour wheels, an LED tube with one red per pixel: across the installed library
  that was **2 679 channels** that simply did not exist. They are there now and
  **numbered** — *Gobo* and *Gobo 2*, `1 gobo 2 at 50`. Where there are more
  repetitions than fit side by side, the programmer bar gets a **Part** switch.
- **Warm white and cold white are two lamps.** They used to be one attribute, and
  on a fixture with both the second channel did not answer at all. The desk now
  knows **41 parameters** instead of 34.
- **The encoder banks show only what the selected fixtures have.** One PAR
  selected means four colour knobs, not thirteen.
- **Wheel slot names are the manufacturer's**, and a **right-click** on the
  encoder opens the list. Every fourth slot used to read *Slot 3*.
- **Profiles with a pixel matrix can be patched.** 90 of the 634 fixtures
  describe their channels as *repeat once per pixel*, and every such mode was
  skipped. The library now yields **2 871 profiles** instead of 2 157.
- **Every DMX channel of a patched fixture has a knob**, without exception. Where
  the profile does not say what a channel does, it sits on the *Control* bank
  under the manufacturer's name or as `Ch 7`; from the line, `1 raw 3 at 50`.
  **707 channels** across 337 profiles used to be unreachable.
- **Every encoder carries the name the manufacturer gave the channel**, and a
  wheel sits on the bank its *content* names — a colour wheel under *Colour*,
  even where the profile calls it something else.

A `.prism` file from `0.9.1` opens unchanged.

## 0.9.1 — the second closed beta

*6 September 2026 ·
[Release](https://github.com/flakesystems/PrismDMX/releases/tag/v0.9.1)*

The first version built from what the first beta found. Six entries, four of them
faults.

- **The crossfade fader works like a crossfade fader.** It used to snap back to
  zero after every fade. Now **nothing ever moves it**, and there are two modes
  per executor: **XFade** fades up to the next cue and down to the one after,
  **Fade** fades the running one out and the next one in. Stopping half way holds
  the mixture.
- **`Clear` lets go before it forgets.** First press: the **selection** goes, the
  look stays. Second: the values. Third: bank and page. That is the order in
  which people build a look out of several fixtures.
- **A third of the fixture library arrived with channels missing.** 5 037 of
  15 150 channels reached nothing at all — all CMY colour mixing, every colour
  wheel, every built-in effect, frost, haze, the shutters. The desk now knows
  **34 parameters** instead of 15, and **no channel in the library is left
  unmapped**.
- **Channels with named slots say so.** The encoder names the slot it is in and
  offers the list.
- **Your own fixture profiles have a place:** `fixtures/` in the data directory,
  which no installation touches.
- **`F11` and `Alt` + `Enter` toggle full screen.**
- **A desk killed from the Task Manager no longer leaves a dead icon** in the
  notification area.

## 0.9.0 — the first closed beta

*1 September 2026 ·
[Release](https://github.com/flakesystems/PrismDMX/releases/tag/v0.9.0)*

The first version anybody could install: two processes became one program.

- **An installer**, per user and without administrator rights, carrying the
  engine, the interface and the fixture library together — and coming out of CI
  rather than off somebody's machine.
- **A window you can close without ending the show**, with an icon in the
  notification area that says so, and an orderly shutdown.
- **A second start attaches to the running desk** instead of starting another
  one.
- **The operating system's own file dialogs** for every path.
- **Autostart** as a checkbox, without administrator rights, with a line that
  says what the machine actually has.

Underneath it lay what the sessions before had built: the desk as a process of
its own, the merge and the 44 Hz tick, Art-Net, sACN and Open DMX USB, the
programmer, cue lists with tracking, executors, the X-Touch, the command line as
the interface, and the settings window.
