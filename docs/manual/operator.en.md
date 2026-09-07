# PrismDMX — Operator's manual

**For:** whoever builds and runs a show with it.
**Applies to:** version `0.9.2`. Which version you have is in *Settings → This
machine*, and `prismd --version` says it too.
**A note on language:** this manual exists in English and German. The labels on
screen are English, and they are given here exactly as they appear there —
`Store`, `Clear`, *Fixture Sheet* — so that the word in the text and the word on
the button are the same word.

> **Open beta.** This program is complete enough to build a rig, program a show
> and run it on real hardware. It has not yet run a performance in a building
> that does not belong to the author. That is why
> [When something goes wrong mid-show](#9-when-something-goes-wrong-mid-show)
> sits further forward than it would in a 1.0 manual.

---

## Contents

1. [What this desk is](#1-what-this-desk-is)
2. [The first start](#2-the-first-start)
3. [The screen](#3-the-screen)
4. [The windows](#4-the-windows)
5. [Patching a rig](#5-patching-a-rig)
6. [Selecting and programming](#6-selecting-and-programming)
7. [Storing: cues, groups, presets, views](#7-storing-cues-groups-presets-views)
8. [Playback](#8-playback)
9. [When something goes wrong mid-show](#9-when-something-goes-wrong-mid-show)
10. [The command line, word by word](#10-the-command-line-word-by-word)
11. [The X-Touch](#11-the-x-touch)
12. [Show files, backup and moving house](#12-show-files-backup-and-moving-house)
13. [What this desk cannot do yet](#13-what-this-desk-cannot-do-yet)

---

## 1. What this desk is

PrismDMX is **two processes shipped as one program**.

- **`prismd`** is the desk. It owns the show, the DMX output, the timing and the
  control surface. It keeps running whether anybody is looking or not.
- **The interface** is a window. It draws what `prismd` says and sends it what
  you press. It holds **nothing** itself.

The one consequence you notice in use: **closing the window does not end the
show.** The window goes, the light stays, and an icon in the notification area
says so. A second screen, a laptop on the other side of the hall and the faders
on the X-Touch all see *the same* desk — not copies of it.

The second consequence you notice when something breaks: if the interface
crashes, the output does not stop. That is exactly what the split is for.

---

## 2. The first start

**Installing.** Windows 10 or 11, 64-bit. Download
`PrismDMX_<version>_x64-setup.exe` from the
[release page](https://github.com/flakesystems/PrismDMX/releases) and run it. The
installer needs **no administrator rights** and installs into your own profile —
a school laptop an operator is not allowed to administer is the normal case, and
everything up to and including autostart works without elevation.

The first time, Windows will show *Windows protected your PC*. The build is **not
signed**; that is not a judgement about the file, it is what Windows says about
any executable it has rarely seen. *More info* → *Run anyway*. If that is further
than you want to go, that is an entirely reasonable position: check the file's
SHA-256 against the one on the release page first. The long way round is in the
[installer's manual](installer.en.md).

**Starting.** *PrismDMX* in the Start menu. The first start creates a data
directory, makes an empty show and this desk's identity, and opens a window.

**A second start attaches to the desk that is already running.** There are never
two engines. Two desks on one rig is the worst fault this program could have, and
it is prevented structurally rather than by discipline.

**The notification-area icon** has three entries, and they say what they do:

| Entry | What happens |
|---|---|
| *Show the desk window* | Bring the window back |
| *Close this window, leave the desk running* | Quit the interface. The show carries on |
| *Stop the desk* | Stop the desk — **in order**: clients are told, the configured blackout-or-hold goes to the stage, and the outputs are given time to send it before anything closes |

**`F11` or `Alt` + `Enter` toggles full screen** and back. A desk in the hall runs
without a title bar.

---

## 3. The screen

The screen is built as a **device screen** and not as a web page: nothing scrolls
except inside a window. What you see is all there is.

```
 ┌──────────────────────────────────────────────────────────────┐
 │  Header: show name · version · one lamp: is the desk          │
 │          answering?  ·  Add window  ·  View 1 View 2 …        │
 ├──────────────────────────────────────────────────────────────┤
 │                                                              │
 │   The canvas: the windows live here (chapter 4)              │
 │                                                              │
 ├──────────────────────────────────────────────────────────────┤
 │  Command line:  1 thru 6 at full              ⏎              │
 │  under it: what is allowed at this point                     │
 ├──────────────────────────────────────────────────────────────┤
 │  Programmer bar: bank tabs · eight encoders · Clear          │
 └──────────────────────────────────────────────────────────────┘
```

Four parts, and three of them are worth a sentence each.

**The lamp in the header** is the one indicator that has to be true without
anybody having opened a window: *is the engine answering*. Everything else —
frames, outputs, session — is in the *Status* window.

**The command line is the interface.** Every key, tile and button on this screen
**writes a line** rather than acting on its own. What a gesture means is
therefore what the line means — and any line can be put on an X-Touch key.
Chapter 10 is the word list.

**What you are typing appears on every screen.** The line belongs to the desk,
not to this window. What you typed *before* belongs to you: the up and down arrows
walk your own history, and two operators at two screens each have their own.

**The programmer bar** is at the bottom and always there: the seven bank tabs,
eight encoders and the `Clear` key. It shows **only parameters the selected
fixtures actually have** — a four-colour PAR is four knobs, not a bank you page
through.

---

## 4. The windows

Windows arrive on the canvas with **Add window** in the header, or with an F-key
on the X-Touch. The desk finds a free place itself; windows do not overlap.
Dragging and resizing belong to this screen alone — they are not a line, because
there is no word for them.

**A layout is a view.** `Store View 2 "Programming"` files the open windows with
their rectangles, `View 2` brings them back, and because a view is session state
it switches on **every** attached screen — including from the X-Touch, with no
client running.

Fourteen window types, and this table is generated from the code: it cannot go
stale without a test going red.

<!-- generated:window-types -->
| Window | What for |
|---|---|
| `FixtureSheet` | *Fixture Sheet* — the **state**: per fixture and parameter, what the programmer holds and what is on the wire |
| `DmxSheet` | *DMX Sheet* — the **wire**: universe by universe, channel by channel, with no fixtures at all |
| `SequenceSheet` | *Sequence Sheet* — the **cue list**: which sequences exist, which one is current, and its cues with number, name, times and trigger |
| `Groups` | *Groups* — the **group pool**: lists of fixtures, not looks. That is what makes `Group 3` a selection |
| `Viewer3D` | *Viewer 3D* — the 3D stage view. **Not built in this build**; the window says so rather than staying empty |
| `PhaserEditor` | *Phaser Editor* — the effect editor. **Not built in this build**; there is no effect engine behind it yet |
| `ClockViewer` | *Clock Viewer* — the clock, big enough to read from the back, and what is running. Timecode columns are missing because there is no timecode |
| `CueViewer` | *Cue Viewer* — the **cue**: what it sets, fixture by fixture, with the preset references. For looking at, not for editing |
| `PresetPool` | *Preset Pool* — the **pools**: named looks per category, with the colour the scribble strips show |
| `Patch` | *Patch* — the **rig**: which fixtures exist and where their channels are. The only window that changes the shape of the show |
| `Settings` | *Settings* — the **desk**: outputs, devices, key bindings, show files and this machine. Chapter 5 of the [installer's manual](installer.en.md) |
| `Executors` | *Executors* — the **strip**: the eight executors of the current page, their faders and their four keys each, and the editor for them |
| `CommandKeys` | *Command Keys* — the words of the command line as buttons. Whoever knows the words closes the window; whoever is learning them leaves it open |
| `Status` | *Status* — the readings: the show, the session, the engine and the outputs |
<!-- /generated -->

### Fixture Sheet · `FixtureSheet`

One row per fixture, one column per parameter. Two values sit one above the
other: what the **programmer** holds (your change in progress) and what is
actually **on the wire**. Telling those apart is the whole point of the window —
a value you set and a value a cue is running look identical on a desk without
that separation.

Clicking a row selects the fixture. If a verb is in the line, the same click
appends its word instead — see chapter 10.

### DMX Sheet · `DmxSheet`

What the building actually receives: 512 channels per universe, with no
interpretation at all. It is the only window that can contradict the *Fixture
Sheet*, and that is exactly why both exist. When patching, it is the window that
tells you whether an address really arrives where you think it does.

### Sequence Sheet · `SequenceSheet`

The cue lists. At the top, which exist and which is **selected** — that is the
one `Store Cue 5` stores into. Below it, its cues: number, name, fade, delay,
trigger. Clicking a cue row is a selection; with `Goto` or `Edit` in the line the
same click is the missing argument.

### Groups · `Groups`

The group pool. A group is a **list of fixtures**, not a look: `Group 3` selects,
it sets nothing. `Store Group 3 "Front light"` files the current **selection**.

### Viewer 3D · `Viewer3D`

Not built yet. The window opens and says so in one sentence — and that is
deliberate: whoever opens it from an X-Touch F-key should find an answer, not an
empty rectangle they will write a fault report about.

### Phaser Editor · `PhaserEditor`

Also not built yet; what is missing behind it is the effect engine, not the
window.

### Clock Viewer · `ClockViewer`

The time, large, and what is running. What is missing is the timecode half, and
the window names the missing columns itself rather than showing them empty.

### Cue Viewer · `CueViewer`

What an executor's running cue actually sets, fixture by fixture, and where a
value comes from a preset. A window for looking at: you edit with `Edit Cue 3`,
which loads the cue into the programmer.

### Preset Pool · `PresetPool`

Eight pools: the seven categories and **Multi**. A preset from a category holds
only that category's values — a colour, a position — and a `Multi` preset holds
everything the programmer has. The pool is where a *finished look* is filed; the
categories are the *ingredients*.

The colour you give a preset is the colour the scribble strips on the X-Touch
light up. A strip has three lamps, so there are seven words for it — chapter 10.

### Patch · `Patch`

The rig: which fixtures exist, which profile, which number, which address.
Chapter 5.

### Settings · `Settings`

Five tabs: *Outputs*, *Devices*, *Controls*, *Show files*, *This machine*. It is
a **window** and not a dialog: it lies on the canvas like any other, the show
keeps running behind it, and you can leave it open on a second screen. Everything
in it is described in the [installer's manual](installer.en.md), except *Show
files*, which is in chapter 12.

### Executors · `Executors`

The eight executors of the current page: a fader, an encoder and four keys each,
and the editor that says what they do. Chapter 8.

### Command Keys · `CommandKeys`

The words of the command line as buttons. Each of them writes its word into the
line — it does nothing of its own. That is why typing `Store` and pressing it are
the same thing.

### Status · `Status`

The readings, in one place: which show is open and whether it has unsaved
changes, how many clients are attached, at what rate the engine is ticking and
how each output is doing. For a network output it says underneath which node is
answering and which is not — a configured output onto an empty rack reads
**Degraded**, not *OK*.

---

## 5. Patching a rig

In the order it is usually done:

**1. Say how the building is wired.** *Settings → Outputs*. That is the
installer's work and is in [their manual](installer.en.md); if you are in a
building that has been set up, it is already there.

**2. Say what is hanging on it.** The *Patch* window. The fixture library is
searched by name; every fixture gets a **number** (the one you type on the
command line) and an **address** (universe and start channel).

The desk tells you **before** you send whether an address collides with another.
An overlap is allowed — two fixtures on one address is sometimes exactly what you
want — but it is named rather than discovered later.

**3. Check that it arrives.** Open the *DMX Sheet*, pull the fixture to full,
look. If nothing happens there, it is the outputs and not the patch.

### Your own fixture profiles

A lamp the Open Fixture Library does not know is a profile you can write
yourself. It goes in **`fixtures/` in the desk's data directory** — on Windows
`%APPDATA%\PrismDMX\fixtures` — in the Open Fixture Library's JSON format, and is
read at start-up.

That directory and **not** `profiles/fixtures/`: the second is a **download**
that `tools/fetch-fixtures` empties on every run. No installer touches the data
directory.

| Where | What for |
|---|---|
| `fixtures/my-lamp.json` | A lamp there is no profile for. It appears under *Custom* beside everything else in the picker |
| `fixtures/<manufacturer>/<fixture>.json` | A **correction** to a bundled profile. Same manufacturer folder, same file name as in the library — yours replaces it |

The *Source* column in the picker marks your profiles as **yours**. And as soon
as you have patched with one, the profile is **copied into the show**: a `.prism`
file is complete in itself and opens the same way on a desk that has never seen
your directory.

---

## 6. Selecting and programming

**Selecting** works three ways, and all three are the same line:

```
1 thru 6            fixtures 1 to 6
1 + 3               1 and 3  ( "," means the same )
Group 3             the fixtures of group 3
```

Clicking a fixture row in the *Fixture Sheet* or a group tile does the same
thing. `1thru4` with no spaces is a range too — a desk keyboard has no space bar
you enjoy hitting.

**Setting values** works with `at`, with the encoders or with the jog wheel:

```
at 50               the selection to 50 %
1 thru 4 at 50      select and set — two commands in one line
5 pan at 25         a parameter other than the dimmer
at full · at out    the words for the two ends
Full                the selection to full, with nothing else in the line
```

### The encoder banks

Seven banks — *Dimmer*, *Position*, *Gobo*, *Color*, *Beam*, *Focus*, *Control* —
carrying **only what the selected fixtures have**. Select a four-colour PAR and
the colour bank has four knobs.

**Every knob carries the name the manufacturer gave that channel** — *Rotating
Gobo*, *Color Wheel 2* — and not the desk's generic word. Select two heads that
call the same knob different things and the generic word comes back, because
either of the two names would then be wrong.

**A wheel sits on the bank its content names**: a wheel full of colours is under
*Color*, even if the profile calls it something else.

### A fixture with two parameters of the same kind

A head with two colour wheels, a tube with one red per pixel, a white light with
warm and cold white: channels like that are **numbered**.

- On the bank they sit side by side: *Gobo*, *Gobo 2*.
- On the line: `1 gobo 2 at 50` — the **second** of that kind on fixture 1.
  Counting starts at one, so `gobo 1` and `gobo` are the same wheel.
- If there are more than fit side by side — a tube with twenty-four pixels — the
  bar gets a **Part** switch instead and shows one part at a time.

The rule for the line is deliberately narrow, because the shortest thing that
could be an ordinal is also a fixture number: it applies only when the parameter
word is **not the first word** of the line and the number sits **directly before
`at`**. `pan 5 at 25` still selects fixture 5.

### Predefined slots

A gobo wheel, a colour wheel or an effect channel is a list of named positions in
the profile. **Right-click the encoder** to open exactly that list, under the
manufacturer's names, and selecting one writes the middle of the slot. An encoder
with no slots opens nothing.

### Every channel has a knob

Without exception. Where the profile says what a channel does, the knob is that —
a red, a gobo wheel, an iris. Where it does not, or where the meaning of one
channel depends on the value of another, the knob is there anyway: on the
**Control** bank, named as the profile names it, or `Ch 7` — the channel's place
in the fixture, counted from one. From the line: `1 raw 3 at 50` drives the third
such channel of fixture 1 to half.

Two things that do **not** hold here, and that you should know:

- A knob like that has **no named slots**. There is nothing to read them from.
- On a channel whose meaning depends on a mode channel, **the desk does not
  follow the switch while the show is running**. Turn the mode and the
  neighbouring knob keeps the name it had. This is known and is `B52` in
  [`../ISSUES.md`](../ISSUES.md).

### `Clear` lets go before it forgets

Three stages, and the key itself says which one the next press would be:

| Press | What happens |
|---|---|
| 1 | The **selection** goes, the values stay. The next fixture joins the same look |
| 2 | The **values** go |
| 3 | Encoder bank and page go back to the start |

That is how you build a look out of several fixtures: select, set, one `Clear`,
select the next, set. The first value is still there.

---

## 7. Storing: cues, groups, presets, views

```
Store Cue 5                     into cue 5 of the selected sequence
Store Sequence 5 Cue 3          into a cue of a named list
Store Sequence 4                into cue list 4 — creating it if the number is free
Store Preset 1                  into preset 1, in the bank the encoders are showing
Store Preset 1 Color            into a named pool
Store Preset 1 Multi "The look"  everything the programmer holds, across the banks
Store Group 3 "Front light"     the selection as group 3
Store View 2 "Programming"      the canvas as view 2
```

**If the target is taken, the line asks** — *merge*, *override* or *cancel*, on
the command line itself and not in a window over the canvas. The show keeps
running while the question stands: no other screen is blocked, and a `Go` from
the X-Touch does not wait for it. `Escape` cancels, and a cancelled question
changes **nothing at all**.

A cue list is asked with *append*, *override* or *merge* instead, because storing
a sequence is about **cues** and storing a cue is about **values**.

**A cue list newly created on a free number becomes the selected one.**
`Store Cue 1` names no list and means the selected one — a desk that creates list
4 and goes on pointing at list 1 would send the next store to the wrong place.
Storing into a list that already exists does not change the selection: what you
are editing is what `Sequence 4` says.

**`Update`** stores the programmer back into the cue it was loaded from, in
override mode. The key **blinks** when there is something to write back — and
because that is session state, it blinks on every screen at once.

**`Oops`** takes back the last change. Two hundred steps deep.

---

## 8. Playback

**Putting a cue list on a fader:**

```
Assign Sequence 5 Executor 1
```

After that, executor 1's keys and fader run that list. `Page 2` turns the fader
bank.

**A cue list that is on no fader plays anyway** — and it is *the same* playback a
fader would run. `On Sequence 1` and a `Go` on the executor somebody later puts
it on are one playback with one cue pointer. Two executors on one list are two
**handles**, not two players: both `Master` faders read the same value, and
pulling either moves both.

**Transport:**

```
Go+ · Go-        forward and back, on the selected list
On · Off         start and stop
Go+ Executor 1   on an executor
Go+ Sequence 2   on a cue list, wherever it is playing
Goto Cue 5       jump straight to cue 5
```

A cue list **ends up in the same place** whether you walked to a cue or jumped to
it: what a cue does not say itself is computed from the list, not accumulated
from wherever the playback happened to be.

### What an executor can do, and how to change it

Every executor has **one fader**, **one encoder** and **four keys**, and what
they do is configurable — in the *Executors* window or on the line:

```
Assign Executor 1 Fader Master     Empty · Master · Speed · XFade · Fade
Assign Executor 1 Encoder Speed    Empty · Master · Speed
Assign Executor 1 Button 2 Go+     Empty · Go+ · Go- · LearnSpeed · Off · On ·
                                   Flash · Toggle — the four keys are
                                   numbered from one
Assign Executor 1 Button 4 Command "Go+ Sequence 3"
```

The last line is the interesting one: a key can **send a line**. That makes
everything you can type into a key as well — and the same line can go on an
X-Touch key, so that window, line and hardware are one path rather than three.

> A line with punctuation in it **belongs in quotes**. `go+`, `+` and `,` are
> rewritten by the tokeniser so that `1 + 2` means what it says. So:
> `Command "Go+ Sequence 3"`, not `Command Go+ Sequence 3`.

### A crossfade fader is walked, not reset

An executor's fader can be one of two crossfades:

| Mode | Up | Down |
|---|---|---|
| **XFade** | fades from the running cue to the next | from that one to the one after |
| **Fade** | fades the running cue out | fades the next one in |

In both cases you run a cue list by **moving one fader up and down without
lifting your hand**. Stopping half way holds the mixture half way — which is
exactly what a crossfade on a fader is for.

**The desk never moves a crossfade fader itself.** There is no snap-back, on no
screen and on no motor fader.

---

## 9. When something goes wrong mid-show

In order, and the order matters.

### The picture is frozen, the light is standing

**Probably the interface is gone, not the desk.** Look at the lamp in the header.
If it is out, or the window is not there at all:

1. **Nothing on stage changes because of it.** `prismd` is still running and
   still sending DMX. The last look is standing.
2. Start *PrismDMX* from the Start menu again. It **attaches to the desk that is
   already running** — it does not start a second one. You get the same picture
   back, with the same show, the same faders and the same cue position.
3. Once the window is back, carry on. Nothing was lost: the window was holding
   nothing anyway.

### The light is standing, but nothing responds

Then it is the desk and not the interface. If the notification-area icon says
*the desk has stopped*, `prismd` has been ended. Restart the program — the show
is loaded from the file last opened; everything since the last save is in the
**recovery copy** beside the show (chapter 12).

### Light is on that nobody programmed

That is the case that deserves reporting first. Before you report it:

1. **Blackout is not a key here** — the way is `Off` on the running list, or the
   grand master to zero.
2. Open the *DMX Sheet* and see **which** channels those are. If something is
   there and nothing is in the *Fixture Sheet*, it is not coming from the show.
3. Is there a second desk in the building on the same universes? Two senders on
   one sACN universe is the commonest case of this kind, and neither of them
   notices.

### An output has gone

The *Status* window says which. A USB adapter somebody pulled is the **expected**
case: the driver keeps trying, and a cable plugged back in is picked up without a
restart. A network output reading **Degraded** either has no node answering or
the desk is not listening; underneath it says which of the two it is.

### Before the performance starts

- **Save.** `Ctrl-S`, or *Settings → Show files*.
- **Check whether a universe goes nowhere.** *Settings → Outputs* names the
  patched universes no output carries. Reading that before the show is cheaper
  than noticing it on a lamp that does not come on.
- **Have a way back.** A second desk, a house-light switch within reach, or a
  rehearsal you can stop. This request is here because the version is a beta.

---

## 10. The command line, word by word

Case does not matter. Spaces mostly do not either. A name may be in quotes and
does not have to be.

Under the input field is **what is allowed at this point in the line** — the
words, never the numbers. `Tab` takes the first. It is an answer from the
**grammar** and not from the show: it offers the word `sequence`, never the
sequences that exist. What exists is what the pools on the canvas are for.

This list is generated from the code — from `prism_core::console::CONSOLE_WORDS`,
the same table the completion reads from.

<!-- generated:console-words -->
| Word | What it does |
|---|---|
| `assign` | Puts a cue list on an executor (`Assign Sequence 5 Executor 1`), or says what a fader, an encoder or one of the four keys does (`Assign Executor 1 Fader XFade`). Which of the two it means is decided by the first noun |
| `at` | Sets a value: `at 50`, `at full`, `at out`, `5 pan at 25` |
| `clear` | The three stages from chapter 6: first the selection, then the values, then bank and page |
| `color` | Gives an object a colour for the scribble strip: `Color Sequence 4 blue`. With no last word it **takes the colour away** |
| `copy` | Copies an object to another number: `Copy Sequence 2 Sequence 6`. The same for cues, groups, presets and views |
| `cue` | The noun for a cue. On its own it is **not a line** — `Cue 5` could be a goto or an edit, and the desk says so rather than guessing |
| `delete` | Deletes an object: `Delete Cue 3` |
| `edit` | Loads a cue into the programmer to change it: `Edit Cue 3`, `Edit Sequence 5 Cue 3` |
| `executor` | The noun for an executor. On its own — `Executor 3` — it picks the one the transport acts on |
| `fixture` | The noun for a fixture, for anybody who types it out of habit: `Fixture 12 thru 16` is the same as `12 thru 16` |
| `full` | The selection to full, with nothing else in the line |
| `go` | Starts playback. `go 3` is an executor — it was that when it could not be anything else, and it stays that |
| `go+` | One cue forward: on the selected list, or on the named one (`Go+ Sequence 2`) |
| `go-` | One cue back, in the same forms |
| `goto` | Jumps straight to a cue: `Goto Cue 5`, `Goto Sequence 2 Cue 5` |
| `group` | The noun for a group. On its own — `Group 3` — it selects its fixtures |
| `label` | Names an object: `Label View 1 "Programming"`. With no last word it **takes the name away** |
| `move` | Renumbers: `Move Cue 3 Cue 8`. For executors and views it **swaps** if the target is taken |
| `new` | Makes an empty view and switches to it: `New View 3 "Running"` |
| `off` | Stops a playback. `off 3` is an executor |
| `on` | Starts a playback without advancing a cue |
| `oops` | Takes back the last change |
| `page` | Turns the fader bank: `Page 2` |
| `preset` | The noun for a preset. On its own — `Preset 4` — it applies it to the selection |
| `sequence` | The noun for a cue list. On its own — `Sequence 5` — it makes it the one that is stored into |
| `store` | Stores: cue, sequence, preset, group or view. Chapter 7 |
| `thru` | A range, in the direction it is written: `1 thru 4`, `6 thru 2` |
| `update` | Stores the programmer back into the cue it was loaded from |
| `view` | The noun for a view. On its own — `View 2` — it switches the canvas to it |
<!-- /generated -->

### A pool is a key

If a **verb** is in the line, clicking a tile appends its word instead of
selecting. `Store` and a click on sequence 2 makes `Store Sequence 2` and sends
it, because the pointer supplied the missing argument. With nothing in the line,
the same click does what the tile always does.

Three cases where nothing is sent, and all three for the same reason — that what
you typed must not vanish:

- **The line is not a verb line.** `1 thru` plus a click on a group is a range
  under construction, not a group being named.
- **The verb is `Label` or `Color`.** Both are valid with no last word and **take
  something away**. A click that deletes a name would be the worst shortcut
  there is.
- **It is not a complete command yet.** `Store Fixture 5` stays put, with the
  desk's explanation underneath — rather than the typed `Store` quietly
  disappearing and a fixture being selected.

### What is **not** a line

Deliberately, and completely: the executor keys and faders (a `Go` is a gesture
with time in it, a fader a stream of positions); the encoders, the five bank keys
and the two parameter arrows; dragging and resizing windows; **Add window**,
because it names a window *type* and not a number; and a cue's three times and
its trigger, and the fields in the patch form — those carry a value rather than
naming a place.

---

## 11. The X-Touch

A Behringer X-Touch over USB, in **MC mode**. It is a full control surface for
this program and not a remote control for it: it works **with no window open at
all**.

It is connected in *Settings → Devices*: the MIDI port is selected by its
**name**, because that is the only thing that survives unplugging and replugging.
A surface that is not plugged in is a warning and not a start-up problem; one
plugged in later is picked up without a restart.

What the keys do is in *Settings → Controls* and **every one of them is
rebindable**, with a *Learn*: open the editor, press a key, and it is named
rather than fired. A key can also send a whole line — the same possibility as the
executor in chapter 8, and the same path.

**What is known and unpleasant about the hardware:** saturate both directions at
once and the X-Touch can stop **sending** while it goes on receiving, and only a
power cycle brings it back. That is a property of the device that this desk is
explicitly designed against (it sends feedback as a difference rather than a
redraw), and it is documented in [`../MCU_MAPPING.md`](../MCU_MAPPING.md) §2.7.

Every note number, every CC number and every marking has been verified control by
control on a real X-Touch (MC mode, USB, firmware V1.25). The recordings are in
the repository and run in the ordinary test suite with nothing plugged in.

---

## 12. Show files, backup and moving house

**Saving:** `Ctrl-S`, or *Settings → Show files*. That is also where *Save as*,
*Open* and *New* are, each with the **operating system's file dialog**.

**A `.prism` file is complete in itself.** The fixture profiles you patched with
are inside it. That is why carrying a show to another building on a stick works —
and what does **not** travel is the cabling: the outputs belong to the building
and are in `machine.json` beside the show, not in it. That is deliberate, and the
other manual explains it.

**The recovery copy.** While anything is unsaved, the desk writes a copy beside
the show every thirty seconds. That **does not count as saved** — the save
indicator stays on until you really save.

**Where the data is** (Windows):

| | Where |
|---|---|
| The program | `%LOCALAPPDATA%\PrismDMX` |
| Shows, settings, your profiles, the log | `%APPDATA%\PrismDMX` |

**Uninstalling removes the program and nothing else.** Your shows,
`machine.json`, your fixture corrections and your key bindings stay where they
are, and a later installation finds them again.

---

## 13. What this desk cannot do yet

Named here rather than discovered by you:

- **No 3D visualiser.** Planned.
- **No web remote** — a phone or tablet cannot run the desk yet. Planned.
- **No timecode, no OSC, no PSN.** Planned.
- **No effect engine** — the *Phaser Editor* window is empty because what goes
  under it is missing.
- **The desk does not follow a mode channel** while the show is running
  (chapter 6, `B52`).
- **Autostart on Windows only.** The setting exists everywhere, the entry is only
  written on Windows.
- **macOS and Linux are not published.** The engine is portable and is built for
  ARM64 on every commit, but there is an installer for Windows only.

### If you find something

The most useful thing is not a list of faults but **what happened when you tried
to do something real**. An entry that turns out to be intended costs one line of
explanation; a fault nobody wrote down costs the first evening somebody else runs
a show with it.

Report at
[github.com/flakesystems/PrismDMX/issues](https://github.com/flakesystems/PrismDMX/issues),
with: what you did (the steps, in order), what happened, what you expected, **the
version** (*Settings → This machine*) and **the rig** — and whether it also
happens with nothing plugged in.

With it, if you can: the log from `%APPDATA%\PrismDMX` (the level can be turned
up in *Settings → This machine*), the show file, and `machine.json` if it is
about outputs or the control surface — **read that first**, it contains this
desk's access token if you have set one.
