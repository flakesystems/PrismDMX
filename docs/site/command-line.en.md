# The command line

**Applies to: 0.9.2.**

The command line is not a shortcut for people who dislike the mouse. It is the
desk's actual interface: every key on the surface writes a line, and every line
can be put on a key. If you can say it, you can bind it — and if you can do it
with a button, you can read what it said.

A line reads left to right and does nothing until you press `Enter`. Until then
you can still change it, and `Escape` throws it away.

## The shape of a line

```
1 thru 8 at 50            fixtures 1 to 8 to half
1 + 5 + 9 at full         three fixtures, all the way up
Group 3 at 25             everything in group 3
5 pan at 25               one parameter of one fixture
1 at out                  and back to zero
```

**A bare number is a fixture.** `1 at full` and `Fixture 1 at full` are the same
line; the noun is there for people who type it out of habit from another desk.

**`thru` is a range, `+` adds, `-` removes.** They combine: `1 thru 8 - 5` is
seven fixtures.

## The words

Twenty-nine of them, and this table is checked against the desk's own list by a
test — so a word that exists here exists there.

| Word | What it does |
|---|---|
| `assign` | Puts a cue list on an executor (`Assign Sequence 5 Executor 1`), or says what a fader, an encoder or one of the four keys does (`Assign Executor 1 Fader XFade`). Which of the two it means is decided by the first noun |
| `at` | Sets a value: `at 50`, `at full`, `at out`, `5 pan at 25` |
| `clear` | The three stages: first the selection, then the values, then the bank and page |
| `color` | Gives an object a colour for the scribble strip: `Color Sequence 4 blue`. With no last word it **takes the colour away** |
| `copy` | Copies an object to another number: `Copy Sequence 2 Sequence 6`. The same for cues, groups, presets and views |
| `cue` | The noun for a cue. On its own it is **not a line** — `Cue 5` could be a goto or an edit, and the desk says so rather than guessing |
| `delete` | Deletes an object: `Delete Cue 3` |
| `edit` | Loads a cue into the programmer to change it: `Edit Cue 3`, `Edit Sequence 5 Cue 3` |
| `executor` | The noun for an executor. On its own — `Executor 3` — it picks the one the transport keys act on |
| `fixture` | The noun for a fixture: `Fixture 12 thru 16` is the same as `12 thru 16` |
| `full` | The selection all the way up, with nothing else in the line |
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
| `sequence` | The noun for a cue list. On its own — `Sequence 2` — it selects it |
| `store` | Writes what the programmer holds into a cue, a group, a preset or a view |
| `thru` | A range: `1 thru 8` |
| `update` | Writes changes back into the cue they came from |
| `view` | The noun for a view — a saved arrangement of the canvas |

## When a line is wrong

The desk does not guess. A line it cannot read is refused **with the reason on
the command line itself**, not in a dialog over the canvas: the show keeps
running, nothing is blocked, and a `Go` from the X-Touch does not wait for you to
dismiss anything.

If a line would overwrite something, you are asked — again on the command line —
whether you meant *merge*, *override* or *cancel*. `Escape` cancels, and a
cancelled question changes nothing at all.

## Where to go next

The [operator's manual](/en/manual/operator/) has the whole of it in context: how
selection works, what the programmer is, and what each of the three stages of
`Clear` actually clears.
