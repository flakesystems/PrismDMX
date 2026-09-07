//! What belongs to *this desk* rather than to the show.
//!
//! # Where the sACN CID lives, and why it is not in the show file
//!
//! E1.31 receivers track sources by CID. S10 established the two failure modes
//! and refused to guess between them: a desk that invents a fresh CID at every
//! start is a *new source* every time, and the old one goes on holding the
//! universe until its 2.5 s network-data-loss timeout expires — two and a half
//! seconds of two equal-priority sources fighting. Two desks *sharing* a CID is
//! worse: to a receiver they are one source that contradicts itself, and no
//! amount of priority sorts it out.
//!
//! Both of those follow from where the number is stored, so the decision is
//! this one:
//!
//! **The CID belongs to the machine, not to the show.** It lives in
//! [`MachineConfig`], which is written beside the daemon's own settings and
//! never inside a `.prism` file. A show copied to a second machine — on a stick,
//! from a backup, as the school's template for next term — arrives with no
//! identity in it and picks up the identity of the desk that opens it, which is
//! the only behaviour that cannot produce two senders sharing a CID.
//!
//! The alternative, storing it with the show, fails exactly there: copying a
//! show is the ordinary way a second machine comes to exist, and it would carry
//! the identity of the first with it. Nothing in a show file can tell "this is
//! the same desk" from "this is a copy".
//!
//! Three rules follow, and they bind later sessions:
//!
//! - **S15 must not write the desk identity into the show, and must not
//!   regenerate it on save.** A save is a show operation; the identity is not
//!   show content. [`Show`](crate::Show) has no field for it, which is the
//!   structural half of that promise, and `desk_id_is_not_show_content` is the
//!   asserted half.
//! - **S17 generates it once**, on first start, when the machine configuration
//!   does not exist yet, and writes it before opening any output. This crate
//!   deliberately cannot: a UUID needs an entropy source, and `prism-core` is
//!   platform-neutral and dependency-free by rule. [`DeskId`] is therefore a
//!   value that is parsed, held and formatted here, and generated one layer up.
//! - **[`DeskId::NIL`] is not an identity.** `prism-protocols` already refuses
//!   to connect an sACN output holding the nil CID, and
//!   [`MachineConfig::is_configured`] is the same question asked one layer up,
//!   so a daemon can say "this desk has no identity yet" rather than
//!   transmitting under the value every unconfigured desk would share.

use core::fmt;
use core::str::FromStr;

use prism_domain::{
    Command, ExitAction, LogLevel, MachineChange, OutputId, OutputInstance, SurfaceBinding,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::command::Applied;
use crate::outputs::MachineError;

/// The UUID that identifies this desk on the network.
///
/// The same sixteen bytes `prism_protocols::Cid` puts in an E1.31 root layer.
/// Held here as a value rather than as a string so that "is this an identity at
/// all" is a question with an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DeskId([u8; 16]);

impl DeskId {
    /// The all-zero identity: not an identity at all, and the default.
    pub const NIL: Self = Self([0; 16]);

    /// A desk identity from its sixteen bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The sixteen bytes, in the order they travel in an E1.31 root layer.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }

    /// Whether this is the nil identity, i.e. no identity at all.
    #[must_use]
    pub const fn is_nil(self) -> bool {
        u128::from_be_bytes(self.0) == 0
    }

    /// Parses the canonical UUID text, with or without its hyphens and in
    /// either case.
    ///
    /// Returns `None` for anything that is not exactly 32 hexadecimal digits,
    /// because an identity silently read as something else is a desk that
    /// claims to be a different desk.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut digits = text.chars().filter(|character| *character != '-');
        let mut bytes = [0u8; 16];
        for byte in &mut bytes {
            let high = digits.next().and_then(|digit| digit.to_digit(16))?;
            let low = digits.next().and_then(|digit| digit.to_digit(16))?;
            *byte = u8::try_from((high << 4) | low).ok()?;
        }
        if digits.next().is_some() {
            return None;
        }
        Some(Self(bytes))
    }
}

impl fmt::Display for DeskId {
    /// The canonical 8-4-4-4-12 form, lower case.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                write!(f, "-")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Returned when text is offered as a [`DeskId`] and is not a UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidDeskId(pub String);

impl fmt::Display for InvalidDeskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a UUID", self.0)
    }
}

impl core::error::Error for InvalidDeskId {}

impl FromStr for DeskId {
    type Err = InvalidDeskId;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| InvalidDeskId(text.to_owned()))
    }
}

impl Serialize for DeskId {
    /// As canonical UUID text: a machine configuration is a file a technician
    /// reads and occasionally edits, and sixteen numbers would not be.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DeskId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| serde::de::Error::custom(InvalidDeskId(text)))
    }
}

/// Settings that belong to this machine and travel with it, not with the show.
///
/// Two things today, and the second is the one this type was predicted for: S11
/// wrote *"the question it answers — what is this desk, as opposed to what is it
/// playing? — comes up again for output configuration and for the DMX adapter on
/// this machine, and those must not end up in the show file either"*. S33 is
/// that session, and `crates/prism-core/src/outputs.rs` has the argument in full.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineConfig {
    /// The sACN CID of this desk. See the module documentation.
    desk_id: DeskId,
    /// This building's rig: which universes leave by which cable — S33.
    ///
    /// Kept in output-number order, because a settings panel draws it in that
    /// order and a delta that reordered the rows would move them under an
    /// operator's hand.
    ///
    /// `#[serde(default)]` so that a machine configuration written before S33 —
    /// which is every one written so far — still opens, and opens with no rig
    /// rather than with an error.
    #[serde(default)]
    outputs: Vec<OutputInstance>,
    /// The MIDI port this building's control surface is on — S36.
    ///
    /// A **name**, because a port index renumbers itself when somebody moves a
    /// plug and a name does not; `prism_midi::selects` is the rule that makes
    /// one survive the decoration a platform puts round it. `None` is *no
    /// surface*, and it is the ordinary state of a laptop.
    ///
    /// Here for the rig's reason exactly: the desk in the rack belongs to the
    /// **building**. A show carried to another hall on a stick must not bring a
    /// port name with it, and S33 said so in as many words on its way out — *a
    /// later session that wants anything else about this building puts it
    /// here*.
    ///
    /// `#[serde(default)]` for S33's reason too: every machine configuration
    /// written so far has no such field, and one of them must open with no
    /// surface rather than with an error.
    #[serde(default)]
    surface_port: Option<String>,
    /// Everything `prismd` used to be told on a command line — S37.
    ///
    /// The third thing S33 predicted and the last of them: *a later session that
    /// wants anything else about this building puts it here*. What is in it is
    /// [`Settings`]; why it is here rather than in the show is the argument this
    /// module has been making since S11.
    ///
    /// `#[serde(default)]` for the reason the two fields above it carry, and it
    /// matters more here than it did there: a desk that had been running since
    /// S11 would otherwise stop opening its own configuration on the day it was
    /// updated.
    #[serde(default)]
    settings: Settings,
    /// What this building's control surface's keys **do** — S38.
    ///
    /// Layer 3 of `docs/MCU_MAPPING.md` §4, stored here for the port's reason
    /// one step along: the desk in the rack belongs to the building, and so does
    /// what somebody has made its keys do. A show carried to another hall on a
    /// stick must not arrive with the last hall's F-keys on it.
    ///
    /// `None` is *this desk has never been told* — the built-in defaults, or
    /// whatever profile file the settings name, are what is in force. It is not
    /// the same as `Some(vec![])`, which is a table an operator has deliberately
    /// emptied, and a desk whose keys do nothing is a legitimate thing to ask
    /// for.
    ///
    /// **A file is an import rather than a live source**, and that is S38's
    /// decision. Naming a profile reads it *into* this field; from then on this
    /// is the table and `Settings::surface_profile` is only the record of where
    /// it came from. The alternative — the file winning at every start — would
    /// mean an operator who rebound a key at the desk found it back the way it
    /// was the next morning, which is the one outcome an editor may not have.
    ///
    /// `#[serde(default)]` for the three fields above it's reason: every
    /// machine configuration written before S38 has no such key, and one of them
    /// must open with no table rather than with an error.
    #[serde(default)]
    surface_bindings: Option<Vec<SurfaceBinding>>,
    /// The `.prism` file this desk had open, and the ones before it — S37.
    ///
    /// Written when a show is opened, made or saved under a new name, so a desk
    /// starts where it was left. Kept beside the settings rather than in them
    /// because it is not something an operator *sets*: it is where they were.
    #[serde(default)]
    shows: RecentShows,
}

/// What `prismd` used to be told on its command line — S37.
///
/// Held as a struct of its own inside [`MachineConfig`] so that a settings file
/// reads as three groups rather than as fifteen loose keys, and so that
/// [`MachineConfig::apply`] has one place to write. Every field is
/// `#[serde(default)]` by virtue of the whole struct being one, which is what
/// lets a later session add a sixteenth without any existing desk noticing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Whether the named pipe / Unix domain socket is opened. Default `true`.
    pub local: bool,
    /// Where the WebSocket listener binds, or `None` for not at all.
    ///
    /// **Default `Some(127.0.0.1:7373)`, and that is S37's decision.** Until
    /// then the listener was opt-in, which meant a browser could not reach a
    /// desk that nobody had passed a flag to — and the Web Remote (S31), the
    /// end-to-end suite and the settings window this field is part of all speak
    /// WebSocket. Loopback is what makes it safe to have on: `docs/IPC_PROTOCOL.md`
    /// §2.1's rule is about reaching *off* the machine, and that still needs
    /// both an address and a token.
    #[serde(with = "prism_domain::socket::option")]
    pub websocket: Option<std::net::SocketAddr>,
    /// The §2.1 token a listener off loopback requires.
    pub token: Option<String>,
    /// How much the daemon logs.
    pub log_level: LogLevel,
    /// How many universes the frame layout carries, `1..=64`.
    pub universes: u32,
    /// What the stage does when the daemon is stopped on purpose.
    pub exit_action: ExitAction,
    /// Whether the desk starts with the machine — `ARCHITECTURE_SPEC.md` §10.3.
    ///
    /// Stored here and acted on by the shell (S29), which is the one process
    /// that can write a `HKCU\…\Run` entry or a user unit. A daemon reads it
    /// only to report it.
    pub autostart: bool,
    /// Where the installed fixture library is, or `None` to look beside the
    /// executable.
    pub fixture_library: Option<String>,
    /// Which X-Touch binding profile is in force, or `None` for the built-in
    /// table.
    pub surface_profile: Option<String>,
}

impl Default for Settings {
    /// What a desk that has never been configured runs as.
    ///
    /// Two of these are not zero values and both are decisions: the local
    /// transport is **on**, because it is the one the desktop shell uses and it
    /// touches no network at all; and the WebSocket listener is **on, on
    /// loopback**, which is S37's change and is argued at
    /// [`Settings::websocket`].
    fn default() -> Self {
        Self {
            local: true,
            websocket: Some(std::net::SocketAddr::from((
                std::net::Ipv4Addr::LOCALHOST,
                DEFAULT_WEBSOCKET_PORT,
            ))),
            token: None,
            log_level: LogLevel::default(),
            universes: DEFAULT_UNIVERSES,
            exit_action: ExitAction::default(),
            autostart: false,
            fixture_library: None,
            surface_profile: None,
        }
    }
}

/// The port the WebSocket listener binds unless it is told otherwise.
pub const DEFAULT_WEBSOCKET_PORT: u16 = 7373;

/// Universes the frame layout carries unless it is told otherwise — the desk's
/// whole range.
pub const DEFAULT_UNIVERSES: u32 = 64;

/// How many shows are remembered.
///
/// A list an operator reads rather than an archive: eight is two more than fits
/// on a menu without scrolling, which is the number a panel wants.
pub const RECENT_SHOWS: usize = 8;

/// The `.prism` files this desk has had open, most recent first — S37.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecentShows {
    /// Every show, most recent first, the current one included.
    ///
    /// One list rather than *the current one* and *the rest*, because the two
    /// would have to be kept in step and the interesting question — *which shows
    /// has this desk had open* — is answered by the list either way.
    pub paths: Vec<String>,
}

impl RecentShows {
    /// Puts `path` at the front, removing it from wherever it was.
    ///
    /// Compared as text rather than as a canonicalised path, deliberately: a
    /// canonical form needs a file system, this crate has none, and two spellings
    /// of one file in a menu is a smaller fault than a menu that needs a disk to
    /// be drawn.
    pub fn remember(&mut self, path: &str) {
        self.paths.retain(|held| held != path);
        self.paths.insert(0, path.to_owned());
        self.paths.truncate(RECENT_SHOWS);
    }

    /// The shows other than `current`, most recent first.
    #[must_use]
    pub fn without(&self, current: &str) -> Vec<String> {
        self.paths
            .iter()
            .filter(|held| held.as_str() != current)
            .cloned()
            .collect()
    }
}

impl MachineConfig {
    /// A configuration for a desk that has an identity and no rig yet.
    #[must_use]
    pub fn new(desk_id: DeskId) -> Self {
        Self {
            desk_id,
            outputs: Vec::new(),
            surface_port: None,
            settings: Settings::default(),
            surface_bindings: None,
            shows: RecentShows::default(),
        }
    }

    /// A configuration for a desk whose rig is already decided.
    ///
    /// Every row is validated on the way in and one that is not usable is left
    /// out — the caller has already reported it (`prismd::daemon::rig_for`), and
    /// a row nothing can open must not reach a driver.
    ///
    /// It exists for the one case the four commands do not cover: a daemon whose
    /// outputs were named on its command line, which holds a rig it did not read
    /// from a file and will not write back to one.
    #[must_use]
    pub fn with_outputs(
        desk_id: DeskId,
        outputs: impl IntoIterator<Item = OutputInstance>,
    ) -> Self {
        let mut config = Self::new(desk_id);
        for output in outputs {
            if crate::outputs::validate(&output).is_ok() {
                config.insert_output(output);
            }
        }
        config
    }

    /// This desk's identity.
    #[must_use]
    pub const fn desk_id(&self) -> DeskId {
        self.desk_id
    }

    /// This building's rig, in output-number order.
    #[must_use]
    pub fn outputs(&self) -> &[OutputInstance] {
        &self.outputs
    }

    /// The MIDI port this building's control surface is on, or `None` — S36.
    #[must_use]
    pub fn surface_port(&self) -> Option<&str> {
        self.surface_port.as_deref()
    }

    /// What this machine is set to — S37.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The shows this desk has had open, most recent first — S37.
    #[must_use]
    pub const fn shows(&self) -> &RecentShows {
        &self.shows
    }

    /// Records that a show was opened, made or saved under a new name — S37.
    ///
    /// Not a command and not on [`Self::apply`]'s path: which file is open is
    /// the *daemon's* answer, because only it knows whether the path it was
    /// given actually opened.
    pub fn remember_show(&mut self, path: &str) {
        self.shows.remember(path);
    }

    /// Gives this desk an identity — S37's `MachineChange::NewIdentity`,
    /// carried out one layer up.
    ///
    /// `pub` rather than `pub(crate)`, unlike `set_surface_port`, and
    /// the difference is where the value comes from: a port name travels in a
    /// command and this does not exist until `prismd` has made it. The rule that
    /// nothing reaches the configuration without having been a command still
    /// holds — the command is `ConfigureMachine`, and this is the second half of
    /// carrying it out.
    pub fn set_desk_id(&mut self, desk_id: DeskId) {
        self.desk_id = desk_id;
    }

    /// The §2.1 token, once `prismd` has made one — S37's
    /// `MachineChange::NewToken`.
    ///
    /// [`Self::set_desk_id`]'s shape and its reason: a token needs entropy.
    pub fn set_token(&mut self, token: &str) {
        self.settings.token = Some(token.to_owned());
    }

    /// Names the surface's port, or takes the name away.
    ///
    /// `pub(crate)` for [`Self::insert_output`]'s reason: [`Self::apply`] is the
    /// only door, so nothing reaches this without having been a command.
    /// **Blank is `None`**, because a settings window that cleared its text box
    /// means *no surface* and a configuration holding `""` would be a name
    /// nothing can ever match.
    pub(crate) fn set_surface_port(&mut self, port: Option<&str>) {
        self.surface_port = port
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
    }

    /// What this building's surface's keys do, or `None` for *never told* — S38.
    #[must_use]
    pub fn surface_bindings(&self) -> Option<&[SurfaceBinding]> {
        self.surface_bindings.as_deref()
    }

    /// Writes the whole table down — S38.
    ///
    /// `pub` rather than `pub(crate)`, unlike `insert_output`, and for
    /// [`Self::set_token`]'s reason exactly: the value has to be made somewhere
    /// this crate cannot reach. A token needs entropy; a **table** needs the
    /// built-in defaults of `docs/MCU_MAPPING.md` §4.1, and those live in
    /// `prism_surface::Bindings` — a MIDI codec, which the show model must never
    /// depend on. So `Command::SetSurfaceBinding` is validated here, answers with
    /// [`crate::Effect::SurfaceBinding`], and `prismd` puts the row on the table
    /// in force and hands the whole of it back through this door.
    pub fn set_surface_bindings(&mut self, rows: Vec<SurfaceBinding>) {
        self.surface_bindings = Some(rows);
    }

    /// Applies one setting — S37, and [`Self::apply`] is the only door.
    ///
    /// # Errors
    ///
    /// [`MachineError`], and nothing is written after one: the two refusals are
    /// a universe count outside the desk's range and a listener off loopback
    /// with no token, which is `docs/IPC_PROTOCOL.md` §2.1.
    pub(crate) fn configure(&mut self, change: &MachineChange) -> Result<(), MachineError> {
        match change {
            MachineChange::Local { local } => self.settings.local = *local,
            MachineChange::Websocket { address } => {
                // §2.1, and it is checked here rather than in an interface so a
                // second client cannot open a school's network without one.
                // Loopback is exempt because the rule is about reaching *off*
                // the machine, and a token for a listener nothing outside the
                // machine can reach would be a password on an inside door.
                if let Some(address) = address
                    && !address.ip().is_loopback()
                    && self.settings.token.is_none()
                {
                    return Err(MachineError::NoTokenForNetwork(*address));
                }
                self.settings.websocket = *address;
            }
            // **Clearing the token closes the door it was holding open.** A
            // configuration with a listener off loopback and no token is exactly
            // the state the check above refuses to reach, so it must not be
            // reachable from the other side either: the listener goes back to
            // loopback and the operator is told by the delta that follows.
            MachineChange::Token { token } => {
                self.settings.token = token
                    .as_deref()
                    .map(str::trim)
                    .filter(|token| !token.is_empty())
                    .map(str::to_owned);
                if self.settings.token.is_none() {
                    self.close_network_listener();
                }
            }
            // The daemon makes the token; nothing is written here. It is not an
            // error and it is not a no-op — see `Effect::NewToken`.
            MachineChange::NewToken | MachineChange::NewIdentity => {}
            MachineChange::LogLevel { level } => self.settings.log_level = *level,
            MachineChange::Universes { universes } => {
                if *universes == 0 || *universes > DEFAULT_UNIVERSES {
                    return Err(MachineError::UniverseCountOutOfRange(*universes));
                }
                self.settings.universes = *universes;
            }
            MachineChange::ExitAction { action } => self.settings.exit_action = *action,
            MachineChange::Autostart { autostart } => self.settings.autostart = *autostart,
            MachineChange::FixtureLibrary { path } => {
                self.settings.fixture_library = blank_is_none(path.as_deref());
            }
            MachineChange::SurfaceProfile { path } => {
                self.settings.surface_profile = blank_is_none(path.as_deref());
            }
            // S38, and both write **nothing here** — see `crate::outputs::apply`,
            // which turns them into effects the daemon carries out. What this arm
            // is for is the one thing that can be decided without the built-in
            // table: **the reserved control is refused before anything is
            // written**, by name and with the reason. Clearing it is allowed,
            // because unbound is the state `docs/MCU_MAPPING.md` §4.3 wants it
            // in.
            MachineChange::SurfaceBinding { control, action } => {
                if let (Some(button), Some(_)) = (control.reserved(), action) {
                    return Err(MachineError::ReservedControl(button));
                }
            }
            MachineChange::SurfaceLearn { .. } => {}
        }
        Ok(())
    }

    /// Puts the listener back on loopback when the token that was guarding it
    /// has gone.
    fn close_network_listener(&mut self) {
        if let Some(address) = self.settings.websocket
            && !address.ip().is_loopback()
        {
            self.settings.websocket = Some(std::net::SocketAddr::from((
                std::net::Ipv4Addr::LOCALHOST,
                address.port(),
            )));
        }
    }

    /// One output by number.
    #[must_use]
    pub fn output(&self, id: OutputId) -> Option<&OutputInstance> {
        self.outputs.iter().find(|output| output.id == id)
    }

    /// Puts an output into the rig, replacing one with the same number.
    ///
    /// `pub(crate)` on purpose: [`Self::apply`] is the only door, so a row
    /// cannot reach the rig without passing [`crate::outputs::validate`] — the
    /// same rule the journal follows in [`crate::ShowFile`].
    pub(crate) fn insert_output(&mut self, output: OutputInstance) {
        match self
            .outputs
            .binary_search_by_key(&output.id, |existing| existing.id)
        {
            Ok(index) => match self.outputs.get_mut(index) {
                Some(slot) => *slot = output,
                // Unreachable: `binary_search_by_key` answered with an index
                // into this vector. A push rather than a panic, because
                // `CLAUDE.md`'s zero-crash invariant does not make exceptions
                // for lines that cannot happen.
                None => self.outputs.push(output),
            },
            Err(index) => self.outputs.insert(index, output),
        }
    }

    /// Takes an output out of the rig and answers with it.
    pub(crate) fn remove_output(&mut self, id: OutputId) -> Option<OutputInstance> {
        let index = self.outputs.iter().position(|output| output.id == id)?;
        Some(self.outputs.remove(index))
    }

    /// The universes this rig carries, in order and without repeats.
    ///
    /// A **disabled** output carries nothing here, and that is deliberate: the
    /// question this answers is *where does the light actually go*, and an
    /// operator who switched a node off is told that its universes now go
    /// nowhere rather than being reassured by a row that is not sending.
    #[must_use]
    pub fn carried_universes(&self) -> Vec<prism_domain::UniverseId> {
        let mut carried = Vec::new();
        for output in self.outputs.iter().filter(|output| output.enabled) {
            for universe in &output.universes {
                if !carried.contains(universe) {
                    carried.push(*universe);
                }
            }
        }
        carried.sort_unstable();
        carried
    }

    /// The outputs that carry `universe` — the routing, read the other way.
    #[must_use]
    pub fn outputs_for(&self, universe: prism_domain::UniverseId) -> Vec<OutputId> {
        self.outputs
            .iter()
            .filter(|output| output.enabled && output.carries(universe))
            .map(|output| output.id)
            .collect()
    }

    /// The universes a show patches that this rig does not carry — S33.
    ///
    /// Reported rather than refused, and reported rather than dropped in
    /// silence: see [`crate::ShowIssue::UniverseNotOutput`]. It is a method on
    /// the machine rather than on the show because a show has no business
    /// knowing about cables, which is the whole of this module's argument.
    #[must_use]
    pub fn dark_universes(&self, show: &crate::Show) -> Vec<crate::ShowIssue> {
        crate::conflict::dark_universes(show, &self.carried_universes())
    }

    /// Applies one of S33's four output commands.
    ///
    /// The third applier — see `crates/prism-core/src/outputs.rs` for why the
    /// rig is neither
    /// show state nor session state, and `Command::is_machine_command` for the
    /// predicate a daemon routes on.
    ///
    /// # Errors
    ///
    /// [`MachineError`], and the configuration is unchanged after any of them.
    pub fn apply(&mut self, command: &Command) -> Result<Applied, MachineError> {
        crate::outputs::apply(self, command)
    }

    /// Whether this desk has an identity at all.
    ///
    /// A daemon that answers `false` here must generate one and write the
    /// configuration back before opening an sACN output — an output holding the
    /// nil CID refuses to connect, which is `prism-protocols`' half of the same
    /// rule.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        !self.desk_id.is_nil()
    }
}

/// A setting whose blank spelling means *not set at all*.
///
/// `MachineConfig::set_surface_port`'s rule, applied to the two paths: a
/// settings window that cleared its box means *no profile*, and a stored `""`
/// would be a path nothing can open that would still count as one.
fn blank_is_none(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{DeskId, InvalidDeskId, MachineConfig};
    use crate::Show;
    use crate::testkit::par_type;
    use core::str::FromStr as _;
    use prism_domain::{Command, OutputId, OutputInstance, OutputKind, UniverseId};

    const TEXT: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";
    const BYTES: [u8; 16] = [
        0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ];

    #[test]
    fn a_desk_id_round_trips_through_its_canonical_text() {
        let id = DeskId::parse(TEXT).unwrap();
        assert_eq!(id.into_bytes(), BYTES);
        assert_eq!(id.to_string(), TEXT);
        assert_eq!(DeskId::from_bytes(BYTES), id);
    }

    #[test]
    fn hyphens_and_case_are_not_part_of_the_identity() {
        assert_eq!(
            DeskId::parse("6BA7B8109DAD11D180B400C04FD430C8").unwrap(),
            DeskId::parse(TEXT).unwrap()
        );
    }

    #[test]
    fn anything_that_is_not_thirty_two_hex_digits_is_refused() {
        for text in [
            "",
            "6ba7b810",
            "6ba7b810-9dad-11d1-80b4-00c04fd430c",
            "6ba7b810-9dad-11d1-80b4-00c04fd430c88",
            "6ba7b810-9dad-11d1-80b4-00c04fd430cg",
        ] {
            assert_eq!(DeskId::parse(text), None, "{text:?}");
        }
        assert_eq!(
            DeskId::from_str("nonsense"),
            Err(InvalidDeskId("nonsense".to_owned()))
        );
        assert_eq!(
            InvalidDeskId("nonsense".to_owned()).to_string(),
            "\"nonsense\" is not a UUID"
        );
    }

    #[test]
    fn the_nil_identity_is_not_an_identity() {
        assert!(DeskId::NIL.is_nil());
        assert_eq!(DeskId::default(), DeskId::NIL);
        assert_eq!(
            DeskId::NIL.to_string(),
            "00000000-0000-0000-0000-000000000000"
        );
        assert!(!MachineConfig::default().is_configured());
        assert!(MachineConfig::new(DeskId::parse(TEXT).unwrap()).is_configured());
    }

    #[test]
    fn a_machine_configuration_is_readable_text() {
        let config = MachineConfig::new(DeskId::parse(TEXT).unwrap());
        let json = serde_json::to_string_pretty(&config).unwrap();
        // Read the way a technician reads it: the identity as a UUID, the
        // settings as words, and the WebSocket address as an address rather
        // than as a map of octets (`prism_domain::socket`).
        for expected in [
            &format!(r#""deskId": "{TEXT}""#),
            r#""outputs": []"#,
            r#""surfacePort": null"#,
            r#""local": true"#,
            r#""websocket": "127.0.0.1:7373""#,
            r#""token": null"#,
            r#""logLevel": "Info""#,
            r#""universes": 64"#,
            r#""exitAction": "Hold""#,
            r#""autostart": false"#,
        ] {
            assert!(json.contains(expected), "{expected} missing from {json}");
        }
        let back: MachineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, config);
        assert_eq!(back.desk_id(), DeskId::parse(TEXT).unwrap());
        assert!(back.outputs().is_empty(), "a fresh desk has no rig yet");
        assert_eq!(back.surface_port(), None, "and no surface yet either");
        // **The WebSocket listener is on by default, and that is S37's
        // change.** Until then a browser could not reach a desk nobody had
        // passed a flag to, which is not a default a school can use.
        assert_eq!(
            back.settings().websocket.map(|address| address.to_string()),
            Some("127.0.0.1:7373".to_owned())
        );
        assert!(back.settings().local);
    }

    /// Every machine configuration written before S33 has no `outputs` key and
    /// every one written before S36 has no `surfacePort`, and they all still
    /// open — with no rig and no surface rather than with an error.
    #[test]
    fn a_configuration_written_before_the_rig_existed_still_opens() {
        let config: MachineConfig =
            serde_json::from_str(&format!(r#"{{"deskId":"{TEXT}"}}"#)).unwrap();
        assert_eq!(config.desk_id(), DeskId::parse(TEXT).unwrap());
        assert!(config.outputs().is_empty());
        assert_eq!(config.surface_port(), None);
        assert!(config.is_configured());

        // And one written by S33, which has the rig and not the surface.
        let config: MachineConfig =
            serde_json::from_str(&format!(r#"{{"deskId":"{TEXT}","outputs":[]}}"#)).unwrap();
        assert_eq!(config.surface_port(), None);
    }

    /// **S36's half of the same decision**, written beside S33's because it is
    /// the same argument: the desk in the rack belongs to the *building*.
    ///
    /// A school's template for next term names a port that the hall it is
    /// opened in has never heard of, exactly as it would name an Art-Net node,
    /// and a show that carried one would silently point a surface at nothing.
    #[test]
    fn the_surface_port_is_not_show_content() {
        // The structural half: a show has nowhere to put a port name.
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        let mut machine = MachineConfig::new(DeskId::parse(TEXT).unwrap());
        machine
            .apply(&Command::SetSurfacePort {
                port: Some("2- X-Touch".to_owned()),
            })
            .unwrap();
        assert_eq!(machine.surface_port(), Some("2- X-Touch"));

        // And the asserted half, on the bytes, beside `desk_id` and the rig.
        let machine_json = serde_json::to_string(&machine).unwrap();
        assert!(machine_json.contains("2- X-Touch"), "{machine_json}");
        assert!(
            !serde_json::to_string(&show).unwrap().contains("X-Touch"),
            "the show learned which desk is in the rack"
        );

        // No surface is an ordinary configuration rather than an absent one,
        // and a name that is nothing but spaces is that same state: a
        // configuration holding `""` would be a name nothing can ever match.
        for blank in [None, Some(String::new()), Some("   ".to_owned())] {
            machine
                .apply(&Command::SetSurfacePort {
                    port: blank.clone(),
                })
                .unwrap();
            assert_eq!(machine.surface_port(), None, "{blank:?}");
        }
    }

    /// S33's half of the decision `desk_id_is_not_show_content` states for the
    /// identity: the **rig belongs to the building**, so a show carried to
    /// another hall on a stick arrives with no cabling in it at all.
    #[test]
    fn outputs_are_not_show_content() {
        // The structural half: a show has nowhere to put an output, so nothing
        // can write one into a `.prism` file by accident.
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        let json = serde_json::to_string(&show).unwrap();
        for word in ["output", "artNet", "sacn", "openDmx", "universes"] {
            assert!(!json.contains(word), "the show mentions {word}");
        }

        // And the other half: a configured rig is in the machine's document and
        // in no other. This is the same assertion one layer up from
        // `desk_id_is_not_show_content`, and it is written out rather than
        // implied because the whole of S33 rests on it.
        let mut machine = MachineConfig::new(DeskId::parse(TEXT).unwrap());
        machine
            .apply(&Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(1),
                    "Stage left node",
                    OutputKind::ArtNet {
                        nodes: vec!["192.168.1.50:6454".parse().unwrap()],
                        sync: false,
                        ports: Vec::new(),
                    },
                    [UniverseId::new(5)],
                ),
            })
            .unwrap();
        let machine_json = serde_json::to_string(&machine).unwrap();
        assert!(machine_json.contains("192.168.1.50:6454"), "{machine_json}");
        assert!(
            !serde_json::to_string(&show)
                .unwrap()
                .contains("192.168.1.50"),
            "the show learned the hall's cabling"
        );
    }

    /// A rig decided somewhere other than by the four commands — a daemon whose
    /// outputs were named on its command line — still passes the same
    /// validation, and a row that fails it is left out rather than carried.
    #[test]
    fn a_rig_built_all_at_once_is_validated_row_by_row() {
        let good = OutputInstance::new(
            OutputId::new(2),
            "Hall",
            OutputKind::Mock,
            [UniverseId::new(1)],
        );
        // Two universes on a cable that is one DMX line, which
        // `crate::outputs::validate` refuses.
        let bad = OutputInstance::new(
            OutputId::new(1),
            "Cable",
            OutputKind::OpenDmx { serial: None },
            [UniverseId::new(1), UniverseId::new(2)],
        );

        let config = MachineConfig::with_outputs(DeskId::parse(TEXT).unwrap(), [bad, good.clone()]);
        assert_eq!(config.outputs(), [good]);
        assert_eq!(config.desk_id(), DeskId::parse(TEXT).unwrap());
        assert!(config.is_configured());
        assert!(
            MachineConfig::with_outputs(DeskId::NIL, [])
                .outputs()
                .is_empty()
        );
    }

    /// The routing, read both ways — and a disabled output carries nothing,
    /// because the question is *where does the light actually go*.
    #[test]
    fn the_rig_answers_which_universes_go_out_and_by_which_cable() {
        let mut config = MachineConfig::default();
        for (id, universes) in [(1u32, vec![1u32, 2]), (2, vec![2, 3])] {
            config
                .apply(&Command::AddOutput {
                    output: OutputInstance::new(
                        OutputId::new(id),
                        format!("Output {id}"),
                        OutputKind::Mock,
                        universes.into_iter().map(UniverseId::new),
                    ),
                })
                .unwrap();
        }

        assert_eq!(
            config.carried_universes(),
            vec![UniverseId::new(1), UniverseId::new(2), UniverseId::new(3)],
            "once each, in order, however many outputs carry them"
        );
        assert_eq!(
            config.outputs_for(UniverseId::new(2)),
            vec![OutputId::new(1), OutputId::new(2)],
            "one universe may go to several outputs"
        );

        config
            .apply(&Command::SetOutputEnabled {
                id: OutputId::new(2),
                enabled: false,
            })
            .unwrap();
        assert_eq!(
            config.carried_universes(),
            vec![UniverseId::new(1), UniverseId::new(2)],
            "universe 3 goes nowhere now, and saying otherwise would be a lie"
        );
        assert_eq!(config.outputs_for(UniverseId::new(3)), Vec::new());
    }

    #[test]
    fn text_that_is_not_a_uuid_does_not_deserialise() {
        assert!(serde_json::from_str::<MachineConfig>(r#"{"deskId":"nope"}"#).is_err());
    }

    #[test]
    fn desk_id_is_not_show_content() {
        // The structural half of the decision: a show has nowhere to put one,
        // so S15 cannot write one into a `.prism` file by accident.
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        let json = serde_json::to_string(&show).unwrap();
        for word in ["deskId", "cid", "uuid"] {
            assert!(!json.contains(word), "the show mentions {word}");
        }
    }
}
