# Known faults

**Applies to: 0.9.2.** What is broken in the released version, written down here
so you find it before it finds you during a show.

This page lists **only faults that are still open**. Everything that has been
fixed is in the [changelog](/en/changelog/), and the full register — every entry
ever filed, with what was done about it and which test now holds it — lives in
the repository, because it is a working document rather than a public one.

At the time of writing there is **one** open fault out of fifty-one filed.

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

## Something else is wrong

If you hit anything not on this page, that is worth reporting even if you are not
sure it is a fault — especially if you got stuck following the documentation,
because that is a fault in the documentation and not in you.

[Open an issue on GitHub](https://github.com/flakesystems/PrismDMX/issues).
What helps most: what you did, what you expected, what happened instead, and
whether it happens every time.
