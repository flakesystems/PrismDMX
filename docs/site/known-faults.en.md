# Known faults

**Applies to: 0.9.2.** What is broken in the released version, written down here
so you find it before it finds you during a show.

This page lists **only faults that are still open**. Everything that has been
fixed is in the [changelog](/en/changelog/), and the full register — every entry
ever filed, with what was done about it and which test now holds it — lives in
the repository, because it is a working document rather than a public one.

At the time of writing **eleven** faults are open in 0.9.2 out of sixty-one
filed — **nine** of them are already fixed for the next release and marked so
here.

## B52 — the desk does not follow a switching channel while the show runs

**Where:** the fixture library, and the programmer's encoder bar.
**Severity:** cosmetic.

**What happens.** Some fixtures use one channel to change what the *other*
channels mean — a mode channel, in the Open Fixture Library's terms a *switching
channel*. Since S54 the affected slot is always reachable and, wherever every
mode agrees on what a slot does, it is labelled correctly. But if you turn the
mode channel while the show is running, the meaning of the neighbouring slot
changes and its label does not follow. The knob keeps the name it had.

**What you will see.** Patch a fixture that has a mode channel — a laser, most
often — and turn the mode. The neighbouring encoder still says `Channel 2`, or
whatever it said before.

**What it does not do.** It does not send a wrong value: the channel is still
addressed correctly and `1 raw 3 at 50` still drives the third such channel of
fixture 1. It is the *label* that is stale, not the output.

**Why it is still open.** This is not a missing line in the fixture reader. An
attribute whose meaning depends on the value of another channel is something the
data model does not have a concept for — and the key a cue stores a value under
must not change while that cue is running, or the cue would mean something
different on playback than it did when it was stored. So the fix is a question
about the model rather than an afternoon's work, and it is being treated as one.

## B53 — required input fields do not allow being fully cleared during editing

**Fixed for the next release.** A number field can be emptied while typing; it is checked when applied.

**Where:** UI, required input fields.
**Severity:** annoying.

**What happens.** Fields marked as required do not allow their contents to be
fully cleared while typing — this prevents changing the first digit of a number
or the first letter of a word.

**What you will see.** Select a required field (e.g. an address or name field)
and try to clear the whole value — the field rejects the empty state.

## B54 — the `fixtures/` directory is not created on installation

**Fixed for the next release.** The desk makes `fixtures/` on its first start, with a `README.txt` inside.

**Where:** installation, daemon data directory.
**Severity:** annoying.

**What happens.** `fixtures/` does not exist after a fresh installation. Anyone
wanting to place a custom fixture profile must create the directory by hand,
with no indication that it is missing.

**What you will see.** Install PrismDMX fresh, open the data directory —
`fixtures/` is absent.

## B55 — the Controls menu crashes when a new key is bound

**Fixed for the next release.** Binding a key no longer disconnects the client, whatever the action.

**Where:** Settings, Controls menu.
**Severity:** blocker.

**What happens.** Binding a new key in the Controls menu disconnects and
reconnects the client, jumping back to the Outputs menu. The crash recurs on
every subsequent opening of the Controls menu, even after a daemon restart.

**Workaround.** Remove the bound control from `machine.json` by hand and restart
the daemon.

**What you will see.** Open Settings → Controls → bind a new key.

## B56 — switching the view clears the command line

**Fixed for the next release.** Changing view leaves a half-typed line where it is.

**Where:** canvas / views, command line.
**Severity:** annoying.

**What happens.** Switching the active view while a command is being typed clears
the command line, because view changes are dispatched as commands internally.

**What you will see.** Type something into the command line, then switch the
view — the line is empty.

## B57 — window focus prevents selection on the first click

**Fixed for the next release.** One click selects, also in a window that was not focused.

**Where:** canvas, all windows.
**Severity:** annoying.

**What happens.** When a different window has focus, the first click on an
element in another window only focuses that window — the actual selection
happens only on the second click.

**What you will see.** Focus a different window, then click a fixture in the
Fixture Sheet — the first click focuses only, the second selects.

## B58 — oops does not delete command line words

**Fixed for the next release.** Oops takes the line's last word first, on the X-Touch as well.

**Where:** command line, Oops key.
**Severity:** annoying.

**What happens.** When the command line is not empty, Oops acts immediately as
undo without first deleting the typed input. On a console without a keyboard
there is no way to correct a mistyped command.

**What you will see.** Type something into the command line and press Oops — the
input stays, but an action is undone.

## B59 — crossfade fader position is not synchronised between clients

**Fixed for the next release.** Every handle on a crossfade shows the same position, the motor fader included.

**Where:** executor strip, crossfade.
**Severity:** annoying.

**What happens.** The progress of a crossfade executor is not synchronised
between a web client and a MIDI client. The output is correct, but the fader
snaps back to the web client's position when the MIDI fader is released.

**What you will see.** Set up a crossfade executor, move it with the X-Touch
fader while a web client is connected.

## B60 — patch window: several bugs and missing features

**Being rebuilt as a session of its own (S57)**, because the eight points depend on each other. Until then the number fields can at least be emptied and retyped.

**Where:** patch window, fixture library.
**Severity:** annoying.

**What happens.** Several known problems in the patch window: fixtures of the
same type with different modes appear as separate entries instead of one fixture
with a mode selector; only the first column of a library row is clickable;
the list does not load more fixtures when scrolling to the bottom; the add
fixture button does not open the library directly; the overlap error message
does not state the next free address; new fixtures do not start at the next free
address; unnamed fixtures do not get their type as a name automatically; it is
not possible to patch several fixtures of the same type at once.

## B61 — command feedback shifts the layout

**Fixed for the next release.** The command line's feedback sits in one row of fixed height.

**Where:** command line, command feedback.
**Severity:** annoying.

**What happens.** Typing in the command line causes a feedback element to
appear that shifts parts of the UI.

**What you will see.** Type something in the command line and watch the UI jump.

## B62 — store bar in the cue viewer

**Fixed for the next release.** The store bar is gone; the Update key blinks in the *CommandKeys* window.

**Where:** cue viewer.
**Severity:** cosmetic.

**What happens.** The cue viewer has a section at the bottom for storing the
programmer into a cue or creating a new cue. This is redundant — cues are stored
via the command line.

## Something else is wrong

If you hit anything not on this page, that is worth reporting even if you are not
sure it is a fault — especially if you got stuck following the documentation,
because that is a fault in the documentation and not in you.

[Open an issue on GitHub](https://github.com/flakesystems/PrismDMX/issues).
What helps most: what you did, what you expected, what happened instead, and
whether it happens every time.
