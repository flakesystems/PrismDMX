//! The flat command form that crosses into the tick.
//!
//! `prism_domain::Command` is the wire vocabulary: rich, owned, and full of
//! `String`s. The tick cannot handle one — dropping an owned field calls the
//! allocator, which `ARCHITECTURE_SPEC.md` §3.1 forbids. The daemon's core
//! thread therefore resolves each `Command` against the show (looking up names,
//! validating ranges, expanding selections) and pushes the flat, `Copy` result
//! here. Anything that cannot be expressed flatly never belonged in the tick.
//!
//! Sessions **S3-S5** extend this enum as the merge, the executors and the
//! programmer arrive. The encoding is versionless on purpose: both ends are
//! compiled together, and the queue is an in-process channel, not a wire.

use prism_domain::{ExecutorId, GoDirection};

use crate::spsc::{PAYLOAD_BYTES, TickPayload};

/// A command in the form the tick can act on.
///
/// Every variant is `Copy` and flat. Anything that would need an owned field —
/// a fixture name, a preset, a show edit — is resolved on the core thread and
/// arrives here as numbers, or does not reach the engine at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum TickCommand {
    /// Grand master level, `0..=65535`. Intensity only — `docs/DMX_MERGE.md`.
    SetGrandMaster(u16),
    /// An executor's fader position, `0..=65535`.
    SetExecutorLevel {
        /// Which executor.
        executor: ExecutorId,
        /// The new position.
        level: u16,
    },
    /// Step an executor to its next or previous cue.
    Go {
        /// Which executor.
        executor: ExecutorId,
        /// Which way.
        direction: GoDirection,
    },
    /// Blackout on or off.
    SetBlackout(bool),
}

impl TickCommand {
    /// Bytes the encoding actually uses. The rest of a queue slot is headroom
    /// for S3-S5; the assertion that it still fits lives in the tests.
    pub const MAX_ENCODED: usize = 7;

    const TAG_GRAND_MASTER: u8 = 1;
    const TAG_EXECUTOR_LEVEL: u8 = 2;
    const TAG_GO: u8 = 3;
    const TAG_BLACKOUT: u8 = 4;
}

/// A variant added in a later session that outgrows a queue slot must fail to
/// build, not to run.
const _: () = assert!(
    TickCommand::MAX_ENCODED <= PAYLOAD_BYTES,
    "TickCommand no longer fits a queue slot: raise PAYLOAD_BYTES"
);

/// Every variant flattens to the same shape — a tag, an executor and a 16-bit
/// value — which keeps the codec free of per-variant byte arithmetic and makes
/// adding a variant a matter of adding a tag.
impl TickPayload for TickCommand {
    fn encode(self, out: &mut [u8; PAYLOAD_BYTES]) {
        let (tag, executor, value) = match self {
            Self::SetGrandMaster(level) => (Self::TAG_GRAND_MASTER, 0, level),
            Self::SetExecutorLevel { executor, level } => {
                (Self::TAG_EXECUTOR_LEVEL, executor.get(), level)
            }
            Self::Go {
                executor,
                direction,
            } => {
                let direction = match direction {
                    GoDirection::Next => 0,
                    GoDirection::Prev => 1,
                };
                (Self::TAG_GO, executor.get(), direction)
            }
            Self::SetBlackout(on) => (Self::TAG_BLACKOUT, 0, u16::from(on)),
        };
        let [e0, e1, e2, e3] = executor.to_le_bytes();
        let [v0, v1] = value.to_le_bytes();
        let encoded = [tag, e0, e1, e2, e3, v0, v1];
        *out = [0; PAYLOAD_BYTES];
        for (slot, byte) in out.iter_mut().zip(encoded) {
            *slot = byte;
        }
    }

    fn decode(bytes: &[u8; PAYLOAD_BYTES]) -> Option<Self> {
        let mut fixed = [0u8; Self::MAX_ENCODED];
        for (slot, byte) in fixed.iter_mut().zip(bytes.iter()) {
            *slot = *byte;
        }
        let [tag, e0, e1, e2, e3, v0, v1] = fixed;
        let executor = ExecutorId::new(u32::from_le_bytes([e0, e1, e2, e3]));
        let value = u16::from_le_bytes([v0, v1]);
        match tag {
            Self::TAG_GRAND_MASTER => Some(Self::SetGrandMaster(value)),
            Self::TAG_EXECUTOR_LEVEL => Some(Self::SetExecutorLevel {
                executor,
                level: value,
            }),
            Self::TAG_GO => {
                let direction = match value {
                    0 => GoDirection::Next,
                    1 => GoDirection::Prev,
                    _ => return None,
                };
                Some(Self::Go {
                    executor,
                    direction,
                })
            }
            Self::TAG_BLACKOUT => match value {
                0 => Some(Self::SetBlackout(false)),
                1 => Some(Self::SetBlackout(true)),
                _ => None,
            },
            _ => None,
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::TickCommand;
    use crate::spsc::{PAYLOAD_BYTES, TickPayload};
    use prism_domain::{ExecutorId, GoDirection};
    use proptest::prelude::*;

    fn round_trip(command: TickCommand) -> Option<TickCommand> {
        let mut bytes = [0u8; PAYLOAD_BYTES];
        command.encode(&mut bytes);
        TickCommand::decode(&bytes)
    }

    #[test]
    fn every_variant_survives_the_queue_encoding() {
        let commands = [
            TickCommand::SetGrandMaster(0),
            TickCommand::SetGrandMaster(u16::MAX),
            TickCommand::SetExecutorLevel {
                executor: ExecutorId::new(u32::MAX),
                level: 32_768,
            },
            TickCommand::Go {
                executor: ExecutorId::new(9),
                direction: GoDirection::Prev,
            },
            TickCommand::Go {
                executor: ExecutorId::new(0),
                direction: GoDirection::Next,
            },
            TickCommand::SetBlackout(true),
            TickCommand::SetBlackout(false),
        ];
        for command in commands {
            assert_eq!(round_trip(command), Some(command), "{command:?}");
        }
    }

    #[test]
    fn an_unknown_tag_decodes_to_nothing() {
        let mut bytes = [0u8; PAYLOAD_BYTES];
        bytes[0] = 0xFF;
        assert_eq!(TickCommand::decode(&bytes), None);
    }

    #[test]
    fn a_command_uses_only_its_declared_bytes_and_leaves_the_rest_zero() {
        // The declared size is what the compile-time guard checks against, so
        // it has to be the size the encoder actually writes. The rest of the
        // slot is headroom for the variants S3-S5 add.
        let mut bytes = [0xFFu8; PAYLOAD_BYTES];
        TickCommand::SetGrandMaster(u16::MAX).encode(&mut bytes);
        assert_eq!(
            bytes[..TickCommand::MAX_ENCODED],
            [1, 0, 0, 0, 0, 0xFF, 0xFF]
        );
        assert!(bytes[TickCommand::MAX_ENCODED..].iter().all(|&b| b == 0));
    }

    proptest! {
        #[test]
        fn any_command_survives_the_queue_encoding(command in any::<TickCommand>()) {
            prop_assert_eq!(round_trip(command), Some(command));
        }
    }
}
