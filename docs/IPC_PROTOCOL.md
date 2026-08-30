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
        D-->>C: Snapshot { show, session, programmer, outputs, health,<br/>fixtureLibrary, machine, showFile }
        Note over C,D: fixtureLibrary is a *count* (S44);<br/>machine and showFile are S37's
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

> **`OutputSnapshot` is seven fields since S33**, and the four it gained are what
> a settings window has to draw:
>
> ```typescript
> interface OutputSnapshot {
>   id: OutputId;
>   name: string;
>   health: OutputHealth;
>   output: OutputInstance | null;   // the configured row: kind, parameters,
>                                    // universes, enabled
>   framesSent: number;              // universes put on the wire, not datagrams
>   lastError: string | null;
>   lastErrorAgoMs: number | null;   // an age, not a time
> }
> ```
>
> Before S33 a client could be told an output was red and nothing about **why**,
> or about **what had gone dark with it**. The configured row travels here as
> well as in `Delta::OutputsChanged` (§6) so that the configuration and the
> health arrive together: a client that had to join two lists to draw one line
> would be doing arithmetic to learn something it can be told.
>
> `lastErrorAgoMs` is an **age rather than a time**, and that is the decision in
> the field rather than a convenience. The daemon and the client have no shared
> clock — one may be a browser on another machine — and a `std::time::Instant` is
> not a thing that goes on a wire at all. The age is measured when the snapshot
> is taken, so *four seconds ago* is true when it is read.
>
> All four are optional on the wire (`#[serde(default)]`), because a snapshot is
> a message rather than a file: a client one version behind should meet a missing
> field rather than a decode error.

> **The snapshot carries `fixtureLibrary` as a number** *(S27, changed in S44)*. It is how many profiles **this desk** can embed — `prism_core::library` — and it is a property of the build rather than of the show: a show that has embedded one of them owns its copy from then on (§5's `EmbedFixtureType`, and the embedding rule on `Command::PatchFixture`).
>
> S27 carried the whole list here, which was right for the four built-in profiles and impossible for the two thousand S44 brought: the Open Fixture Library is 634 fixtures across 2 798 modes, which is several megabytes and would not fit in the 1 MiB frame §3 defines — and a menu of two thousand entries is not a menu. So the list is **searched** (§5.2's `SearchLibrary`) and this field is only what a client needs in order to say *2 157 profiles* beside the box. Zero is an ordinary state: it means no library is installed and the built-in profiles are all that is offered, which the daemon logs on the way up.

> **The snapshot carries what this machine is set to, and which show it has
> open** *(S37)*. `machine` is `MachineSettings` and `showFile` is
> `ShowFileInfo`; both arrive with the world rather than being asked for, which
> is `outputs`' reason exactly — both are state the daemon owns, both change only
> when a command changes them, and a settings window that had to ask would draw
> an empty panel for a round trip.
>
> ```typescript
> interface MachineSettings {
>   deskId: string;                 // the sACN CID, as canonical UUID text
>   dataDir: string;                // read, never written — see below
>   local: boolean;
>   websocket: string | null;       // where it is configured
>   websocketOpen: string | null;   // where it actually bound, or null
>   token: string | null;           // §2.1's, shown rather than hidden
>   logLevel: "Debug" | "Info" | "Warn" | "Error" | "Off";
>   universes: number;
>   exitAction: "Hold" | "Blackout";
>   autostart: boolean;
>   fixtureLibrary: string | null;
>   surfaceProfile: string | null;
>   overrides: MachineOverride[];   // which rows a flag is holding this run
> }
>
> interface ShowFileInfo {
>   path: string;
>   recent: string[];               // most recent first, without this one
>   unsavedChanges: boolean;
>   recovery: boolean;              // a recovery copy is standing beside it
>   autosaveSeconds: number;
> }
> ```
>
> **`websocket` and `websocketOpen` are two fields on purpose**, and it is S36's
> `configured` and `open` for a MIDI port one device along: *configured here,
> listening nowhere* is an ordinary state since S37, because a listener that
> cannot bind is a warning and a daemon that starts. Two daemons on one machine
> both want 7373, and refusing to start over it would let one stray process make a
> desk unstartable half an hour before a show.
>
> **The token is carried rather than hidden**, because §2.1 says the daemon
> generates it and the UI displays it: an operator who cannot read it cannot type
> it into the phone in the auditorium, and a token nobody can read is a network
> exposure nobody can use.
>
> **The data directory is read and never written.** The settings themselves are in
> it, so a daemon told to move it would have to be told somewhere else —
> `ARCHITECTURE_SPEC.md` §10.3 has the mechanism that would need and does not have
> one.
>
> **There is no age in `ShowFileInfo`, deliberately.** S33's rule is that a status
> a client reads carries an age rather than a time, because the daemon and a
> browser have no shared clock; the rule one step further along is that a field
> which would have to carry an age cannot travel in a *delta* at all, since the age
> is stale the moment it is sent. What the autosave is really being asked is *is
> there a recovery copy sitting beside my show*, and that is a fact rather than a
> moment.
>
> Both are `#[serde(default)]`, for `OutputSnapshot`'s four S33 fields' reason: a
> snapshot is a message rather than a file, so a client one version behind should
> meet a missing field rather than a decode error. The interface's reader tolerates
> the same absence from the other end.

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
  | { t: "SelectGroup"; groupId: GroupId; mode: "Set" | "Add" | "Toggle" }
  | { t: "StoreCue"; sequenceId: SequenceId | null; cueNumber: string; mode: StoreMode }
  | { t: "StorePreset"; presetId: PresetId; pool: FeatureGroup | null; name: string; color: RgbColor | null; mode: StoreMode }
  | { t: "StoreSequence"; sequenceId: SequenceId; name: string; mode: SequenceStoreMode }
  | { t: "StoreGroup"; groupId: GroupId; name: string; mode: OverwriteMode }
  | { t: "EditCue"; sequenceId: SequenceId | null; cueNumber: string }
  | { t: "Update" }
  | { t: "SetCueProperty"; sequenceId: SequenceId | null; cueNumber: string; property: CueProperty }
  | { t: "SetCueTracking"; sequenceId: SequenceId | null; cueNumber: string; tracking: CueTrackingMode }
  | { t: "Delete"; target: ObjectRef }
  | { t: "Copy"; from: ObjectRef; to: ObjectRef; mode: OverwriteMode }
  | { t: "Move"; from: ObjectRef; to: ObjectRef; mode: OverwriteMode }
  | { t: "Label"; target: ObjectRef; name: string }
  | { t: "Color"; target: ObjectRef; color: RgbColor | null }
  | { t: "Goto"; target: PlaybackTarget; cueNumber: string }
  | { t: "ExecutorOn"; target: PlaybackTarget }
  | { t: "AssignExecutor"; executorId: ExecutorId; sequenceId: SequenceId | null }
  | { t: "ConfigureExecutor"; executorId: ExecutorId; change: ExecutorChange }
  | { t: "ExecutorGo"; target: PlaybackTarget; direction: "Next" | "Prev" }
  | { t: "ExecutorOff"; target: PlaybackTarget }
  | { t: "ExecutorButton"; executorId: ExecutorId; button: ExecutorButtonRef; pressed: boolean }
  | { t: "SetExecutorMaster"; executorId: ExecutorId; level: number }
  | { t: "PatchFixture"; /* … */ }
  | { t: "UnpatchFixture"; id: FixtureId }
  | { t: "RenumberFixture"; id: FixtureId; to: FixtureId }
  | { t: "EmbedFixtureType"; typeId: string }
  | { t: "Oops" } | { t: "Redo" }
  // ---- The show file (S37) — only the first of the five existed before ----
  | { t: "SaveShow" }
  | { t: "SaveShowAs"; path: string }
  | { t: "OpenShow"; path: string }
  | { t: "NewShow"; path: string }
  | { t: "ExportShow"; path: string }
  | { t: "ImportShow"; path: string }
  // ---- Session and interface (D11) — issued by console and UI alike ----
  | { t: "SelectView"; viewId: number }
  | { t: "StoreView"; viewId: number; name: string }
  | { t: "OpenWindow"; window: WindowType; params?: Record<string, unknown> }
  | { t: "CloseWindow"; instanceId: number }
  | { t: "FocusWindow"; instanceId: number }
  | { t: "PlaceWindow"; instanceId: number; x: number; y: number; w: number; h: number }
  | { t: "SetExecutorPage"; page: number }
  | { t: "SelectExecutor"; executorId: ExecutorId }
  | { t: "SelectSequence"; sequenceId: SequenceId }
  | { t: "SetEncoderBank"; group: FeatureGroup }
  | { t: "SetProgrammerPage"; page: number }
  | { t: "SelectProgrammerParam"; direction: "Prev" | "Next" }
  | { t: "CommandLineInput"; text: string }
  // ---- This machine's own rig (S33) — neither the show's nor the session's ----
  | { t: "AddOutput"; output: OutputInstance }
  | { t: "ConfigureOutput"; id: OutputId; change: OutputChange }
  | { t: "RemoveOutput"; id: OutputId }
  | { t: "SetOutputEnabled"; id: OutputId; enabled: boolean }
  // ---- This machine's own control surface (S36) ----
  | { t: "SetSurfacePort"; port: string | null }
  // ---- Everything else about this machine (S37) ----
  | { t: "ConfigureMachine"; change: MachineChange };
```

The second group is the concrete form of **D11**. The console and the UI draw on one vocabulary; there is no separate surface command set to keep in sync.

> **Four commands for the venue's rig, and a third applier** *(S33)*. Until S33
> an output was a `prismd` command-line flag built once at start-up, and every
> network output was handed the show's whole set of universes. Two Art-Net nodes
> therefore both received every universe, an sACN output could not be told to
> carry only 3 and 4, and nothing could be added, removed or re-addressed without
> restarting the daemon.
>
> ```typescript
> interface OutputInstance {
>   id: OutputId;            // the operator's number: `Output 3`
>   name: string;
>   kind: OutputKind;
>   universes: UniverseId[]; // what this interface puts on the wire, in order
>   enabled: boolean;
> }
>
> type OutputKind =
>   | { t: "Mock" }
>   | { t: "OpenDmx"; serial: string | null }
>   | { t: "ArtNet"; nodes: string[]; sync: boolean; ports: ArtNetPort[] }
>   | { t: "Sacn"; receivers: string[]; ttl: number; ports: SacnPort[] };
>
> interface ArtNetPort { universe: UniverseId; net: number; subNet: number; port: number; }
> interface SacnPort   { universe: UniverseId; sacnUniverse: number; priority: number; }
>
> type OutputChange =
>   | { t: "Name"; name: string }
>   | { t: "Kind"; kind: OutputKind }
>   | { t: "Universes"; universes: UniverseId[] };
> ```
>
> **Where the rig lives is the decision, and it is neither the show nor the
> session.** It is `prism_core::MachineConfig`, beside the desk identity: a show
> carried to another hall on a stick must not bring the first hall's cabling with
> it, which is the argument `prism_core::desk` already makes for the sACN CID, and
> a rig is a property of the *building*. The session is not a home for it either,
> because `ARCHITECTURE_SPEC.md` §4.1 persists the session **with the show** and
> it would travel by the same route. So `Command::is_machine_command` is a third
> predicate, `MachineConfig::apply` is a third applier, and both of the other two
> refuse these four by name.
>
> **None of the four is undoable**, and that follows from where they live rather
> than from a separate decision: the Oops journal is the show's, it is cleared
> when a show is loaded, and an undo that re-addressed a node would move light on
> a stage while somebody was driving it — which is §6.1's rule for playback
> actions, arrived at by another road.
>
> `ConfigureOutput` carries **one field**, for `SetCueProperty`'s reason: one
> command carrying the whole row would make a client read it, change one member
> and send the rest back, and two people in a settings window — one re-addressing
> a node, one renaming it — would each undo the other. The number is not among the
> fields, because it is the key the output is filed under: changing it is a
> `RemoveOutput` and an `AddOutput`, said out loud.
>
> **A rename costs the rig nothing.** `prismd`'s supervisor restarts a driver only
> when the kind, the universes, the number or the enabled flag changed, so an
> operator who typed a better name does not watch their rig blink.
>
> **What is validated and what is not.** An Open DMX adapter carries exactly one
> universe because that is what the cable is (`ARCHITECTURE_SPEC.md` §7.1); an
> Art-Net output with no node address would unicast to nobody; an sACN priority
> above 200 is refused rather than clamped, because a desk that quietly lowered a
> number an operator typed would take over a rig it was told not to. Whether the
> *device is there* is deliberately not validated: an unplugged cable is an
> ordinary state of a correct configuration, and a desk that refused the row could
> not be configured before the get-in.
>
> **The desk in the rack is the same kind of fact as the cabling** *(S36)*.
> `SetSurfacePort` names the MIDI port this machine's X-Touch is on, and it is a
> **machine** command for the four above it's reason exactly: a show carried to
> another hall on a stick must not bring a port name with it. So it lands in
> `prism_core::MachineConfig`, both of the other appliers refuse it by name, and
> it is not undoable.
>
> A **name** rather than an index, because an index renumbers itself when
> somebody moves a plug. What a name has to survive is the decoration each
> platform puts round it — a Windows driver-instance prefix, an ALSA client
> address — and that is `prism_midi::selects`' rule rather than the protocol's:
> what travels is exactly what the operator picked out of `Answer::MidiPorts`.
> `null` is *no surface at all*, which is the ordinary state of a laptop.
>
> **A port that is not there is accepted**, for the reason an output row naming a
> node that is switched off is: a show is prepared before the get-in, and a desk
> that refused the name could not be configured until the van arrived. The daemon
> warns, keeps trying, and starts. `Delta::SurfaceChanged` carries the new name
> to every client, so two settings windows cannot disagree about which desk this
> is.
>
> **A daemon told its port on the command line refuses to change it** —
> `--surface <port>` is the surface for that run, the configured port is neither
> read nor written, and the refusal names that flag rather than the output one.
> The same rule `--mock-output` puts the four output commands under, and a
> separate flag from it because the two are separate facts: a daemon may take its
> rig from `machine.json` and its surface from a command line at the same time.

> **Five commands for the show file, and only the first of them existed**
> *(S37)*. `SaveShow` could write the file the daemon already had open, and
> nothing else about a show file could be said from an interface at all: the file
> an operator was working in was whatever `--show` had named at start-up, and
> renaming it, opening another or making a new one meant stopping the daemon.
>
> ```typescript
> | { t: "SaveShowAs"; path: string }
> | { t: "OpenShow"; path: string }
> | { t: "NewShow"; path: string }
> | { t: "ExportShow"; path: string }
> | { t: "ImportShow"; path: string }
> ```
>
> **A path is validated in two places and they are different questions.**
> `prism_core::file::show_path` decides what can be decided from the string — it
> is not empty, it names a file rather than a bare extension, and its extension is
> the format this command actually writes (`ShowError::NotAShowPath`). Whether the
> file is *there*, whether it can be written and whether it is a show at all needs
> a disk, and that is `prismd`'s. The extension check is not cosmetic: a `.prism`
> file is a SQLite database and a `.json` export is text, so a path with the wrong
> one would either fail obscurely or write one format under the other's name.
>
> **A relative path is resolved against the daemon's data directory**, and it is
> resolved *there* rather than by a client: the two do not share a working
> directory, and a browser on another machine has no idea what the daemon's is. An
> absolute path is taken as it stands, which is how a show on a memory stick is
> opened.
>
> Three of the five differ from each other in one careful way each:
>
> - **`OpenShow` never creates.** `ShowStore::open` makes a file that is not
>   there, which is right for a daemon starting up and wrong for an operator who
>   mistyped a name — the show they meant would still be on the disk beside the
>   empty one they got. A path that names nothing is refused.
> - **`NewShow` never replaces.** A *new* show over an existing one would be the
>   most destructive command in this section and would look like the least.
> - **`ImportShow` does not save.** The show is replaced in memory and the Save
>   lamp is left **lit**, because an import somebody did not mean to do must be one
>   they can walk away from.
>
> **None of the five is undoable**, and three of them are the reason the other two
> are not either: `OpenShow`, `NewShow` and `ImportShow` replace the show, which
> empties the Oops journal (`prism_core::journal` — a record is an assertion about
> the show that was open a moment ago), and `SaveShowAs` and `ExportShow` change no
> show state at all.
>
> They are **not** command-line words. `ARCHITECTURE_SPEC.md` §4.5 makes the
> command line *the* interface and names the exceptions — the ones a line cannot
> express — and these are in that company for `OpenWindow`'s reason: S40's grammar
> has no noun for a file, and a path is not a word an operator types into a console
> line.

> **Everything else about this machine, and the command line stops being the
> only way to say it** *(S37)*. S33 made the rig data and S36 made the surface
> data; this is the rest of what `prismd` used to be told on a command line, and
> it lands in `prism_core::MachineConfig` beside them for the same reason — a
> school's caretaker does not edit a shortcut's arguments, and a setting that only
> exists on a command line is one nobody can read back.
>
> ```typescript
> type MachineChange =
>   | { t: "Local"; local: boolean }
>   | { t: "Websocket"; address: string | null }
>   | { t: "Token"; token: string | null }
>   | { t: "NewToken" }
>   | { t: "LogLevel"; level: LogLevel }
>   | { t: "Universes"; universes: number }
>   | { t: "ExitAction"; action: "Hold" | "Blackout" }
>   | { t: "Autostart"; autostart: boolean }
>   | { t: "FixtureLibrary"; path: string | null }
>   | { t: "SurfaceProfile"; path: string | null }
>   | { t: "NewIdentity" }
>   // ---- the control surface's table, control by control (S38) ----
>   | { t: "SurfaceBinding"; control: BoundControl; action: SurfaceAction | null }
>   | { t: "SurfaceLearn"; learning: boolean };
> ```
>
> **One field per command**, which is `OutputChange`'s rule and `CueProperty`'s
> before it: a command carrying the whole of `MachineSettings` would make a client
> read it, change one member and send the rest back, and two operators in two
> settings windows would each undo the other.
>
> **Two of the variants carry no value, and that is the decision.** A token and a
> desk identity both need entropy, which `prism-core` is not allowed to have — and
> which a **client** must not supply: one would be choosing this desk's password
> and the other would be able to give two desks one sACN CID. So the applier
> answers with an effect, `prismd` makes the value, and §2.1's sentence — *the
> daemon generates it and the UI displays it* — is what the protocol does rather
> than what it hopes.
>
> **The listener off loopback is refused without a token**
> (`MachineError::NoTokenForNetwork`), and it is refused by the *daemon* rather
> than discouraged by an interface: a second client could otherwise put an
> unauthenticated lighting console on a school's network, which is the exact thing
> §2.1 exists to prevent. Taking the token away closes the same door from the other
> side — the listener goes back to loopback on the port it was on, because a
> configuration with a public listener and no token is a state this desk will not
> be in.
>
> **A running daemon cannot make all of them**, and `MachineChange::needs_restart`
> is the domain's answer to which: a listener is bound once and a frame layout is
> built once. **A new desk identity is deliberately in that group** — an sACN
> source that changed its CID mid-show would be a *new* source fighting the old one
> until its 2.5 s network-data-loss timeout expires (`prism_core::desk`), so the
> right moment for it is a start. Four are made on the spot: the log level, the
> exit action, the autostart flag and the binding profile, where **naming the file
> again is the reload**, so there is no second command for it.
>
> `Delta::MachineChanged` is what comes back, and it is built by **`prismd`**
> rather than by the applier, unlike every other machine command's: half of what it
> carries is the daemon's knowledge rather than the configuration's — where its
> data directory is, what is actually listening, and which settings this run's
> command line is holding.

> **Layer 3 is edited at the desk, and it is a `MachineChange` rather than a
> command of its own** *(S38)*. `docs/MCU_MAPPING.md` §4's binding table was read
> once, from a file, at start-up; there was no command that read the table in
> force and none that wrote one, so an operator who wanted a key to do something
> else edited JSON beside the daemon and restarted it.
>
> It belongs in this enum **conceptually**: what this building's desk does is one
> of this machine's settings, and `SurfaceProfile` — the *file* the table is read
> from — has been sitting here since S37. It belongs here **structurally** as
> well, and that half has a measurement behind it: `proptest_derive` builds one
> `Command` value tree with a slot for every variant, and S38 measured that slot
> at **560 bytes whatever the variant carries**, so two more commands would have
> cost 1 120 bytes of a budget four sessions have already been surprised by.
> `MachineChange` travels boxed inside `ConfigureMachine`, so variants here cost
> that budget nothing. `prism_domain::wire` has the numbers.
>
> **One control at a time**, which is this enum's own rule and `OutputChange`'s
> before it. A change carrying the whole table would make a client read it, alter
> one row and send the other seventy-two back — and two operators with the editor
> open would each undo the other. One row per change is what makes *two clients,
> one table* a property of the protocol rather than a race nobody has run yet.
>
> `action: null` **unbinds** the control, and that is a state worth being able to
> reach: §4.1 leaves the strip encoder and four of the F-keys deliberately empty.
>
> **The reserved control is refused** — §4.3's SMPTE/Beats, by name and with the
> reason, before anything is written. It is refused in `prism-core` rather than
> discouraged in an interface, for the same reason a network listener without a
> token is: a second client must not be able to reach a state this desk will not
> be in. Clearing it is always allowed, because unbound is the state §4.3 wants
> it in.
>
> **`SurfaceLearn` is the one member of this enum that is written down nowhere.**
> It arms the learn of `docs/MCU_MAPPING.md` §4.4 — the next control an operator
> touches is *named* rather than obeyed — and a desk that restarted into learn
> mode would be a desk whose keys do nothing. It is one shot, so the first
> control disarms it and a client that went away mid-learn cannot leave the desk
> in it.

> **Where the table lives was S38's decision, and it is `MachineConfig`**
> *(S38)*. Three places were possible — a file beside the daemon, the machine
> configuration, or both — and `docs/MCU_MAPPING.md` §4.4 has the argument in
> full. In short: it is the same kind of fact as the rig and the port, so a show
> carried to another hall must not bring the last hall's F-keys; it is written by
> one thread under one lock, which is what makes *two editors, one table*
> structural rather than hopeful; and the shipped `profiles/surface/xtouch.json`
> stays what a test since S22 says it is — the built-in defaults — which an
> editor writing to it would not.
>
> So **a profile file is an import**. Naming one replaces the stored table;
> `Settings::surfaceProfile` is the record of where the table came from rather
> than where it lives. The alternative — the file winning at every start — has
> one unacceptable consequence: an operator who rebound a key at the desk would
> find it back the way it was the next morning.
>
> This splits S22's rule in two, and both halves are it. *A malformed profile
> never blocks anything* now means **the table in force stands**, which for a
> desk that has never been edited is the built-in default and for one that has is
> its own — because throwing an operator's work away over a typo in a file would
> be the one outcome worse than ignoring the file.

> **A flag is still the value for that run, and now the interface is told which**
> *(S37)*. S33's rule for `--mock-output` and S36's for `--surface`, generalised
> over every setting: a flag names the value for that run, the stored setting is
> neither read nor written, and `MachineSettings.overrides` carries the list. A
> settings window draws a held row, disables it and names the flag — because a box
> an operator can type into that does nothing is worse than a box that is not
> there. `prismd::cli::resolve` decides it once, so the value in force and the list
> of held rows cannot disagree.

> **A patched universe no output carries is reported, not refused**
> (`prism_core::ShowIssue::UniverseNotOutput`): a rig is built over an afternoon,
> and an operator whose universe 7 goes nowhere has to read that before the show
> rather than discover it when the light does not come up. The daemon says it on
> the way up and again as a `Delta::Notice` whenever the rig changes.

> **What an executor's controls do, and it is a `Command`** *(S45)*. Until this
> session, what a fader and its four keys did was decided by
> `prism_core::Show::assign_executor`'s defaults the moment a cue list was put on
> the slot, and could not be changed from anywhere at all — not from a window,
> not from the line, not from the desk. That is punch-list entry **B15**.
>
> ```typescript
> type ExecutorChange =
>   | { t: "Fader"; function: ExecutorFaderFunction }
>   | { t: "Encoder"; function: ExecutorEncoderFunction }
>   | { t: "Button"; index: number; function: ExecutorButtonFunction };
> ```
>
> **One control at a time**, which is `OutputChange`'s rule and `CueProperty`'s
> and `MachineChange`'s: a command carrying the whole executor would make a
> client read it, change one member and send the rest back, and two operators
> with the editor open would each undo the other. Which cue list stands on the
> slot stays `AssignExecutor`, because that is a different act.
>
> ### The decision this session exists to make
>
> **A `Command`, not a `MachineChange`** — and both precedents were real, which
> is why it is a decision and not a derivation:
>
> - what an executor **carries** is show content (**S11**: a show survives the
>   hall it was written in, so the cue list on fader 3 travels with it);
> - what a desk's keys **do** is not (**S38**: a show carried to another hall
>   must not bring the last hall's F-keys, so the binding table lives in
>   `MachineConfig` beside the rig and the MIDI port).
>
> An executor is both of those things at once. What settles it is that **an
> executor is not a control of this building**. It is one of the six numbered
> things `ObjectRef` names; it is copied, moved, labelled and coloured by show
> verbs; it is written into the `.prism` file and has been since S1; and a Web
> Remote with no X-Touch anywhere near it still has eight of them with four keys
> each. S38's table binds *hardware* — this strip's third key, this F-key — and
> `ExecutorButtonRef` has kept that apart from *what the third key does* since
> S34: a profile row names a **position**, and the executor decides what a press
> at that position means. S45 makes the second half editable and leaves the first
> where S38 put it.
>
> The consequence is the one that makes the choice testable rather than tidy: an
> executor's assignment **travels with the show**. Carry a show to another hall
> and its Go keys are still Go keys; carry it to a desk with no X-Touch and they
> still are.
>
> **Undoable**, unlike every playback command (`ARCHITECTURE_SPEC.md` §6.1): it
> is an edit to the show rather than something happening on a stage. One image
> per executor row covers it and `AssignExecutor` both.
>
> **The ninth button function carries a line** —
> `ExecutorButtonFunction::CommandLine { line }`, the *custom row* of S45's
> control editor. It is a variant rather than eight more variants somebody has to
> invent, because the owner's answer in S43's third round was that every desk
> needs a different number of them. `SurfaceAction::WriteCommandLine` is the same
> answer for a key on the desk; this is it for a key on an executor, and it
> shares that action's stop-gap — the daemon writes `Session::commandLine`, bumps
> `Session::commandLineRun`, and the client holding the keyboard focus parses it,
> until **S49** moves the parser into the daemon and the arrangement goes for
> both at once.
>
> **No new `SurfaceAction` was added**, and that is the same test being applied:
> a key that wants to say `Assign Executor 1 Fader Master` already has a way to
> say it.

> **A playback is a cue list's, and there is exactly one of them per list**
> *(S45)*. `Delta::PlaybackState` carries a `PlaybackId`, which was a tagged pair
> — an executor or a sequence — from S40 until this session and is now the cue
> list's own number.
>
> Punch-list entry **B18** is why. Put one cue list on two executors and `Go` on
> either started a playback of its own, each with its own cue pointer and its own
> fade: two contributors to the same slots in the merge, and neither of them
> wrong. S40 had already written down the rule its own two kinds obeyed — *two
> players of one cue list would fight over the same slots* — and had only half of
> it.
>
> So `Show::playback_of` resolves an **executor** to the list standing on it and
> a **sequence** to itself, `MergeBody` is built with one playback per cue list,
> and the master level, the rate, whether it is running and which cue it stands
> on moved from `Executor` onto `Sequence`. An executor is a **handle**: which
> list, what its fader does, what its encoder does, what each of its four keys
> does — and nothing about what the list is doing.
>
> Two consequences worth naming:
>
> - **`AssignExecutor` no longer reaches the engine.** The set of playbacks does
>   not mention slots, so putting a list on a fader moves a handle and nothing
>   else — and taking one off does not stop the list, for the same reason a
>   second executor still holding it would not. `Off` is the command that stops a
>   playback.
> - **An executor written before S45 loads and means the same thing.** The four
>   fields are ignored on the way in, and the *level* it carried is read out of
>   the same row and put on the cue list —
>   `prism_core::ShowStore::carry_levels_onto_their_cue_lists`, which is the half
>   a `#[serde(default)]` on the sequence cannot do. When two executors of one
>   list carried different levels, the lowest-numbered one wins: the file records
>   a state this model says cannot exist, and *the first fader* is the one an
>   operator would point at.

> **Four verbs over six things, and one command each** *(S40)*. `Delete`,
> `Copy`, `Move` and `Label` name a **thing** rather than a pool:
>
> ```typescript
> type ObjectRef =
>   | { t: "Sequence"; sequenceId: SequenceId }
>   | { t: "Cue"; sequenceId: SequenceId | null; cueNumber: string }
>   | { t: "Group"; groupId: GroupId }
>   | { t: "Preset"; presetId: PresetId }
>   | { t: "View"; viewId: ViewId }
>   | { t: "Executor"; executorId: ExecutorId };
>
> type OverwriteMode = "Merge" | "Override";
> ```
>
> S40 made the command line **the** interface (`ARCHITECTURE_SPEC.md` §4.5), and
> the grammar it needs is one production: `verb object number [object number]`.
> *Delete sequence 4* and *delete group 4* are the same act on two things, so an
> operator learns one word rather than six — and the parser never has to *choose*
> a command, which would be a client deciding what a line means rather than what
> it says. Written out as twenty-four commands, §5 would have gained twenty-four
> variants whose only difference is which pool they index.
>
> The price is paid here rather than by an operator: **one of the six things is
> session state**, since §4.1 puts the view library in the session. So
> `Command::is_session_command` reads the *target*, `ShowFile::apply` routes on
> it, and `is_undoable` follows — deleting a view is not undoable (§6.1: an Oops
> must not pull a window out from under an operator) and deleting a cue is.
>
> They absorbed four commands that said the same thing in narrower words:
> `DeleteCue` (S28), `DeleteView`, `RenameView` and `MoveView` (S35). `MoveView`
> was **relative** and is now absolute — `Move View 1 View 3` — because the bar
> knows its neighbour's number and writes the line; S35's decision that the
> number *is* the order is unchanged, and a move still exchanges the two views'
> contents while their numbers stay put. `CueProperty` lost its `Number` and its
> `Name` to `Move` and `Label` for the same reason.
>
> **`Color` is the fifth verb and reaches two of the six.** A colour is drawn on
> a scribble strip, and only a cue list has one: `Color Sequence 4 Red` puts it
> there and `Color Executor 1 Red` colours the list on that fader, which is the
> same indirection `Label` has and the same reason for it — the strip is what the
> operator is looking at. The other four are refused with the noun that was
> typed, because a group is a set of fixtures and a view is a window layout and
> neither is ever on a strip. **The colour lives on the sequence**, so a list
> moved to another fader takes it along; `null` is *no colour chosen* rather than
> black, and an uncoloured strip is lit white, since an unlit one cannot be read
> (`docs/MCU_MAPPING.md` §2.3).
>
> What each verb means pool by pool is `prism_core::objects`, and two of them are
> worth naming here because they are decisions rather than deletions:
>
> - **an executor swaps** rather than overwriting. A desk's faders are places,
>   and an operator rearranging them is not throwing half of them away. A view
>   swaps for the same reason.
> - **a move brings the references along**: moving a sequence repoints every
>   executor that played it, and moving a preset rewrites every `CuePart` that
>   linked to it. Both would otherwise be silent — nothing looks different until
>   somebody presses Go, or edits the preset and watches the cue not follow.
>
> **`Delete Executor` empties the slot and leaves the place.** The row leaves the
> show, and executor 1 is still executor 1 on the bar, because the eight strips
> of a page are `page * 8 + slot` arithmetic (**D7**) rather than rows. That is
> also the exact inverse of the `AssignExecutor` that made the row, which is what
> lets an Oops put the grid back rather than leaving a slot behind carrying a
> deleted executor's fader and buttons. (Its *master* has been the cue list's since S45, so a deleted slot never carried one.)

> **A playback is no longer always an executor** *(S40)*. Every playback command
> carried an executor number until S40, so `On Sequence 1` — a cue list nobody
> has put on a fader — had no representation at all:
>
> ```typescript
> type PlaybackTarget =
>   | { t: "Executor"; executorId: ExecutorId }
>   | { t: "Sequence"; sequenceId: SequenceId }
>   | { t: "Selected" };
> ```
>
> A client sends one of these and the daemon resolves it, because all three cases
> are facts a client may not know: an **executor** becomes the cue list standing
> on it and a **sequence** becomes itself — a client that worked the first out
> would race an `AssignExecutor` from a second client (**D3**) — and **Selected**
> becomes whatever `Session::selectedSequence` names, which is session state a
> client filling in would be sending a command whose meaning had already moved.
> The last is what makes a bare `Go+` on the command line mean something.
>
> **Since S45 there is exactly one playback per cue list** and
> `prism_domain::PlaybackId` is that list's own number. S40 had two kinds and a
> rule keeping them apart — *two players of one list would fight over the same
> slots in the merge and neither would be wrong* — and the rule was true of two
> executors as well, which nothing enforced. See §5's S45 note above.
>
> **`Goto` is new at every layer.** There was no `Goto` in this list *and* none
> in `prism_engine::TickCommand`, so `Goto Cue 5` needed a message all the way
> down to the tick. It enters the cue with its own delay and fade rather than
> stepping to it, and it names the cue by **number**: the daemon resolves the
> index, because the tick resolves nothing (`ARCHITECTURE_SPEC.md` §3.1) and a
> client that sent an index would be reading a cue list it may be a delta behind
> on.
>
> **`ExecutorOn`** is the command form of `ExecutorButtonFunction::On`, which
> until S40 could only be reached by pressing a key *of an executor* — so
> `On Sequence 1` had nothing to send. `ExecutorButton` is still the right
> command for a strip and is still not the same thing: that one says *which key
> went down* and lets the executor decide, which is **D3** for playback.

> **Three commands a line needed, and three fields that became optional**
> *(S40)*. `SelectGroup` expands a group **in the daemon**, because which
> fixtures a group holds is show state and a client that expanded it would be
> sending a selection a second client's edit had already made wrong — the same
> rule that keeps channels out of `PatchFixture`. `StoreGroup` is the command
> `prism_core::Show::store_group` had been waiting for since S11: it stores the
> programmer's **selection**, because a group is a list of fixtures rather than a
> look. And `StoreSequence` absorbed `CreateSequence`: `Store Sequence 4` creates
> the cue list when the number is free, because the command line cannot know
> which of the two acts it is (the parser does not read the show — S26).
>
> **A cue list made that way becomes the selected one**, and it is the one place
> a show command writes a session field. `Store Cue 1` names no cue list and
> means `Session::selectedSequence` (§4.1), so a desk that made list 4 and went
> on pointing at list 1 would send the next store into the wrong place — and the
> operator would not find out until they read the sheet. A store into a list that
> **already exists** moves no selection: choosing what to edit is `SelectSequence`'s
> job, and storing into a second list is not saying you want to move there. The
> journal images the field with the sequence, so an Oops takes both back together
> (`ARCHITECTURE_SPEC.md` §6.1).
>
> `StoreCue`, `EditCue` and `SetCueProperty` name their sequence as
> `SequenceId | null`, and `StorePreset` its pool as `FeatureGroup | null`. A
> `null` is not *not supplied*: it means **the one the session has** —
> `Session::selectedSequence` and `Session::encoderBank` — and the daemon
> resolves it in `ShowFile::apply`. A client that read the session and filled the
> number in would be sending a command whose meaning had already moved on a
> second screen. `StorePreset`'s `color` is the same shape one step further: a
> `null` **keeps** the colour that is there, so a relabel through the command line
> cannot throw away something an operator chose with a picker.

> **`PatchFixture` gained one field in S43, and `Command::StorePreset` gained a
> pool it did not have** *(S43)*.
>
> `softwareDimmer: boolean` says whether the desk supplies this fixture's
> intensity when its profile has none. It travels with the rest of the patch form
> rather than as a command of its own, for the reason the name and the address do
> — the form sends what the row **is**, and a second command for one checkbox
> would be a second way for the row and the show to disagree. It is
> `#[serde(default)]` and the default is **true**, which is the safe answer: a
> colour-only fixture without one comes up lit, because a colour rests open
> (`docs/DMX_MERGE.md` §5.1). It says nothing about a fixture whose profile has a
> dimmer of its own.
>
> `StorePreset::pool` is `PresetPool` rather than `FeatureGroup`: seven banks and
> **`Multi`**, which takes every value the programmer holds across the banks. The
> two are separate types on purpose and both conversions are exhaustive, so a
> bank added to `FeatureGroup` is a compile error here rather than a pool that
> silently cannot be stored into. `None` still means `Session::encoderBank`.
>
> Two session fields arrived with them and are documented in
> `ARCHITECTURE_SPEC.md` §4.1: `commandLineRun`, a **counter** that asks the
> focused client to run the line (a stop-gap S49 removes), and `windowPicker`,
> which is session state because an X-Touch key opens the chooser. `ProgrammerState`
> gained `selectedGroups` and `manualSelection`, both `#[serde(default)]`, because
> *why* a fixture is in the selection is the fact a group deselect needs and
> cannot recover from the selection alone.

> **Three commands the patch needed** *(S27)*. `PatchFixture` alone can only ever *add* to a rig, so a patch nobody could correct was the state the interface was in until S27. `UnpatchFixture` takes one out, and does **not** cascade into groups, presets or cues — a show outlives the rig it was written on (S11), and `Show::issues` reports what now dangles rather than deleting an operator's stored looks. `RenumberFixture` is one command and not an unpatch plus a patch, because the number is the key the patch is filed under: doing it in two steps leaves the rig without that fixture in between, and leaves it deleted if the second step is refused. `EmbedFixtureType` carries **a key and nothing else**, resolved by the daemon against `prism_core::library` — the same rule `PatchFixture` follows in carrying no channels, since a client that sent a whole `FixtureType` would be authoring show content for the daemon to validate. Without it a brand-new show, which carries no profiles at all, could not be patched from an interface.

> **Five commands a show needed** *(S28)*. Before them the protocol could store a cue and apply a preset, and nothing else about a show could be written from an interface: there was no way to make a sequence to store into, no way to put one on an executor so it could be fired, no way to correct a cue that had been stored, and no way to make a preset for `ApplyPreset` to apply. So a show could only ever be written by hand, in a file, somewhere else.
>
> *(`CreateSequence` was absorbed into `StoreSequence` in **S40** — see the S40 note above. What it did:)* it makes an **empty** cue list and is refused when the number is taken — a *create* that replaced a running cue list would empty a playback that is on stage. It is deliberately not S39's `StoreSequence`, which is a different act with a mode on it: that one stores the *programmer* into a sequence.
>
> **`SetCueTracking` says what a whole cue does about tracking** *(S48)*. Three
> answers to one question, and `CueTrackingMode` carries which: `Track` — every
> value in the cue carries forward, which is what a stored cue does; `CueOnly` —
> every value is taken back when the list leaves the cue; `Block` — the cue
> asserts **everything**, so nothing above it reaches past it and the list can be
> cut into sections an operator can rehearse from. `docs/DMX_MERGE.md` §2.4 is
> what each one means at the output.
>
> One command rather than three, for `ExecutorButton`'s reason: an operator picks
> one of three, and a grammar with three verbs for one act is three things to
> learn. It is **not** a `CueProperty`, and that is the interesting half: two of
> the modes rewrite every part's `tracking`, and `Block` writes new *parts* —
> which is exactly what `CueProperty` excludes, because what a cue sets comes
> from the programmer and not from a client. This is the exception that proves
> that rule rather than a hole in it: **the values a block writes are the
> daemon's own**, folded out of the cues above by `prism_domain::CueTrack`, and
> the command carries none of them. A client that sent them would be authoring
> the show, which is the thing being prevented.
>
> A **show edit**, therefore undoable, and it rebuilds the merge body because it
> changes what the list puts out. A mode that changes nothing produces **no
> operations at all** — `SetCueProperty`'s rule, and an Oops step an operator
> would otherwise press and watch do nothing.

> `SetCueProperty` carries **one field** (`CueProperty`: number, name, fade in, fade out, delay, trigger). The alternative — one command carrying every editable field — makes a client read the cue, change one member and send the rest back, which is a read-modify-write over state the daemon owns; two operators editing two different columns would then each undo the other. What a cue *sets* is not among the fields, for the reason `PatchFixture` carries no channels: values come from the programmer.
>
> *(`DeleteCue` became `Delete` over an `ObjectRef::Cue` in **S40**; the rule did not move with it.)* It does not renumber what is left. A cue number is what an operator has written on a running order and what a Goto names.
>
> `AssignExecutor` puts a sequence on a slot, or takes one off. An empty slot gains an executor with **the desk's defaults** — the three button functions the protocol can actually press and a master at full — because what a fader and four buttons do is show content, and a client that chose it would be authoring the show. Taking the sequence off keeps everything else the slot has.
>
> `StorePreset` is `StoreCue`'s mirror, with two differences that both come from the type: it carries a name and a colour, so a store with an **empty programmer** onto a preset that exists is an ordinary relabel (and onto one that does not exist it is refused, because an empty preset applies nothing); and which values go in depends on the `pool`, since a colour preset takes the colour values and the bank an attribute is filed under is the *profile's* answer rather than the attribute name's.
>
> **None of the five carried a store mode**, and that was S28's one deliberate omission: `prism_core::Programmer` merged unconditionally, and a client carrying a mode the daemon did not honour would have been describing an outcome that did not happen. What S28 did instead was *say so first* — see `Query::StorePreview` in §5.2. **S39 built the other two modes and put the mode on the commands**, which is the paragraph below.

> **The store modes, and the four commands S39 added** *(S39)*. `StoreMode` has three values and the **operator** chooses one, so it travels in the command and the daemon never guesses — and the outcome no longer depends on which client sent it.
>
> ```typescript
> type StoreMode = "Merge" | "Override" | "Remove";
> type SequenceStoreMode = "Append" | "Override" | "Merge";
> ```
>
> Against a cue: `Merge` writes the programmer's values in and leaves everything else standing; `Override` makes the cue's parts **exactly** the programmer's, so what it does not mention is gone; `Remove` takes the programmer's values **out** and does not use their levels at all. A cue keeps its name, its times and its trigger under all three — a store is about the look, and a cue's name is `SetCueProperty`'s. A `Remove` against a cue that is not there, or one that would remove nothing, is **refused**: a store that writes nothing and removes nothing is one an operator would press twice. A `Remove` that empties a cue leaves an **empty cue**, because deleting one is `DeleteCue`'s job and a store that took a number off a running order as a side effect would be a surprise. `StorePreset` carries the same three, and it has to: §5.2's preview can be asked about a preset in any of them, and an answer describing an outcome no command can produce is exactly what S28 refused to ship.
>
> **`StoreSequence` is a different act** and therefore a different command: it names a cue *list* and not a cue. `Append` adds a cue at **one past the highest whole number** the list has, so pressing it repeatedly gives `1`, `2`, `3`; `Override` makes the sequence *be* this look — one cue, numbered `1`, and the list that was there is gone; `Merge` writes the look into **every** cue, which is the cue-level Merge one level up. A `Merge` into a cue list with no cues is refused rather than quietly appending, because appending would be a different command than the one that was sent. It is deliberately **not** `CreateSequence`, which makes an empty list and stores nothing. There is no preview for it: §5.2's counts are about the values of one cue, and a sequence store is about cues — an honest answer needs a different shape, and the interface that needs one is S40's.
>
> **`EditCue` loads a stored cue back into the programmer, with every `presetRef` kept.** A load that took the values and dropped the links would break every preset link in the cue the next time it was stored, and it would break it *invisibly*: nothing looks different until somebody edits the preset and the cue does not follow. The values arrive as `ProgrammerValueSource::Recalled`, which is what that variant has been for since S1, and the cue's fixtures become the selection so an encoder reaches them.
>
> **`Update` carries nothing at all**, because everything it needs is the desk's: `Session::editingCue` says which cue, and the mode is `Override` by definition — an Update that merged could never take a value *out* of the cue it is updating, which is why an operator loads one. It is refused when nothing is loaded, and it is a **show edit**, so §6.1 makes it undoable however playback-shaped the key on the desk looks. A cue loaded with `EditCue` and updated with nothing changed is byte-identical to the cue that was loaded.
>
> **`SelectSequence` is the sixteenth session command**, and the one of the four §4.4 gained since S12 that a console *can* issue — `Sequence 5` on the command line. `ARCHITECTURE_SPEC.md` §4.1 has why it exists at all.

> **One command presses an executor's button** *(S34)*. `ExecutorButton` carries *which button*, never what it means:
>
> ```typescript
> type ExecutorButtonRef =
>   | { t: "Slot"; index: number }                         // a hardware position
>   | { t: "Function"; function: ExecutorButtonFunction };  // a profile's own row
> ```
>
> `prism_core::Show::apply` resolves it against that executor's `buttonFunctions`, which is **show data**, and it is the only place `isActive` is read to decide what a `Toggle` comes out as. A client that resolved one for itself would race a second client doing the same, and the daemon would be told to do something nobody pressed. `pressed` exists for `Flash`, which is momentary: the release is half the gesture, and every other function ignores it.
>
> Before it, three of the eight `ExecutorButtonFunction` values had commands and five did not. `docs/MCU_MAPPING.md` §4.2.1 records the whole history — S22 found the gap in the binding table, S26 met it again in the executor bar and drew four keys disabled with the reason on them, and this closed all three of §4.1's unresolved rows at once.
>
> **`SetExecutorMaster` did not change and its meaning did.** It is still one command for a fader, and since S34 the daemon routes it through the executor's own `faderFunction`: `Master` moves the master, `Speed` moves the speed master (`docs/DMX_MERGE.md` §4.1), `XFade` drives a manual crossfade and carries no show state at all, and `Empty` does nothing. `ARCHITECTURE_SPEC.md` §6 has said what a fader does is the executor's setting since S1; this is the daemon reading it.

> **`PlaceWindow` is twelfth and is not in `ARCHITECTURE_SPEC.md` §4.4** *(S25)*. §4.4 lists what the *console* issues, and an X-Touch opens and closes windows without ever dragging one. But §4.1 puts `x`, `y`, `w` and `h` in the session, so a window moved on one screen has to move on every other one — and a client that kept the geometry to itself would be holding session state locally, which is precisely what **D11** exists to prevent. The gap was found when the canvas was built and there was no honest way to drag a window; the four coordinates are canvas units and are rejected as NaN or infinity in both directions, like every other `f64` in the domain.

> **Three commands a view library needed** *(S35)*. Until S35 a view could be stored and selected and nothing else: no rename, no delete, and no way to put views in the order an operator wants to step through. All three are session commands that `ARCHITECTURE_SPEC.md` §4.4 does not list, for `PlaceWindow`'s reason — §4.4 is what a *console* issues, and an X-Touch cannot type a name — while §4.1 puts the view library in the session, so a client that reordered views locally would be holding session state.
>
> **`MoveView` exchanges the two views' numbers**, and that is a decision rather than an implementation detail. *(It became `Move` over an `ObjectRef::View` in **S40**, and absolute rather than relative; the decision below is unchanged and the contents swap while the numbers stay.)* A view library carries its order either in the numbers or in an ordering beside them; the second gives two things that can disagree, and the disagreement an operator would meet is `Channel ◀▶` stepping to a view other than the one drawn next. Since `views` is keyed by number, the bar draws in number order and `SelectView` names a number, making the number *be* the order leaves nothing to keep in step. The price, paid deliberately: after a move `SelectView 3` names a different layout, and an F-key bound to a view number follows the **place** rather than the layout that used to be there — which is how a console's page numbers behave.
>
> **`DeleteView` refuses the last view** (`SessionError::LastView`) *(it is `Delete` over an `ObjectRef::View` since **S40**; the refusal is unchanged)*: `activeViewId` names a view from the first moment and `ShowStore` refuses a file whose active view is not stored, so a session with no views could satisfy neither. Deleting the **active** view is allowed, and what the canvas then shows is the *daemon's*: it selects the neighbour before it, or the one after it when there is none, exactly as `SelectView` would have. A client does not choose a successor, so two screens cannot choose differently.

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
  | { t: "SearchLibrary"; text: string; limit: number }
  | { t: "StorePreview"; target: StoreTarget; mode: StoreMode }
  | { t: "MidiPorts" }
  | { t: "DarkUniverses" }
  | { t: "OutputStatus" }
  | { t: "ArtNetNodes" }
  | { t: "SurfaceBindings" }
  | { t: "CueTracking"; sequenceId: SequenceId };

type StoreTarget =
  | { t: "Cue"; sequenceId: SequenceId; cueNumber: string }
  | { t: "Preset"; presetId: PresetId; pool: FeatureGroup };

type Answer =
  | { t: "PatchConflicts"; conflicts: PatchConflict[] }
  | { t: "PatchPreview"; preview: PatchPreview }
  | { t: "LibraryMatches"; matches: LibraryEntry[]; total: number }
  | { t: "StorePreview"; preview: StorePreview }
  | { t: "MidiPorts"; ports: MidiPortInfo[]; configured: string | null;
      open: string | null; status: SurfaceStatus | null }
  | { t: "DarkUniverses"; universes: UniverseId[] }
  | { t: "ArtNetNodes"; nodes: ArtNetNodeInfo[]; listening: boolean; error: string | null;
      counters: ArtNetCounters; remedy: string | null }
  | { t: "SurfaceBindings"; controls: SurfaceControl[]; device: string;
      profile: string | null; revision: number; learning: boolean }
  | { t: "CueTracking"; sequenceId: SequenceId; cues: CueTrackingRow[] };

interface CueTrackingRow {               // S48 — one cue of one list
  number: string;                        // the cue, as an operator typed it
  inherited: TrackedValue[];             // what it holds that it does not name
  blocks: boolean;                       // it asserts everything — derived
}

interface TrackedValue { fixture: FixtureId; attribute: AttributeType; value: number }

interface SurfaceControl {               // S38 — one row of the binding table
  control: BoundControl;                 // which control
  name: string;                          // §4.2's `control` string
  action: SurfaceAction | null;          // what it does now
  permanent: boolean;                    // §4.3: ours in the combined mode?
  reserved: boolean;                     // §4.3: may never be bound
}

interface MidiPortInfo { name: string; input: boolean; output: boolean }

type NodeHealth =                         // S46 — whether a node answers
  | "Answering"                           // a reply inside the timeout
  | "NeverAnswered"                       // polled, and never heard from
  | "Stopped";                            // answered once, and not lately

interface NodeReach {                     // S46 — one node an output sends to
  address: string;                        // as the rig spells it
  health: NodeHealth;
  name: string | null;                    // its short name, once it has given one
  lastReplyAgoMs: number | null;          // an AGE, not a time
}

interface ArtNetCounters {                // S46 — what the discovery has done
  pollsSent: number; pollsFailed: number;
  replies: number; malformed: number;     // read, and dropped rather than believed
  dropped: number; readErrors: number;    // no room in the table; refused reads
}

interface ArtNetNodeInfo {                // S46 — a node that answered ArtPoll
  address: string;                        // where the reply came from
  ip: string;                             // the address the node claims
  shortName: string; longName: string;    // ShortName[18], LongName[64]
  mac: string;                            // six hexadecimal pairs
  firmware: number; style: number;
  status1: number; status2: number;       // carried, not interpreted
  ports: number[];                        // its OUTPUT port addresses, 15-bit
  inputs: number[];                       // its input port addresses
  configured: boolean;                    // the rig already addresses it
  unaddressedPorts: number[];             // it outputs these; this desk sends none
  missingPorts: number[];                 // this desk sends these; it lists none
  replies: number; lastReplyAgoMs: number;
}

interface SurfaceStatus {                 // S37 — what the desk is doing
  health: "Disconnected" | "Connected" | "Live" | "Probing" | "Unresponsive";
  remedy: string | null;                  // the daemon's words, not a client's
  sent: number; superseded: number; touchSuppressed: number;
  resyncs: number; reserved: number; probes: number; reconnects: number;
  profile: string | null; boundControls: number;
}

interface StorePreview {
  accepted: boolean; refusal: string | null;
  exists: boolean;  name: string;        // what is filed there now
  mode: StoreMode;                        // echoed back from the question (S39)
  added: number; replaced: number; kept: number; removed: number;
}
```

> **`SearchLibrary` is the variant that made the mechanism necessary** *(S44)*. `PatchPreview` could conceivably have been a client's own arithmetic, wrongly; the fixture library could not be sent at all. The desk knows some two thousand profiles, an answer has to fit in a frame, and a `LibraryEntry` is deliberately not a `FixtureType` — a key, a manufacturer, a name, a mode and a footprint, which is what a menu row shows. The profile itself never leaves the daemon: `Command::EmbedFixtureType` names the one that was chosen by its key, and the daemon copies it into the show. The `limit` a client asks for is **clamped by the daemon**, because a client that asked for two thousand would otherwise get an answer no frame can carry.

> **`StorePreview` is the variant S28 needed** *(S28)*. The exit criterion was *a store that would overwrite says what it will do **before** it does it, even where the only mode available is Merge* — which is `PatchPreview`'s shape one gesture along, and for the same reason: a store that reported afterwards would have reported it by *doing* it, on a show somebody is about to run.
>
> Four of the fields are the counts, and they account for **every value on both sides** whichever mode was asked about. Writing *S* for what is filed there now and *I* for what the programmer would bring:
>
> | Mode | `added` | `replaced` | `kept` | `removed` |
> |---|---|---|---|---|
> | `Merge` | I∖S | I∩S | S∖I | 0 |
> | `Override` | I∖S | I∩S | 0 | S∖I |
> | `Remove` | 0 | 0 | S∖I | I∩S |
>
> so `kept + replaced + removed` is what is filed there now, and what the store leaves behind is `added + replaced + kept`. The one number an operator is really reading is whichever of `kept` and `removed` is not zero: they are the same values, named by what the chosen mode does to them. `removed` arrived in **S39** and is zero under Merge, which is why S28 could ship without it.
>
> **The mode is asked rather than answered, since S39.** S28 put it on the *answer*, because `prism_core::Programmer` was what decided it and an interface that had spelled `"Merge"` itself would have gone on looking right and been wrong. Now the operator chooses, the choice travels in the question, and the answer **echoes it** — so a bar drawing an answer beside a chooser that has since moved cannot describe the wrong one. A client still renders the word it is given rather than the word it sent.
>
> It takes its refusal from the **same** builders the store runs (`Programmer::cue`, `Programmer::preset`), so a preview and the store after it cannot disagree — the rule `PatchPreview` and `check_patch` already share.

> **`MidiPorts` is the variant S36 needed** *(S36)*. What is plugged into the
> daemon's machine is not state the daemon owns: it changes when a person moves a
> plug, no command causes it, and a client that mirrored it would hold the
> operating system's opinion from whenever it last connected. So a settings
> window asks when it opens, and asks again when the operator presses *rescan* —
> the gesture that exists precisely because plugging a desk in produces no
> message. Enumerating is also a system call, which is the other reason it is not
> a field of the snapshot.
>
> The answer carries the **configuration** as well as the enumeration, and that
> is not a convenience: a list of ports with no mark against the chosen one is a
> list an operator cannot act on, and joining it against `Delta::SurfaceChanged`
> would be arithmetic to learn something the daemon can say. `configured` need
> not be in `ports` — a desk that is switched off is named and absent at the same
> time, which is exactly the state a panel has to draw — and `open` is what is
> actually there, which differs from `configured` in that one case and equals it
> whenever all is well.
>
> **An empty `ports` is an ordinary answer.** A laptop with nothing attached and a
> build with no MIDI backend produce the same one, because from a client's side
> they are the same fact.

> **`CueTracking` is the variant S48 needed** *(S48)*, and it is this section's
> own rule met by the deepest derived thing in the project. What a cue
> **inherits** is folded out of every cue above it: everything an earlier cue
> asserted and nothing since has overwritten, with the cue-only values of the cue
> before it handed back. A `Cue` that carried it would be a stored copy of
> something the cues already say, and a stale one the moment cue 2 is edited;
> writing it into the `.prism` file would be worse still, because a file that
> stored the resolved state is a file that cannot be corrected by editing cue 2.
>
> A client **has** every cue of the list in its mirror, so it could fold them
> itself — and must not, for the reason `PatchPreview` exists. That fold is the
> rule `prism_engine` resolves a `Goto` through: what a cue-only value falls back
> to, what a cue naming one attribute twice means, which order cues play in. Two
> answers to *where am I* is the fault S48 exists to remove, not one to
> reintroduce in TypeScript.
>
> The answer carries **only what is inherited**, because the cue's own values are
> already the client's; and `blocks` beside it, which is the daemon's reading of
> *this cue asserts everything*. The **first cue of a list blocks by
> construction** — there is nothing above it — and that is drawn rather than
> hidden, because it is what makes the top of a list a rehearsable place. A
> sequence that is not there answers with **no rows**: a window asking about a
> list somebody has just deleted is a race and not a mistake, and this section
> has no refusal shape by design.
>
> Asked when a cue sheet opens and again when that list's **cues** move — not
> when the show moves, and not when a playback advances a cue, which is S45's
> finding one window along.

> **`DarkUniverses` is the variant S37 needed** *(S37)*. It is
> `prism_core::ShowIssue::UniverseNotOutput` as a question rather than as the
> `Delta::Notice` S33 says it with, and the two are for different moments: the
> notice is *the rig just changed and here is what that cost*, and a settings
> window needs the same fact **standing** — an operator opening the Outputs panel
> has to see that universe 7 goes nowhere without having changed anything to be
> told.
>
> It is a query rather than a field of the snapshot for §5.2's own rule: it is
> **derived**, from the patch (which is the show's) and the rig (which is the
> machine's), and a client that intersected the two would be a second opinion
> about something `prism_core::dark_universes` already decides. That is exactly
> the trap `PatchPreview` was built to avoid, one panel along.

> **`MidiPorts` grew a fourth field** *(S37)*. `status` is what the attached desk
> is *doing* — the health, the remedy in the daemon's own words, the six feedback
> counters and the reconnection count, and which binding table is in force. It
> belongs in an answer rather than in a delta because it moves continuously:
> `sent` climbs whenever anything on the desk changes, so a delta per change would
> be a broadcast at feedback rates about something only an open settings window is
> looking at.
>
> `null` means **no surface is attached at all**, which is not the same fact as
> one that is attached and `Disconnected`: a laptop with no port configured has
> nothing to report, and a desk that is switched off has a health and a set of
> counters that happen to be zero. A panel draws the two differently, so the
> protocol tells them apart.
>
> The **remedy is carried as words** rather than derived from `health` by a
> client, and that is the one field worth arguing about: the obvious advice for a
> desk that has stopped sending is *reconnect*, and S20 established that
> reconnecting is the one thing that cannot recover it (`docs/MCU_MAPPING.md`
> §2.7). A client that wrote its own sentence would eventually write that one.

> **`ArtNetNodes` is the variant S46 needed** *(S46)*. It answers **what is on
> this network**, and it exists because `Health::Ok` on an Art-Net output has
> meant *the socket accepted the datagram* since S9 — and UDP always accepts it.
> That is punch-list **B6**, and it is not repairable inside the outputs: knowing
> whether anything is listening needs an answer from the far end, which is
> `ArtPoll` and `ArtPollReply` (Art-Net 4 §6).
>
> **Why a question rather than the rig.** A discovered node is neither show nor
> machine. The rig lives in `prism_core::MachineConfig` because it is a decision
> somebody made and a file has to remember; a discovered node is an *observation
> about the network* — it changes while nobody does anything, no command causes
> it, and it is gone at the next start. Writing it into the configuration would
> mean a `machine.json` that changes because somebody switched a node off. That
> is `MidiPorts`' argument exactly, one protocol along.
>
> **Why not a delta.** `OutputStatusInfo`'s reason: a table that moves at the poll
> cadence, broadcast to every client whether or not anybody has the panel open,
> to carry something only that panel draws.
>
> One thing differs from `MidiPorts` and is why this could not simply be
> *enumerate on the asking thread*: there is no call that answers *what is on this
> network*. Discovery is a conversation over time — poll, wait, hear — so the
> daemon holds a table its own receive thread keeps and the question reads it. The
> query still **sends nothing and changes nothing**, which is this section's first
> rule.
>
> **`listening` must be read before `nodes` is.** `false` has two ordinary causes
> — this desk has no Art-Net output, so nothing is listening on its behalf; or the
> socket would not bind, which on a fixed port number is an everyday outcome — and
> `error` tells them apart in the daemon's own words, for `MidiPorts`' remedy's
> reason. An empty list under a socket that never opened says nothing at all about
> the network, and reading it as *no nodes* would be B6's mistake pointed the
> other way.
>
> **`remedy` is what an evening cost.** A node that was connected, reachable and
> answering every poll read `Degraded`, because Windows' inbound firewall rule
> for `prismd` covered the *Private* profile while the lighting network was
> *Public*: the polls went out, the replies were dropped before the process saw
> them, and the desk said nothing more useful than the word *Degraded*. The
> daemon could see that fingerprint exactly — polls sent, and **not one datagram
> back, readable or not** — and that state has one overwhelmingly common cause.
> So it says so, after three unanswered polls, and stops saying it the moment
> anything at all arrives.
>
> It is **words** rather than a flag a client turns into a sentence, which is
> `SurfaceStatus::remedy`'s rule (S37) and for exactly its reason: the obvious
> sentence a client would compose is *check the node*, and the node is the one
> thing that is working when this appears.
>
> **The counters are there because the panel could not answer a question the
> daemon could.** A node that was connected, reachable and answering every poll
> read `Degraded`, and the panel showed an empty list with no way to say whether
> that meant *nobody is out there*, *nobody was asked*, or *something came back
> and could not be read*. Three faults, three remedies, one picture. `pollsSent`
> climbing with `replies` at zero is the first two; `malformed` climbing beside
> it is the third, and the third is a bug in this desk rather than a fault in the
> hall. A client renders them and interprets nothing.
>
> **Both disagreements are the daemon's arithmetic.** `unaddressedPorts` and
> `missingPorts` are the rig intersected with the discovery table, and a client
> that did that intersection itself would be a second opinion about something the
> daemon holds both halves of — `PatchPreview`'s trap, and the one that drifts the
> first time a port-address default changes.

> **`OutputStatus` grew `nodes`, and health grew a third value** *(S46)*. A row's
> `nodes` is one `NodeReach` per configured node of an Art-Net output, in rig
> order, and **empty for every other kind**: an Open DMX cable and an sACN stream
> are told, never asked, so a `NeverAnswered` on one of them would be a state it
> could enter and never leave. It is also empty while nothing is listening, which
> is a different fact from *nothing answers*.
>
> `NodeHealth` is therefore a **second word** rather than three more
> `OutputHealth` variants. What an output *reports* is still `OutputHealth`, and
> the daemon folds the two: an Art-Net output whose driver says `Ok` while a node
> of its is not answering is reported **`Degraded`** — it is sending, and it is not
> sending cleanly, which is what that word has meant since S7. The fold happens in
> the snapshot, in `Delta::OutputHealth` and in this answer, so a delta and an
> answer cannot say different things about one output.
>
> The fold happens **only while the discovery is listening**. A desk that is not
> listening knows nothing about the far end, and reporting `Degraded` out of
> ignorance would be the same mistake in the other direction.
>
> The *stopped* state carries a **time**, and the time is the daemon's:
> `lastReplyAgoMs` is an age in milliseconds, never a timestamp, because the daemon
> and a browser share no clock — S33's rule for `lastErrorAgoMs` one field along.
> Every age in one answer is measured against one `Instant::now()` on the daemon,
> so two rows of one answer cannot disagree about *now*.

> **`SurfaceBindings` is the variant S38 needed** *(S38)*. It answers what a
> desk's keys **do**, and it is a question for this section's own rule: the table
> in force is **derived** — `docs/MCU_MAPPING.md` §4.1's built-in defaults, or a
> profile read into this machine's rows, or the rows an operator has typed since
> — and a client that layered those three for itself would be a second opinion
> about something `prism_surface::Bindings` already decides. That is the trap
> `PatchPreview` was built to avoid, one panel along.
>
> It is not a field of the snapshot for `SurfaceStatus`' reason: seventy-three
> rows are only ever looked at by an open control editor, and a handshake that
> carried them would pay for them on every connection.
>
> Each row carries **two facts the table does not have**, and they are the device
> profile's: whether the control keeps reaching PrismDMX in the combined Xctl+MC
> mode (§4.3), and whether it may be bound at all. A client holds no profile, and
> an interface that guessed would tell an operator that a key is always in reach
> when it is not — which is precisely the mistake §4.3 exists to prevent.
>
> `revision` is **echoed** from the daemon's own counter, exactly as
> `StorePreview` echoes the mode it was asked about: it is what lets an editor
> tell a current answer from one overtaken in flight, and what lets two clients
> say out loud that they are holding the same table.

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
  | { t: "PlaybackState"; playback: PlaybackId; isActive: boolean; cueIndex: number | null }
  | { t: "OutputsChanged"; outputs: OutputInstance[] }   // this machine's rig (S33)
  | { t: "SurfaceChanged"; port: string | null }  // this machine's desk (S36)
  | { t: "SurfaceBindingsChanged"; revision: number }              // its table (S38)
  | { t: "SurfaceLearnChanged"; learning: boolean;
      control: BoundControl | null }                               // and its learn (S38)
  | { t: "MachineChanged"; settings: MachineSettings }  // its settings (S37)
  | { t: "ShowFileChanged"; file: ShowFileInfo }        // its show file (S37)
  | { t: "OutputHealth"; outputId: OutputId; health: OutputHealth }
  | { t: "DirtyFlag"; unsavedChanges: boolean }   // drives the X-Touch Save LED
  | { t: "Notice"; level: "Info" | "Warn" | "Error"; message: string };
```

Deltas are ordered per connection. A client that has applied every delta since its snapshot holds state identical to the daemon's.

> **`PlaybackState` is the tick's, and only the tick's** *(S34)*. `isActive` and `cueIndex` are what `prism_engine::PlaybackReport` published on the last tick, sampled by the daemon at 25 ms and broadcast **only when one of them has changed**. Two things follow.
>
> It **does not arrive with the command that caused it**: an `ExecutorGo` is acknowledged with no delta at all, and the state follows a fraction of a second later. That is deliberate — until S34 the daemon wrote `isActive` on the way past because nothing else could, and with a readback that becomes two authors racing, whose symptom is a strip that lights, goes dark and lights again. `ARCHITECTURE_SPEC.md` §3.1.1 has the rest.
>
> **It was `ExecutorState` until S40**, and the rename is that session's playback change in one line: a cue list on no fader can now play, so what reports is a `PlaybackId` rather than an executor number. S40 made that a tagged pair; **S45 made it one number**, because a playback is a cue list's and there is exactly one of them per list — see §5. The two fields are therefore written onto the **sequence**, and an executor row draws them by reading through to the list standing on it, which is what makes the second executor of one cue list say the same thing as the first (punch-list entry B18).
>
> And it is **silent while a fade runs**. A cue index changes when a cue changes, not when a level does, so this delta does not move at playback rates — which matters because it moves the show *document*, and a client that re-asks a question on every show change (`Query::StorePreview`, §5.2) would otherwise be asking it per frame. **S45 narrowed one such dependency**: the cue viewer's store preview watched the whole `/sequences` subtree, which a cue advance now rewrites, so it watches that list's `cues` node instead — `ui/src/show/looks.ts::cuesDocument`.

> **`OutputsChanged` carries the rig whole** *(S33)*, like `ProgrammerChanged`
> and for the same reason: it is small and sparse, a rig is a handful of rows
> rather than a document, and a client that had to diff a JSON patch to redraw
> five status lights would be doing arithmetic to learn something it can be told.
>
> It is deliberately **not** a `ShowPatch`. The rig belongs to the *machine* and
> not to the show (§5's S33 note), so putting it in the document a client mirrors
> as the show would write a hall's cabling into what that client believes the
> show to be — and a save would be next. `prism_core::ShowMirror` ignores it by
> name for exactly that reason.
>
> **`SurfaceChanged` is the same shape for the other device** *(S36)*: one name,
> or none, sent whole because it *is* whole, and ignored by both mirrors for
> exactly the reason above — which desk is in the rack is the machine's and not
> the show's. What it deliberately does **not** carry is the list of ports that
> exist, which is `Query::MidiPorts`' answer (§5.2): a delta describes a change
> the daemon made, and somebody plugging a desk in is not one.
>
> **`SurfaceBindingsChanged` carries a change token rather than the table**
> *(S38)*. `SurfaceChanged`'s shape for what the desk's keys *do*, and for that
> delta's reason exactly: the table is seventy-three rows that only an open
> editor is looking at, and it is derived, so it is **asked for**
> (`Query::SurfaceBindings`) and this is what says asking again is worth it.
>
> The revision is what makes *two clients, one table* something a test can assert
> rather than something a design hopes for: both editors are told the same
> number, and an answer naming an older one has been overtaken.
>
> **`SurfaceLearnChanged` is broadcast, and that is the decision in it** *(S38)*.
> There is one desk, so there is one learn: two editors must not both believe
> they have armed it, and the operator standing at the console pressing a key has
> no idea which browser asked. It is the argument `ARCHITECTURE_SPEC.md` §4 makes
> for the active view, applied to a mode instead of a layout. The two moments are
> one variant because they are one fact read at two times — arming is
> `{ learning: true, control: null }` and a control being named is
> `{ learning: false, control: … }`, because learn is one shot.

> **`MachineChanged` and `ShowFileChanged` are the same shape for the rest of
> the machine** *(S37)*: whole, because they are a handful of fields rather than a
> document, and ignored by both mirrors because what this desk is set to and which
> file it has open are no more show content than its cabling is.
>
> `MachineChanged` is built by the **daemon** rather than by
> `MachineConfig::apply`, unlike every other machine command's delta, and the
> reason is what it carries: the data directory, what is actually listening and
> which settings this run's command line is holding are the daemon's knowledge and
> not the configuration's.
>
> `ShowFileChanged` repeats the dirty flag that `DirtyFlag` also carries. That is
> deliberate and it is one fact drawn in two places: the flag is what the
> console's Save LED reads and it moves far more often, so it stays a delta of its
> own; this one carries it so a panel that has just been told the file changed does
> not draw a stale lamp for one round trip. A client keeps them level by letting
> whichever arrives last win in the one place each is read.
>
> It says what the rig **is**; `OutputHealth` says what a driver is *doing*. A
> row that has just arrived is `Disconnected` until its driver says otherwise,
> which is the truth rather than an omission.

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
| **A store says what it will do first (§5.2)** | Record a script of stores and questions off a running daemon; assert every question was answered, broadcast **no** deltas, and left the sequences, the presets and the executors exactly as the step before it did — and that the counts a preview answered with are the cue the daemon ended up holding *(S28: `crates/prismd/tests/ui_show.rs`. The same file asserts the session's hardest claim, that **editing a preset moves the values of the cues that reference it**: the recorded cue's blues change and its whites, which the store never mentioned, do not)* |
| **Each store mode does what its name says (§5)** | Assert on the **stored cue** rather than on the command being accepted, for every mode and against a cue that already exists; assert that a cue loaded with `EditCue` and updated unchanged is **byte-identical**, that every `presetRef` survives the round trip, and that the update state clears on a Clear, a delete and another load *(S39: `crates/prism-core/tests/store_modes.rs`, plus the same claims off a running daemon in `crates/prismd/tests/ui_show.rs` — where the preview of each mode is compared against the cue the daemon ended up holding)* |
| **Queries change nothing (§5.2)** | Record a script of commands and questions off a running daemon; assert every question was answered, broadcast **no** deltas at all, and left the patch and the profiles exactly as the step before it did *(S27: `crates/prismd/tests/ui_patch.rs`. The same file asserts the harder half — that a preview is what the patch that follows it does: the address a preview called free is the address the fixture ends up at, and the overlap a preview named before the command is the overlap `Show::conflicts` reports afterwards)* |
| **The output patch (S33)** | Twelve universes across five outputs of three kinds, configured entirely from commands against a running daemon, with **every driver a recording double** (`--mock-devices`) — each is asserted to have been given exactly the universes its row named and no others. Adding, removing and re-addressing one mid-show is asserted on the *captured frame sequence*: the outputs that did not change have no silence over 250 ms across the whole reconfiguration, which is S18's threshold. A driver that loses its device and then panics degrades alone, and the tick misses nothing over it *(S33: `crates/prismd/tests/outputs.rs`)* |
| **A rig is not show content (S33)** | A show saved twice on a desk with a configured rig is read back as **bytes** and must not contain a node address, an adapter serial or an output name; the machine configuration beside it must contain all three, and a second start must find the rig where it left it *(S33: `crates/prism-core/tests/outputs.rs`, `crates/prismd/tests/outputs.rs`)*. The structural half is `desk.rs`'s `outputs_are_not_show_content`, written after `desk_id_is_not_show_content` |
| **The settings, and the show file (S37)** | Every setting written from a command against a running daemon, read back out of `machine.json`, and then read back again by a **second** daemon started over the same data directory. A show saved, saved under a new name and reopened; a new show refused over a file that is there; an open refused for a file that is not; a JSON export read back over the running show with the Save lamp left lit *(S37: `crates/prismd/tests/settings.rs`, and `crates/prism-core/tests/settings.rs` for the half that needs no disk)*. The daemon these run against is the one a **venue** runs — no output flags, `--mock-devices`, the rig built with `AddOutput` — because a daemon whose outputs came off its command line refuses every machine command |
| **A listener that cannot bind (S37)** | Bind a real socket to an address, start a daemon configured for it, and assert the daemon **runs**: `websocket` names it, `websocketOpen` is `null`, and the rig is driven throughout. The rule S36 wrote for a MIDI port that is not there, one device along, and it matters more because the listener is on by default *(S37: `crates/prismd/tests/settings.rs`)* |
| **The settings window (S37)** | The four panels driven through the whole interface against a socket, asserting on what happens **before** the delta: a gesture, the bytes that went out, and the panel not having changed *(S37: `ui/src/settings/settingswindow.test.tsx`)*. In a browser against a real daemon: S33's five-output rig built from nothing but the window with the frame counters climbing, one output re-addressed while the other four keep sending, a show saved-as and reopened, a **second tab** seeing what the first changed, and a panel of twelve rows scrolling inside its window with the document and the canvas both reading zero *(S37: `ui/e2e/settings.spec.ts`)* |
| Transport parity | Run the full suite over both named pipe / UDS and WebSocket; results must be identical *(S16: one suite, called three times — the third transport is the in-process duplex — plus a scripted session recorded over each and compared as bytes)* |
