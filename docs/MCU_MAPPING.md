# MCU_MAPPING.md — Mackie Control / Behringer X-Touch Integration

**Status:** specification, and §2 is **verified against the device** as of 2026-08-13 — a Behringer X-Touch in MC mode over USB, firmware V1.25, serial `0156406`. §2.7 is the measurement, §7 is the checklist it closes, and `crates/prism-surface/tests/captures/` holds the recordings the ordinary test suite replays.
**Parent document:** [`ARCHITECTURE_SPEC.md`](../ARCHITECTURE_SPEC.md) (decisions D6, D8, D11).
**Source of requirements:** `XTouch.txt`.

---

## 1. Three layers

The surface controller is split so that protocol detail, device abstraction and user configuration never mix. Each layer is testable in isolation.

```mermaid
flowchart LR
    subgraph IN["Inbound"]
        M1["MIDI bytes"] --> M2["Layer 1: MCU codec"] --> M3["Layer 2: surface model"]
        M3 --> M4["Layer 3: binding table (JSON)"] --> M5["Command → core"]
    end
    subgraph OUT["Outbound"]
        S1["Show + session delta"] --> S2["Surface shadow model"] --> S3["Diff + coalesce @30 Hz"]
        S3 --> S4["MCU encoder"] --> S5["MIDI bytes"]
    end
    S2 -. "touch suppression" .-> S3
```

| Layer | Responsibility | Knows about |
|---|---|---|
| **1 — MCU codec** | MIDI bytes ↔ logical control events. Pure, no domain logic, no state beyond the running-status parser. | MIDI only |
| **2 — Surface model** | Device-independent controls: `Strip{n}.Fader`, `Strip{n}.Button.Select`, `Global.Play`. Holds the shadow model used for outbound diffing. | Control topology |
| **3 — Binding table** | Maps logical controls to `Command`s. Ships as JSON, user-editable. | PrismDMX domain |

Adding an X-Touch Extender or another MCU device means adding a codec profile in layer 1 — layers 2 and 3 are untouched.

---

## 2. Layer 1 — MCU protocol table

> ✅ **VERIFIED AGAINST OUR OWN DEVICE, 2026-08-13 (S20).** Every note number,
> CC number, MIDI channel and buffer offset below was worked control by control
> against a Behringer X-Touch in MC mode over USB (firmware V1.25) and **not one
> of them had to change**. §2.7 records what was measured, including the six
> places where the surface does something these tables did not say. Rows that
> §2.7 corrects or qualifies are marked **⚠ see §2.7**.
>
> The numbers came from three sources that agree with each other (§2.5) and they
> were right. That is worth knowing before the next device is guessed at — and so
> is the reason it can be *stated*: the table is data, so the verification pass
> was a data edit and not a refactor.
>
> **All note and CC numbers are on MIDI channel 1** (status byte `0x90` / `0xB0`,
> "channel 0" in the zero-based convention most of the sources use) unless a row
> says otherwise.

### 2.1 Inbound (X-Touch → PrismDMX)

The X-Touch in **MC mode is a 1:1 emulation of the Mackie Control**: same button
complement, same numbers. Ardour's manual states it plainly — *"a direct
emulation of the Mackie Control and has all the buttons the Mackie device
does"* — and lists no deviations, which is why the table below is the MCU table
rather than an X-Touch table.

**Channel strips** (n = 0…7 from the left):

| Control | Message | Notes |
|---|---|---|
| Rec / Arm | Note 0–7 | velocity 127 = press, 0 = release |
| Solo | Note 8–15 | |
| Mute | Note 16–23 | |
| Select | Note 24–31 | |
| V-Pot push | Note 32–39 | |
| V-Pot rotation | CC 16–23 | relative, sign-magnitude: bit 6 is the sign, bits 0–5 the step count. `0x01` = +1, `0x41` = −1. **⚠ measured: a fast turn does raise the magnitude — 1…8 observed — and the jog wheel never does. See §2.7** |
| Fader touch, strips 1–8 | Note 104–111 | velocity 127 = touched, 0 = released |
| Fader touch, main | Note 112 | as above |
| Strip fader position | Pitch Bend, channels 1–8 | 14-bit, LSB first. **⚠ measured: the range is 0…16380 in steps of 4, not 0…16383 — see §2.7** |
| Main fader position | Pitch Bend, channel 9 | 14-bit, as above |

**Everything else on the panel.** These are the numbers `XTouch.txt` describes
by name and D6/D7/D8 need by number:

| Group | Buttons and notes |
|---|---|
| Encoder Assign | Track 40, Send 41, Pan/Surround 42, Plug-in 43, EQ 44, Instrument 45 |
| Fader banks | Bank ◀ 46, Bank ▶ 47, Channel ◀ 48, Channel ▶ 49, **Flip 50**, Global View 51 |
| Display | Name/Value 52, SMPTE/Beats 53 — **⚠ both send their note and neither has an LED, §2.7. SMPTE/Beats is reserved: in the combined Xctl+MC mode it switches the surface between hosts, so PrismDMX never binds it — §4.3** |
| Function | F1–F8 = 54–61 |
| Global View group | MIDI Tracks 62, Inputs 63, Audio Tracks 64, Audio Instruments 65, Aux 66, Busses 67, Outputs 68, User 69 |
| Modifiers | Shift 70, Option 71, Control 72, Alt 73 — held, not latched |
| Automation | Read/Off 74, Write 75, Trim 76, Touch 77, Latch 78, Group 79 |
| Utility | Save 80, Undo 81, Cancel 82, Enter 83 |
| Transport (upper) | Markers 84, Nudge 85, Cycle 86, Drop 87, Replace 88, Click 89, Solo 90 |
| Transport | Rewind 91, Forward 92, Stop 93, Play 94, Record 95 |
| Cursor | Up 96, Down 97, Left 98, Right 99, Zoom 100, Scrub 101 |
| Foot switches | User switch 1 = 102, User switch 2 = 103 |
| Jog wheel | CC 60, relative, same sign-magnitude encoding as the V-Pots — **⚠ but it only ever sends ±1, however fast it is spun, §2.7** |

**Where this table lives in the code.** `prism_surface::profile::X_TOUCH`, one
constant, and *only* there — nothing in the codec matches on a literal number.
Earlier drafts of this document said `profiles/surface/xtouch.json` held these
numbers as well; it does not, and it must not. That file is **layer 3**, the
binding table of §4, and a second copy of a note map is a second thing to keep in
step — which is exactly what holding it in one constant was for. The JSON records
*which* device profile was verified and *when*, and binds controls to commands.

### 2.2 Outbound (PrismDMX → X-Touch)

| Target | Message | Notes |
|---|---|---|
| Button LED | Note On to the same note number | velocity 0 = off, **1 = flashing**, 127 = on — **all three measured**, and the flashing is the surface's own, not the host blinking it. The surface never lights its own LEDs, which is why a control that is not driven by the host stays dark. **⚠ two buttons have no LED at all, §2.7** |
| Motor fader | Pitch Bend on the strip's channel | suppressed while touched — §5. **The printed scale is a dB scale and its `0` mark is not the top**: **measured — 12 700 of 16 383 puts the pointer on the printed 0**, roughly 77 % of travel. PrismDMX's faders are 0–100 % linear, so the printing is decorative here — worth knowing before somebody "fixes" the mapping to match it. All 16 384 values are accepted outbound even though only multiples of 4 come back (§2.7) |
| V-Pot ring LEDs | CC 48–55, value `0b0LMMVVVV` | **MM** (bits 5–4) = mode — `00` single dot, `01` boost/cut from centre, `10` wrap (fill from the left), `11` spread (symmetrical about centre); **VVVV** = position 1…11, or 0 for all off (mode `11` uses 0…6). **All four modes measured and all four draw as described.** **L** (bit 6) is the small LED under the encoder — **⚠ the X-Touch has no such lamp, so the bit does nothing here, §2.7** |
| Scribble strip text | SysEx `F0 00 00 66 14 12 <offset> <ascii…> F7` | one 2×56 character buffer for the whole row of strips, **not** one message per strip: offset `0x00` starts the upper line, `0x38` (56) the lower one, and strip *n* owns 7 characters at `7n` and `0x38 + 7n`. **All measured, including the 56th character, which does arrive** — Ardour's habit of sending 55 is caution, not a device limit. It is **one continuous 112-character buffer**: two characters written at offset 55 put the second on the lower line's first character, and a single 112-byte write fills both lines. Lower case is folded to upper by the hardware |
| **Scribble strip colour** | SysEx `F0 00 00 66 14 72 <c0…c7> F7` | **the vendor extension this device is bought for — see §2.3** |
| Level meters | Channel Pressure `D0`, data `(strip << 4) \| level` | level `0x0`…`0xC` is silence…0 dB, `0xD` over 0 dB, `0xE` sets the overload flag and `0xF` clears it — **all measured, and the nibble split is this way round**. **⚠ the decay is much faster than the ~300 ms per division claimed: under a second from 0 dB to empty, §2.7** |
| 7-segment display | CC 64–75, **right to left** | value `0b0DCCCCCC`: bit 6 is that digit's dot, bits 5–0 are the character. **Measured: digit 0 really is the rightmost** — `0 1 2 … 9 A B` sent to digits 0…11 reads `BA9876543210` across the panel. The character set is ASCII with bit 6 stripped: `1`–`26` = `A`–`Z`, `48`–`57` = `0`–`9`, punctuation in between (`#$%&'()*+,` at 35–44, `;` 59, `<` 60). **⚠ `0` is a blank and so is 32, so `@` cannot be shown at all, §2.7.** CC 64–73 are the ten timecode digits, CC 74–75 the two assignment digits; CC 76 does nothing. **Measured: this surface accepts the display on MIDI channel 16 as well as channel 1** |
| Global LCD meter mode | SysEx `F0 00 00 66 14 21 <mode> F7` | `0x00` horizontal, `0x01` vertical. **⚠ measured: neither does anything on the X-Touch, whose meters are hardware LEDs rather than drawn on the LCD — and the overload marker works regardless, which the "horizontal only" claim said it would not. §2.7** |

### 2.3 What the X-Touch adds to the standard — the scribble strip colours

This is the row `ARCHITECTURE_SPEC.md` §7 flags as "vendor extension, not part of
MCU", and it is the one thing in this document that had to be found rather than
looked up. Behringer never documented it; it appeared in firmware **1.22**
(21 June 2022) and is absent from that release's notes.

**Verified in full on 2026-08-13** (§2.7), on firmware V1.25: the command, all
eight values, the additive bit order, and three things the sources only implied —
there is no inverted variant in MC mode, a message that does not carry exactly
eight colour bytes is ignored, and writing text does not disturb the colour.

```
F0 00 00 66 14 72 c0 c1 c2 c3 c4 c5 c6 c7 F7
   └──┬──┘ └┬┘ └┬┘ └──────────┬──────────┘
  Mackie   ID  cmd     one byte per strip, left to right
```

| Byte | Meaning |
|---|---|
| `00 00 66` | Mackie's manufacturer ID — the extension lives inside the emulated vendor's namespace, not Behringer's |
| `14` | device ID: `0x14` = Mackie Control (what the X-Touch calls itself in MC mode), `0x15` = an extender |
| `72` | the colour command |
| `c0…c7` | one colour per strip. **All eight are always sent**; there is no per-strip form |

**The colour byte is an additive RGB triple, not a palette index** — bit 0 = red,
bit 1 = green, bit 2 = blue:

| Value | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| Colour | off (black) | red | green | yellow | blue | magenta | cyan | white |

Two consequences for the layers above. **Black is off, not "dark"** — a strip set
to 0 has its backlight off and its text is unreadable, so an "unassigned"
executor wants white rather than 0, and black is worth reserving as the deliberate
marker for *nothing here*. And **the eight colours are exactly the eight corners
of the RGB cube**, so mapping an arbitrary colour to a strip is a nearest-corner
quantisation. Two shipping implementations disagree usefully about how: Ardour
normalises to the brightest component and thresholds each channel at half; the
Ableton script measures distance in RGB **or** in hue (its author found that
hue-first keeps a pale orange orange instead of collapsing it to white), and
sends greys to white and only an explicitly chosen black to black. **Hue-first is
the better default for a lighting desk**, where a pastel is still the colour it
is a pastel of.

**Where the colour comes from.** `Command::Color` — `Color Sequence 4 Red` on
the command line, or `Color Executor 1 Red` for the fader an operator is looking
at, which colours the cue list standing on it. It is stored on the **sequence**,
beside its name, for the reason a strip shows the sequence's name and not the
executor's: a cue list moved to another fader takes what is written on it along.
`prismd::surface` paints strip *n* from the list on executor *n* of the current
page, `prism_surface::color` quantises hue-first, and a list with **no** colour
is drawn white rather than off — which is the "unassigned wants white" rule
above, applied to the commoner case of a list nobody has coloured yet.

**There is a way to fake more than eight colours, and it is worth knowing about
before somebody invents it badly.** The Ableton script has a "colour mix mode"
that cycles a strip between two or three of the eight at speed, so the eye
integrates them — magenta and blue alternating read as violet. It is honest
about the price: it needs a message every ~10 ms, it freezes everything else
while it runs, its author limits it to two-second bursts by default, and the
readme carries an explicit warning about **wear on the display and about
flicker sensitivity**. For PrismDMX that is a no by default — a lighting desk's
scribble strips are read, not admired, and an operator staring at a flickering
strip during a get-in is a worse outcome than an approximate colour.

**Other X-Touch behaviour worth knowing before the codec is written:**

- **The handshake is optional, and now measured.** The host may send the device
  query `F0 00 00 66 14 00 F7`. **Our desk answers**
  `F0 00 00 66 14 01 30 31 35 36 34 30 36 35 42 43 35 F7` — command `0x01`, then
  eleven **printable ASCII** bytes: a seven-character serial (`0156406`) and a
  four-character challenge (`5BC5`). Not `0x06`. The challenge/response cipher in
  the MCU specification belongs to **Logic Control** (device IDs `0x10`/`0x11`)
  and is not required here; nothing was sent back and the surface drove happily
  for the whole session. A host that simply begins sending also works.
- **There is an undocumented firmware version request**, found by asking:
  `F0 00 00 66 14 13 F7` is answered with `F0 00 00 66 14 14 56 31 2E 32 35 F7` —
  command `0x14`, payload the ASCII string `V1.25`. That is how the colour
  extension's firmware requirement can be checked at run time rather than by
  asking an operator to power-cycle the desk and read the boot screen.
- **The surface needs pacing, and it is worse than "loses the tail" — §2.7.**
  Two unrelated projects reported that firing several dozen messages back to back
  loses the tail and that ~1 ms fixes it. Measured here: a burst of 64 large
  SysEx messages on its own is harmless, and so is a burst of 64 small ones, but
  **saturating both directions at once loses messages and can stop the surface
  transmitting altogether until it is power-cycled.** §2.7 has the recipe and the
  consequences.
- **Modes and connections are chosen at power-up** by holding the channel 1
  SELECT button: encoder 1 picks the emulation (**MC**, HUI, and on current
  firmware the Behringer-native **Xctl** and combined Xctl+MC / Xctl+HUI), and
  encoder 2 picks USB, MIDI DIN or Ethernet. **PrismDMX targets MC over USB**, and
  the desk this document was verified against is set to exactly that — confirmed
  at the front panel and by the device ID it answers on (§2.7).
- **Xctl is a different protocol and is deliberately out of scope *as a protocol
  PrismDMX speaks*.** It is Behringer's own, used by the X32/M32 consoles,
  carried over UDP (port 10111 in the one public implementation) and it uses
  device ID `0x58` with *per-strip* display messages that carry the text **and**
  the colour **and** an inversion flag together. It is better than MC for
  scribble strips and it is undocumented, single-vendor and unusable over plain
  MIDI. PrismDMX will not implement it.
- **But the surface will be run in the combined Xctl+MC mode, and that is a
  requirement rather than a curiosity — see §4.3.** The intended deployment is
  one X-Touch driving the venue's sound console over Xctl *and* PrismDMX over MC
  at the same time. That changes what layer 3 may assume: most of the panel
  belongs to the other host, and only what Xctl leaves unused reaches us.

### 2.4 Robustness requirements for the codec

- **Running status** must be handled — a compliant sender may omit repeated status bytes.
- **Malformed or truncated messages are discarded silently** and counted in a diagnostics counter. Per `CLAUDE.md`, an invalid MIDI packet must never propagate a failure; it certainly must never reach the engine thread.
- **SysEx reassembly** across packet boundaries, with a maximum buffer size and a timeout that drops an unterminated message.
- The codec allocates nothing per message; it writes into caller-provided buffers.

### 2.5 Where these numbers come from

Recorded because a table with no provenance cannot be argued with, and because
§7 needs to know which rows are worth re-measuring first. None of this is a
Behringer document: **Behringer publishes a MIDI implementation for the
X-Touch's *Ctrl* (plain MIDI) mode only**, and nothing for MC mode, which is
why the community reverse-engineered it.

| Source | What it is | Weight |
|---|---|---|
| **Our own X-Touch, 2026-08-13 (§2.7)** | The desk PrismDMX is built for, worked control by control, with the recordings kept in `crates/prism-surface/tests/captures/` | **Highest, and it now outranks everything below.** Where a source and the device disagree, the device wins and the row is marked ⚠. Six such places exist; none of them is a note number |
| [Ardour](https://github.com/Ardour/ardour/tree/master/libs/surfaces/mackie), `libs/surfaces/mackie/` | A production implementation with a dedicated X-Touch profile, in use for years | **Highest.** The SysEx header, the LCD offsets, the ring encoding, the colour command and the handshake below were read out of this code |
| [TouchMCU protocol reference](https://github.com/NicoG60/TouchMCU/blob/main/doc/mackie_control_protocol.md) | A careful reverse-engineering write-up of the whole MCU protocol, with the complete note map | High. The note table in §2.1 is this, cross-checked against Ardour |
| [X-Touch script for Ableton Live](https://github.com/Kik07L/Behringer-X-Touch-for-ableton) | **Ableton's own `MackieControl` remote script, adapted for the X-Touch** and in use by a community that reports it working | **Highest, and it is the best single source here.** It is a fourth-party check that agrees with the others *number for number*: its `consts.py` note table is §2.1 exactly, its `MainDisplay.py` sends `F0 00 00 66 <14\|15> 72 …` for the colours, `0x00`/`0x38` for the two display lines, and `0/1/127` for LED off/flashing/on. The 7-segment character set, the fader's 0 dB point, the colour-matching strategies and the colour-mixing trick in §2.2 and §2.3 are all from here |
| [x-touch-xctl](https://github.com/pythag/x-touch-xctl) | An X-Touch driver for Behringer's own Xctl protocol | The Xctl notes in §2.3, including the inversion flag MC mode does not have |
| [Ardour manual, Behringer devices in MC mode](https://manual.ardour.org/using-control-surfaces/mackie-control-protocol/behringer-devices-in-mackielogic-control-mode/) | Vendor-neutral documentation of how these surfaces behave | The "1:1 emulation, no deviations" claim in §2.1 |
| Community reports (forum threads, an independent Rust and a C++ driver) | Firmware 1.22, the colour codes, the message pacing | Lowest, and each is marked in the text. **The pacing claim in particular is somebody else's measurement of somebody else's device** |

### 2.6 As built (S19)

Layer 1 exists: [`crates/prism-surface`](../crates/prism-surface), 112 tests,
**99.24 % line coverage**. Everything in §2.1 and §2.2 above is in
`profile::X_TOUCH`, which carried **`verified: false`** — the banner as a value,
so a log line or a status panel could say so where a comment could not. Nothing in
the codec matches on a literal number, so S20 was an edit to one table and to the
hand-written byte table in `tests/round_trip.rs`, and to nothing else.

**And that bet paid in a way worth recording: neither table needed a single byte
changed.** The verification cost three added fields, one corrected character
mapping and a new test target — see §2.7 and, for what was actually done, S20's
record in `PROGRESS.md`.

Four things the implementation decided that this document did not say:

- **Inbound SysEx is counted, not interpreted.** The handshake is optional
  (§2.3) and no source settles what an X-Touch's *Device Ready* payload looks
  like. Writing a decoder for a message shape nobody has seen would be inventing
  data in a table made of citations, so `McuCodec` counts it (`sysex_ignored`).
  The *question* — `F0 00 00 66 14 00 F7` — is written, and the reassembly, the
  bounded buffer and the timeout are all exercised by the scribble strip
  messages, which are longer. **§2.3 now records exactly what the desk answers**,
  so a decoder could be written; it still is not, because nothing above layer 1
  has asked for the serial number yet and inventing an API for it would be S21
  guessing at its own requirements. The bytes are written down, which is the part
  that was missing.
- **A release is written as a Note On with velocity 0**, because that is what
  the surface sends; a real Note Off is accepted on the way in and normalises
  onto it. The round-trip table separates *canonical* rows, which are byte-equal
  both ways, from *alias* rows, which are not and say so.
- **The V-Pot magnitude is passed through, not interpreted** — and that turned
  out to be the right call twice over. A fast turn *does* send a larger number
  (1…8) and the jog wheel *never* does (§2.7), so the codec reporting what the
  wire carried is the only behaviour that is correct for both. The acceleration
  curve belongs to layer 2, and layer 2 now knows it needs two of them.
- **A ring position the mode cannot draw, and a colour byte outside the eight,
  are refused rather than masked.** Both directions, so the round trip stays
  honest.

The four robustness requirements of §2.4 are met and measured rather than
asserted: no allocation at all, inbound or outbound, under 250 kB of hostile
input (`tests/codec_allocations.rs`, a counting global allocator).

### 2.7 As measured at the device (S20, 2026-08-13)

**The desk.** Behringer X-Touch, **MC mode over USB** — chosen at power-up with
channel 1 SELECT held and read off the front panel, and confirmed independently by
the device ID it answers SysEx on (`0x14`). **Firmware V1.25**, so the colour
extension of §2.3 (which needs ≥ 1.22) is available. Serial `0156406`. USB
`VID 1397 PID 00B1`, enumerating two MIDI ports; everything here is on the first
(`X-Touch`), the second being the MIDI DIN pair.

**Every number in §2 survived.** Not one note number, CC number, MIDI channel or
buffer offset had to change. What follows is therefore not a correction list for
the tables — it is the list of things the tables did not say, plus one claim that
was wrong.

#### What was verified, and how

| Claim | Method | Result |
|---|---|---|
| The 40 strip-button notes (0–39) | pressed, strip by strip, all five rows | ✅ exactly as tabulated; the block tiles 0…39 with no hole |
| The 64 global notes (40–103) | the host lit **one LED at a time** and the lit button was pressed | ✅ **60 confirmed in both directions at once.** Name/Value and SMPTE/Beats have no LED and were pressed blind (notes 52, 53 ✅). The two foot-switch notes need a pedal nobody had — **untested, not wrong** |
| Fader pitch-bend channels and 14-bit order | all nine faders swept end to end | ✅ channels 1–8 for the strips in left-to-right order, channel 9 for the main fader, **LSB first**: at the top of travel the message is `E0 7C 7F` = 124 + 128 × 127 |
| Fader touch notes | the same sweeps | ✅ 104–111 in order, 112 for the main fader |
| V-Pot CCs and the jog wheel | one detent each, then spun hard | ✅ CC 16–23 in left-to-right order, jog CC 60 |
| Ring LED modes | all four drawn on all eight rings | ✅ Dot, BoostCut, Wrap and Spread all draw as described |
| Scribble strip offsets | one strip written at offset 21, then whole lines | ✅ strip *n* owns 7 characters at `7n`; **all 56 of a line arrive** |
| Colours | all eight, one per strip and all-same | ✅ off, red, green, yellow, blue, magenta, cyan, white — the additive order exactly |
| Meter message and overload flag | staircase, then `0xE` / `0xF` | ✅ `(strip << 4) \| level`, marker sets and clears |
| 7-segment addressing | `0 1 2 … 9 A B` to digits 0…11 | ✅ reads `BA9876543210` — digit 0 is the **rightmost** |
| Round-trip latency | 60 device queries, one at a time | ✅ **median 0.71 ms**, min 0.61, max 1.02 (host → surface → host) |

#### The six things the sources did not say

1. **The faders are 12-bit, and the top is 16380.** Every one of 576 captured
   positions is a multiple of **4**; the LSB only ever takes the 32 values
   `00,04,…,7C`. So the surface reports 4096 distinct positions inside a 14-bit
   field and **never sends 16383**. It *accepts* all 16 384 outbound. Held as
   `McuProfile::fader_step`, because a layer that turns an inbound position into a
   percentage by dividing by 16383 gives 99.98 % for a fader against its end stop,
   and an executor master that cannot reach full is wrong.
   A moving fader reports every **19.8 ms** (≈50 Hz), very evenly.
2. **The V-Pots accelerate; the jog wheel does not.** §2.1 described both as
   "relative, sign-magnitude" and left the magnitude open. A fast V-Pot turn
   carries **1…8 detents** in one message (`0x08` / `0x48` at the extremes,
   distribution peaking at 4–6). The jog wheel sent **±1 and nothing else in 404
   messages**, however hard it was spun — it raises its *rate*, not its
   magnitude. A layer 2 that applied one acceleration curve to both would be
   wrong about one of them. Neither ever sends a zero-magnitude message.
3. **Two buttons have no LED.** Name/Value (52) and SMPTE/Beats (53) are printed
   on the panel, send their notes when pressed, and stay dark at any velocity.
   The Ardour manual's "a direct emulation … with no deviations" is not quite
   true, and `McuProfile::unlit_buttons` now says so as data.
4. **The encoders have no lamp under them.** Bit 6 of the ring value — the "small
   LED beneath the encoder" of §2.2 — has nothing to light on this surface.
   Harmless to send; pointless to model.
5. **A 7-segment value of `0` blanks the digit, and so does 32.** This settles the
   contradiction §7 raised: the "ASCII with bit 6 stripped" rule stops one
   character short, because the code `'@'` strips to draws *nothing*.
   `SegmentChar::from_ascii` therefore now **refuses `'@'`** rather than handing
   back a code that silently disappears, and `to_ascii(0)` answers `' '`.
6. **The meters decay in well under a second**, not the ~300 ms per division the
   sources claim, which would be 2–4 s from 0 dB. A host that wants a meter to
   look continuous must refresh it far more often than that — and §5.2's
   permission to drop meters first is still right, because a dropped meter simply
   falls rather than freezing.

#### What the surface ignores, which is what the codec assumed

Every one of these was sent **by hand**, because the codec refuses to encode
them — and in every case the surface did nothing at all rather than wrapping the
message onto a control it does have:

- a meter for strip index **8** and **15** (`D0 8C`, `D0 FC`) — nothing lit;
- a ring LED on **CC 56** and **CC 63**, past the eighth ring;
- **note 120**, which is in no table;
- **pitch bend on channel 10**, a tenth fader;
- **CC 76**, a thirteenth 7-segment digit;
- a colour message with **0, 4 or 9** colour bytes — only exactly eight is acted
  on, so there is no per-strip form and `Feedback::DisplayColors`'s fixed array
  is exactly right;
- colour bytes with **bit 3, 4 or 6 set** (`08…0F`, `10…17`, `40…47`) — the high
  bits are masked away and the strip shows the plain colour, so **there is no
  inverted variant in MC mode** as there is in Xctl. Note the codec is *stricter*
  than the device here: it refuses a colour byte outside 0…7 rather than masking
  it, deliberately, so a caller's mistake is visible instead of approximated;
- a scribble strip write and a device query on the **extender's** device ID
  (`0x15`) — no display change, no answer, so a two-desk rig can be addressed
  separately.

**Writing text does not disturb the colour**, confirmed: eight strips set to cyan
kept it across a full text write. Text and colour are independent state, which is
what both shipping implementations assume and what lets S21's shadow model track
them separately.

#### The pacing finding, which is worse than the reports said

Two independent projects reported the X-Touch "losing the tail" of a burst and
~1 ms fixing it. What is actually there is sharper, and it is the one genuinely
unwelcome result of this session. Measured, twice, reproducibly:

| What was sent | Result |
|---|---|
| 64 device queries (7 bytes) back to back, no gap | **64 answers. Nothing lost** — a burst of small messages is fine |
| 64 full scribble strip writes (63 bytes) back to back, nothing asked of the desk | **Harmless.** 30.9 ms for the burst, and the desk answered a query straight after |
| 64 × (63-byte write + query), each answer *waited for* | **64 answers**, 7.2 ms per exchange |
| 64 × (63-byte write + query) with **nothing waited for** | **17–34 of 64 answers lost**, and on both attempts the surface then **stopped transmitting entirely** |
| 2000 × 63-byte writes while an operator swept a fader | No hang, but the inbound stream **thinned from ~40 to ~11 messages a second** for the duration, recovering immediately afterwards |

**The failure mode is one-directional and it is not a disconnection.** When it
happens, the surface keeps *receiving* perfectly — scribble strip text written
while it was in that state appeared on the display — and sends nothing at all:
no button, no fader, no SysEx reply. Closing and reopening the MIDI port does not
help; a fresh process does not help. **Only a power cycle brings it back.**
(Unplugging USB was not tried.)

Three consequences, all for S21:

- **Pace the outbound path.** §5.2's 30 Hz coalescing is not an optimisation, it
  is what keeps the desk out of this state, and a minimum gap between messages
  belongs in the send queue. The safe figure this session can defend is the one
  it measured: at 30 Hz with a diffed shadow model the traffic is nowhere near
  the burst above.
- **Never poll the handshake.** The device query is the only inbound reply the
  surface generates, and the failure needs replies in flight. Use it once at
  connect, if at all — not as a keep-alive.
- **Silence is a fault state, and §5.3 does not currently cover it.** "Device
  disappearance" assumes the port goes away. Here the port stays open, writes
  still land, and the desk is deaf-mute in one direction. A surface layer that
  notices no inbound traffic for some seconds should say *the desk has stopped
  responding — power-cycle it*, because reconnecting will not fix it.

---

## 3. Layer 2 — surface model

The logical control set, independent of any device:

```
Strip[0..8].Fader            Strip[0..8].Touch
Strip[0..8].Encoder          Strip[0..8].EncoderPush
Strip[0..8].Button.Rec | .Solo | .Mute | .Select
Strip[0..8].Display.{ text, color, value }
Strip[0..8].Meter
Main.Fader                   Main.Touch                Main.Flip
Global.Play | .Stop | .Forward | .Backward | .Record
Global.Faderbank.Prev | .Next
Global.Channel.Prev | .Next
Global.Zoom.Up | .Down | .Left | .Right
Global.Jog
Global.F[1..8]
Global.Save | .Undo | .Cancel | .Enter
Global.Modifier.Shift | .Option | .Control | .Alt
Global.SevenSegment
```

This layer also owns the **shadow model**: the last state believed to be displayed on the device. Outbound traffic is generated by diffing new state against the shadow, never by re-sending everything.

### 3.1 As built (S21)

`prism_surface::SurfaceController` is layer 2 whole: the codec, two
`SurfaceState`s — what the show wants shown and what the desk was last told —
and the rules of §5 between them. It owns no port, no thread and **no clock**:
`push(bytes, now, sink)` and `pump(now, sink)` are handed the instant, which is
S19's rule one layer up and what lets every rule below be tested with arithmetic
instead of with a wait.

Four things this layer decides that §3 did not say, each because §2.7 measured
something:

- **A fader is a level, not a position.** Inbound positions are scaled against
  `McuProfile::max_reported_position()` — 16380, not 16383 — and outbound levels
  are scaled back against the same number, so a master at full parks the fader
  exactly where the surface itself reports full. Dividing by `FADER_MAX` gives
  99.98 % at the end stop.
- **Two acceleration curves, because the two relative controls measure different
  things.** `VPotAcceleration` reads the magnitude the desk reported (1…8);
  `JogAcceleration` reads the *interval since the last message*, because the
  wheel raises its rate and never its magnitude. Both are data, because how much
  a detent is worth is taste rather than measurement.
- **Colour quantisation lives here** (`prism_surface::quantize`), hue-first per
  §2.3: greys to white, and **only an exactly black colour to black**, because a
  strip that rounded itself onto an unlit backlight is an executor whose name
  cannot be read.
- **Ownership is data on the profile.** `McuProfile::permanent` names what still
  reaches MC in the combined mode (§4.3) and `reserved_buttons` names what
  PrismDMX must never touch. The shadow model consults them; the diffing has no
  condition in it. Inbound events are **not** filtered by ownership — if a
  control reaches us at all, the operator pressed it — but a *reserved* button's
  press is dropped and counted, so no binding above can ever be given it.

The state is two fixed-size arrays and the frame's queue is a third, so the whole
outbound path allocates nothing at all: `tests/surface_allocations.rs` measures
it with a counting allocator, the way S19 measured the codec.

---

## 4. Layer 3 — binding table

Shipped as `profiles/surface/xtouch.json`, validated on load. A malformed profile falls back and raises a warning — it never prevents startup.

> **Since S38 the table is edited at the desk, and the file is an *import***
> (§4.4). What a desk's keys do lives in `prism_core::MachineConfig` beside its
> rig and its port; naming a profile reads that file **into** it, and
> `Settings::surfaceProfile` is the record of where the table came from rather
> than where it lives. What "falls back" means therefore split in two, and both
> halves are S22's rule: a file that will not parse leaves **the table in force**
> standing, which for a desk that has never been edited is the built-in default
> and for one that has is its own.

### 4.1 Default bindings (from `XTouch.txt`)

The "Acts on" column is the practical consequence of **D11**: some controls reach the engine directly because latency matters, others change session state and the UI follows. Both travel the same command bus; there is no special path.

| Control | Default | Acts on | Configurable |
|---|---|---|---|
| Strip fader 1–8 | `Master` of the executor on that strip | Engine | yes — Empty / Master / Speed / XFade |
| Strip Rec / Solo / Mute / Select | `Go+` | Engine | yes — Empty / Go+ / Go− / LearnSpeed / Off / On / Flash / Toggle · **the binding is the button's *position* and the list above is the executor's own `buttonFunctions` (S34)**: `Command::ExecutorButton` carries which key was pressed, and `prism_core::Show::apply` resolves it. All eight are reachable |
| Strip encoder | `Empty` | Engine | yes — Empty / Master / Speed |
| Strip display | colour, name and value of the executor | — | — |
| Main fader | `XFade` of the selected executor | Engine | yes — Empty / Master / XFade · **one command, `SetExecutorMaster`, routed through the executor's own `faderFunction` (S34)** — so a fader set to `XFade` crossfades, one set to `Speed` moves the speed master, and one set to `Empty` does nothing |
| Flip button | `Go+` of the selected executor | Engine | yes |
| **Play / Stop / Forward / Backward** | On / Off / Go+ / Go− on the selected executor | Engine | yes — bound as written since **S34**, as `ExecutorButton` carrying the *function*, which is what a profile is allowed to name. **And this row is the one to spend carefully: it is the part of the panel that stays PrismDMX's in shared operation (§4.3), so it wants live-show functions, free assignments included** |
| **Record** | `Clear` (three-stage) | Programmer | yes — same section, same reasoning |
| **Faderbank ◀▶** | executor **page** down / up — 8 per page (D7) | **Session** | — |
| **Channel ◀▶** | **switch UI view** — `SelectView` (D8) | **Session** | — · **steps the order the View Selector Bar draws (S35)**: `MoveView` exchanges two views' numbers, so there is one order and the console cannot reach a view other than the one drawn next |
| **Zoom ▲▼** | programmer page up / down | **Session** | — · **this pages the encoder bar (S35)**. `programmerPage` had been in `ARCHITECTURE_SPEC.md` §4.1 since S12 with nothing reading it; the bar now draws four parameters to a page. How many fit is the *interface's* decision and the upper bound is too — `prism-core` deliberately does not know how many parameters a bank has (S13), the same split `SelectProgrammerParam` already has |
| **Zoom ◀▶** | select previous / next programmer parameter | **Session** | — |
| Jog wheel | change the value of the selected programmer parameter | Programmer | — · **which parameter that is comes from `FeatureGroup::attributes` (S26)** — one table, exported to the interface as `FEATURE_GROUP_ATTRIBUTES`, so the encoder bar highlights what the wheel turns |
| Encoder Assign section | switch encoder bank — Dimmer / Position / Color / Beam / Focus | **Session** | yes |
| **F1–F8 (XKeys)** | free: open window, jump to view, macro, executor | **Session** or Engine | yes |
| Save | save show file; **LED lit while unsaved changes exist** | Core | — |
| Undo | `Oops` | Core | — |

### 4.2 Profile shape

```jsonc
{
  "profileVersion": 1,
  "device": "behringer-x-touch",
  "bindings": [
    { "control": "Strip[*].Fader",  "action": { "t": "SetExecutorMaster" } },
    { "control": "Strip[*].Button.Select", "action": { "t": "ExecutorGo", "direction": "Next" } },
    { "control": "Global.Channel.Next",    "action": { "t": "SelectView", "relative": 1 } },
    { "control": "Global.F1", "action": { "t": "OpenWindow", "window": "FixtureSheet" } }
  ]
}
```

`Strip[*]` expands per strip with the strip index bound to the executor at `executorPage * 8 + index`.

### 4.2.1 As built (S22), and the three rows closed in S34

`prism_surface::Bindings` is layer 3 whole, and it is a table lookup with no
arithmetic in it: a fader arrives as a level and a detent as a parameter step,
both already worked out one layer down. Five things it decides that §4 did not
say:

- **Loading cannot fail.** `Bindings::load(text, profile)` answers with the
  profile when it parses and with the built-in defaults and a `ProfileError`
  when it does not — a function without an error path, because "a malformed
  profile never blocks startup" is stronger as a type than as a habit. It takes
  a **`&str`**: which file it was is `prismd`'s business
  (`prismd::surface::load_profile`), and that is what lets the fallback be
  asserted without a filesystem.
- **A profile is refused whole, not row by row.** A table half of which was
  understood is a desk that does some of what its author intended, which is
  worse to operate than one that does what the defaults say and complains. The
  document may carry prose — the verification record and this deployment's notes
  live in the same file — but a *binding* may not carry an unknown key, because
  there a typo is an argument that silently did not arrive.
- **The reserved button is refused by name.** A profile that binds SMPTE/Beats
  is rejected with a message that says why (§4.3). Layer 2 already drops its
  presses and counts them, so the binding could never have fired; the refusal
  exists for the person who wrote the profile.
- **`SurfaceContext` is the seam to the session.** `SetExecutorMaster` needs an
  executor number and a strip only knows it is the fourth strip; `Channel ▶`
  means *the next view* and this crate holds no view library. So the answers —
  executor page, selected executor, the neighbouring views, the programmer page,
  the attribute under the jog wheel — arrive as plain `Copy` data resolved by
  whoever holds the session. `prism-surface` still knows no show and no session.
- **The shipped file *is* the built-in defaults**, asserted by a test. Editing
  `profiles/surface/xtouch.json` is how the defaults are changed, and deleting it
  changes nothing.

#### The three rows that could not be bound, and how they were closed (S22 → S26 → S34)

**This is history now.** It is kept because it is the clearest statement in the
project of *why* a binding table may not decide what a press means, and because
the shape it arrived at is the shape the settings editor (S38) will offer.

S22 found that three rows of §4.1 named something the command vocabulary had not
got, and refused to invent it:

| §4.1 says | What S22 bound | Why it could not be bound as written |
|---|---|---|
| Main fader = `XFade` of the selected executor | `SetExecutorMaster` on the selected executor | `XFade` is an `ExecutorFaderFunction` — show data on the executor (`ARCHITECTURE_SPEC.md` §6). There is one fader command for all four functions, and nothing read the setting |
| Play = `On` | `ExecutorGo`/`Next` on the selected executor | There was no command that could carry `On`; it is an `ExecutorButtonFunction`, and `ExecutorGo` was the nearest thing that started a sequence |
| Strip buttons configurable to `LearnSpeed`, `Flash`, `Toggle`… | `ExecutorGo`, `ExecutorOff` | Same list, same reason. A binding table can only send commands; those names are the executor's own button functions |

S22 named it as **one** gap rather than three: *the protocol had no command that
pressed an executor's button and let the executor decide what that meant.*

S26 met the same gap from the interface and resolved it the same way — a strip
drew the four buttons the show assigned it, pressed the three that had commands,
and drew the other four **disabled with the reason on the button**. Resolving
`Toggle` against `isActive` in a client would have been a client deciding what a
show's own setting means: two clients would race, and the daemon would be told to
do something nobody pressed. S26 kept that as a mutation check, and it is still
there.

**S34 built the command, and all three rows are bound as written.** What each
function needed, and what it got:

| Function | What it needed | Where it lives |
|---|---|---|
| `On`, `Off` | `prism_engine::TickCommand::SetExecutorActive`, which already existed — "on a loaded executor this is *start the sequence* and *stop it*, not a raw activation" (S5) | `Effect::ExecutorOn` / `Effect::ExecutorOff` |
| `Toggle` | The same command, with the **daemon** reading `isActive` — which the daemon may do and a client may not | `prism_core::Show::apply`, one line, and S26's mutation check still guards it |
| `Flash` | A **temporary** master override that does not disturb the stored master: press raises, release restores | `PlaybackSource::set_flash` — a layer applied where `docs/DMX_MERGE.md` §2.1 applies the master, so the stored one is never written to |
| `LearnSpeed` | Speed masters, named in `docs/DMX_MERGE.md` §4 item 3 with nothing implementing them | `Executor::speed`, `prism_domain::SPEED_UNITY`, and `CuePlayer`'s accumulated clock |
| `XFade` on the fader | Something to read `faderFunction` | `prism_core::Show::apply` routes `SetExecutorMaster` through it; the crossfade replaces the transition's *clock* and nothing else |

The command is `Command::ExecutorButton { executorId, button, pressed }`. The
`pressed` flag is what `Flash` needs, and `button` is a
`prism_domain::ExecutorButtonRef`, which is either:

- **a `Slot`** — a hardware position on a strip. What it does is the executor's
  `buttonFunctions`, and nothing but the executor may decide that. This is what
  the four strip buttons bind to, and it is the whole of D3 for playback.
- **a `Function`** — a function named outright by *this file*. That is the desk's
  own configuration, written by a person, and it is what the transport row has
  always been: §4.1 assigns Play `On` in prose, and the profile says the same
  thing in JSON. It is not a client inferring anything at run time — `Toggle`
  bound this way is still resolved against `isActive` by the daemon.

`deviationsFromSection41` in the shipped profile is therefore **empty**, and
`deviationsClosedInS34` records what it used to say.

### 4.3 Sharing the surface with a sound console (Xctl+MC)

> **Provenance: this section is the operator's, not a measurement.** Everything in
> §2.7 was read off the desk in S20; everything here was stated by the person who
> owns and runs it, describing how it is to be deployed and how the combined mode
> behaves. It is written down because it changes what layer 3 may assume, and it
> is marked because the difference between *measured* and *reported* is the whole
> point of §2.5. Verifying it needs the sound console present as well and is
> recorded as an open item in §7.

**The intended deployment is one X-Touch driving two hosts at once**: the venue's
sound console over Xctl, and PrismDMX over MC, in the surface's combined
**Xctl+MC** mode. That is not the configuration S20 measured — the desk was in
plain MC — and it does not change a single number in §2, because the MC half of
the combined mode is the same MC. What it changes is **how much of the panel is
ours**.

**Only what Xctl does not use reaches MC.** In the combined mode the surface
routes each control to one host or the other: whatever the Xctl side claims
belongs to the sound console, and the rest keeps driving MC output *and* keeps
listening to MC input, permanently. In practice the reliably-ours set is:

| Guaranteed to reach PrismDMX | Note / CC |
|---|---|
| The whole transport section — Rewind, Forward, Stop, Play, Record | 91–95 |
| The jog wheel | CC 60 |

These reach PrismDMX **permanently**, whichever host the surface is currently
showing. Everything else follows the switch.

**The rest of the panel is not lost — it is one button away.** The operator can
switch the whole surface to MC at any time (with the reserved SMPTE/Beats button,
below), and then every strip, fader, encoder and F-key is PrismDMX's, exactly as
in the dedicated deployment; the sound console is simply not operable meanwhile.
So the split is **not** a restriction on what the profile may contain.

What the permanently-MC set buys is different and more valuable: **no switching**.
Whatever sits on the transport section and the jog wheel is in reach *during a
show*, without taking the sound desk away from whoever is using it.

**Three consequences.**

1. **The transport section is the always-hot part of the console, so what sits
   there should be worth having mid-show.** Not "everything important must fit
   into five buttons" — nothing has to fit, because the full surface is a button
   press away — but "these five are the ones reachable without a mode change, so
   spend them on live-show work". That includes **freely assignable buttons**:
   an operator will want a *tap for speed* against a particular speed master, a
   macro, or a look for a moment in the show on the keys that are always in reach,
   and that is a better use of them than a transport metaphor PrismDMX does not
   have. §4.1's defaults (go / off / next / previous / clear) are a reasonable
   starting point rather than a fixed layout, and that row is already marked
   configurable.
2. **Nothing is unreachable, but D7 and D8 need a mode change.** `Faderbank ◀▶`
   (executor paging) and `Channel ◀▶` (`SelectView`) sit outside the permanent
   set, so in shared operation they cost a switch to the full surface. That is
   fine for paging and view changes, which are setup-shaped rather than
   cue-shaped; it is a reason not to put anything *time-critical* there, and it
   is why the UI and the Web Remote keep their own paths to both.
3. **Feedback must not assume it owns a control.** The shadow model may only
   drive LEDs for controls MC actually holds at that moment; lighting a strip's
   Select LED while the surface is showing the sound console is either ignored
   or, worse, fights that console's own feedback. S21 should hold the ownership
   set as **data on the profile**, in the same way `unlit_buttons` is, so that
   "which controls are ours" is one edit rather than a condition scattered
   through the diffing.

**What "tap for speed" needs — built in S34.** `LearnSpeed` existed only as an
*executor button* function (`ExecutorButtonFunction` in `ARCHITECTURE_SPEC.md`
§6, and `XTouch.txt`, which offers it on a strip's buttons but not on the
selected executor's), and the speed masters it taps were named in
`docs/DMX_MERGE.md` §4 item 3 with nothing implementing them and no domain type
carrying one. S34 built both: `Executor::speed` is the rate, in units of
`prism_domain::SPEED_UNITY`, and two taps inside four seconds mean *the running
cue's transition should take that long*.

It is therefore bindable on a transport key today —
`{ "t": "ExecutorButton", "target": "Selected", "button": { "t": "Function", "function": "LearnSpeed" } }`
— which is what this section asked for. The default profile does **not** do it,
because §4.1's five transport defaults are what the operator has seen so far and
choosing their layout is theirs; the point is that the row can now be spent.

What is still *not* here: a speed master that is not an executor's. `Executor::speed`
is per executor, which is what the domain carries and what an X-Touch strip
addresses. A named speed master shared by several executors is a bigger idea and
nothing has asked for one.

**SMPTE/Beats (note 53) is reserved and must never be bound.** In the combined
mode **it is the button that switches the surface between the two hosts** — it is
the operator's way back to the sound desk, and a console that steals it is a
console somebody has to power-cycle to get out of. The decision goes further than
the shared mode, and deliberately: **PrismDMX leaves it unbound in every mode**,
so that the same profile is safe on a desk whose mode nobody has checked.

It is worth noting, without making more of it than the evidence supports, that
note 53 is one of the two buttons S20 found to have **no LED at all** (§2.7). A
button the firmware reserves for itself is a button it would have no reason to
give a host-controllable lamp. That is a consistent story rather than a
demonstrated one — Name/Value has no LED either and switches nothing — but it is
one more reason not to build anything on top of it.


### 4.4 The control editor *(S38)*

**The table is data an operator edits, not a file an installer writes.** Until
S38 `prism_surface::Bindings` was layer 3 whole and it was read **once, from a
path, at start-up**: there was no command that read the table in force and none
that wrote one, so an operator who wanted a key to do something else edited JSON
beside the daemon and restarted it. S37 could *name* the file and thereby re-read
it, and that was all.

#### Where the table lives, and why it is not the file

Three places were possible — a file beside the daemon, `MachineConfig`, or both
— and the answer is **`MachineConfig`**, for three reasons in descending order of
force:

1. **It is the same kind of fact as the rig and the port.** S33 put the venue's
   cabling there and said in as many words that *a later session that wants
   anything else about this building puts it here*; S36 put the desk's MIDI port
   there; what somebody has made that desk's keys do belongs to the building for
   exactly the same reason. A show carried to another hall on a stick must not
   arrive with the last hall's F-keys on it.
2. **It is what makes *two editors, one table* structural.** `machine.json` is
   written by one thread under one lock, and every edit is one
   `MachineChange::SurfaceBinding` naming **one control** — so two operators
   changing two keys cannot undo each other. A command carrying the whole table
   would have made that a race; §4.2's file format is a whole table because a
   *document* is written by one person at a time, and a protocol is not.
3. **The shipped profile stays what a test says it is.** S22 asserts that
   `profiles/surface/xtouch.json` **is** `Bindings::defaults()`; an editor that
   wrote to it would change the defaults, and it would write into an installation
   directory a school's account often cannot.

**A file is therefore an import.** Naming one replaces the stored table with what
the file says; the path is kept so the panel can say where the table came from
and offer to read it again. The alternative — the file winning at every start —
was rejected because it has one unacceptable consequence: an operator who rebound
a key at the desk would find it back the way it was the next morning.

#### What travels

| Shape | What it is for |
|---|---|
| `Query::SurfaceBindings` | The table **in force**, one row per control the surface has. A question rather than a snapshot field because it is *derived* — the built-in defaults, a profile read into this machine's rows, and the rows typed since — and because seventy-three rows are only ever looked at by an open editor |
| `Delta::SurfaceBindingsChanged { revision }` | The change **token**, not the table: `Delta::SurfaceChanged`'s shape for a port, one device along. Both editors are told the same number, which is what makes *one table* something a test can assert |
| `MachineChange::SurfaceBinding { control, action }` | **One control at a time**, which is `OutputChange`'s rule and `CueProperty`'s before it |
| `MachineChange::SurfaceLearn { learning }` | Arms learn. The one member of `MachineChange` that is written down nowhere |
| `Delta::SurfaceLearnChanged { learning, control }` | Both edges of learn, **broadcast**: there is one desk, so there is one learn, and the operator pressing a key has no idea which browser asked |

Each row carries two facts that are the **device profile's** rather than the
table's, because a client holds no profile: whether the control keeps reaching
PrismDMX in the combined mode (§4.3) and whether it may be bound at all.

#### Learn is §2.7's method rule, run backwards

S20 established that the way to find out what a control sends is to **press it
and read what arrives**, never to ask the profile. An editor has the same
question about the same desk, so it gets the same answer: the operator presses
the key they mean and the daemon names it, rather than hunting for
`Global.AssignPlugin` in a list of sixty-four.

Two things about it are worth stating because they are what the code is for:

- **While learn is armed the control does not fire.** An operator finding out
  what the Record key is called would otherwise clear their programmer to find
  out, and one learning a transport key would start a cue on a stage. The
  *release* of a learned button is swallowed with it, because layer 3 forwards
  both edges of an `ExecutorButton` and a `Flash` released without ever having
  been held is a master put back that was never taken.
- **It is one shot.** The first control disarms it, so a client that went away
  mid-learn cannot leave a desk whose keys do nothing.

And it can never name the reserved control, for a reason that is not the
editor's: layer 2 drops SMPTE/Beats' presses and counts them, so they do not
reach layer 3 at all.

#### The reserved control is refused three times over

§4.3 says PrismDMX never binds SMPTE/Beats. That is now enforced in three places
and the wording is stated **once**, as `prism_domain::RESERVED_REASON`:

| Where | What it refuses |
|---|---|
| `prism_surface::Bindings::parse` (S22) | A profile **file** that names it |
| `prism_core::MachineConfig::configure` (S38) | A **command** that names it, before anything is written |
| `prism_surface::Bindings::from_rows` (S38) | A **stored** table that names it — a `machine.json` from an older build, or edited by hand — which falls back to the defaults rather than blocking a desk |

`prism_domain::RESERVED_BUTTONS` is the array and `McuProfile::reserved_buttons`
points at it rather than carrying a copy, so a device profile and the protocol
cannot come to disagree about which control an operator is forbidden to spend.

#### The vocabulary moved to `prism-domain`, and the note map did not

`BoundControl`, `SurfaceAction`, `ExecutorTarget`, `Step`, `GlobalButton` and
`StripButton` are `prism-domain`'s since S38 and are re-exported from
`prism-surface`. The reason is that they **travel**: a `MachineChange` carries
one, an editor draws one, and the interface's copy of each type is generated from
the domain. `IMPLEMENTATION_PLAN.md` S32 predicted the split from the other side
— an OSC control that fires a command belongs in the same editor rather than in a
second mapping system — and a vocabulary that is not MCU-specific cannot live in
the MCU crate.

**§2.1's note and CC numbers did not move and must not**, which is the promise
that section makes in as many words: they are `prism_surface::profile::X_TOUCH`
and nowhere else. So are the shadow model, the codec, and the resolution of an
action into a `Command` — which needs an event and a session, and the domain has
neither. The split is *names* against *meaning*, and it is the same line §4.2.1
has drawn since S22.

---

## 5. Feedback rules

These two rules are not optimisations; without them the surface misbehaves visibly.

### 5.1 Touch suppression

While a fader reports touch (Note On 104–111 with velocity 127), PrismDMX sends **no** position updates to that fader. On release it waits 150 ms, then resynchronises to the authoritative value.

Without this the motor fights the operator: the engine echoes the value back, the motor drives to it, the movement generates a new inbound value, and the loop oscillates.

### 5.2 Coalescing and priority

Outbound updates are batched at a maximum of 30 Hz and sent only where the value differs from the shadow model. USB MIDI cannot absorb an unthrottled delta stream, and a saturated buffer shows up as laggy faders.

When bandwidth is constrained, the send order is:

1. **Motor faders** — most visible, most misleading if stale
2. **Button LEDs** — state feedback the operator acts on
3. **Scribble strips** — names and values, tolerant of a late update
4. **Meters** — purely decorative; dropped first

### 5.3 Connection handling

On connect or reconnect the shadow model is invalidated and the full surface state is transmitted once (a "resync burst"), rate-limited so the device is not flooded. Device disappearance is treated like any other hardware fault: the MIDI threads report `Disconnected`, the engine is unaffected, and reconnection is attempted with backoff.

**A second fault state, measured in S20, that this rule does not cover.** The
X-Touch can stop transmitting while remaining fully present: the port stays open,
writes still land on the display, and no button, fader or reply ever comes back
(§2.7). Reopening the port does not help and neither does a fresh process — only
a power cycle. So *the port is there* is not evidence the desk is listening, and
a surface layer that only watches for disappearance would show a green light
beside a dead console. Silence beyond a few seconds on a desk that was talking is
a fault worth naming to the operator, and the message has to say **power-cycle
it**, because reconnecting is the one thing that will not work.

The resync burst is also the traffic pattern most likely to *cause* it, which is
why it is rate-limited rather than sent as fast as the port accepts.

### 5.4 As built (S21)

| Rule | How it is kept |
|---|---|
| **Touch suppression** (§5.1) | A touch sets a flag *and* **invalidates** what the shadow model believes about that fader, because the operator has just moved the motor. Nothing is queued for a touched fader; on release the fader is held for 150 ms and then queued once. The invalidation is what makes it **exactly one** rather than *one if the value happened to change* — a mutation that dropped it left the ordinary test green and only the fader-nobody-moved test red |
| **Coalescing** (§5.2) | The diff runs at most once per 33.3 ms, and a control is queued at most once per diff. 1000 changes in 100 ms cost at most three messages. Anything not sent is still different at the next frame, because the shadow moves only when a message goes out — so coalescing and dropping are the same mechanism |
| **Priority** (§5.2) | The diff is walked in the documented order and the queue is sent in the order it was built, so the ordering is the loop rather than a comparison. Rings are LEDs; the 7-segment display is a display. Under pressure the tail is simply not reached, which is why meters are last |
| **Pacing** (§2.7) | A floor of 1 ms between outbound messages, enforced against the caller's clock: pumping more often than that sends no faster. A full resync burst is 156 messages and therefore ~156 ms, spread across five frames |
| **The handshake** (§2.7) | Sent **once**, and only when the surface has gone quiet after having talked, and only when the send queue is empty — the opposite of the condition that caused the fault. If it goes unanswered the health is `Unresponsive`, whose remedy text says *power-cycle*, because reconnecting is the one thing that will not help |
| **Connection** (§5.3) | `connected` invalidates everything, so the resync burst is the ordinary diff rather than a special path — and so are a reconnect and a switch back from the sound console. `disconnected` keeps the picture and sends nothing; the engine is never told |

### 5.5 As wired to a real port (S36)

S21 built the two transitions and asserted them against a mock; what did not
exist until S36 was anything that could *cause* one, because no code in the
workspace opened a MIDI port. `prism-midi` is that code, and the rules it keeps
are these:

| Rule | How it is kept |
|---|---|
| **A port is named, not numbered** | A configuration holds the port's *name*, because an index renumbers itself when somebody moves a plug. `prism_midi::selects` compares names after taking the platform's decoration off — a Windows driver-instance prefix (`2- `), an ALSA client address (` 24:0`) — with the exact name tried first, the undecorated name second and a case-insensitive fragment last. Looser rules come last so a venue with two X-Touches can always write both names out in full and neither row can drift onto the other |
| **A port that is not there is not a failure** | `MidiSurfacePort::attach` never fails. The daemon warns with the reason, starts, and keeps trying on a backoff that doubles from 250 ms to a five-second ceiling. A desk switched off half an hour before a show must not be why the show cannot be run |
| **Only two things close a port** | It disappearing from the enumeration — checked once a second, not once a millisecond — and a write the operating system refuses. Which is to say: **not silence** |
| **Silence is not a reason to reconnect** (§2.7) | The whole of S20's unwelcome finding, kept as a rule one layer below the health state that reports it. A surface that has stopped transmitting while still receiving is `SurfaceHealth::Unresponsive`, only a power cycle recovers it, and a port layer that reopened on silence would churn the one state that cannot be recovered that way *and* take the diagnosis away from the operator. Ten seconds of nothing inbound with writes still landing leaves the port open and the count of opens at one — `a_desk_that_has_gone_quiet_is_not_reopened` |
| **The clock is still an argument** | `refresh(now)`, like `push(bytes, now, sink)` and `pump(now, sink)` above it. The surface thread in `prismd` reads the only real instant in the stack, and the reconnection schedule is arithmetic a test can walk in milliseconds |
| **Nothing is paced here** | The minimum gap, the 30 Hz coalescing and the once-only handshake are all §5's, enforced one layer up. A second pacer would be a second opinion about §2.7 |

**One deliberate non-rule, and it is the reverse of what §5.1 suggests.** An
inbound fader *move* does not invalidate the shadow, only a *touch* does. §5.1's
own description of the oscillation says the motor's movement generates an inbound
value — so a layer that forgot its belief on every inbound position would resend
after its own echo, once per frame, for ever. Touch is the signal that the desk
is somewhere we did not put it; a move without a touch is either that echo or a
fader moved with a pen, and the second one costs a stale value until the next
change rather than a permanent fight.

---

## 6. Testing

| Test | Method |
|---|---|
| Codec round trip | Table-driven: MIDI bytes → control event → MIDI bytes, asserting byte equality both ways |
| Running status | Feed a stream with omitted status bytes; assert identical decoding |
| Malformed input | Fuzz with truncated and out-of-range messages; assert no panic, no allocation growth, correct discard counters |
| SysEx reassembly | Split a scribble strip message across packet boundaries; assert single correct event; assert timeout drops an unterminated message |
| Touch suppression | Simulate touch → engine value change → release; assert no outbound pitch bend during touch and exactly one resync 150 ms after release — `tests/feedback_rules.rs` (S21), which also asserts it for a fader *nobody moved*, the case that separates a guarantee from a coincidence |
| Coalescing | Drive 1000 value changes in 100 ms; assert at most 3 outbound messages for that control — `tests/feedback_rules.rs` (S21) |
| **Priority and pacing (S21)** | Every control on the surface different at once, drained one message per minimum gap: the classes must come out in §5.2's order, and a message is classified by a `match` on its **status byte** written out of §2.2 by hand rather than by the crate's own classifier, which is the code under test |
| **Surface → UI (D11 gate)** | Send `Channel ▶` and F1 with **no UI client connected**; then connect a client and assert its `Snapshot` contains both the new view and the opened window — **passed 2026-08-13 (S22)**, `crates/prismd/tests/surface_gate.rs`. The precondition is asserted rather than described: the daemon's client count is **0** when the buttons are pressed and still 0 when the session has changed. The two note numbers are written out by hand from §2.1 (49 and 54), because asking the profile what to press would be asking the code under test |
| **The binding table (S22)** | Every row of §4.1 transcribed **by hand** into `crates/prism-surface/tests/bindings.rs`, plus the complement — the twenty-six panel buttons that *are* bound and the thirty-eight that are not. A test that built its expectations out of `Bindings::defaults()` would pass for any defaults at all, including ones with Play and Stop swapped. A second test asserts the shipped profile equals those defaults, and a third that no profile whatever it says can stop a desk starting |
| **The device's own bytes (S20)** | `tests/hardware_capture.rs` replays four recordings of the real X-Touch — every strip button, the whole panel, all nine faders, all nine relative controls — and asserts that each message decodes to the control the table names, that the profile finds **nothing** it cannot describe, and that every message re-encodes to exactly the bytes the desk sent |

Target coverage on `prism-surface`: **> 95 %**, per `CLAUDE.md`. All tests run against a mock MIDI port or a recorded capture — **no hardware required**, including the ones that verify a specific physical device.

One trap worth knowing, because a mutation check found it: replaying real bytes
and re-encoding them is *still* a function composed with its own inverse. Swapping
the two halves of the 14-bit fader split in both directions leaves the byte
comparison green on genuine device recordings. What catches it is a claim about the
values rather than the bytes — the measured fact that positions are multiples of 4
and stop at 16380, plus a small table of captured messages whose meaning was
worked out **by hand**. This is S19's finding (§2.6) surviving contact with real
data, which is worth stating plainly: *real* input does not make a round trip
self-validating.

---

## 7. Hardware verification checklist — closed 2026-08-13 (S20)

**Done.** Every item below was worked against a real Behringer X-Touch in
Mackie Control mode; §2.7 is the measurement and `crates/prism-surface/tests/`
holds the recordings and the test that replays them. Two items are marked
◻ *untested* rather than ticked, and both say why — an absent pedal and an
untried cable pull are not results.

- [x] **Mode and connection** — **MC over USB**, read off the front panel with channel 1 SELECT held at power-up, and confirmed by the device ID the surface answers on (§2.7)
- [x] **Firmware version** — **V1.25**, so ≥ 1.22 and the colour extension exists. Found by SysEx rather than by reading the boot screen: `F0 00 00 66 14 13 F7` answers `…14 "V1.25"`, an undocumented request now written into §2.3
- [x] **Every button press reconciled against §2.1** — the whole map, not a sample. All 40 strip notes pressed strip by strip; the 64 panel notes checked by **lighting one LED at a time and pressing the button that lit**, which verifies the inbound *and* outbound halves in one pass. 60 of 64 agreed in both directions, notes 52 and 53 inbound-only (no LED — §2.7), and the note map needed no change
- [x] **Fader pitch-bend channels and 14-bit byte order** — channels 1–8 left to right, 9 for the main fader, **LSB first**, proved by the top-of-travel message `E0 7C 7F`. Also found: the faders are 12-bit and stop at 16380 (§2.7)
- [x] **Fader touch notes including the main fader** — 104–111 and 112
- [x] **V-Pot relative encoding at speed** — **it accelerates: magnitudes 1…8.** And the jog wheel does not: ±1 in 404 messages. The two are not the same control (§2.7)
- [x] **Scribble strip offsets, and whether the 56th character arrives** — `7n` confirmed by writing one strip; **all 56 arrive**, Ardour's 55 is caution. It is one continuous 112-character buffer: a write at offset 55 spills onto the lower line
- [x] **Colours** — the command and all eight values confirmed in the documented additive order. **No inverted variant in MC mode** (bits 3, 4 and 6 are masked away); **a message without exactly eight colour bytes is ignored** (0, 4 and 9 all tried), so there is no per-strip form; and **text does not reset the colour**, confirmed rather than assumed
- [x] **Meter format, overload flag and decay** — `(strip << 4) | level` confirmed, `0xE`/`0xF` set and clear the marker, **but the decay is under a second** rather than ~300 ms per division. The LCD meter-mode SysEx does nothing at all on this surface, and the overload marker works anyway
- [x] **7-segment addressing, character encoding and channel** — digit 0 is the rightmost (`BA9876543210` from `0…9 A B`), CC 76 ignored, and **the surface accepts the display on channel 16 as well as channel 1**, which justifies the codec taking both
- [x] **What a 7-segment `0` draws** — **a blank**, and so does 32. The stripping rule stops one character short, `'@'` cannot be displayed at all, and `SegmentChar::from_ascii` now refuses it instead of returning a code that draws nothing
- [x] **A strip index above 7 is ignored**, not wrapped — meters at index 8 and 15, ring LEDs on CC 56 and 63, note 120, pitch bend on channel 10 and CC 76 all did nothing. Also the extender device ID `0x15`, which this surface neither displays nor answers
- [x] **Whether back-to-back messages are lost, and the pacing needed** — measured, and it is worse than the reports: a burst of small messages is lossless, a burst of large ones is harmless on its own, but **saturating both directions at once loses replies and can stop the surface transmitting until it is power-cycled.** §2.7 has the recipe, the numbers and the three consequences for S21
- [x] **Round-trip latency** — **median 0.71 ms** host → surface → host over 60 exchanges (min 0.61, max 1.02). Measured through the handshake, because a motor fader generates no reply; a fader move the *operator* makes is reported every 19.8 ms, which is the figure that bounds §4.3's budget
- [x] **§2 updated and the UNVERIFIED banner removed**; `profiles/surface/xtouch.json` written with the verification recorded in it. `prism_surface::X_TOUCH.verified` is `true`, and `ARCHITECTURE_SPEC.md` §14's row is closed
- ◻ **Foot switches (notes 102, 103)** — *untested.* Nothing is plugged into either jack, and there is no way to make an absent pedal send a note. Their LEDs were driven and, as expected for a jack rather than a button, nothing lit
- ◻ **The combined Xctl+MC mode** — *untested, and it is the deployment that is actually planned* (§4.3). The desk was in plain MC for all of the above, which is the right mode to have verified first: the MC half of the combined mode is the same MC, so nothing in §2 depends on it. What is **not** verified is the division of the panel — that only what Xctl leaves unused reaches MC, and that the transport section and the jog wheel are what reliably remain. That is the operator's account of the surface, not a measurement, and checking it needs the sound console on the other end of it. Worth doing before S22 finalises a default profile, because it decides which bindings can be relied on. **S21 has modelled it as data** (`McuProfile::permanent`, `SurfaceMode::Shared`), so confirming or correcting it is a one-line edit rather than a change to the diffing
- ◻ **Behaviour when the USB cable is pulled** — *untested at the device, and **S36 answered everything except the device*** (§5.5). S21 built the handling against a mock and S36 built the thing that can produce the event: `prism_midi::MidiSurfacePort` closes a port when it leaves the enumeration or refuses a write, reopens it on a backoff, and `prismd::surface::SurfaceLink::follow_the_cable` turns those two edges into `disconnected` and `connected` — so a reconnect redraws the whole surface once, paced, and the engine hears about neither. All of that is asserted with nothing plugged in. What is **still** left for a device is the one thing a mock cannot say: whether `midir` produces exactly one disappearance and one reappearance for one real unplug and replug, rather than a flap. The backend is now chosen and named, which is the half of this row S21 could not close. `ARCHITECTURE_SPEC.md` §14 carries it as a 🔌 row with the recipe

Findings go into this document. Discrepancies against the MCU standard are recorded explicitly rather than silently corrected, so the next device profile can reuse the knowledge — see §2.7, which is that list.

---

## 8. Reproducing any of this

`tools/xtouch-probe` is the tool the measurements were taken with. It is a
**standalone crate, deliberately outside the workspace**: opening a MIDI port is
platform code, and `ARCHITECTURE_SPEC.md` §10.1 allows `prism-surface` none. So
`cargo test --workspace`, `cargo clippy --workspace --all-targets` and the ARM64
cross-check never see `midir`, and `prism-surface` gained no dependency at all.

**S36 did not move it, and `prism-midi` is not a replacement for it.** The
shipping backend now lives in the workspace, in a crate whose whole purpose is to
hold the platform split — but the probe does things no test may: it walks a panel
button by button, it lights one LED at a time and asks which button lit, and it
floods the surface in both directions until it stops answering. That last one is
how §2.7 was found and it is not something to run against a desk somebody is
about to use. The division is the same as it always was — **platform code and a
device in a tool, evidence in a fixture** — with one line added: the *shipping*
port is `prism-midi`, and it opens nothing that a name in a configuration did not
ask for.

To read the names this machine offers, without starting anything:

```bash
cargo run -p prismd -- --midi-ports
```

It depends on `prism-surface` by path, which is the point: **every byte it sent
was produced by `Feedback::encode_into` and every byte it read was decoded by
`McuCodec`**, so what was verified is the shipping codec rather than a
transcription of it. The exceptions are marked in its output — the `raw` command
and the steps that deliberately send what the codec refuses to encode.

`PROGRESS.md` §3.4 carries the commands.
