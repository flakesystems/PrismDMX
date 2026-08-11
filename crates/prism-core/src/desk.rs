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

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
/// One field today. It exists as a type rather than as a loose value because
/// the question it answers — "what is this desk, as opposed to what is it
/// playing?" — comes up again for output configuration and for the DMX adapter
/// on this machine, and those must not end up in the show file either.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineConfig {
    /// The sACN CID of this desk. See the module documentation.
    desk_id: DeskId,
}

impl MachineConfig {
    /// A configuration for a desk that has an identity.
    #[must_use]
    pub const fn new(desk_id: DeskId) -> Self {
        Self { desk_id }
    }

    /// This desk's identity.
    #[must_use]
    pub const fn desk_id(&self) -> DeskId {
        self.desk_id
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
        assert_eq!(json, format!(r#"{{"deskId":"{TEXT}"}}"#));
        let back: MachineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, config);
        assert_eq!(back.desk_id(), DeskId::parse(TEXT).unwrap());
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
