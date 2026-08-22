//! Naming a port in a configuration so that it survives a cable — S36.
//!
//! A settings window writes down a name, and the name has to mean the same
//! device tomorrow. Two things get in the way, and both are decoration the
//! platform adds rather than anything the operator typed:
//!
//! | Platform | What the surface is called | What is decoration |
//! |---|---|---|
//! | Windows (WinMM) | `X-Touch`, `2- X-Touch`, `MIDIIN2 (X-Touch)` | the `2- ` prefix, which is the driver instance number and changes with the USB port it is in |
//! | Linux (ALSA) | `X-Touch:X-Touch MIDI 1 24:0` | the ` 24:0` address, which is the ALSA client number and changes at every boot |
//! | macOS (CoreMIDI) | `X-Touch` | nothing |
//!
//! So the rule is: **compare what is left after the decoration comes off**, and
//! only then fall back to something looser.

/// A port name with the platform's decoration taken off, folded to lower case.
///
/// Two things are removed, and nothing else — a rule that rewrote more of the
/// name would start matching devices an operator did not choose:
///
/// - a leading Windows driver-instance prefix (`2- `, `10- `), and
/// - a trailing ALSA client address (` 24:0`).
///
/// The result is only ever compared against another one of these. It is public
/// because it is worth being able to show an operator *why* two names are the
/// same port.
#[must_use]
pub fn normalise(name: &str) -> String {
    let mut rest = name.trim();
    // `2- X-Touch`: digits, a hyphen, a space. Only at the start, and only when
    // something follows it, so a device genuinely called `2- something` in the
    // middle of its name keeps it.
    if let Some((prefix, tail)) = rest.split_once("- ")
        && !prefix.is_empty()
        && prefix.chars().all(|character| character.is_ascii_digit())
        && !tail.trim().is_empty()
    {
        rest = tail.trim();
    }
    // ` 24:0`: a space, digits, a colon, digits, at the end.
    if let Some((head, tail)) = rest.rsplit_once(' ')
        && let Some((client, port)) = tail.split_once(':')
        && !client.is_empty()
        && !port.is_empty()
        && client.chars().all(|character| character.is_ascii_digit())
        && port.chars().all(|character| character.is_ascii_digit())
        && !head.trim().is_empty()
    {
        rest = head.trim();
    }
    rest.to_lowercase()
}

/// Whether a name written in a configuration selects this port.
///
/// Three rules, tried in this order, and the order is the whole design:
///
/// 1. **The name as it stands.** An operator who copied the name out of the
///    enumeration gets exactly the port they picked.
/// 2. **The name with the platform's decoration off** ([`normalise`]). This is
///    the rule that survives a replug: `X-Touch` still finds `2- X-Touch` after
///    somebody moved the plug to another socket.
/// 3. **The name as a fragment of the port's**, case-insensitively. The rule for
///    a person typing `--surface x-touch` at a shell rather than reading a list.
///
/// Looser rules come last so a configuration can always be made exact: a venue
/// with two X-Touches writes both names in full and neither row can drift onto
/// the other. An empty configured name selects **nothing** — a desk that
/// silently attached itself to whatever was plugged in would be a desk that
/// moved somebody else's faders.
#[must_use]
pub fn selects(configured: &str, port: &str) -> bool {
    let configured = configured.trim();
    if configured.is_empty() {
        return false;
    }
    if configured == port.trim() {
        return true;
    }
    // Safe as a `contains` needle because [`normalise`] never empties a name
    // that was not empty already — it only ever strips a *prefix* that
    // something follows and a *suffix* that something precedes.
    // `normalising_never_empties_a_name` is that invariant as a test, and it is
    // load-bearing: an empty needle is contained in everything, which would be
    // a configuration that selects every port on the machine.
    let wanted = normalise(configured);
    if wanted == normalise(port) {
        return true;
    }
    normalise(port).contains(&wanted)
}

/// The first port a configured name selects, with its index.
///
/// First rather than best: the three rules of [`selects`] are already ordered
/// by how exact they are, but they are asked one port at a time, so a fragment
/// that matches port 3 exactly and port 1 loosely would answer 1. That is the
/// right answer for the one case it costs — a surface that enumerates twice,
/// where the second port is the MIDI DIN pair rather than the control surface
/// (S20 measured exactly that on the X-Touch) — and any other case is one an
/// operator fixes by writing the name out in full.
#[must_use]
pub fn choose<'a>(configured: &str, names: &'a [String]) -> Option<(usize, &'a str)> {
    // Two passes rather than one, so an exact name beats a fragment wherever
    // the two land in the list.
    let exact = names.iter().position(|name| {
        name.trim() == configured.trim() || normalise(name) == normalise(configured)
    });
    let index = match exact {
        Some(index) => index,
        None => names.iter().position(|name| selects(configured, name))?,
    };
    Some((index, names.get(index)?.as_str()))
}

#[cfg(test)]
mod tests {
    use super::{choose, normalise, selects};

    #[test]
    fn the_platforms_decoration_comes_off_and_nothing_else_does() {
        // Windows: the driver instance number.
        assert_eq!(normalise("2- X-Touch"), "x-touch");
        assert_eq!(normalise("10- X-Touch"), "x-touch");
        // ALSA: the client address.
        assert_eq!(
            normalise("X-Touch:X-Touch MIDI 1 24:0"),
            "x-touch:x-touch midi 1"
        );
        // CoreMIDI, and a name that has neither.
        assert_eq!(normalise("X-Touch"), "x-touch");
        assert_eq!(normalise("  X-Touch  "), "x-touch");
        // What must **not** come off: a hyphen that is part of the name, a
        // number that is not an instance prefix, and a colon that is not an
        // address.
        assert_eq!(normalise("MIDIIN2 (X-Touch)"), "midiin2 (x-touch)");
        assert_eq!(normalise("A- B"), "a- b");
        assert_eq!(normalise("2-X-Touch"), "2-x-touch");
        assert_eq!(normalise("Desk 1:2:3"), "desk 1:2:3");
        assert_eq!(normalise("X-Touch 24:"), "x-touch 24:");
        // A name that is nothing but decoration keeps itself, because the
        // alternative is a configuration that matches everything.
        assert_eq!(normalise("2- "), "2-");
        assert_eq!(normalise("24:0"), "24:0");
    }

    /// The invariant [`selects`] leans on, asserted rather than guarded against.
    ///
    /// An empty needle is contained in every string, so a `normalise` that
    /// emptied a name would turn the third rule into *this configuration
    /// selects everything*. S21's precedent is that an unreachable branch is
    /// removed rather than covered — so the branch is gone and this is what
    /// stands in its place.
    #[test]
    fn normalising_never_empties_a_name() {
        for name in [
            "X",
            "2- X",
            "2- ",
            "24:0",
            " 24:0",
            "1:2",
            "a 1:2",
            "-",
            "- ",
            "0- 0- x",
            "x 0:0 0:0",
            "\u{1F50C}",
        ] {
            assert!(
                !normalise(name).is_empty(),
                "{name:?} normalised to nothing"
            );
        }
    }

    #[test]
    fn a_configured_name_survives_being_unplugged_and_plugged_back_in() {
        // The case the whole module is for: the operator picked `X-Touch`, the
        // cable moved to another socket, and Windows now calls it `2- X-Touch`.
        assert!(selects("X-Touch", "2- X-Touch"));
        assert!(selects("2- X-Touch", "X-Touch"));
        // And on a Pi, where the number changes at every boot.
        assert!(selects(
            "X-Touch:X-Touch MIDI 1 24:0",
            "X-Touch:X-Touch MIDI 1 28:0"
        ));
    }

    #[test]
    fn a_fragment_is_the_last_rule_rather_than_the_first() {
        assert!(selects("x-touch", "2- X-Touch"), "typed at a shell");
        assert!(selects("Touch", "X-Touch"));
        // Not the other way round: a configuration naming more than the port
        // does is a configuration for a different port.
        assert!(!selects("X-Touch Extender", "X-Touch"));
        assert!(!selects("nanoKONTROL", "X-Touch"));
    }

    #[test]
    fn an_empty_name_selects_nothing_at_all() {
        // A desk that attached itself to whatever was plugged in would move
        // somebody else's faders.
        assert!(!selects("", "X-Touch"));
        assert!(!selects("   ", "X-Touch"));
        assert!(!selects("2- ", "X-Touch"));
        assert_eq!(choose("", &["X-Touch".to_owned()]), None);
    }

    #[test]
    fn an_exact_name_wins_wherever_it_is_in_the_list() {
        // S20's finding: the X-Touch enumerates twice and only the first is the
        // control surface. A venue that has to have the second writes it out.
        let names = vec![
            "X-Touch".to_owned(),
            "MIDIIN2 (X-Touch)".to_owned(),
            "nanoKONTROL2".to_owned(),
        ];
        assert_eq!(choose("X-Touch", &names), Some((0, "X-Touch")));
        assert_eq!(
            choose("MIDIIN2 (X-Touch)", &names),
            Some((1, "MIDIIN2 (X-Touch)")),
            "an exact name reaches the second port even though the first matches it loosely"
        );
        assert_eq!(choose("nano", &names), Some((2, "nanoKONTROL2")));
        assert_eq!(choose("Bcf2000", &names), None);
        assert_eq!(choose("X-Touch", &[]), None);
    }

    /// The renumbering case, end to end: the same configuration picks the same
    /// device out of a list whose decoration has changed under it.
    #[test]
    fn the_same_configuration_picks_the_same_desk_after_a_replug() {
        let before = vec!["X-Touch".to_owned(), "MIDIIN2 (X-Touch)".to_owned()];
        let after = vec!["Wavetable".to_owned(), "2- X-Touch".to_owned()];
        assert_eq!(choose("X-Touch", &before), Some((0, "X-Touch")));
        assert_eq!(choose("X-Touch", &after), Some((1, "2- X-Touch")));
    }
}
