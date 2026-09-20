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
owner is holding the release until the Controls round (S59) and the 3D viewer
(S30) are in it.

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
