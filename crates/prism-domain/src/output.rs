//! Output health, as reported per DMX interface.
//!
//! `ARCHITECTURE_SPEC.md` §7: every driver runs on its own thread and reports
//! health outwards, so an unplugged USB adapter shows as a red light in the UI
//! instead of stopping the engine. The `DmxOutput` trait itself belongs to
//! `prism-protocols`; only the reported value is domain vocabulary.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Health of one DMX output, as shown by the status light per interface.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum OutputHealth {
    /// Sending frames at the expected rate.
    Ok,
    /// Sending, but not cleanly: late frames, retries, or a reduced rate.
    Degraded,
    /// Not sending. The driver is retrying with backoff.
    #[default]
    Disconnected,
}

impl OutputHealth {
    /// Whether frames are reaching the fixtures at all.
    #[must_use]
    pub const fn is_sending(self) -> bool {
        matches!(self, Self::Ok | Self::Degraded)
    }
}

#[cfg(test)]
mod tests {
    use crate::OutputHealth;
    use ts_rs::{Config, TS};

    #[test]
    fn health_states_are_named() {
        for (health, text) in [
            (OutputHealth::Ok, "\"Ok\""),
            (OutputHealth::Degraded, "\"Degraded\""),
            (OutputHealth::Disconnected, "\"Disconnected\""),
        ] {
            assert_eq!(serde_json::to_string(&health).unwrap(), text);
        }
    }

    #[test]
    fn an_output_that_has_not_reported_yet_counts_as_disconnected() {
        assert_eq!(OutputHealth::default(), OutputHealth::Disconnected);
    }

    #[test]
    fn only_ok_means_the_show_is_going_out() {
        assert!(OutputHealth::Ok.is_sending());
        assert!(OutputHealth::Degraded.is_sending());
        assert!(!OutputHealth::Disconnected.is_sending());
    }

    #[test]
    fn typescript_union_matches_the_specification() {
        assert_eq!(
            OutputHealth::inline(&Config::new()),
            "\"Ok\" | \"Degraded\" | \"Disconnected\""
        );
    }
}
