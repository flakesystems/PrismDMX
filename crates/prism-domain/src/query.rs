//! Questions — `docs/IPC_PROTOCOL.md` §5.2.
//!
//! # Why the protocol grew a third shape
//!
//! A [`crate::Command`] expresses intent and a [`crate::Delta`] describes a
//! change that has already happened. Neither of them can answer *what would
//! happen if*, and S27 needed exactly that: **an address conflict has to be
//! shown before it is committed**, not reported afterwards as a warning beside
//! a patch that has already moved.
//!
//! The two ways of getting there without a question are both worse. A client
//! that worked the overlap out for itself would be a second opinion about
//! something `prism_core::conflict` already decides — the duplication **D3**
//! exists to prevent, and the one that would drift the first time a footprint
//! rule changed. A command that patched and then offered an undo would show the
//! operator the conflict by *making* it, on a rig that is on stage.
//!
//! So a query is a third message: it changes nothing, it is answered to the one
//! client that asked, and the answer is the daemon's own arithmetic. It is
//! deliberately not a `Delta` — a delta is broadcast, and what one operator is
//! typing into a patch form is nobody else's business (`ARCHITECTURE_SPEC.md`
//! §4.2).
//!
//! # A query is not a way to read the show
//!
//! There is no `Query::Show`. The show and the session arrive as documents in
//! the snapshot and are kept current by deltas; a question that returned a
//! second copy of state a client already mirrors would be a second path to the
//! same fact. Every variant here answers something **derived** that no client
//! may derive for itself.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    ArtNetCounters, ArtNetNodeInfo, FixtureId, MidiPortInfo, OutputStatusInfo, PresetId,
    PresetPool, SequenceId, SurfaceControl, SurfaceStatus, UniverseId,
};

/// Two fixtures sharing DMX channels.
///
/// Not an error. Patching a second fixture onto the first is how an operator
/// clones one and it is a technique in daily use, so the engine makes the
/// outcome deterministic — targets are written in merge-slot order, so the
/// **higher fixture number wins** the shared channels (S4) — and the show model
/// reports the overlap rather than refusing it (S11). What an operator needs is
/// a list they can read, and this is one entry of it.
///
/// Ordered by universe and then by the first shared channel, so a list of them
/// reads like a patch sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct PatchConflict {
    /// The universe the overlap is in.
    pub universe: UniverseId,
    /// First shared channel, `1..=512`.
    pub from: u16,
    /// Last shared channel.
    pub to: u16,
    /// The lower of the two fixture numbers.
    pub first: FixtureId,
    /// The higher of the two fixture numbers — the one that **wins** the shared
    /// channels, because the engine writes its targets last (S4).
    pub second: FixtureId,
}

impl core::fmt::Display for PatchConflict {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "fixtures {} and {} share universe {} channels {}-{}; {} wins",
            self.first, self.second, self.universe, self.from, self.to, self.second
        )
    }
}

/// What patching one fixture *would* do, worked out without doing it.
///
/// The answer to `docs/IPC_PROTOCOL.md` §5.2's `PatchPreview`, and the whole of
/// S27's *conflicts are shown before they are committed*. Every field is the
/// daemon's arithmetic over the show it is holding: a client renders it and
/// decides nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct PatchPreview {
    /// Whether a [`crate::Command::PatchFixture`] carrying these fields would be
    /// applied.
    pub accepted: bool,
    /// Why it would not be, in words an operator can read. `None` when it would.
    pub refusal: Option<String>,
    /// How many channels the fixture would occupy, or 0 when the show does not
    /// carry the profile and therefore cannot say.
    pub footprint: u16,
    /// The last channel it would occupy. `None` when it would not fit — which is
    /// also when `accepted` is false for that reason.
    pub last_address: Option<u16>,
    /// The overlaps this patch would create, in patch-sheet order.
    ///
    /// **An overlap is not a refusal**: `accepted` can perfectly well be true
    /// with entries here, and that is the case an operator has to be shown
    /// before they commit rather than told about afterwards.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub conflicts: Vec<PatchConflict>,
}

/// One profile in the desk's library, as a menu shows it.
///
/// Deliberately **not** a [`crate::FixtureType`]: the library is some two
/// thousand profiles (S44), and a client that was sent whole profiles could not
/// hold them and could not receive them either — `docs/IPC_PROTOCOL.md` §3 caps
/// a frame at 1 MiB. So a search answers with these, and
/// `Command::EmbedFixtureType` names the one that was chosen by its key. The
/// daemon is the only thing that ever holds a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    /// The key `EmbedFixtureType` names it by — `manufacturer/fixture/mode` for
    /// a profile out of the Open Fixture Library.
    pub id: String,
    /// Who makes it.
    pub manufacturer: String,
    /// What it is called.
    pub name: String,
    /// Which mode of it, which is what makes two entries of one fixture
    /// different things to patch.
    pub mode: String,
    /// How many channels one of them occupies.
    pub footprint: u16,
}

/// What a store would be filed under — the cue or the preset it would land in.
///
/// Carries the same fields the command does, minus the ones a preview cannot be
/// affected by: `Command::StorePreset`'s name and colour are written whatever is
/// already there, so asking about them would be asking about the client's own
/// text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum StoreTarget {
    /// A cue of a sequence, by the number an operator types.
    Cue {
        /// The sequence it would go into.
        sequence_id: SequenceId,
        /// The cue number, as typed.
        cue_number: String,
    },
    /// A preset of a pool.
    Preset {
        /// The preset number.
        preset_id: PresetId,
        /// The pool, which is **what decides which programmer values are
        /// taken**: a colour preset stores the colour values and nothing else,
        /// and which bank an attribute is on is the *profile's* answer rather
        /// than the attribute name's. [`PresetPool::Multi`] takes every value
        /// there is, so the count this answers with is the whole programmer.
        pool: PresetPool,
    },
}

/// How a store combines with what is already there.
///
/// **Three values since S39, and the operator picks one.** S28 shipped this
/// type with a single variant and the argument in its own documentation: the
/// daemon answered with the mode it would actually use, because
/// `prism_core::Programmer` merged unconditionally and a client carrying a mode
/// the daemon did not honour would be describing an outcome that did not
/// happen. S39 built the other two, so the mode now travels **in the command**
/// as well as in the answer, and the outcome no longer depends on which client
/// sent it.
///
/// A client still must not spell the word itself where it is *describing* a
/// store: [`StorePreview::mode`] is the daemon's echo of the mode that was
/// asked about, and the counts beside it are what that mode would do.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum StoreMode {
    /// What is stored is added to what is there; nothing is taken away.
    ///
    /// The programmer is sparse by specification, so a store carries only what
    /// was touched this time — and overwriting would delete every value in the
    /// cue the operator did not happen to touch, which is data loss no operator
    /// asked for. The default for that reason: it is the mode that cannot lose
    /// anything.
    #[default]
    Merge,
    /// What is there is replaced by what is stored.
    ///
    /// The cue or the preset ends up holding **exactly** the programmer's
    /// values: everything the store does not mention is thrown away. That is
    /// what [`StorePreview::kept`] counts under Merge, and it is what
    /// [`StorePreview::removed`] counts under this one — the same values, named
    /// by what each mode does to them.
    ///
    /// The name and the times of a cue survive it. A store is about the *look*,
    /// and a cue's name is edited through `Command::SetCueProperty`.
    Override,
    /// The programmer's values are **taken out** of what is there.
    ///
    /// The mode that stores nothing: what the programmer holds names the values
    /// to remove, and their levels are not used at all. It is how an operator
    /// takes a fixture back out of a cue without rebuilding the cue — refused
    /// where there is nothing filed under that number, and refused where none of
    /// the programmer's values are in it, because a store that writes nothing
    /// and removes nothing is one an operator would press twice.
    Remove,
}

/// What storing the programmer into a cue or a preset *would* do.
///
/// S28's exit criterion in one type: **a store that would overwrite says what it
/// will do before it does it.** So the four counts are the whole of what a mode
/// means, said as numbers rather than as a warning an operator learns to click
/// past.
///
/// **The four counts account for every value on both sides**, whichever mode was
/// asked about. Writing *S* for what is filed there now and *I* for what the
/// programmer would bring:
///
/// | Mode | `added` | `replaced` | `kept` | `removed` |
/// |---|---|---|---|---|
/// | [`StoreMode::Merge`] | I∖S | I∩S | S∖I | 0 |
/// | [`StoreMode::Override`] | I∖S | I∩S | 0 | S∖I |
/// | [`StoreMode::Remove`] | 0 | 0 | S∖I | I∩S |
///
/// so `kept + replaced + removed` is what is filed there now, and what the store
/// leaves behind is `added + replaced + kept`. The one number an operator is
/// really reading is whichever of `kept` and `removed` is not zero: they are the
/// same values, named by what the mode they chose does to them (S39).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct StorePreview {
    /// Whether the store would be applied.
    pub accepted: bool,
    /// Why it would not be, in words an operator can read. `None` when it would.
    pub refusal: Option<String>,
    /// Whether something is already filed under that number.
    ///
    /// The difference between *create* and *overwrite*, which is the first thing
    /// an operator needs to know and the one a button label can carry.
    pub exists: bool,
    /// What is filed there now, or empty when nothing is.
    pub name: String,
    /// How this store would combine with what is there.
    pub mode: StoreMode,
    /// Values the programmer would add that are not stored yet.
    pub added: u32,
    /// Values already stored that the programmer would write over.
    pub replaced: u32,
    /// Values already stored that this store would **leave alone**.
    ///
    /// The number that makes Merge legible: it is exactly what an Override would
    /// have thrown away — which is [`Self::removed`] under that mode.
    pub kept: u32,
    /// Values already stored that this store would **take away** (S39).
    ///
    /// Zero under [`StoreMode::Merge`], which is why S28 could ship without it.
    /// Under [`StoreMode::Override`] it is everything the programmer does not
    /// mention; under [`StoreMode::Remove`] it is everything it does.
    pub removed: u32,
}

/// Something a client asks that changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum Query {
    /// Every overlapping address pair in the show as it stands.
    ///
    /// What a patch sheet paints red. Asked rather than mirrored because it is
    /// **derived** from the patch and the embedded profiles: a document field
    /// holding it would be a computed value the daemon then had to keep in step
    /// with the two things it is computed from.
    PatchConflicts,
    /// What patching this fixture at this address would do.
    PatchPreview {
        /// The fixture number that would be patched. An existing number is a
        /// repatch, and repatching a fixture does not conflict with itself.
        id: FixtureId,
        /// The profile it would instantiate.
        type_id: String,
        /// The universe it would go into.
        universe: UniverseId,
        /// The start address it would take.
        address: u16,
    },
    /// Profiles in the desk's library matching what has been typed.
    ///
    /// Asked rather than mirrored because the library is **large** (S44): some
    /// two thousand profiles, which is neither a frame nor a menu. An empty
    /// `text` answers with the first `limit` entries, so a client that has not
    /// typed anything has something to show.
    SearchLibrary {
        /// What the operator typed. Words, in any order, matched against the
        /// manufacturer, the name, the mode and the key.
        text: String,
        /// How many to answer with. Clamped by the daemon, so a client cannot
        /// ask for an answer that would not fit in a frame.
        limit: u32,
    },
    /// What storing the programmer into this cue or preset would do.
    ///
    /// Asked rather than worked out because the answer depends on three things a
    /// client holds none of together: what the programmer holds, what is already
    /// filed under that number, and what a store mode **does** — which is
    /// `prism_core::Programmer`'s arithmetic and nobody else's. S28's criterion
    /// is that a store says what it will do before it does it, and a client that
    /// counted the overlap itself would be a second opinion about that.
    ///
    /// **The mode is asked rather than answered, since S39.** Until the other
    /// two modes existed there was only one answer to give, so the daemon gave
    /// it; now the operator chooses, the choice travels in the question, and the
    /// answer says what *that* choice would cost. `Answer::StorePreview` echoes
    /// it back, so a bar drawing an answer beside a chooser that has since moved
    /// cannot describe the wrong one.
    StorePreview {
        /// Where it would go.
        target: StoreTarget,
        /// Which mode to answer about — the one the operator has chosen.
        mode: StoreMode,
    },
    /// The MIDI ports plugged into **this machine**, and which one the desk is
    /// configured for — S36.
    ///
    /// Asked rather than mirrored because it is not state the daemon owns: it
    /// changes when a person moves a plug, no command causes it, and a client
    /// that mirrored it would hold the operating system's opinion from whenever
    /// it last connected. A settings window asks when it opens, and again when
    /// the operator presses *rescan* — which is the gesture that exists
    /// precisely because plugging a desk in produces no message.
    ///
    /// **The answer carries the configuration as well as the enumeration**, and
    /// that is not a convenience: a list of ports with no mark against the
    /// chosen one is a list an operator cannot act on, and joining it against
    /// [`crate::Delta::SurfaceChanged`] would be arithmetic to learn something
    /// the daemon can simply say.
    MidiPorts,
    /// Which patched universes no output carries — S37.
    ///
    /// `prism_core::ShowIssue::UniverseNotOutput` as a question rather than as
    /// the `Delta::Notice` S33 says it with. The notice is right for *the rig
    /// just changed and here is what that cost*; a settings window needs the
    /// same fact **standing** — an operator opening the Outputs panel has to
    /// see that universe 7 goes nowhere without having changed anything to be
    /// told.
    ///
    /// It is a query rather than a field of the snapshot for S27's rule: it is
    /// **derived** — from the patch, which is the show's, and from the rig,
    /// which is the machine's — and a client that intersected the two would be
    /// a second opinion about something `prism_core::dark_universes` already
    /// decides. That is the same trap `PatchPreview` was built to avoid.
    DarkUniverses,
    /// What each output's driver is **doing** — S37.
    ///
    /// See [`crate::OutputStatusInfo`] for why the counter is asked for rather
    /// than mirrored: S33 left this as the one thing a settings window would
    /// need and could not be told, and *asking* is the half it named.
    OutputStatus,
    /// The Art-Net nodes this desk can hear — S46.
    ///
    /// # Why a question and not the rig, and not a delta
    ///
    /// A discovered node is neither show nor machine. The rig is
    /// `prism_core::MachineConfig` because it is a decision somebody made and a
    /// file has to remember; a discovered node is an **observation about the
    /// network**, it changes while nobody does anything, no command causes it,
    /// and it is gone at the next start. Writing it into the configuration would
    /// mean a `machine.json` that changes because a node was switched off.
    ///
    /// That is [`Query::MidiPorts`]' argument exactly, one protocol along, and
    /// the shape is the same: a settings window asks while it is open and stops
    /// asking when it closes. It is **not** a delta for the reason
    /// [`crate::OutputStatusInfo`] is not one — a table that moves at the poll
    /// cadence, broadcast to every client whether or not anyone has the panel
    /// open, to carry something only that panel draws.
    ///
    /// One thing differs from `MidiPorts` and is worth saying, because it is why
    /// this could not simply be *enumerate on the asking thread*: there is no
    /// call that answers *what is on this network*. Discovery is a conversation
    /// over time — poll, wait, hear — so the daemon holds a table that its own
    /// receive thread keeps, and the question reads it. What the query does
    /// **not** do is send anything, which is §5.2's first rule.
    ArtNetNodes,
    /// The binding table **in force**, control by control — S38.
    ///
    /// A question rather than a field of the snapshot, and the reason is this
    /// section's own rule: it is **derived**. What a desk's keys do is the
    /// built-in defaults of `docs/MCU_MAPPING.md` §4.1, or a table read out of a
    /// profile file, or the rows this machine has been told to store — and a
    /// client that layered those three for itself would be a second opinion
    /// about something `prism_surface::Bindings` already decides, which is
    /// exactly the trap `PatchPreview` was built to avoid one panel along.
    ///
    /// It also carries two facts about each control that live on the **device
    /// profile** rather than in the table: whether the control keeps reaching
    /// PrismDMX in the combined Xctl+MC mode (§4.3) and whether it may be bound
    /// at all. See [`crate::SurfaceControl`].
    ///
    /// Asked when the editor opens and asked again whenever
    /// [`crate::Delta::SurfaceBindingsChanged`] says the table moved — which is
    /// `Query::MidiPorts` and `Delta::SurfaceChanged`'s shape one layer along,
    /// and what makes two editors on two screens draw one table.
    SurfaceBindings,
}

/// The daemon's answer to a [`Query`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum Answer {
    /// The overlaps in the show as it stands.
    PatchConflicts {
        /// In patch-sheet order.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        conflicts: Vec<PatchConflict>,
    },
    /// What that patch would do.
    PatchPreview {
        /// The whole of it.
        preview: PatchPreview,
    },
    /// The profiles that matched, best first.
    LibraryMatches {
        /// At most the number asked for.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        matches: Vec<LibraryEntry>,
        /// How many profiles the library holds in total, so a client can say
        /// *50 of 2 084* rather than implying the list is all there is.
        total: u32,
    },
    /// What that store would do.
    StorePreview {
        /// The whole of it.
        preview: StorePreview,
    },
    /// What is plugged in, and what this desk is configured for — S36.
    ///
    /// An **empty** `ports` is an ordinary answer rather than a failure: a
    /// laptop with nothing attached, and a build with no MIDI backend, produce
    /// the same one, because from a client's side they are the same fact. That
    /// is an exit criterion of S36 and it is what CI runs.
    MidiPorts {
        /// Every port the operating system offers, in its own order.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        ports: Vec<MidiPortInfo>,
        /// The port this machine's configuration names, or `None` for no
        /// surface. It need not be in `ports`: a desk that is switched off is
        /// configured and absent at the same time, which is exactly the state a
        /// settings window has to draw.
        configured: Option<String>,
        /// The port that is actually open, or `None`. Different from
        /// `configured` in the one case that matters — the desk is named and is
        /// not there — and equal to it whenever all is well.
        open: Option<String>,
        /// What the attached surface is doing — S37.
        ///
        /// `None` when no surface is attached at all, which is not the same
        /// thing as one that is `Disconnected`: a daemon with no port
        /// configured has nothing to report, and one whose configured desk is
        /// switched off has a health and a set of counters that are all zero.
        /// A panel draws the two differently, so the protocol tells them apart.
        status: Option<SurfaceStatus>,
    },
    /// What each output's driver is doing, in output-number order — S37.
    OutputStatus {
        /// One row per **configured** output, running or not.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        outputs: Vec<OutputStatusInfo>,
    },
    /// The Art-Net nodes this desk has heard from — S46.
    ArtNetNodes {
        /// One per node that has answered since the daemon started, in the
        /// order they were first heard.
        ///
        /// A node that has since gone quiet **stays in the list** with its age
        /// climbing, because *there was a node here and it stopped* is the fact
        /// an installer is chasing, and a row that vanished would look like a
        /// node that had never existed.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(2)")
        )]
        nodes: Vec<ArtNetNodeInfo>,
        /// Whether the receive socket is bound.
        ///
        /// `false` is an ordinary answer with two ordinary causes: this desk has
        /// no Art-Net output configured, so nothing is listening on its behalf;
        /// or the socket would not bind. They are told apart by `error`. **A
        /// panel must draw this before it draws the list**, because an empty
        /// list under a socket that never opened says nothing at all about the
        /// network, and reading it as *no nodes* would be exactly the mistake
        /// B6 was.
        listening: bool,
        /// Why nothing is listening, in the daemon's own words, or `None`.
        ///
        /// Carried as words rather than derived by a client for
        /// `Answer::MidiPorts`' reason: another Art-Net program already holding
        /// port 6454 is the common cause and no client could guess it.
        error: Option<String>,
        /// What the discovery has done and been sent — see [`ArtNetCounters`]
        /// for why a panel is shown them at all.
        counters: ArtNetCounters,
        /// What to try, in the daemon's own words, or `None` when there is
        /// nothing to suggest.
        ///
        /// # Why the desk says this rather than the operator working it out
        ///
        /// It was worked out the hard way once, and it took an evening. A node
        /// that was connected, reachable and answering every poll read
        /// `Degraded`, because Windows' inbound firewall rule for `prismd`
        /// covered the *Private* profile and the lighting network was *Public*.
        /// The polls went out; nothing came back; and nothing in the desk said
        /// anything more useful than the word *Degraded*.
        ///
        /// The daemon can see that state exactly — it has sent polls and has not
        /// received **one datagram, readable or not** — and that fingerprint has
        /// one overwhelmingly common cause. So it says so.
        ///
        /// Carried as **words** rather than derived from the counters by a
        /// client, which is `Answer::MidiPorts`' rule (S37) and for its reason:
        /// the obvious sentence a client would write is *check the node*, and
        /// the node is the one thing here that is working.
        remedy: Option<String>,
    },
    /// The patched universes no output carries, in order — S37.
    DarkUniverses {
        /// Empty is the answer a correctly wired rig gives.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        universes: Vec<UniverseId>,
    },
    /// The binding table in force, one row per control — S38.
    SurfaceBindings {
        /// **Every** control the surface has, bound or not, in the order an
        /// editor draws them (`crate::BoundControl::all`). A list that left the
        /// unbound ones out would be a picture of the desk with the empty keys
        /// missing, and the empty keys are the ones an operator is looking for.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        controls: Vec<SurfaceControl>,
        /// Which surface this table is for, as a person reads it.
        device: String,
        /// The same surface as a **profile file** names it — S43.
        ///
        /// `McuProfile::key`, which is what `Bindings::parse` checks the file's
        /// `device` against. It is carried rather than known by the client for
        /// the reason [`crate::SurfaceControl::name`] is: the alternative is a
        /// second copy of a constant that already exists, and an export written
        /// against a stale copy is a file the daemon then refuses.
        device_key: String,
        /// The `profileVersion` a profile file must carry — S43.
        ///
        /// `prism_surface::PROFILE_VERSION`, carried for `device_key`'s reason:
        /// an exported table is a profile file, and a client that guessed the
        /// version would write one this build cannot read back.
        profile_version: u32,
        /// The profile file this table was last read from, or `None` for one
        /// that has only ever been the built-in defaults and whatever the desk
        /// has been told since.
        ///
        /// A **record of where it came from** rather than where it lives: since
        /// S38 the table in force is this machine's own, and reading a file is
        /// what *replaces* it. A table an operator edited at the desk must not be
        /// silently overwritten by a file at the next start.
        profile: Option<String>,
        /// How many times this table has moved since the daemon started.
        ///
        /// Echoed from the daemon's own counter, exactly as `StorePreview` echoes
        /// the mode it was asked about: it is what lets an editor tell an answer
        /// that is current from one that was overtaken while it was in flight,
        /// and what lets two clients say out loud that they are holding the same
        /// table.
        revision: u32,
        /// Whether learn is armed — the next control touched will be named
        /// rather than obeyed (`crate::Command::SetSurfaceLearn`).
        learning: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::{Answer, PatchConflict, PatchPreview, Query, StoreMode, StorePreview, StoreTarget};
    use crate::{FixtureId, PresetId, PresetPool, SequenceId, SequenceStoreMode, UniverseId};

    fn conflict() -> PatchConflict {
        PatchConflict {
            universe: UniverseId::new(1),
            from: 3,
            to: 4,
            first: FixtureId::new(1),
            second: FixtureId::new(2),
        }
    }

    #[test]
    fn a_conflict_says_which_fixture_wins() {
        assert_eq!(
            conflict().to_string(),
            "fixtures 1 and 2 share universe 1 channels 3-4; 2 wins"
        );
    }

    #[test]
    fn conflicts_sort_into_patch_sheet_order() {
        let later = PatchConflict {
            universe: UniverseId::new(2),
            from: 1,
            ..conflict()
        };
        let further_along = PatchConflict {
            from: 9,
            ..conflict()
        };
        let mut all = vec![later, further_along, conflict()];
        all.sort_unstable();
        assert_eq!(all, vec![conflict(), further_along, later]);
    }

    #[test]
    fn queries_and_answers_are_internally_tagged_with_t() {
        assert_eq!(
            serde_json::to_string(&Query::PatchConflicts).unwrap(),
            r#"{"t":"PatchConflicts"}"#
        );
        assert_eq!(
            serde_json::to_string(&Query::PatchPreview {
                id: FixtureId::new(7),
                type_id: "generic.dimmer".to_owned(),
                universe: UniverseId::new(2),
                address: 11,
            })
            .unwrap(),
            r#"{"t":"PatchPreview","id":7,"typeId":"generic.dimmer","universe":2,"address":11}"#
        );
        assert_eq!(
            serde_json::to_string(&Answer::PatchConflicts {
                conflicts: vec![conflict()],
            })
            .unwrap(),
            r#"{"t":"PatchConflicts","conflicts":[{"universe":1,"from":3,"to":4,"first":1,"second":2}]}"#
        );
    }

    /// An overlap is **not** a refusal, and the type has to be able to say so.
    #[test]
    fn a_preview_can_be_accepted_and_still_carry_conflicts() {
        let preview = PatchPreview {
            accepted: true,
            refusal: None,
            footprint: 4,
            last_address: Some(6),
            conflicts: vec![conflict()],
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains(r#""accepted":true"#), "{json}");
        assert!(json.contains(r#""refusal":null"#), "{json}");
        assert_eq!(
            serde_json::from_str::<PatchPreview>(&json).unwrap(),
            preview
        );
    }

    #[test]
    fn every_query_and_answer_survives_both_wire_formats() {
        let queries = [
            Query::PatchConflicts,
            Query::PatchPreview {
                id: FixtureId::new(1),
                type_id: String::new(),
                universe: UniverseId::new(64),
                address: 512,
            },
        ];
        for query in queries {
            let json = serde_json::to_string(&query).unwrap();
            assert_eq!(serde_json::from_str::<Query>(&json).unwrap(), query);
            let packed = rmp_serde::to_vec_named(&query).unwrap();
            assert_eq!(rmp_serde::from_slice::<Query>(&packed).unwrap(), query);
        }

        let answers = [
            Answer::PatchConflicts {
                conflicts: Vec::new(),
            },
            Answer::PatchPreview {
                preview: PatchPreview {
                    accepted: false,
                    refusal: Some("fixture 1 does not fit".to_owned()),
                    footprint: 0,
                    last_address: None,
                    conflicts: vec![conflict()],
                },
            },
        ];
        for answer in answers {
            let json = serde_json::to_string(&answer).unwrap();
            assert_eq!(serde_json::from_str::<Answer>(&json).unwrap(), answer);
            let packed = rmp_serde::to_vec_named(&answer).unwrap();
            assert_eq!(rmp_serde::from_slice::<Answer>(&packed).unwrap(), answer);
        }
    }

    /// A store target names the cue or the preset, and nothing about the store.
    #[test]
    fn a_store_target_is_where_it_would_go_and_not_what_would_go_there() {
        assert_eq!(
            serde_json::to_string(&StoreTarget::Cue {
                sequence_id: SequenceId::new(5),
                cue_number: "1.5".to_owned(),
            })
            .unwrap(),
            r#"{"t":"Cue","sequenceId":5,"cueNumber":"1.5"}"#
        );
        assert_eq!(
            serde_json::to_string(&StoreTarget::Preset {
                preset_id: PresetId::new(4),
                pool: PresetPool::Color,
            })
            .unwrap(),
            r#"{"t":"Preset","presetId":4,"pool":"Color"}"#
        );
    }

    /// **The three modes are on the wire, and Merge is still the default.**
    ///
    /// This test used to be `merge_is_the_only_store_mode_this_build_has`, and
    /// S28 wrote it so that it would go **red** the day S39 added the other two
    /// — because every reader of `StorePreview::mode` then had two more cases to
    /// answer for, and an interface that had spelled `"Merge"` itself would have
    /// gone on looking right. It has been turned round rather than deleted: the
    /// spellings are what a client's decoder narrows against, and the default is
    /// what a store falls back to when nothing was chosen, which has to be the
    /// mode that cannot lose anything.
    #[test]
    fn the_three_store_modes_are_on_the_wire() {
        assert_eq!(
            serde_json::to_string(&StoreMode::Merge).unwrap(),
            r#""Merge""#
        );
        assert_eq!(
            serde_json::to_string(&StoreMode::Override).unwrap(),
            r#""Override""#
        );
        assert_eq!(
            serde_json::to_string(&StoreMode::Remove).unwrap(),
            r#""Remove""#
        );
        assert_eq!(StoreMode::default(), StoreMode::Merge);
        for mode in [StoreMode::Merge, StoreMode::Override, StoreMode::Remove] {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(serde_json::from_str::<StoreMode>(&json).unwrap(), mode);
        }
    }

    /// The sequence-level modes are a **different** three, and deliberately so.
    #[test]
    fn a_sequence_store_has_its_own_three_modes() {
        for mode in [
            SequenceStoreMode::Append,
            SequenceStoreMode::Override,
            SequenceStoreMode::Merge,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(
                serde_json::from_str::<SequenceStoreMode>(&json).unwrap(),
                mode
            );
        }
        assert_eq!(
            serde_json::to_string(&SequenceStoreMode::Append).unwrap(),
            r#""Append""#
        );
        assert_eq!(SequenceStoreMode::default(), SequenceStoreMode::Append);
    }

    /// A preview says what a store would do, and *would overwrite* is not the
    /// same fact as *would be refused*.
    #[test]
    fn a_store_preview_can_overwrite_and_still_be_accepted() {
        let preview = StorePreview {
            accepted: true,
            refusal: None,
            exists: true,
            name: "Warm wash".to_owned(),
            mode: StoreMode::Merge,
            added: 2,
            replaced: 1,
            kept: 7,
            removed: 0,
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains(r#""exists":true"#), "{json}");
        assert!(json.contains(r#""kept":7"#), "{json}");
        assert!(json.contains(r#""removed":0"#), "{json}");
        assert_eq!(
            serde_json::from_str::<StorePreview>(&json).unwrap(),
            preview
        );

        let packed = rmp_serde::to_vec_named(&Answer::StorePreview {
            preview: preview.clone(),
        })
        .unwrap();
        assert_eq!(
            rmp_serde::from_slice::<Answer>(&packed).unwrap(),
            Answer::StorePreview { preview }
        );
    }

    /// The question travels in the same envelope as the other three, and since
    /// S39 it **carries the mode** the operator chose.
    #[test]
    fn a_store_preview_is_asked_like_every_other_question() {
        let query = Query::StorePreview {
            target: StoreTarget::Cue {
                sequence_id: SequenceId::new(1),
                cue_number: "2".to_owned(),
            },
            mode: StoreMode::Override,
        };
        assert_eq!(
            serde_json::to_string(&query).unwrap(),
            r#"{"t":"StorePreview","target":{"t":"Cue","sequenceId":1,"cueNumber":"2"},"mode":"Override"}"#
        );
        let packed = rmp_serde::to_vec_named(&query).unwrap();
        assert_eq!(rmp_serde::from_slice::<Query>(&packed).unwrap(), query);
    }

    /// **A preview of a Remove says what it takes away rather than what it
    /// writes**, and the type has to be able to carry that.
    #[test]
    fn a_removing_store_writes_nothing_and_says_what_goes() {
        let preview = StorePreview {
            accepted: true,
            refusal: None,
            exists: true,
            name: "Opening".to_owned(),
            mode: StoreMode::Remove,
            added: 0,
            replaced: 0,
            kept: 2,
            removed: 3,
        };
        let packed = rmp_serde::to_vec_named(&Answer::StorePreview {
            preview: preview.clone(),
        })
        .unwrap();
        assert_eq!(
            rmp_serde::from_slice::<Answer>(&packed).unwrap(),
            Answer::StorePreview { preview }
        );
    }
}
