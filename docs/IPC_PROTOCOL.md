# IPC_PROTOCOL.md — Daemon ↔ Client Protocol

**Status:** specification.
**Parent document:** [`ARCHITECTURE_SPEC.md`](../ARCHITECTURE_SPEC.md) (decisions D2, D3, D11).
**Implemented by:** `prism-ipc` (framing and transport), `prismd` (server), `prism-app` and the Web Remote (clients).

---

## 1. Why there is a protocol at all

**D2** splits PrismDMX into two processes so a UI crash cannot stop DMX output. That boundary needs a wire protocol. **D3** makes the daemon the single source of truth, so the protocol is deliberately asymmetric: clients send intent, the daemon sends facts. A client never computes state it then expects the daemon to accept.

Every client is equivalent. The desktop shell, the Web Remote and the X-Touch surface controller all speak the same command vocabulary. There is no privileged client and no back door for the console — see **D11**.

---

## 2. Transports

| Transport | Used by | Rationale |
|---|---|---|
| Named pipe (Windows) / Unix domain socket (Linux, macOS) | Desktop shell on the same machine | Lowest latency, no TCP stack, no port to firewall, no accidental network exposure |
| WebSocket over `axum` | Web Remote, remote clients | Works from any browser; binds `127.0.0.1` by default |

Both transports carry identical **payloads** and identical messages. Transport choice is not visible above `prism-ipc` — every transport produces a `Wire`, which is a duplex of byte payloads, and the server, the client, the handshake and the backpressure policy are one code path above it.

The length prefix of §3 belongs to the byte-stream transports. A WebSocket binary message already carries its own length, and a second copy of the same number inside it would be two lengths that can disagree. What both transports share is the **limit**: `MAX_FRAME_BYTES` is checked against the length prefix on a stream, and given to the WebSocket implementation as `max_message_size` on a WebSocket — in both cases before a buffer for the body exists. *(S16)*

### 2.1 Network exposure

The WebSocket listener binds to loopback unless the user explicitly enables LAN access in settings. Enabling it requires a token, which the daemon generates and the UI displays. This is a deliberate default: school networks are shared, and an unauthenticated lighting console reachable from any classroom machine is not acceptable.

### 2.2 Discovery

`prismd` writes a lock file into the user data directory containing its PID and the IPC endpoint. Clients read it to find the daemon. A stale file left by a crash is detected and replaced. This same file provides the single-instance guarantee described in `ARCHITECTURE_SPEC.md` §10.3 — two daemons driving the same output must be structurally impossible.

**As built (S17).** `prismd.lock` is the document: `{ pid, local?, websocket?, token? }`, as JSON, rewritten whenever a listener binds — so an endpoint in it is one that already exists. Beside it is `prismd.guard`, an empty file the daemon holds an advisory lock on for its whole life; that lock is the single-instance guarantee and the staleness check at once, because the operating system releases it when the process ends, killed or not. The two are separate files because an exclusive lock on Windows would stop clients reading the document. `ARCHITECTURE_SPEC.md` §10.3 has the rest of the reasoning, including why the process id is reported rather than believed.

The local endpoint is `prism_ipc::local::daemon_address(label)`, where the label is derived from the data directory — so two user accounts on one machine do not ask for the same pipe name, and a client that knows the directory can work out the address without reading anything. Reading the file is still the way to do it: that is what makes discovery one file rather than two programs agreeing about a hash.

---

## 3. Framing

```
┌──────────────┬───────────────────────────┐
│ u32 LE length│ MessagePack payload       │
└──────────────┴───────────────────────────┘
```

- **MessagePack** (`rmp-serde`) — compact, schema-free enough to evolve, and fast to decode in both Rust and the browser. Written with `to_vec_named`: `Command` and `Delta` are internally tagged, and the compact array encoding of a struct has nowhere to put the tag *(S1)*.
- **Maximum frame size** is enforced on both sides, and it is 1 MiB. An oversized frame closes the connection rather than allocating: the length is checked before a body buffer exists, which is a claim about memory and is measured as one *(S16, `crates/prism-ipc/tests/oversized_frame.rs`)*.
- **Maximum nesting depth** is enforced on decode, and it is 128 levels — the same limit `serde_json` applies, against the same attack. `JsonValue` is recursive and MessagePack has no limit of its own, so a payload of a few kilobytes can nest a hundred thousand deep; a stack overflow is not recoverable in Rust and would take the DMX output with it. The depth is established by walking the payload with an explicit stack **before** `serde` sees it, because the recursion to be stopped happens inside serde's own buffering of an internally tagged enum *(S1 finding, S16 implementation)*.
- Message types are distinguished by a tagged enum inside the payload, not by a separate header byte.
- **Serialisation is fallible.** A non-finite `f64` is refused in both directions *(S1)*, so a message that cannot be encoded is reported rather than emitted.

---

## 4. Message types

| Message | Direction | Reliability |
|---|---|---|
| `Hello` | client → daemon | reliable, first message |
| `Snapshot` | daemon → client | reliable, response to `Hello` |
| `Command` | client → daemon | reliable, ordered — never dropped |
| `Query` | client → daemon | reliable, ordered, **changes nothing** |
| `Delta` | daemon → client | reliable, ordered |
| `Answer` | daemon → client | reliable, to the one client that asked |
| `Telemetry` | daemon → client | **droppable**, coalesced |
| `Ack` / `Reject` | daemon → client | reliable |

There are two envelopes, not one: `ClientMessage` carries `Hello` and `Command`, `ServerMessage` carries the rest. A type that could carry either would let a client send a `Delta`, and **D3** says it cannot *(S16)*.

A `Command` carries a `seq`, and `Ack` and `Reject` echo it. The daemon stores that number and never interprets it; it exists because with several commands in flight — the normal case for a fader bank — an unaddressed rejection tells a client only that *something* failed *(S16)*. A `Query` shares that numbering and `Answer` echoes it, because both travel on the one ordered channel and two numberings would let an answer and an acknowledgement collide *(S27)*.

### 4.1 Handshake

```mermaid
sequenceDiagram
    participant C as Client
    participant D as prismd
    C->>D: Hello { protocolVersion, clientKind, token? }
    alt version mismatch
        D-->>C: Reject { reason }
    else accepted
        D-->>C: Snapshot { show, session, programmer, outputs, health, fixtureLibrary }
        Note over C,D: fixtureLibrary is a *count* (S44)
        loop while connected
            C->>D: Command
            D-->>C: Delta
            C->>D: Query
            D-->>C: Answer
            D-->>C: Telemetry
        end
    end
```

The `Snapshot` carries **three** documents: the show model, the session state and the programmer. This is what makes a UI restart an ordinary reconnect rather than a special case: the client asks for the world, receives it, and resumes. It is also what makes **D11** work — a view switched from the X-Touch while the UI was closed is simply part of the snapshot the UI receives when it comes back.

> **The programmer is the third document, and it was added in S16.** S13 found the gap: the programmer is a model of its own with a delta of its own, so a client connecting mid-programming would have seen an empty one. It could have been closed by sending a `ProgrammerChanged` immediately after the snapshot. It is closed in the snapshot instead, because §9's *snapshot completeness* row — a fresh client's snapshot equals the state an existing client reached by accumulating deltas — is false for the programmer under the other reading. The world arrives in one message, or that criterion has to be rewritten.

The show and the session travel as **documents** rather than as models, because `ShowPatch` and `SessionPatch` are RFC 6902 operations and an operation is only meaningful against a document root. `prism_core::ShowMirror` and `SessionMirror` apply them to exactly these two values.

> **The snapshot carries `fixtureLibrary` as a number** *(S27, changed in S44)*. It is how many profiles **this desk** can embed — `prism_core::library` — and it is a property of the build rather than of the show: a show that has embedded one of them owns its copy from then on (§5's `EmbedFixtureType`, and the embedding rule on `Command::PatchFixture`).
>
> S27 carried the whole list here, which was right for the four built-in profiles and impossible for the two thousand S44 brought: the Open Fixture Library is 634 fixtures across 2 798 modes, which is several megabytes and would not fit in the 1 MiB frame §3 defines — and a menu of two thousand entries is not a menu. So the list is **searched** (§5.2's `SearchLibrary`) and this field is only what a client needs in order to say *2 157 profiles* beside the box. Zero is an ordinary state: it means no library is installed and the built-in profiles are all that is offered, which the daemon logs on the way up.

### 4.2 Version negotiation

`protocolVersion` is an integer incremented on any breaking change. A mismatch produces an explicit `Reject` with a human-readable reason, surfaced in the UI as "the interface and the engine are different versions". Undefined behaviour from a silent mismatch is unacceptable in software that controls a show.

---

## 5. Commands

Commands express intent. The daemon validates, applies, journals for Oops where applicable, and broadcasts the resulting delta. A command that cannot be applied yields `Reject` and changes nothing.

```typescript
type Command =
  // ---- Show and light ----
  | { t: "SelectFixtures"; ids: FixtureId[]; mode: "Set" | "Add" | "Toggle" }
  | { t: "SetAttribute"; attribute: AttributeType; value: number; relative: boolean }
  | { t: "ApplyPreset"; presetId: PresetId }
  | { t: "ClearProgrammer" }
  | { t: "StoreCue"; sequenceId: SequenceId; cueNumber: string }
  | { t: "ExecutorGo"; executorId: ExecutorId; direction: "Next" | "Prev" }
  | { t: "ExecutorOff"; executorId: ExecutorId }
  | { t: "SetExecutorMaster"; executorId: ExecutorId; level: number }
  | { t: "PatchFixture"; /* … */ }
  | { t: "UnpatchFixture"; id: FixtureId }
  | { t: "RenumberFixture"; id: FixtureId; to: FixtureId }
  | { t: "EmbedFixtureType"; typeId: string }
  | { t: "Oops" } | { t: "Redo" } | { t: "SaveShow" }
  // ---- Session and interface (D11) — issued by console and UI alike ----
  | { t: "SelectView"; viewId: number }
  | { t: "StoreView"; viewId: number; name: string }
  | { t: "RenameView"; viewId: number; name: string }
  | { t: "DeleteView"; viewId: number }
  | { t: "MoveView"; viewId: number; direction: "Prev" | "Next" }
  | { t: "OpenWindow"; window: WindowType; params?: Record<string, unknown> }
  | { t: "CloseWindow"; instanceId: number }
  | { t: "FocusWindow"; instanceId: number }
  | { t: "PlaceWindow"; instanceId: number; x: number; y: number; w: number; h: number }
  | { t: "SetExecutorPage"; page: number }
  | { t: "SelectExecutor"; executorId: ExecutorId }
  | { t: "SetEncoderBank"; group: FeatureGroup }
  | { t: "SetProgrammerPage"; page: number }
  | { t: "SelectProgrammerParam"; direction: "Prev" | "Next" }
  | { t: "CommandLineInput"; text: string };
```

The second group is the concrete form of **D11**. The console and the UI draw on one vocabulary; there is no separate surface command set to keep in sync.

> **Three commands the patch needed** *(S27)*. `PatchFixture` alone can only ever *add* to a rig, so a patch nobody could correct was the state the interface was in until S27. `UnpatchFixture` takes one out, and does **not** cascade into groups, presets or cues — a show outlives the rig it was written on (S11), and `Show::issues` reports what now dangles rather than deleting an operator's stored looks. `RenumberFixture` is one command and not an unpatch plus a patch, because the number is the key the patch is filed under: doing it in two steps leaves the rig without that fixture in between, and leaves it deleted if the second step is refused. `EmbedFixtureType` carries **a key and nothing else**, resolved by the daemon against `prism_core::library` — the same rule `PatchFixture` follows in carrying no channels, since a client that sent a whole `FixtureType` would be authoring show content for the daemon to validate. Without it a brand-new show, which carries no profiles at all, could not be patched from an interface.

> **`PlaceWindow` is twelfth and is not in `ARCHITECTURE_SPEC.md` §4.4** *(S25)*. §4.4 lists what the *console* issues, and an X-Touch opens and closes windows without ever dragging one. But §4.1 puts `x`, `y`, `w` and `h` in the session, so a window moved on one screen has to move on every other one — and a client that kept the geometry to itself would be holding session state locally, which is precisely what **D11** exists to prevent. The gap was found when the canvas was built and there was no honest way to drag a window; the four coordinates are canvas units and are rejected as NaN or infinity in both directions, like every other `f64` in the domain.

> **Three commands a view library needed** *(S35)*. Until S35 a view could be stored and selected and nothing else: no rename, no delete, and no way to put views in the order an operator wants to step through. All three are session commands that `ARCHITECTURE_SPEC.md` §4.4 does not list, for `PlaceWindow`'s reason — §4.4 is what a *console* issues, and an X-Touch cannot type a name — while §4.1 puts the view library in the session, so a client that reordered views locally would be holding session state.
>
> **`MoveView` exchanges the two views' numbers**, and that is a decision rather than an implementation detail. A view library carries its order either in the numbers or in an ordering beside them; the second gives two things that can disagree, and the disagreement an operator would meet is `Channel ◀▶` stepping to a view other than the one drawn next. Since `views` is keyed by number, the bar draws in number order and `SelectView` names a number, making the number *be* the order leaves nothing to keep in step. The price, paid deliberately: after a move `SelectView 3` names a different layout, and an F-key bound to a view number follows the **place** rather than the layout that used to be there — which is how a console's page numbers behave.
>
> **`DeleteView` refuses the last view** (`SessionError::LastView`): `activeViewId` names a view from the first moment and `ShowStore` refuses a file whose active view is not stored, so a session with no views could satisfy neither. Deleting the **active** view is allowed, and what the canvas then shows is the *daemon's*: it selects the neighbour before it, or the one after it when there is none, exactly as `SelectView` would have. A client does not choose a successor, so two screens cannot choose differently.

### 5.1 Latency path

Commands originating at the X-Touch do **not** traverse this protocol on their way to the engine — the surface controller runs inside `prismd` and pushes straight into the engine's SPSC queue. The protocol carries the resulting deltas outward to clients. The measured budget is in `ARCHITECTURE_SPEC.md` §4.3.

### 5.2 Queries — the third shape *(S27)*

A command expresses intent and a delta describes a change that has already
happened. Neither can answer *what would happen if*, and S27 needed exactly
that: **an address conflict has to be shown before it is committed**, not
reported afterwards beside a patch that has already moved.

```typescript
type Query =
  | { t: "PatchConflicts" }
  | { t: "PatchPreview"; id: FixtureId; typeId: string; universe: UniverseId; address: number }
  | { t: "SearchLibrary"; text: string; limit: number };

type Answer =
  | { t: "PatchConflicts"; conflicts: PatchConflict[] }
  | { t: "PatchPreview"; preview: PatchPreview }
  | { t: "LibraryMatches"; matches: LibraryEntry[]; total: number };
```

> **`SearchLibrary` is the variant that made the mechanism necessary** *(S44)*. `PatchPreview` could conceivably have been a client's own arithmetic, wrongly; the fixture library could not be sent at all. The desk knows some two thousand profiles, an answer has to fit in a frame, and a `LibraryEntry` is deliberately not a `FixtureType` — a key, a manufacturer, a name, a mode and a footprint, which is what a menu row shows. The profile itself never leaves the daemon: `Command::EmbedFixtureType` names the one that was chosen by its key, and the daemon copies it into the show. The `limit` a client asks for is **clamped by the daemon**, because a client that asked for two thousand would otherwise get an answer no frame can carry.

Four rules, and the first three are what make it safe to ask one on a desk that
is running a show:

- **A query changes nothing.** There is no refusal shape and no journal entry,
  and `crates/prismd/tests/ui_patch.rs` asserts on a recording that every query
  step broadcast no delta at all and left the patch exactly as the step before
  it did.
- **The answer goes to the client that asked**, addressed by the query's `seq`.
  It is deliberately not a `Delta`: a delta is broadcast, and what one operator
  is typing into a form is nobody else's business (`ARCHITECTURE_SPEC.md` §4.2).
- **There is no `Query::Show`.** The show and the session arrive as documents and
  are kept current by deltas; a question that returned a second copy of state a
  client already mirrors would be a second path to the same fact. Every variant
  answers something **derived** that no client may derive for itself.
- **The alternatives were both worse.** A client that intersected the address
  spans itself would be a second opinion about something `prism_core::conflict`
  already decides — the duplication **D3** exists to prevent. A command that
  patched and then offered an undo would show the operator the conflict by
  *making* it, on a rig that is on stage.

---

## 6. Deltas

A `Delta` describes a change already applied. Clients apply deltas to their mirror without validation — the daemon has already decided.

```typescript
type Delta =
  | { t: "ShowPatch"; ops: JsonPatchOp[] }        // patch, sequences, presets, groups
  | { t: "SessionPatch"; ops: JsonPatchOp[] }     // views, windows, pages, selection
  | { t: "ProgrammerChanged"; state: ProgrammerState }
  | { t: "ExecutorState"; executorId: ExecutorId; isActive: boolean; cueIndex: number | null }
  | { t: "OutputHealth"; outputId: OutputId; health: OutputHealth }
  | { t: "DirtyFlag"; unsavedChanges: boolean }   // drives the X-Touch Save LED
  | { t: "Notice"; level: "Info" | "Warn" | "Error"; message: string };
```

Deltas are ordered per connection. A client that has applied every delta since its snapshot holds state identical to the daemon's.

---

## 7. Telemetry

Telemetry is separate from the control channel because it is high-rate, lossy by nature and must never delay a command.

| Property | Value |
|---|---|
| Rate | 25–30 Hz, independent of the 44 Hz engine tick |
| Content | DMX output levels, programmer values, executor fader positions, meters, tick health |
| Encoding | Binary, fixed layout — not MessagePack maps |
| Loss policy | Coalesced per client; **dropped** when a client cannot keep up |

**The envelope is MessagePack and the content is not.** A telemetry frame travels as `ServerMessage::Telemetry { data }`, where `data` is a MessagePack *byte string* holding a fixed-layout binary frame. Both sentences above hold: the envelope is the one tagged enum §3 asks for, and the payload is not a MessagePack map. The alternative — a channel discriminator in the framing — is what §3 rules out. The cost is about ten bytes per frame *(S16)*.

The layout is a 16-byte header (`"PTLM"`, layout version, one reserved byte, universe count, sequence number, little-endian) followed by one 514-byte section per universe (number, then 512 levels). A frame announcing a layout version this build does not know is **dropped**, not guessed at — which telemetry can afford, being droppable by definition. What is measured beside the levels is S17's to decide; the channel and its room to grow are S16's.

**Clients must not put telemetry into reactive state.** In the React UI it is written to refs and rendered on `<canvas>`. 64 universes × 512 channels at 30 Hz through React state would make the interface unusable — this is the reason the second channel exists at all, and it is a rendering concern, not a change to where authority lives.

---

## 8. Backpressure and failure

| Situation | Behaviour |
|---|---|
| Client is slow | Telemetry is coalesced then dropped. Control messages are **never** dropped; if the control queue for a client fills, that client is disconnected with a `Reject` and must reconnect and re-snapshot |
| Client disconnects | The daemon frees its state and carries on. Nothing about the show changes. Sessions are not owned by clients |
| Daemon is unreachable at startup | The shell spawns one (per **D9**) or reports that the engine is not running, without pretending to be connected |
| Daemon dies | Clients show a clear disconnected state and retry with backoff. On reconnect, a fresh `Snapshot` resynchronises everything |
| Protocol version mismatch | Explicit `Reject`, surfaced in the UI |

A client is never a dependency of the engine. Disconnecting every client leaves DMX output completely unaffected — that is the property **D2** exists to provide, and §9 tests it directly.

**The `Reject` that ends a connection is best-effort; the disconnection is not.** The commonest reason to send one is the first row of that table — the client's control queue filled because it stopped reading — and a client that has stopped reading is exactly the client that cannot be told why. The daemon waits `ServerConfig::goodbye` (one second by default) for the message to reach the socket and then closes regardless, because the alternative is holding a connection it has already given up on for as long as the process lives *(S16)*.

**A message the daemon cannot decode is not a disconnection.** If the framing delivered a whole payload and it was not a message this version understands, the frame boundary is intact: the client is answered with `Reject { reason: Undecodable }` and the connection carries on. Only a fault that loses the frame boundary — an oversized frame — ends it. The distinction matters because the first case is reachable by an honest client of the wrong version, and disconnecting it would hide the reason *(S16)*.

---

## 9. Testing

| Test | Method |
|---|---|
| Framing round trip | Property test over arbitrary messages: encode → decode → equal *(S16: `tests/framing.rs`, through the whole frame, reading the length back out of the header)* |
| Oversized frame | Assert connection closes without a large allocation *(S16: `tests/oversized_frame.rs` counts what the allocator was asked for; the functional assertion passes for the wrong implementation too)* |
| Nesting depth | Assert a payload nested past 128 levels is refused **before** it is deserialised, and that one at 128 is not *(S16)* |
| Handshake | Version match, mismatch, and missing token on a LAN-bound listener *(S16 through a duplex; S18 against a running daemon over the local transport, both directions of mismatch, asserting the refusal names both versions)* |
| Snapshot completeness | Apply a random command sequence, then assert a fresh client's snapshot equals the state reached by an existing client's accumulated deltas *(S18: `crates/prismd/tests/resilience.rs`, all three documents, with the first client **killed** in between — so it is also the reconnect half of the D2 gate)* |
| **IPC resilience (D2 gate)** | Start the daemon with a mock output, connect a client, kill it mid-show, reconnect — assert the output frame sequence has **no gap** across the whole run *(S18: "no gap" is two claims — no silence longer than 250 ms between consecutive frames for a universe, and the look never changing by itself once it is up. Either on its own passes for a daemon that is broken in the other way)* |
| **Surface → UI (D11 gate)** | Drive `SelectView` and `OpenWindow` from a mock MIDI source with **no client connected**; connect afterwards and assert both appear in the snapshot *(S22: `crates/prismd/tests/surface_gate.rs`. The surface controller runs inside `prismd` and applies its commands through the same `ServerHandler::command` door a client uses, so what the snapshot carries is the daemon's own state and not a second one kept for the console — which is why there is nothing here to keep in step)* |
| Backpressure | Attach a deliberately slow client; assert telemetry is dropped, commands are not, and other clients are unaffected *(S16 built the mechanism through a 512-byte socket; S18 measures it against a running daemon — the slow client loses more telemetry frames than it receives, receives every control message in order afterwards, and the client beside it has its commands answered throughout)* |
| **Telemetry layout, from the client's end** | Decode frames a running daemon sent and compare against what `TelemetryFrame::decode` made of the same bytes; assert a layout version this build does not know is **dropped** *(S24: `crates/prismd/tests/ui_telemetry.rs` records the frames and writes down `decode`'s own answers; `ui/src/telemetry/frame.test.ts` holds the browser to them. There is no encoder in the client — a client never sends telemetry (§4), and one would exist only to feed the decoder its own idea of the format)* |
| **Telemetry into a picture** | Assert the frame reaches a canvas and **not** reactive state, and that drawing it fits the budget *(S24: a `<Profiler>` counts zero React commits over 300 frames of 64 universes; `ui/e2e/telemetry.spec.ts` measures decode and paint in Chromium against a real daemon publishing 64 real universes — 0.30 ms median, 1.10 ms p99. A frame this build cannot read costs one picture and nothing else, asserted by sending malformed frames and then a delta that has to arrive)* |
| **Queries change nothing (§5.2)** | Record a script of commands and questions off a running daemon; assert every question was answered, broadcast **no** deltas at all, and left the patch and the profiles exactly as the step before it did *(S27: `crates/prismd/tests/ui_patch.rs`. The same file asserts the harder half — that a preview is what the patch that follows it does: the address a preview called free is the address the fixture ends up at, and the overlap a preview named before the command is the overlap `Show::conflicts` reports afterwards)* |
| Transport parity | Run the full suite over both named pipe / UDS and WebSocket; results must be identical *(S16: one suite, called three times — the third transport is the in-process duplex — plus a scripted session recorded over each and compared as bytes)* |
