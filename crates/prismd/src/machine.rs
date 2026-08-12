//! This desk's identity, generated once and then never again.
//!
//! S10 and S11 settled where the sACN CID lives and left exactly one job here.
//! `prism_core::desk` puts it in as many words: *S17 generates it once, on first
//! start, when the machine configuration does not exist yet, and writes it
//! before opening any output. This crate deliberately cannot: a UUID needs an
//! entropy source, and `prism-core` is platform-neutral and dependency-free by
//! rule.*
//!
//! # Why it is generated once and not per start
//!
//! E1.31 receivers track sources by CID (S10). A desk that invents a fresh one
//! at every start is a **new source** every start, and the previous one holds
//! the universe until its 2.5 s network-data-loss timeout expires — two and a
//! half seconds of two equal-priority sources fighting over the rig, every time
//! somebody restarts the daemon. So the identity is written to disk the first
//! time and read back afterwards, and [`load_or_create`] is the only function
//! that ever calls [`generate_desk_id`].
//!
//! # Why it is not in the show file
//!
//! Copying a show is how a second machine comes to have one — on a stick, from
//! a backup, as the school's template for next term — and nothing in a show
//! file can tell "this is the same desk" from "this is a copy". Two senders
//! under one CID look to a receiver like one source contradicting itself, and no
//! priority sorts that out. `prism_core::MachineConfig` is what a `.prism` file
//! has no table for.

use std::io;
use std::path::Path;

use prism_core::{DeskId, MachineConfig};

/// Reads this machine's configuration, generating an identity the first time.
///
/// Returns the configuration and whether an identity had to be made, so the
/// caller can say so once rather than every start.
///
/// A file that will not parse is **not** overwritten: an identity that cannot
/// be read is a desk whose CID is unknown rather than a desk that should have a
/// new one, and quietly replacing it is how two desks come to share a number.
///
/// # Errors
///
/// [`io::Error`] if the directory or the file cannot be written, or if the file
/// exists and does not parse — with the reason, so a technician can look at it.
pub fn load_or_create(path: &Path) -> io::Result<(MachineConfig, bool)> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => Some(
            serde_json::from_str::<MachineConfig>(&text).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{} is not a machine configuration: {error}", path.display()),
                )
            })?,
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };

    // `is_configured` rather than `is_some`: a file holding the nil identity is
    // a file that says "no identity", which `prism-protocols` refuses to
    // transmit under. Generating one for it is the fix, not a surprise.
    if let Some(config) = existing.filter(MachineConfig::is_configured) {
        return Ok((config, false));
    }

    let config = MachineConfig::new(generate_desk_id()?);
    write(path, &config)?;
    Ok((config, true))
}

/// Writes a machine configuration.
///
/// # Errors
///
/// [`io::Error`] if the file cannot be written.
pub fn write(path: &Path, config: &MachineConfig) -> io::Result<()> {
    let text = serde_json::to_string_pretty(config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, text)
}

/// A random version-4 UUID from the operating system's entropy.
///
/// Written out rather than taken from a UUID crate because it is two lines
/// beyond the sixteen bytes: version 4 is *random with six bits spoken for*,
/// and those six bits are what tells a receiver this identity was generated
/// rather than derived from a name or a clock.
///
/// # Errors
///
/// [`io::Error`] if the machine has no entropy to give, which is a machine that
/// must not be allowed to invent an identity by other means.
pub fn generate_desk_id() -> io::Result<DeskId> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| io::Error::other(format!("no entropy for a desk identity: {error}")))?;
    // RFC 9562 §5.4: version 4 in the high nibble of octet 6, variant 10 in the
    // top two bits of octet 8.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(DeskId::from_bytes(bytes))
}

/// The desk identity as the sACN driver wants it.
///
/// The two types hold the same sixteen bytes in the same order and are
/// deliberately not one type: `prism-core` may not depend on `prism-protocols`,
/// and the daemon is where the two meet.
#[must_use]
pub fn cid_of(config: &MachineConfig) -> prism_protocols::Cid {
    prism_protocols::Cid::from_bytes(config.desk_id().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::{cid_of, generate_desk_id, load_or_create, write};
    use prism_core::{DeskId, MachineConfig};

    #[test]
    fn a_generated_identity_is_a_version_four_uuid() {
        let id = generate_desk_id().unwrap();
        let bytes = id.into_bytes();
        assert_eq!(bytes[6] & 0xf0, 0x40, "version 4");
        assert_eq!(bytes[8] & 0xc0, 0x80, "variant 10");
        assert!(!id.is_nil());
        // And it round-trips through the text a technician reads in the file.
        assert_eq!(DeskId::parse(&id.to_string()), Some(id));
    }

    #[test]
    fn two_generated_identities_are_not_the_same_identity() {
        // The property the whole module exists for: two desks must not share a
        // CID. A generator that answered a constant would pass every other test
        // in this file.
        let first = generate_desk_id().unwrap();
        let second = generate_desk_id().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn the_first_start_makes_an_identity_and_the_second_reads_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::paths::machine_config_path(dir.path());

        let (first, created) = load_or_create(&path).unwrap();
        assert!(created, "the first start has to make one");
        assert!(first.is_configured());

        let (second, created_again) = load_or_create(&path).unwrap();
        assert!(!created_again, "the second start must not make another");
        assert_eq!(
            second.desk_id(),
            first.desk_id(),
            "an sACN receiver would see two sources otherwise"
        );
    }

    #[test]
    fn a_file_holding_no_identity_gets_one() {
        // `DeskId::NIL` is not an identity, and an output holding it refuses to
        // connect — so a configuration written by hand with the default in it
        // has to be completed rather than accepted.
        let dir = tempfile::tempdir().unwrap();
        let path = crate::paths::machine_config_path(dir.path());
        write(&path, &MachineConfig::default()).unwrap();

        let (config, created) = load_or_create(&path).unwrap();
        assert!(created);
        assert!(config.is_configured());
    }

    #[test]
    fn a_configuration_that_will_not_parse_is_reported_rather_than_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::paths::machine_config_path(dir.path());
        std::fs::write(&path, b"{\"deskId\": \"not a uuid\"}").unwrap();

        let error = load_or_create(&path).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("machine configuration"));
        // And the file is still there, holding whatever a person put in it.
        assert!(path.is_file());
    }

    #[test]
    fn the_directory_is_created_if_it_is_not_there() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("machine.json");
        let (config, created) = load_or_create(&path).unwrap();
        assert!(created);
        assert!(path.is_file());
        assert!(config.is_configured());
    }

    #[test]
    fn the_desk_identity_is_the_cid_byte_for_byte() {
        // Two types, one number. A conversion that reordered the bytes would
        // give the rig a different source at every build.
        let config = MachineConfig::new(generate_desk_id().unwrap());
        assert_eq!(
            cid_of(&config).into_bytes(),
            config.desk_id().into_bytes(),
            "the CID on the wire is this desk's identity"
        );
        assert!(!cid_of(&config).is_nil());
    }
}
