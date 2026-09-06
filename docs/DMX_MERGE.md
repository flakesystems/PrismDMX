# DMX_MERGE.md — Engine Merge Semantics

**Status:** specification.
**Parent document:** [`ARCHITECTURE_SPEC.md`](../ARCHITECTURE_SPEC.md) (§5).
**Implemented by:** `prism-engine` — pure logic, no I/O, no UI dependencies.

This document defines exactly what value ends up on a DMX channel, and which properties must hold under test. It is the reference for the highest-coverage crate in the project (> 95 % per `CLAUDE.md`).

---

## 1. The priority stack

Every attribute value is resolved through a fixed stack, evaluated bottom to top:

```
┌─────────────────────────────┐
│  Grand Master (intensity)   │  scales the result, never selects it
├─────────────────────────────┤
│  Programmer                 │  absolute override where a value exists
├─────────────────────────────┤
│  Playbacks (cue lists)      │  HTP for intensity, LTP for everything else
├─────────────────────────────┤
│  Home / default values      │  always present, never empty
└─────────────────────────────┘
```

Two properties of this stack matter operationally:

- **The bottom layer is never empty.** Every patched attribute has a home value, so an attribute with no active source resolves to a defined state rather than to zero. A moving head with no active cue sits at its home position, not slammed to pan 0.
- **The programmer always wins.** If the operator has touched an attribute, that is what the fixture does. This is what makes live programming predictable: what you grab is what you see, regardless of what any playback is doing.

**A playback is a cue list's, and there is exactly one of them per list**
(S45). The stack's third row said *playbacks (executors)* until then, and it was
literally true: `PlaybackLayer` had one source per executor, so one cue list
on two executors was **two** sources of the same values, each with its own cue
pointer, its own fade and its own master. Nothing rejected it and nothing merged
it — this document saw two contributors and did what it was told, which is
punch-list entry **B18**.

So the row above says *cue lists*. An **executor is a handle** on one:
it says which list, what its fader does, what its encoder does and what each of
its four keys does, and nothing about what the list is doing. Two executors whose
faders are both `Master` are therefore two handles on one number, and a `Master`
and a **crossfade** on that same list are two different handles that stay
independent — which is what the entry asks for in as many words. It is also why
the two crossfade *modes* S51 added are a setting of the **fader** and not of the
cue list: a mode on the list would put a `Fade` handle and an `XFade` handle back
into an argument about one field.

`prism_domain::PlaybackId` carries the whole of it and
`prism_core::Show::playback_of` is where an executor becomes one.

**A flash is not a fourth layer** (S34). `ExecutorButtonFunction::Flash` raises the *master* of the cue list under the key for as long as it is held, and the master is applied inside the playback layer (§2.1) rather than above it — so a flashed playback still merges HTP against everything else, and a flash cannot take light away that another playback is providing. What makes it a flash rather than a fader move is that the **stored** master is never written to: `prism_engine::PlaybackSource` carries the held level beside the stored one, and releasing simply stops using it. A `SetExecutorMaster` that arrives *during* a flash therefore lands on the stored master and is what stands when the key comes up.

---

## 2. Merge modes per attribute

The mode is a property of the attribute definition (`AttributeDef.mergeMode`), not of the fixture or the playback.

> **An attribute is a type and an occurrence (S52).** The unit everything below resolves is one *channel of a kind* of one fixture — `prism_domain::AttributeKey`, which is an `AttributeType` plus a nought-based index. A head with two colour wheels has two `ColorWheel` slots and they merge independently, exactly as two different attributes would; `MergePlan` sorts and searches by `(fixture, attribute, occurrence)`. Until S52 the type alone was the key and a fixture had one of each parameter, so the library reader dropped the second channel of a kind rather than build a fixture that could not be patched. Nothing about the *merge* changed: a slot is a slot.

| Attribute class | Mode | Reason |
|---|---|---|
| Dimmer / Intensity | **HTP** — highest takes precedence | Industry standard. A playback must never be able to remove light that another playback is providing. Fading out playback A cannot darken a fixture that playback B is holding up |
| Pan, Tilt | **LTP** — latest takes precedence | A position is a single physical state; averaging or maximising two positions is meaningless |
| Red, Green, Blue, White, Amber | **LTP** | Same reasoning. HTP on colour components would silently mix a third colour nobody programmed |
| Iris, Zoom, Focus, Gobo, Prism, Shutter | **LTP** | Discrete or single-state parameters |
| Control | **LTP** | Lamp on/off, reset — must be exactly what was last commanded |

### 2.1 HTP in detail

For all active playback sources providing a value for attribute *a* on fixture *f*:

```
value(f, a) = max( source_i.value * source_i.masterLevel )
```

The playback's master level is applied **before** the maximum, not after. A cue at 100 % on a list faded to 50 % contributes 50 %, and a second list at 60 % with its master fully up wins.

`source_i.masterLevel` is the level **in force**, which is the stored master unless a `Flash` is held over it (§1). That is the whole of the flash's place in this document: one substitution, in one term, of one factor that was already here.

### 2.2 LTP in detail

For LTP, sources are ordered by **activation order** — the order in which cue lists were switched on — not by number, page, or the order they appear in any list.

```
value(f, a) = value from the most recently activated source that provides a
```

Order is tracked with a monotonically increasing activation counter stamped on each playback when it goes active. Deactivating a playback removes it from consideration; the value then falls back to the next most recent source that still provides it, and ultimately to home.

This makes the behaviour deterministic and explainable to an operator: *the last thing you turned on wins*. It is also the reason activation order must be part of the persisted show state — reloading a show must reproduce the same output.

### 2.3 Master levels and LTP

A playback master does **not** scale LTP attributes. Half a pan position is not a meaningful value. Master level affects intensity (HTP) and, where configured, fade progress — never position, colour or beam values.

### 2.4 What a playback holds between cues — tracking (S48)

§2 says how a *set* of contributions merges. It does not say where a playback's
own contribution comes from, and until S48 the answer was accumulated rather
than computed — which made it depend on history.

**A cue is an edit, not a state.** It carries what was in the programmer when it
was stored and nothing else (§3 below is where that comes from), so everything a
cue does not name keeps whatever an earlier cue of the same list left it at. That
is **tracking**, it is the right default, and it is not what changed.

What changed is that it is now **derived from the list** rather than from what
the playback happened to be holding. The two are the same thing when a list is
walked from the top, and they were not the same thing at all when it was not:

> Cue 1 puts a wash at 50 %. Cue 5 puts it at 100 %. Cue 3 mentions neither.
> Walk 1→2→3 and the wash is at 50 %; go to cue 7 and then `Goto` cue 3 and it is
> at 100 %. **The same cue, two outputs.**

An operator cannot rehearse cue 3 like that, and a show cannot be handed to
anybody else. So `prism_engine::SequencePlan` carries a **tracking table**: for
each attribute the list touches and each cue, what a walk from the first cue
would leave it at. Every cue entry — a Go forwards, a Go backwards, a `Goto`, a
start — resolves through it, and a forward Go therefore lands exactly where it
landed before.

**Three states, per attribute, per cue** (`prism_domain::CueTracking`):

| What the cue says | How it is written | What the playback holds afterwards |
|---|---|---|
| **asserts, and it carries forward** | a part with `tracking: "Track"` | the value, until a later cue says otherwise |
| **asserts, and takes it back** | a part with `tracking: "CueOnly"` | whatever was underneath it — the value an earlier *tracking* cue left, or nothing at all |
| **inherits** | no part | whatever is already held |

A cue-only value with nothing underneath it leaves the playback holding
**nothing** for that attribute, which is not the same as holding it at zero: an
attribute a playback does not provide falls through to whatever is below it in
§1's stack, and one held at zero wins its slot at zero.

**A cue that inherits nothing is a blocking cue** — it asserts everything, so a
list can be cut into sections an operator can rehearse from. It is made by an
edit rather than by a flag (`Command::SetCueTracking` with `Block` writes the
inherited values into the cue), for the reason the whole of this section turns
on: a flag honoured at playback time would still be *computed from the cues
above*, so editing cue 2 would go on changing what a blocking cue 5 puts out,
which is the one thing blocking is asked for to stop.

**Where the state lives, and where it does not.** It is derived, so it is never
in the `.prism` file: a file that stored the resolved state would be a file that
could not be corrected by editing cue 2. It is built on the **core thread**, in
`SequencePlan::build`, at load and again whenever a cue is stored, edited,
deleted, renumbered or moved — and the tick only ever *reads* it, with one binary
search per slot and no allocator call (`ARCHITECTURE_SPEC.md` §3.1). Clients read
it through `Query::CueTracking` rather than folding the cues themselves, which is
`PatchPreview`'s rule (S27): a second implementation of this fold would be a
second answer to the question this section exists to give one answer to.

**A fade across the boundary starts where the light is.** An attribute inherited
from four cues back and now asserted fades from the value it has been sitting at,
not from anything the tracking state says: the state is where a cue is *going*,
never where it is coming from. An attribute that leaves the state is released the
way §2.3 releases a playback — an intensity fades to home, everything else holds
— and is dropped when that fade is over.

---

## 3. The programmer layer

The programmer holds every value the operator has touched but not yet stored. It sits above all playbacks.

- A programmer value **overrides** the merged playback result for that fixture and attribute.
- An attribute with **no** programmer value is unaffected — the programmer is sparse, not a full frame.
- `ProgrammerValue.source` distinguishes `Manual`, `Preset` and `Recalled` for display and for store behaviour, but all three have identical merge priority.

**The sparseness is the same fact a cue's is** (S48, punch-list **B21**). What
the programmer marks as *overriding* — an attribute it holds, which goes out
whatever the playbacks say — is exactly what a cue has to **assert** for a jump
into that cue to produce the same light twice. `Command::StoreCue` carries the
programmer into a cue, so a cue is sparse because the programmer is; §2.4 is
what makes that computable instead of guessed. A stored cue **tracks**, which is
what an operator storing a look means; a cue-only cue is made by saying so
afterwards rather than by a mode on the store, because what is being decided is
about the *cue* and not about that store — the same programmer stored twice may
be tracking in one cue and a one-off in another.

### 3.1 Three-stage clear

Per `CLAUDE.md`, the Clear function is a three-stage state machine (`ProgrammerState.clearStage`):

| Stage | What the press takes away |
|---|---|
| `1` Selection | The fixture **selection**, keeping every value that has been set |
| `2` Values | The programmer **values** |
| `3` All | Everything else: the active feature group and the page state beside it |
| `0` Nothing | Nothing. There is nothing to clear and the key is dark |

**The stage is derived from the contents, not counted on the button** *(S43, punch-list B2)*. It cannot go stale, a press at `Nothing` changes nothing and reports so, and an operator who clears once and then grabs a fader finds the key offering what is actually there. The stage number is the *press order*, which is what lets an interface colour the key and name it from one integer.

> **The selection comes first, and it did not until S51** *(B37)*. The first press used to take the values and the second the selection. The owner's punch list gives the reason for turning it, and it is not a preference: *andernfalls ist es nicht möglich, mehrere verschiedene Fixtures gleichzeitig zu programmieren*. Building a look out of several fixtures is **select, set, let go, select the next, set** — and *letting go* is the only one of those five steps that needs a key. Under the old order the only key that did it emptied the look at the same time, so a look could never be wider than one selection. It is now: one press releases the fixtures and leaves everything that has been set standing, the next press takes the values, the third takes the rest.
>
> What it costs is that a programmer emptied by hand takes **two** presses rather than one, which is the trade the entry asks for. The order is asserted as a *sequence* rather than one stage at a time — `the_stages_are_the_sequence_a_hand_presses` (`prism_domain::programmer`), `the_clear_key_offers_the_first_thing_there_is_to_clear` and `a_look_is_built_out_of_several_fixtures_one_clear_at_a_time` (`crates/prism-core/tests/programmer.rs`), and *names the selection first and the values second* (`ui/src/App.test.tsx`) — so a later change that reorders them again goes red on the step that moved.
>
> **One thing moved with it that the entry does not mention.** The *update state* — `Session::editingCue`, the cue an `EditCue` loaded and the blinking Update key that goes with it — used to be cleared by any press of Clear, on S39's argument that *the values it was holding are gone*. That argument names the values, and the first press no longer takes them: it hangs on the **values** now, so an operator who lets one fixture go to add the next is still editing the same cue and the key stays lit. A press that moves no value must not take a state away, which is B40's rule one field along.

---

## 4. Masters applied after merging

Applied to the merged result, in this order:

1. **Group masters** — scale intensity for the fixtures in a group
2. **Grand Master** — scales all intensity globally
3. **Speed masters** — affect playback rate, not values; applied during step 2 of the tick, not here

Masters scale **intensity attributes only**. This is a deliberate constraint: a grand master that dimmed colour values would desaturate the rig on the way down instead of dimming it.

### 4.1 Speed masters, as built (S34)

Item 3 above was a sentence with nothing behind it from S3 until S34; `docs/MCU_MAPPING.md` §4.3 recorded twice that no domain type carried one. What it is now:

- **A speed master is a cue list's**, and it is `prism_domain::Sequence::speed`, in units of `prism_domain::SPEED_UNITY` (1 024 = 1×, 0 = frozen, `u16::MAX` = just under 64×). It was `Executor::speed` from S34 until **S45** moved it, with the master beside it, for the reason above: a rate is a property of the list that is running, and two faders set to `Speed` on one list are two handles on one rate. `ExecutorFaderFunction::Speed` and `ExecutorEncoderFunction::Speed` have named the control since S1; what they now move is the list's. A *named* speed master shared between cue lists is a bigger idea and nothing has asked for one.
- **It is a rate on the playback's own clock, not on the tick.** `prism_engine::CuePlayer` accumulates `speed`/`SPEED_UNITY` of a tick per tick slot and carries the remainder, so at unity the arithmetic is exactly what it was before rates existed — every fade in this project is the number it was — and at any other rate a fade is the same curve sampled at a different rate. It advances by the number of tick *slots* that have passed rather than by one, so a tick the scheduler missed still moves the show forward by the time it really took.
- **It changes no value.** A speed of zero freezes a fade where it stands; it does not stop the playback, black it out, or take it out of the merge. Nothing in §1's stack is aware of it.
- **`LearnSpeed` is a tap against it.** Two taps inside `prism_engine::TAP_WINDOW` (four seconds) mean *the running cue's transition should take that long*, so the rate is that transition's own length over the tapped interval. Tapping at the rhythm a list is already keeping therefore changes nothing, a single tap changes nothing, and a tap against a cue with no time in it has no rate to learn.

### 4.1.1 What an executor's fader moves, and how the desk knows

One command — `SetExecutorMaster` — and four meanings, chosen by that executor's
own `faderFunction` (`docs/MCU_MAPPING.md` §4.1). Since S45 all four resolve
through the cue list standing on the slot:

| `faderFunction` | What moves | Where it lives |
|---|---|---|
| `Master` | the list's master level | `Sequence::masterLevel`, show state |
| `Speed` | the list's rate | `Sequence::speed`, show state |
| `XFade` | the transition's clock (§4.2) | nowhere — a gesture in progress |
| `Fade` | the same clock, the other mode (§4.2) | nowhere — a gesture in progress |
| `Empty` | nothing | — |

An executor with **no** cue list on it is refused rather than silently ignored:
*executor 3 has no sequence* is a complaint an operator can act on, and a fader
that wrote a number nobody could reach was what the old model allowed.

### 4.2 The crossfade is a clock, not a master

`ExecutorFaderFunction::XFade` and `Fade` (`ARCHITECTURE_SPEC.md` §6) belong here only to say that they do **not** master anything. A manual crossfade replaces the *clock* of a transition — the same `from`, the same `to`, the same traversal — with how far the fader has travelled. It scales nothing and merges nothing, so nothing in §2 or §4.1 changes because one is moving.

**Two modes since S51** *(punch-list B36)*, and the difference is which transition a movement carries:

| Mode | Pushed up | Pulled down |
|---|---|---|
| `XFade` | crossfade to the next cue | crossfade to the one after it |
| `Fade` | fade the current cue **out**, staying on it | fade the next cue **in**, from wherever the light is |

So `XFade` advances the list by one cue per half of the travel and the stage never goes dark; `Fade` advances it by one per full up-and-down, through black. Either way one fader walks a cue list.

**The unit is a *stroke*** — one journey of the fader from where it was resting to an end of its travel, carrying one transition — and three things about it are decisions rather than mechanics:

- **It is armed by the first movement, not by engaging the fader.** A playback with an untouched crossfade fader reads the cue it is actually on, and switching a fader to a crossfade mid-show moves no light.
- **A finished stroke holds at its end**, and the *next* movement — which can only go back the other way — arms the next one. There is no Go in between and nothing to re-base, which is what the old single mode needed and what made the desk drive the fader back to nought.
- **A stroke stopped half way is a state.** Every entry sits at `interpolate(from, to, progress)`, the clock is not consulted, and the next tick computes the same numbers — so the frames are byte-identical tick after tick, and a walk replayed twice produces the same sequence twice. It is asserted that way (`crates/prismd/tests/crossfade.rs`) rather than on a state field.

A **Go** takes the transition back on to the clock and abandons any stroke, leaving the fader exactly where the operator's hand left it. Nothing anywhere writes a crossfade fader's position — `ARCHITECTURE_SPEC.md` §4.2 has why, and `ExecutorFaderFunction::desk_may_move_it` is where the rule lives.

---

## 5. Attribute to DMX channel encoding

The final step converts resolved attribute values (`0..65535`, always 16-bit internally) into DMX bytes.

| Case | Encoding |
|---|---|
| 8-bit attribute (`fineOffset == null`) | `data[address + coarseOffset] = (value >> 8) as u8` |
| 16-bit attribute | coarse = `value >> 8`, fine = `value & 0xFF`, written at `coarseOffset` and `fineOffset` |
| `invert == true` | applied to the attribute value **before** splitting: `value = 65535 - value` |
| Pan/tilt inversion per fixture | `Fixture.invertPan` / `invertTilt` — applied on top of the attribute-level invert, so a fixture hung upside down is corrected without editing the fixture type |
| Desk-supplied intensity (S43) | Every **colour** channel of a fixture with one is scaled by its `Dimmer` slot before the invert: `value = value * dimmer / 65535` |

### 5.1 The desk-supplied intensity

A fixture whose profile has no `Dimmer` attribute gets one from the desk, unless
the operator switches it off in the patch (`Fixture.softwareDimmer`). It is an
ordinary merge slot — playbacks write it, the masters scale it because it is
`FeatureGroup::Dimmer`, the programmer takes it over — and the one thing it has
not got is a channel. What it does instead is the row above: it scales the
fixture's colour on the way out, which for a fixture whose only output is colour
*is* its intensity.

It rests at **nought**, and that is the point of it. Punch-list B1 gave colour
channels a home value of full, because a colour starts open on a desk and the
dimmer decides whether any of it is seen; an RGBW PAR has no dimmer, so *colour
open* and *lamp at full* are the same eight bits and a rig of them came up white
the moment the daemon started. The supplied intensity is what puts the second
half of B1 back.

The scaling is exact at both ends — `scaled(v, 65535) == v` and
`scaled(v, 0) == 0` — so a fixture whose supplied dimmer is at full is written
byte for byte as it would be with no dimmer at all.

Working in 16-bit internally regardless of the patched resolution keeps fades smooth: an 8-bit dimmer fading over 10 seconds still interpolates in 16-bit and only quantises at the final write, avoiding visible stepping.

Address arithmetic is validated at patch time, never in the tick. A fixture whose footprint would exceed channel 512 is rejected when patched, so the tick loop can write without bounds checks beyond the slice guarantee.

---

## 6. Invariants to test

These are the properties `proptest` must verify. They are the contract of the merge layer.

### 6.1 HTP properties

- **Commutative:** `merge_htp(a, b) == merge_htp(b, a)` — source order must not matter
- **Associative:** `merge_htp(merge_htp(a, b), c) == merge_htp(a, merge_htp(b, c))`
- **Idempotent:** `merge_htp(a, a) == a`
- **Monotone:** raising any source's value can never lower the result
- **Identity:** merging with a source at 0 leaves the result unchanged

### 6.2 LTP properties

- **Order-dependent:** the result equals the value of the source with the highest activation counter — asserted against a shuffled input order with counters preserved
- **Deactivation falls back:** removing the winning source yields the value of the next-highest counter, and removing all sources yields home
- **Not commutative:** explicitly asserted, so nobody "optimises" LTP into a max later

### 6.3 Stack properties

- A programmer value always appears in the output, regardless of playback state
- With no active playbacks and an empty programmer, output equals home for every patched channel
- Grand master at 0 forces every intensity attribute to 0 and leaves every non-intensity attribute unchanged
- Grand master at full is a no-op

### 6.4 Determinism and timing

- Identical input state produces byte-identical output frames across runs
- No allocation occurs during a tick — asserted with a counting allocator in tests
- p99.9 tick jitter stays under 2 ms with 64 universes under full CPU load, over a 10-minute run (`criterion`, CI gate)

### 6.5 Tracking properties (S48)

The contract of §2.4, and the reason it is a property rather than a worked
example: *the same cue reached two ways* is a claim about every cue of every
list, and one list somebody chose is a fixed bug rather than a rule.

- **Route-independent:** for every cue of a generated list, walking to it from
  the top and jumping to it from the end of the list produce the same frame —
  asserted on the bytes a driver would receive, not on what the player holds
- **The walk is unchanged:** a forward Go lands on exactly the values it landed
  on before the tracking table existed, which is what says resolving *always* is
  one rule rather than two
- **A cue-only value is handed back to what was underneath it**, and to *nothing
  held* where nothing was — which is a different output from zero
- **Derived, not stored:** editing cue 2 changes what cue 5 puts out, without
  cue 5 being touched
- **A fade starts where the light is**, never at the tracked value of the cue
  being left
- Reading the tracking state makes **no** allocator call — the ninth measured
  path

---

## 7. Worked example

Fixture 1, a moving head with a 16-bit dimmer and 16-bit pan. Two cue lists are playing.

| Source | Activation | Dimmer | Pan | Master |
|---|---|---|---|---|
| Home | — | 0 | 32768 (centre) | — |
| Cue list 3 | counter 7 | 65535 | 20000 | 50 % |
| Cue list 5 | counter 9 | 30000 | 45000 | 100 % |
| Programmer | — | — | 50000 | — |

The two sources are two **cue lists** since S45, whatever they are being played
from: an executor is a handle, and two of them on one list would be one source
here, not two.

Resolution:

- **Dimmer** is HTP: list 3 contributes `65535 × 0.5 = 32767`, list 5 contributes `30000 × 1.0 = 30000`. Maximum is **32767**. No programmer value, so it stands. Grand master at full leaves it. Written as coarse `0x7F`, fine `0xFF`.
- **Pan** is LTP: list 5 has the higher activation counter and would win with 45000 — but the programmer holds 50000, which overrides everything. Result **50000**, written as coarse `0xC3`, fine `0x50`.

Turning list 5 off changes nothing about pan while the programmer holds it. Clearing the programmer drops pan to 45000 only if list 5 is still active; otherwise it falls back to list 3's 20000, and with both off, to home at 32768.
