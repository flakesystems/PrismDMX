//! Identifier newtypes.
//!
//! `ARCHITECTURE_SPEC.md` §6 writes these as `type FixtureId = number`. A bare
//! `u32` would let a caller pass an executor number where a fixture number is
//! expected and compile happily, which is precisely the class of mistake that
//! shows up as the wrong light moving. Each identifier therefore gets its own
//! type. On the wire they stay plain numbers: a newtype struct is transparent
//! to serde, and `ts-rs` renders it as `export type FixtureId = number`.

use core::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Declares one identifier newtype over `u32`.
///
/// The conversions are deliberately explicit in both directions: `get()` and
/// `From`/`Into` are cheap to write where a raw number is genuinely wanted, and
/// nothing else silently converts.
macro_rules! id_newtype {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS,
        )]
        #[cfg_attr(
            any(test, feature = "proptest"),
            derive(proptest_derive::Arbitrary)
        )]
        pub struct $name(u32);

        impl $name {
            /// Wraps a raw number.
            #[must_use]
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            /// The raw number, for arithmetic, indexing and the wire.
            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self(value)
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

id_newtype!(
    /// User-facing fixture number, as typed on the command line.
    FixtureId
);
id_newtype!(
    /// Fixture group number.
    GroupId
);
id_newtype!(
    /// Sequence (cue list) number.
    SequenceId
);
id_newtype!(
    /// Executor number: `page * 8 + slot` (D7 — one page is eight executors).
    ExecutorId
);
id_newtype!(
    /// Preset number within its pool.
    PresetId
);
id_newtype!(
    /// DMX universe number. Valid values are [`UniverseId::MIN`]..=[`UniverseId::MAX`].
    UniverseId
);
id_newtype!(
    /// Session number. V1 has exactly one session, "Main".
    SessionId
);
id_newtype!(
    /// Canvas layout number, selected from the View Selector Bar or the console.
    ViewId
);
id_newtype!(
    /// Identifies one open window on the canvas within a session.
    WindowInstanceId
);
id_newtype!(
    /// Identifies one configured DMX output (an adapter, an ArtNet node, ...).
    OutputId
);

impl UniverseId {
    /// Lowest valid universe number.
    pub const MIN: Self = Self(1);

    /// Highest valid universe number (`ARCHITECTURE_SPEC.md` §6: `1..=64`).
    pub const MAX: Self = Self(64);

    /// Whether this universe number is within the supported range.
    ///
    /// Enforcement belongs to patch validation in `prism-engine`; the domain
    /// only states the range.
    #[must_use]
    pub const fn is_in_range(self) -> bool {
        self.0 >= Self::MIN.0 && self.0 <= Self::MAX.0
    }
}

#[cfg(test)]
mod tests {
    use crate::{ExecutorId, FixtureId, PresetId, UniverseId};
    use ts_rs::{Config, TS};

    #[test]
    fn serialises_as_a_bare_number() {
        assert_eq!(serde_json::to_string(&FixtureId::new(7)).unwrap(), "7");
        assert_eq!(serde_json::to_string(&UniverseId::new(64)).unwrap(), "64");
    }

    #[test]
    fn deserialises_from_a_bare_number() {
        let id: FixtureId = serde_json::from_str("7").unwrap();
        assert_eq!(id, FixtureId::new(7));
    }

    #[test]
    fn exposes_the_inner_value() {
        assert_eq!(FixtureId::new(7).get(), 7);
        assert_eq!(u32::from(ExecutorId::new(3)), 3);
        assert_eq!(PresetId::from(9), PresetId::new(9));
    }

    #[test]
    fn orders_numerically() {
        let mut ids = [FixtureId::new(10), FixtureId::new(2), FixtureId::new(1)];
        ids.sort();
        assert_eq!(
            ids,
            [FixtureId::new(1), FixtureId::new(2), FixtureId::new(10)]
        );
    }

    #[test]
    fn displays_as_the_inner_value() {
        assert_eq!(FixtureId::new(12).to_string(), "12");
    }

    #[test]
    fn generates_a_number_alias_for_typescript() {
        let cfg = Config::new();
        assert_eq!(FixtureId::name(&cfg), "FixtureId");
        assert_eq!(FixtureId::inline(&cfg), "number");
        assert_eq!(FixtureId::decl(&cfg), "type FixtureId = number;".to_owned());
    }

    #[test]
    fn universe_range_is_documented_as_constants() {
        assert_eq!(UniverseId::MIN, UniverseId::new(1));
        assert_eq!(UniverseId::MAX, UniverseId::new(64));
        assert!(UniverseId::new(1).is_in_range());
        assert!(UniverseId::new(64).is_in_range());
        assert!(!UniverseId::new(0).is_in_range());
        assert!(!UniverseId::new(65).is_in_range());
    }
}
