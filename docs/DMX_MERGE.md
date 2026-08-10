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
│  Playbacks (executors)      │  HTP for intensity, LTP for everything else
├─────────────────────────────┤
│  Home / default values      │  always present, never empty
└─────────────────────────────┘
```

Two properties of this stack matter operationally:

- **The bottom layer is never empty.** Every patched attribute has a home value, so an attribute with no active source resolves to a defined state rather than to zero. A moving head with no active cue sits at its home position, not slammed to pan 0.
- **The programmer always wins.** If the operator has touched an attribute, that is what the fixture does. This is what makes live programming predictable: what you grab is what you see, regardless of what any playback is doing.

---

## 2. Merge modes per attribute

The mode is a property of the attribute definition (`AttributeDef.mergeMode`), not of the fixture or the executor.

| Attribute class | Mode | Reason |
|---|---|---|
| Dimmer / Intensity | **HTP** — highest takes precedence | Industry standard. An executor must never be able to remove light that another executor is providing. Fading out playback A cannot darken a fixture that playback B is holding up |
| Pan, Tilt | **LTP** — latest takes precedence | A position is a single physical state; averaging or maximising two positions is meaningless |
| Red, Green, Blue, White, Amber | **LTP** | Same reasoning. HTP on colour components would silently mix a third colour nobody programmed |
| Iris, Zoom, Focus, Gobo, Prism, Shutter | **LTP** | Discrete or single-state parameters |
| Control | **LTP** | Lamp on/off, reset — must be exactly what was last commanded |

### 2.1 HTP in detail

For all active playback sources providing a value for attribute *a* on fixture *f*:

```
value(f, a) = max( source_i.value * source_i.masterLevel )
```

The executor's master level is applied **before** the maximum, not after. A cue at 100 % on an executor faded to 50 % contributes 50 %, and a second executor at 60 % with its master fully up wins.

### 2.2 LTP in detail

For LTP, sources are ordered by **activation order** — the sequence in which executors were switched on — not by executor number, page, or the order they appear in any list.

```
value(f, a) = value from the most recently activated source that provides a
```

Order is tracked with a monotonically increasing activation counter stamped on each executor when it goes active. Deactivating an executor removes it from consideration; the value then falls back to the next most recent source that still provides it, and ultimately to home.

This makes the behaviour deterministic and explainable to an operator: *the last thing you turned on wins*. It is also the reason activation order must be part of the persisted show state — reloading a show must reproduce the same output.

### 2.3 Master levels and LTP

An executor master does **not** scale LTP attributes. Half a pan position is not a meaningful value. Master level affects intensity (HTP) and, where configured, fade progress — never position, colour or beam values.

---

## 3. The programmer layer

The programmer holds every value the operator has touched but not yet stored. It sits above all playbacks.

- A programmer value **overrides** the merged playback result for that fixture and attribute.
- An attribute with **no** programmer value is unaffected — the programmer is sparse, not a full frame.
- `ProgrammerValue.source` distinguishes `Manual`, `Preset` and `Recalled` for display and for store behaviour, but all three have identical merge priority.

### 3.1 Three-stage clear

Per `CLAUDE.md`, the Clear function is a three-stage state machine (`ProgrammerState.clearStage`):

| Stage | Action |
|---|---|
| 0 → 1 | Clear programmer **values**, keep the fixture selection |
| 1 → 2 | Clear the **selection** |
| 2 → 0 | Clear everything, including the active feature group and page state |

The stage resets to 0 on any other programmer interaction, so an operator who clears once and then grabs a fader does not find a later Clear press in an unexpected stage.

---

## 4. Masters applied after merging

Applied to the merged result, in this order:

1. **Group masters** — scale intensity for the fixtures in a group
2. **Grand Master** — scales all intensity globally
3. **Speed masters** — affect playback rate, not values; applied during step 2 of the tick, not here

Masters scale **intensity attributes only**. This is a deliberate constraint: a grand master that dimmed colour values would desaturate the rig on the way down instead of dimming it.

---

## 5. Attribute to DMX channel encoding

The final step converts resolved attribute values (`0..65535`, always 16-bit internally) into DMX bytes.

| Case | Encoding |
|---|---|
| 8-bit attribute (`fineOffset == null`) | `data[address + coarseOffset] = (value >> 8) as u8` |
| 16-bit attribute | coarse = `value >> 8`, fine = `value & 0xFF`, written at `coarseOffset` and `fineOffset` |
| `invert == true` | applied to the attribute value **before** splitting: `value = 65535 - value` |
| Pan/tilt inversion per fixture | `Fixture.invertPan` / `invertTilt` — applied on top of the attribute-level invert, so a fixture hung upside down is corrected without editing the fixture type |

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

---

## 7. Worked example

Fixture 1, a moving head with a 16-bit dimmer and 16-bit pan. Two executors are active.

| Source | Activation | Dimmer | Pan | Master |
|---|---|---|---|---|
| Home | — | 0 | 32768 (centre) | — |
| Executor 3 | counter 7 | 65535 | 20000 | 50 % |
| Executor 5 | counter 9 | 30000 | 45000 | 100 % |
| Programmer | — | — | 50000 | — |

Resolution:

- **Dimmer** is HTP: executor 3 contributes `65535 × 0.5 = 32767`, executor 5 contributes `30000 × 1.0 = 30000`. Maximum is **32767**. No programmer value, so it stands. Grand master at full leaves it. Written as coarse `0x7F`, fine `0xFF`.
- **Pan** is LTP: executor 5 has the higher activation counter and would win with 45000 — but the programmer holds 50000, which overrides everything. Result **50000**, written as coarse `0xC3`, fine `0x50`.

Turning executor 5 off changes nothing about pan while the programmer holds it. Clearing the programmer drops pan to 45000 only if executor 5 is still active; otherwise it falls back to executor 3's 20000, and with both off, to home at 32768.
