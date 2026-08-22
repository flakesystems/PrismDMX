//! Socket addresses on the wire, as text in both codecs.
//!
//! # Why this module exists at all *(S33)*
//!
//! `std::net::SocketAddr` has a serde implementation and it is **not codec
//! symmetric under `rmp-serde`**: the serialiser reports itself as not
//! human-readable and writes the address as a tagged map of octets, while the
//! deserialiser reports itself as human-readable and asks for a string. A value
//! encoded with `rmp_serde::to_vec_named` therefore cannot be read back by
//! `rmp_serde::from_slice`, and `wire.rs`'s round-trip property is exactly what
//! found it.
//!
//! Rather than fix that with a serializer setting nobody would remember — the
//! IPC frame is built in `prism-ipc` and the machine configuration is written in
//! `prismd`, so there would be two places to remember it in — the address is
//! written as **text** by this module, on both codecs. `192.168.1.50:6454` is
//! also what a technician reading `machine.json` expects to see and what
//! `ts-rs` renders the field as, so the three agree by construction.
//!
//! It is the same shape as [`crate::finite`], which guards every `f64` in the
//! domain for the same class of reason: a codec that is nearly right about a
//! type is a bug that only appears on one of the two wires.

use std::net::SocketAddr;

use serde::Deserialize;
use serde::de::{Deserializer, Error as _};
use serde::ser::{SerializeSeq as _, Serializer};

/// Writes a list of addresses as a list of strings.
///
/// # Errors
///
/// Whatever the serializer says.
pub fn serialize<S: Serializer>(
    addresses: &Vec<SocketAddr>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(addresses.len()))?;
    for address in addresses {
        sequence.serialize_element(&address.to_string())?;
    }
    sequence.end()
}

/// Reads a list of strings back as addresses.
///
/// # Errors
///
/// A string that is not an address is refused with the text in the message, so
/// a hand-edited machine configuration says which line is wrong rather than
/// losing a node in silence.
pub fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<SocketAddr>, D::Error> {
    let text = Vec::<String>::deserialize(deserializer)?;
    text.into_iter()
        .map(|entry| {
            entry
                .parse::<SocketAddr>()
                .map_err(|_| D::Error::custom(format!("{entry:?} is not an address")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{OutputKind, SacnPort, UniverseId};

    fn art_net() -> OutputKind {
        OutputKind::ArtNet {
            nodes: vec![
                "192.168.1.50:6454".parse().unwrap(),
                "192.168.1.51:6454".parse().unwrap(),
            ],
            sync: false,
            ports: Vec::new(),
        }
    }

    /// The property the module exists for, and the one `rmp-serde`'s own
    /// `SocketAddr` fails: what one codec writes, the *same* codec reads.
    #[test]
    fn an_address_survives_both_codecs() {
        let kind = art_net();
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains(r#""192.168.1.50:6454""#), "{json}");
        assert_eq!(serde_json::from_str::<OutputKind>(&json).unwrap(), kind);

        let packed = rmp_serde::to_vec_named(&kind).unwrap();
        assert_eq!(rmp_serde::from_slice::<OutputKind>(&packed).unwrap(), kind);
    }

    #[test]
    fn text_that_is_not_an_address_is_refused_by_name() {
        let error = serde_json::from_str::<OutputKind>(
            r#"{"t":"ArtNet","nodes":["stage left"],"sync":false,"ports":[]}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("stage left"), "{error}");
    }

    #[test]
    fn a_receiver_list_that_is_empty_stays_empty() {
        let kind = OutputKind::Sacn {
            receivers: Vec::new(),
            ttl: 4,
            ports: vec![SacnPort::for_universe(UniverseId::new(3))],
        };
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains(r#""receivers":[]"#), "{json}");
        assert_eq!(serde_json::from_str::<OutputKind>(&json).unwrap(), kind);
        let packed = rmp_serde::to_vec_named(&kind).unwrap();
        assert_eq!(rmp_serde::from_slice::<OutputKind>(&packed).unwrap(), kind);
    }
}
