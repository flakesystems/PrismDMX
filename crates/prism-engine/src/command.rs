//! The flat command form that crosses into the tick.
//!
//! `prism_domain::Command` is the wire vocabulary: rich, owned, and full of
//! `String`s. The tick cannot handle one — dropping an owned field calls the
//! allocator, which `ARCHITECTURE_SPEC.md` §3.1 forbids. The daemon's core
//! thread therefore resolves each `Command` against the show (looking up names,
//! validating ranges, expanding selections) and pushes the flat, `Copy` result
//! here. Anything that cannot be expressed flatly never belonged in the tick.
//!
//! Sessions **S3-S6** extend this enum as the merge, the executors, the
//! programmer and the masters arrive. The encoding is versionless on purpose:
//! both ends are compiled together, and the queue is an in-process channel, not
//! a wire.

use prism_domain::{CrossfadeMode, GoDirection, GroupId, PlaybackId};

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
    /// A playback's fader position, `0..=65535`.
    SetExecutorLevel {
        /// Which playback.
        executor: PlaybackId,
        /// The new position.
        level: u16,
    },
    /// Switch an executor on or off.
    ///
    /// Switching one on stamps it with the next activation counter, which is
    /// what orders LTP attributes (`docs/DMX_MERGE.md` §2.2). Switching one off
    /// removes it from the merge and every attribute it was holding falls back
    /// to the next most recently activated source, or to home.
    SetExecutorActive {
        /// Which playback.
        executor: PlaybackId,
        /// On or off.
        on: bool,
    },
    /// Hold or release a flash over an executor's master.
    ///
    /// **A layer, not a write.** `docs/DMX_MERGE.md` §2.1 applies an executor's
    /// master before the maximum; a flash replaces the master that is applied
    /// and leaves the stored one alone, so releasing it restores exactly what
    /// was there — including a level that arrived *while* the flash was held,
    /// which a save-and-restore would throw away. On a loaded executor a flash
    /// also starts the sequence, and the release stops what the flash started
    /// and nothing else.
    SetExecutorFlash {
        /// Which playback.
        executor: PlaybackId,
        /// Held or released.
        on: bool,
    },
    /// An executor's playback rate, in units of `prism_domain::SPEED_UNITY`.
    ///
    /// The speed master of `docs/DMX_MERGE.md` §4 item 3: it is applied in step
    /// 2 of the tick, to the playback's own clock, and changes no value at all.
    SetExecutorSpeed {
        /// Which playback.
        executor: PlaybackId,
        /// The new rate. `0` freezes the playback where it is.
        speed: u16,
    },
    /// A tap of `ExecutorButtonFunction::LearnSpeed` against an executor.
    ///
    /// Two taps inside `crate::TAP_WINDOW` mean *the running cue's transition
    /// should take that long*, and the rate follows from that. One tap on its
    /// own changes nothing.
    TapExecutorSpeed {
        /// Which playback.
        executor: PlaybackId,
    },
    /// Move an executor's manual crossfade fader — **S51, punch-list B36**.
    ///
    /// The transition is the one the cue list already has; what this replaces is
    /// its *clock*. See `crate::player`'s module documentation for the two modes
    /// and for the state a stroke stopped half way holds.
    ///
    /// # The mode travels with every movement, and it is one field rather than
    /// a second command
    ///
    /// Which of the two a fader is belongs to the **show**
    /// (`ExecutorFaderFunction`), and the tick holds no show — `ARCHITECTURE_SPEC.md`
    /// §3.1: it resolves nothing. So the core thread, which is the one that
    /// resolved the fader to a playback in the first place, says which mode the
    /// movement is; the player compares it with the one it is holding and starts
    /// again if it changed. It costs one tag rather than one byte because the
    /// payload is a tag, a target and a value, and a mode is neither.
    SetExecutorCrossfade {
        /// Which playback.
        executor: PlaybackId,
        /// Which of the two crossfades this fader is.
        mode: CrossfadeMode,
        /// Where the fader is, `0..=65535`.
        position: u16,
    },
    /// Step a playback to its next or previous cue.
    Go {
        /// Which playback.
        executor: PlaybackId,
        /// Which way.
        direction: GoDirection,
    },
    /// Jump a playback straight to one cue of its list - S40's `Goto`.
    ///
    /// The **index**, not the number: a cue number is a string an operator
    /// typed, and the tick resolves nothing (`ARCHITECTURE_SPEC.md` §3.1). The
    /// core thread turns one into the other against the show it holds, which is
    /// the same split `SetProgrammerValue` makes with a merge-plan slot.
    GotoCue {
        /// Which playback.
        executor: PlaybackId,
        /// Which cue, by index into the compiled list.
        cue_index: u16,
    },
    /// Blackout on or off. Intensity only, like every other master.
    SetBlackout(bool),
    /// A group master's fader position, `0..=65535`. Intensity only.
    SetGroupMaster {
        /// Which group.
        group: GroupId,
        /// The new position.
        level: u16,
    },
    /// Put a value into the programmer, overriding every playback for that
    /// attribute (`docs/DMX_MERGE.md` §3).
    ///
    /// Addressed by [`crate::MergePlan`] slot, not by fixture and attribute:
    /// the tick resolves nothing, and the core thread that owns the
    /// `prism_domain::ProgrammerState` already knows the plan's ordering —
    /// fixture, then attribute — because it is part of the plan's contract.
    SetProgrammerValue {
        /// Which slot of the merge plan.
        slot: u32,
        /// The value the operator has set.
        value: u16,
    },
    /// Take one attribute back out of the programmer, so the playbacks below it
    /// decide again.
    ClearProgrammerValue {
        /// Which slot of the merge plan.
        slot: u32,
    },
    /// Empty the programmer — the first stage of the operator's Clear.
    ///
    /// The other two stages are selection state, which never reaches the tick.
    ClearProgrammer,
}

impl TickCommand {
    /// Bytes the encoding actually uses. The rest of a queue slot is headroom
    /// for S3-S5; the assertion that it still fits lives in the tests.
    ///
    /// Eight in S40, when a target grew a kind byte to say whether it named an
    /// executor or a cue list, and **seven again since S45**: a playback is a
    /// cue list's, so the target is one number and there is nothing left to
    /// choose between.
    pub const MAX_ENCODED: usize = 7;

    const TAG_GRAND_MASTER: u8 = 1;
    const TAG_EXECUTOR_LEVEL: u8 = 2;
    const TAG_GO: u8 = 3;
    const TAG_BLACKOUT: u8 = 4;
    const TAG_EXECUTOR_ACTIVE: u8 = 5;
    const TAG_GROUP_MASTER: u8 = 6;
    const TAG_PROGRAMMER_VALUE: u8 = 7;
    const TAG_PROGRAMMER_CLEAR_VALUE: u8 = 8;
    const TAG_PROGRAMMER_CLEAR: u8 = 9;
    const TAG_EXECUTOR_FLASH: u8 = 10;
    const TAG_EXECUTOR_SPEED: u8 = 11;
    const TAG_EXECUTOR_TAP: u8 = 12;
    const TAG_EXECUTOR_XFADE: u8 = 13;
    const TAG_GOTO: u8 = 14;
    /// The second crossfade mode — S51, B36. A tag of its own rather than a
    /// byte in the payload, because the encoding is a tag, a `u32` target and a
    /// `u16` value, and a mode is not a target or a value.
    const TAG_EXECUTOR_FADE: u8 = 15;

    /// The target as the number the codec carries.
    const fn split(target: PlaybackId) -> u32 {
        target.sequence().get()
    }
}

/// A variant added in a later session that outgrows a queue slot must fail to
/// build, not to run.
const _: () = assert!(
    TickCommand::MAX_ENCODED <= PAYLOAD_BYTES,
    "TickCommand no longer fits a queue slot: raise PAYLOAD_BYTES"
);

/// Every variant flattens to the same shape — a tag, a 32-bit target and a
/// 16-bit value — which keeps the codec free of per-variant byte arithmetic and
/// makes adding a variant a matter of adding a tag. The target is a playback, a
/// group number or a merge-plan slot, according to the tag.
impl TickPayload for TickCommand {
    fn encode(self, out: &mut [u8; PAYLOAD_BYTES]) {
        let (tag, target, value) = match self {
            Self::SetGrandMaster(level) => (Self::TAG_GRAND_MASTER, 0, level),
            Self::SetExecutorLevel { executor, level } => {
                (Self::TAG_EXECUTOR_LEVEL, Self::split(executor), level)
            }
            Self::SetExecutorActive { executor, on } => (
                Self::TAG_EXECUTOR_ACTIVE,
                Self::split(executor),
                u16::from(on),
            ),
            Self::Go {
                executor,
                direction,
            } => {
                let direction = match direction {
                    GoDirection::Next => 0,
                    GoDirection::Prev => 1,
                };
                (Self::TAG_GO, Self::split(executor), direction)
            }
            Self::GotoCue {
                executor,
                cue_index,
            } => (Self::TAG_GOTO, Self::split(executor), cue_index),
            Self::SetExecutorFlash { executor, on } => (
                Self::TAG_EXECUTOR_FLASH,
                Self::split(executor),
                u16::from(on),
            ),
            Self::SetExecutorSpeed { executor, speed } => {
                (Self::TAG_EXECUTOR_SPEED, Self::split(executor), speed)
            }
            Self::TapExecutorSpeed { executor } => {
                (Self::TAG_EXECUTOR_TAP, Self::split(executor), 0)
            }
            Self::SetExecutorCrossfade {
                executor,
                mode,
                position,
            } => (
                match mode {
                    CrossfadeMode::Fade => Self::TAG_EXECUTOR_FADE,
                    CrossfadeMode::XFade => Self::TAG_EXECUTOR_XFADE,
                },
                Self::split(executor),
                position,
            ),
            Self::SetBlackout(on) => (Self::TAG_BLACKOUT, 0, u16::from(on)),
            Self::SetGroupMaster { group, level } => (Self::TAG_GROUP_MASTER, group.get(), level),
            Self::SetProgrammerValue { slot, value } => (Self::TAG_PROGRAMMER_VALUE, slot, value),
            Self::ClearProgrammerValue { slot } => (Self::TAG_PROGRAMMER_CLEAR_VALUE, slot, 0),
            Self::ClearProgrammer => (Self::TAG_PROGRAMMER_CLEAR, 0, 0),
        };
        let [e0, e1, e2, e3] = target.to_le_bytes();
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
        let target = u32::from_le_bytes([e0, e1, e2, e3]);
        let executor = PlaybackId::of_sequence(prism_domain::SequenceId::new(target));
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
            Self::TAG_EXECUTOR_ACTIVE => match value {
                0 => Some(Self::SetExecutorActive {
                    executor,
                    on: false,
                }),
                1 => Some(Self::SetExecutorActive { executor, on: true }),
                _ => None,
            },
            Self::TAG_EXECUTOR_FLASH => match value {
                0 => Some(Self::SetExecutorFlash {
                    executor,
                    on: false,
                }),
                1 => Some(Self::SetExecutorFlash { executor, on: true }),
                _ => None,
            },
            Self::TAG_EXECUTOR_SPEED => Some(Self::SetExecutorSpeed {
                executor,
                speed: value,
            }),
            Self::TAG_EXECUTOR_TAP => Some(Self::TapExecutorSpeed { executor }),
            Self::TAG_GOTO => Some(Self::GotoCue {
                executor,
                cue_index: value,
            }),
            Self::TAG_EXECUTOR_XFADE => Some(Self::SetExecutorCrossfade {
                executor,
                mode: CrossfadeMode::XFade,
                position: value,
            }),
            Self::TAG_EXECUTOR_FADE => Some(Self::SetExecutorCrossfade {
                executor,
                mode: CrossfadeMode::Fade,
                position: value,
            }),
            Self::TAG_BLACKOUT => match value {
                0 => Some(Self::SetBlackout(false)),
                1 => Some(Self::SetBlackout(true)),
                _ => None,
            },
            Self::TAG_GROUP_MASTER => Some(Self::SetGroupMaster {
                group: GroupId::new(target),
                level: value,
            }),
            Self::TAG_PROGRAMMER_VALUE => Some(Self::SetProgrammerValue {
                slot: target,
                value,
            }),
            Self::TAG_PROGRAMMER_CLEAR_VALUE => Some(Self::ClearProgrammerValue { slot: target }),
            Self::TAG_PROGRAMMER_CLEAR => Some(Self::ClearProgrammer),
            _ => None,
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::TickCommand;
    use crate::spsc::{PAYLOAD_BYTES, TickPayload};
    use prism_domain::{GoDirection, GroupId, SequenceId};
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
                executor: SequenceId::new(u32::MAX).into(),
                level: 32_768,
            },
            TickCommand::Go {
                executor: SequenceId::new(9).into(),
                direction: GoDirection::Prev,
            },
            TickCommand::Go {
                executor: SequenceId::new(0).into(),
                direction: GoDirection::Next,
            },
            TickCommand::SetBlackout(true),
            TickCommand::SetBlackout(false),
            TickCommand::SetExecutorActive {
                executor: SequenceId::new(3).into(),
                on: true,
            },
            TickCommand::SetExecutorActive {
                executor: SequenceId::new(u32::MAX).into(),
                on: false,
            },
            TickCommand::SetExecutorFlash {
                executor: SequenceId::new(3).into(),
                on: true,
            },
            TickCommand::SetExecutorFlash {
                executor: SequenceId::new(u32::MAX).into(),
                on: false,
            },
            TickCommand::SetExecutorSpeed {
                executor: SequenceId::new(4).into(),
                speed: prism_domain::SPEED_UNITY,
            },
            TickCommand::TapExecutorSpeed {
                executor: SequenceId::new(5).into(),
            },
            TickCommand::SetExecutorCrossfade {
                mode: prism_domain::CrossfadeMode::XFade,
                executor: SequenceId::new(6).into(),
                position: u16::MAX,
            },
            TickCommand::SetGroupMaster {
                group: GroupId::new(12),
                level: 32_767,
            },
            TickCommand::SetGroupMaster {
                group: GroupId::new(u32::MAX),
                level: 0,
            },
            TickCommand::SetProgrammerValue {
                slot: 0,
                value: 65_535,
            },
            TickCommand::SetProgrammerValue {
                slot: u32::MAX,
                value: 0,
            },
            TickCommand::ClearProgrammerValue { slot: 4_242 },
            TickCommand::ClearProgrammer,
        ];
        for command in commands {
            assert_eq!(round_trip(command), Some(command), "{command:?}");
        }
    }

    #[test]
    fn the_programmer_addresses_a_slot_number_rather_than_a_fixture_and_attribute() {
        // The tick knows slots, not names: `MergePlan`'s ordering (fixture, then
        // attribute) is the address, and the core thread resolves a
        // `prism_domain::Command` into one before it ever reaches the queue. A
        // fixture number and an attribute would need a lookup table in the tick
        // and would not fit a queue slot beside a 16-bit value.
        let command = TickCommand::SetProgrammerValue {
            slot: 0x0102_0304,
            value: 0x0506,
        };
        let mut bytes = [0u8; PAYLOAD_BYTES];
        command.encode(&mut bytes);
        assert_eq!(
            bytes[..TickCommand::MAX_ENCODED],
            [7, 0x04, 0x03, 0x02, 0x01, 0x06, 0x05]
        );
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
