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

use prism_domain::{Command, OutputId, OutputInstance};
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
/// that session, and [`crate::outputs`] has the argument in full.
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
}

impl MachineConfig {
    /// A configuration for a desk that has an identity and no rig yet.
    #[must_use]
    pub const fn new(desk_id: DeskId) -> Self {
        Self {
            desk_id,
            outputs: Vec::new(),
            surface_port: None,
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
    /// The third applier — see [`crate::outputs`] for why the rig is neither
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
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            json,
            format!(r#"{{"deskId":"{TEXT}","outputs":[],"surfacePort":null}}"#)
        );
        let back: MachineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, config);
        assert_eq!(back.desk_id(), DeskId::parse(TEXT).unwrap());
        assert!(back.outputs().is_empty(), "a fresh desk has no rig yet");
        assert_eq!(back.surface_port(), None, "and no surface yet either");
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
