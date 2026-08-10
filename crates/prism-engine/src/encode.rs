//! Attribute values to DMX channel bytes: step 7 of the pipeline
//! (`ARCHITECTURE_SPEC.md` §5), specified in `docs/DMX_MERGE.md` §5.
//!
//! The merge resolves one 16-bit value per [`crate::MergePlan`] slot and knows
//! nothing about addresses. This module owns everything the plan deliberately
//! left out: which universe a fixture is in, where in it, whether an attribute
//! occupies one channel or two, and both levels of invert.
//!
//! # Where the work happens
//!
//! Not in the tick. A [`ChannelPlan`] is built once from the patch, and
//! *building* it is where every address is checked — a fixture whose footprint
//! would run past channel 512, or that is patched into a universe the frame
//! layout does not carry, is rejected there with a [`PatchError`] an operator
//! can read. What is left for the tick is [`ChannelPlan::encode`]: a walk over a
//! precomputed target list, one or two byte writes each, no branches on the
//! patch and no allocation.
//!
//! The two inverts are resolved at build time as well. `AttributeDef.invert` and
//! `Fixture.invert_pan`/`invert_tilt` compose by exclusive or — inverting twice
//! is the identity — so the tick carries one flag per target rather than two.
//!
//! # What the encoder does not touch
//!
//! Channels no attribute claims. The encoder writes the patch, not the frame:
//! every patched channel is rewritten on every tick, and nothing else ever
//! writes one, so an unpatched channel keeps the value it had. Two fixtures
//! sharing an address is likewise allowed rather than rejected — patching a
//! second fixture onto the first is how an operator clones one — and resolves
//! deterministically, because targets are written in slot order.

use core::fmt;
use std::collections::BTreeSet;

use prism_domain::{AttributeDef, AttributeType, Fixture, FixtureId, FixtureType, UniverseId};

use crate::frame::{DmxFrame, FrameLayout, UNIVERSE_CHANNELS};
use crate::plan::{MergeError, MergePlan};

/// Mirrors a value: `65535 - value`.
///
/// `docs/DMX_MERGE.md` §5 applies this to the attribute value **before** the
/// split, which is why an inverted 16-bit attribute mirrors both of its bytes
/// together rather than each on its own.
#[must_use]
pub const fn invert(value: u16) -> u16 {
    u16::MAX - value
}

/// The high byte: what an 8-bit channel gets, and the coarse half of a 16-bit
/// pair.
#[must_use]
pub const fn coarse_byte(value: u16) -> u8 {
    (value >> 8) as u8
}

/// The low byte: the fine half of a 16-bit pair.
#[must_use]
pub const fn fine_byte(value: u16) -> u8 {
    (value & 0xFF) as u8
}

/// Why a patch cannot be encoded into a frame.
///
/// Every variant is a patch-time answer. None of them can arise in the tick:
/// that is the point of validating here (`docs/DMX_MERGE.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchError {
    /// The patch could not be flattened into merge slots in the first place.
    Plan(MergeError),
    /// The fixture is patched into a universe the [`FrameLayout`] does not
    /// carry, so there is nowhere to write it.
    UniverseNotPatched {
        /// The fixture.
        fixture: FixtureId,
        /// The universe it asks for.
        universe: UniverseId,
    },
    /// The fixture's footprint does not fit at its address: address 0, an empty
    /// footprint, or a range running past channel 512.
    AddressOutOfRange {
        /// The fixture.
        fixture: FixtureId,
        /// Its start address.
        address: u16,
        /// The footprint of the type it instantiates.
        footprint: u16,
    },
    /// An attribute's coarse or fine offset lies outside its type's footprint,
    /// which would put it in the next fixture's channels — or the next
    /// universe's.
    AttributeOutsideFootprint {
        /// The fixture the type was patched as.
        fixture: FixtureId,
        /// The attribute.
        attribute: AttributeType,
        /// The offending offset, coarse or fine.
        offset: u16,
        /// The footprint it has to fit inside.
        footprint: u16,
    },
    /// The fixture defines an attribute the merge plan has no slot for, so the
    /// channel would have no value to carry. The two plans describe different
    /// patches.
    AttributeNotInPlan {
        /// The fixture.
        fixture: FixtureId,
        /// The attribute with no slot.
        attribute: AttributeType,
    },
    /// Two channel targets claim one merge slot — a fixture patched twice, or a
    /// type naming one attribute twice.
    DuplicateTarget {
        /// The fixture the second target belongs to.
        fixture: FixtureId,
        /// The attribute claimed twice.
        attribute: AttributeType,
    },
    /// A [`ChannelPlan`] was paired with a [`MergePlan`] it was not built from.
    PlanMismatch {
        /// Slots in the merge plan.
        plan: usize,
        /// Slots the channel plan was built against.
        channels: usize,
    },
}

impl From<MergeError> for PatchError {
    fn from(error: MergeError) -> Self {
        Self::Plan(error)
    }
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => error.fmt(f),
            Self::UniverseNotPatched { fixture, universe } => write!(
                f,
                "fixture {fixture} is patched into universe {universe}, \
                 which the frame layout does not carry"
            ),
            Self::AddressOutOfRange {
                fixture,
                address,
                footprint,
            } => write!(
                f,
                "fixture {fixture} at address {address} needs {footprint} channels, \
                 which runs past channel {CHANNELS}",
                CHANNELS = prism_domain::CHANNELS_PER_UNIVERSE
            ),
            Self::AttributeOutsideFootprint {
                fixture,
                attribute,
                offset,
                footprint,
            } => write!(
                f,
                "fixture {fixture} puts the attribute {attribute:?} at offset {offset} \
                 of a {footprint}-channel footprint"
            ),
            Self::AttributeNotInPlan { fixture, attribute } => write!(
                f,
                "fixture {fixture} defines the attribute {attribute:?}, \
                 which the merge plan has no slot for"
            ),
            Self::DuplicateTarget { fixture, attribute } => write!(
                f,
                "fixture {fixture} has two channel targets for the attribute {attribute:?}"
            ),
            Self::PlanMismatch { plan, channels } => write!(
                f,
                "the merge plan has {plan} slots and the channel plan was built for {channels}"
            ),
        }
    }
}

impl std::error::Error for PatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Plan(error) => Some(error),
            _ => None,
        }
    }
}

/// One merge slot's channels, resolved down to what the tick needs.
///
/// Positions are flat indices into [`DmxFrame::channels`], not universe and
/// address: the conversion depends only on the patch and the frame layout, both
/// of which are fixed before the tick starts.
///
/// The fields are `u32` rather than `usize` deliberately. A target is 16 bytes
/// wide, so the table for a large rig stays half the size it would otherwise be
/// — and the bounds that make the narrowing lossless are structural:
/// [`crate::MAX_SLOTS`] slots and [`crate::MAX_UNIVERSES`] × 512 channels are
/// both far below `u32::MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelTarget {
    slot: u32,
    coarse: u32,
    fine: Option<u32>,
    invert: bool,
}

impl ChannelTarget {
    /// Index into the merged values — a slot of the [`MergePlan`] this target
    /// was built against.
    #[must_use]
    pub const fn slot(&self) -> usize {
        self.slot as usize
    }

    /// Position of the coarse channel in the frame.
    #[must_use]
    pub const fn coarse(&self) -> usize {
        self.coarse as usize
    }

    /// Position of the fine channel in the frame, if the attribute has one.
    #[must_use]
    pub const fn fine(&self) -> Option<usize> {
        match self.fine {
            Some(fine) => Some(fine as usize),
            None => None,
        }
    }

    /// Whether this attribute occupies two channels.
    #[must_use]
    pub const fn is_sixteen_bit(&self) -> bool {
        self.fine.is_some()
    }

    /// Whether the value is mirrored before it is written — the attribute's own
    /// invert flag composed with the fixture's pan/tilt inversion.
    #[must_use]
    pub const fn is_inverted(&self) -> bool {
        self.invert
    }
}

/// The patch, flattened into channel writes.
///
/// Built once from the fixtures and the [`FrameLayout`], and never changed while
/// the tick runs. Targets are ordered by slot, so a plan is a function of the
/// patch rather than of the order the patch happened to be iterated in, and the
/// encode loop reads the merged values in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelPlan {
    targets: Box<[ChannelTarget]>,
    slot_count: usize,
    channel_count: usize,
}

impl ChannelPlan {
    /// Resolves a patch into channel targets, validating every address.
    ///
    /// `fixtures` must be the same patch `plan` was built from; each entry is a
    /// patched fixture and the type it instantiates.
    ///
    /// # Errors
    ///
    /// [`PatchError`] if a fixture is in a universe the layout does not carry,
    /// its footprint does not fit at its address, an attribute sits outside that
    /// footprint, or the fixture list and the merge plan disagree.
    pub fn build<'a, I>(
        plan: &MergePlan,
        layout: &FrameLayout,
        fixtures: I,
    ) -> Result<Self, PatchError>
    where
        I: IntoIterator<Item = (&'a Fixture, &'a FixtureType)>,
    {
        // The iterator is collected and the work happens in one non-generic
        // function, so validating a patch is one copy of this code rather than
        // one per caller's iterator type. Patch time, so the allocation is free.
        let fixtures: Vec<(&Fixture, &FixtureType)> = fixtures.into_iter().collect();
        Self::build_from_slice(plan, layout, &fixtures)
    }

    fn build_from_slice(
        plan: &MergePlan,
        layout: &FrameLayout,
        fixtures: &[(&Fixture, &FixtureType)],
    ) -> Result<Self, PatchError> {
        let mut targets: Vec<ChannelTarget> = Vec::new();
        let mut claimed: BTreeSet<usize> = BTreeSet::new();
        for (fixture, fixture_type) in fixtures {
            let footprint = fixture_type.footprint;
            let Some(position) = layout.index_of(fixture.universe) else {
                return Err(PatchError::UniverseNotPatched {
                    fixture: fixture.id,
                    universe: fixture.universe,
                });
            };
            // The whole of the address arithmetic, done once, here: everything
            // below is an offset inside a range that is known to fit.
            if fixture.last_address(footprint).is_none() {
                return Err(PatchError::AddressOutOfRange {
                    fixture: fixture.id,
                    address: fixture.address,
                    footprint,
                });
            }
            let base = position * UNIVERSE_CHANNELS + usize::from(fixture.address - 1);

            for def in &fixture_type.attributes {
                let Some(slot) = plan.index_of(fixture.id, def.attribute) else {
                    return Err(PatchError::AttributeNotInPlan {
                        fixture: fixture.id,
                        attribute: def.attribute,
                    });
                };
                if !claimed.insert(slot) {
                    return Err(PatchError::DuplicateTarget {
                        fixture: fixture.id,
                        attribute: def.attribute,
                    });
                }
                let coarse = channel_of(base, def.coarse_offset, footprint).ok_or(
                    PatchError::AttributeOutsideFootprint {
                        fixture: fixture.id,
                        attribute: def.attribute,
                        offset: def.coarse_offset,
                        footprint,
                    },
                )?;
                let fine = match def.fine_offset {
                    Some(offset) => Some(channel_of(base, offset, footprint).ok_or(
                        PatchError::AttributeOutsideFootprint {
                            fixture: fixture.id,
                            attribute: def.attribute,
                            offset,
                            footprint,
                        },
                    )?),
                    None => None,
                };
                targets.push(ChannelTarget {
                    slot: slot as u32,
                    coarse: coarse as u32,
                    fine: fine.map(|fine| fine as u32),
                    invert: def.invert ^ fixture_invert(fixture, def),
                });
            }
        }
        targets.sort_unstable_by_key(ChannelTarget::slot);
        Ok(Self {
            targets: targets.into_boxed_slice(),
            slot_count: plan.slot_count(),
            channel_count: layout.channel_count(),
        })
    }

    /// Every channel target, in slot order.
    #[must_use]
    pub const fn targets(&self) -> &[ChannelTarget] {
        &self.targets
    }

    /// How many attributes this plan writes.
    #[must_use]
    pub const fn target_count(&self) -> usize {
        self.targets.len()
    }

    /// Slots in the [`MergePlan`] this plan was built against.
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// Channels in a frame of the layout this plan was built against.
    #[must_use]
    pub const fn channel_count(&self) -> usize {
        self.channel_count
    }

    /// Writes merged attribute values into a frame. This is the tick's work.
    ///
    /// `values` is one 16-bit value per merge slot, in slot order — exactly what
    /// [`crate::MergeBody::values`] holds. Channels no attribute claims are left
    /// as they are.
    ///
    /// Allocation-free, lock-free, and does no I/O. Every write was proved
    /// in-range when the plan was built; the bounds check that remains costs
    /// nothing measurable and means that a frame of the wrong layout, or a short
    /// value slice, degrades to a missing write rather than to a panic on the
    /// tick thread (`CLAUDE.md`).
    pub fn encode(&self, values: &[u16], frame: &mut DmxFrame) {
        let channels = frame.channels_mut();
        for target in &self.targets {
            let Some(&value) = values.get(target.slot()) else {
                continue;
            };
            let value = if target.invert { invert(value) } else { value };
            if let Some(byte) = channels.get_mut(target.coarse()) {
                *byte = coarse_byte(value);
            }
            if let Some(fine) = target.fine()
                && let Some(byte) = channels.get_mut(fine)
            {
                *byte = fine_byte(value);
            }
        }
    }
}

/// The frame position of one offset inside a footprint that is known to fit,
/// or `None` if the offset is outside it.
fn channel_of(base: usize, offset: u16, footprint: u16) -> Option<usize> {
    (offset < footprint).then(|| base + usize::from(offset))
}

/// Whether the fixture's own inversion applies to this attribute.
///
/// `docs/DMX_MERGE.md` §5: pan and tilt only. A fixture hung upside down still
/// dims the right way round.
const fn fixture_invert(fixture: &Fixture, def: &AttributeDef) -> bool {
    match def.attribute {
        AttributeType::Pan => fixture.invert_pan,
        AttributeType::Tilt => fixture.invert_tilt,
        _ => false,
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::encode::{ChannelPlan, PatchError, coarse_byte, fine_byte, invert};
    use crate::frame::UNIVERSE_CHANNELS;
    use crate::testkit::{
        attribute_at, fixture, fixture_type, moving_head, moving_head_16, sized_fixture_type,
    };
    use crate::{DmxFrame, FrameLayout, MergeError, MergePlan};
    use prism_domain::{AttributeType, Fixture, FixtureId, FixtureType, UniverseId};
    use proptest::prelude::*;

    fn layout(ids: &[u32]) -> FrameLayout {
        FrameLayout::new(ids.iter().copied().map(UniverseId::new)).unwrap()
    }

    fn plan_of(entries: &[(&Fixture, &FixtureType)]) -> MergePlan {
        MergePlan::build(
            entries
                .iter()
                .map(|(fixture, fixture_type)| (fixture.id, *fixture_type)),
        )
        .unwrap()
    }

    /// Patches `entries` into `layout` and builds the channel plan for them.
    fn channels_of(
        layout: &FrameLayout,
        entries: &[(&Fixture, &FixtureType)],
    ) -> Result<ChannelPlan, PatchError> {
        let plan = plan_of(entries);
        ChannelPlan::build(&plan, layout, entries.iter().copied())
    }

    /// One fixture, patched into a single universe, encoded from `values`.
    fn encoded(fixture: &Fixture, fixture_type: &FixtureType, values: &[u16]) -> DmxFrame {
        let layout = layout(&[1]);
        let channels = channels_of(&layout, &[(fixture, fixture_type)]).unwrap();
        let mut frame = DmxFrame::new(&layout);
        channels.encode(values, &mut frame);
        frame
    }

    /// The six boundary values the exit criteria name, and what each becomes.
    const BOUNDARIES: [(u16, u8, u8); 6] = [
        (0, 0x00, 0x00),
        (1, 0x00, 0x01),
        (32_767, 0x7F, 0xFF),
        (32_768, 0x80, 0x00),
        (65_534, 0xFF, 0xFE),
        (65_535, 0xFF, 0xFF),
    ];

    #[test]
    fn an_eight_bit_attribute_takes_the_high_byte_of_the_value() {
        // docs/DMX_MERGE.md §5: `data[address + coarseOffset] = (value >> 8)`.
        let head = fixture_type(
            "test.dimmer",
            vec![attribute_at(AttributeType::Dimmer, 0, 0, None)],
        );
        let patched = fixture(1, "test.dimmer", 1, 1);
        for (value, coarse, _) in BOUNDARIES {
            let frame = encoded(&patched, &head, &[value]);
            assert_eq!(frame.channel(0, 1), Some(coarse), "value {value}");
            // The fine byte is dropped, not written somewhere else.
            assert_eq!(frame.channel(0, 2), Some(0), "value {value}");
        }
    }

    #[test]
    fn a_sixteen_bit_attribute_takes_both_bytes() {
        let head = sized_fixture_type(
            "test.dimmer16",
            2,
            vec![attribute_at(AttributeType::Dimmer, 0, 0, Some(1))],
        );
        let patched = fixture(1, "test.dimmer16", 1, 1);
        for (value, coarse, fine) in BOUNDARIES {
            let frame = encoded(&patched, &head, &[value]);
            assert_eq!(frame.channel(0, 1), Some(coarse), "value {value}");
            assert_eq!(frame.channel(0, 2), Some(fine), "value {value}");
        }
    }

    #[test]
    fn the_fine_channel_can_sit_anywhere_in_the_footprint() {
        // A profile is free to put the fine channel before the coarse one, or
        // several channels away from it. The offsets are addresses, not an
        // ordering.
        let head = sized_fixture_type(
            "test.odd",
            4,
            vec![attribute_at(AttributeType::Pan, 0, 3, Some(0))],
        );
        let frame = encoded(&fixture(1, "test.odd", 1, 1), &head, &[0xABCD]);
        assert_eq!(frame.channel(0, 4), Some(0xAB));
        assert_eq!(frame.channel(0, 1), Some(0xCD));
    }

    #[test]
    fn a_fixture_is_encoded_at_its_own_address_in_its_own_universe() {
        let head = moving_head_16();
        let layout = layout(&[7, 1, 4]);
        let one = fixture(1, "test.movinghead16", 1, 100);
        let two = fixture(2, "test.movinghead16", 4, 509);
        let channels = channels_of(&layout, &[(&one, &head), (&two, &head)]).unwrap();
        let mut frame = DmxFrame::new(&layout);
        // Slot order is fixture then attribute: 1/Dimmer, 1/Pan, 2/Dimmer, 2/Pan.
        channels.encode(&[0x1122, 0x3344, 0x5566, 0x7788], &mut frame);

        // Universe 1 is frame position 1, not 0: the layout's order is the
        // patch's, not the universe numbers'.
        assert_eq!(frame.channel(1, 100), Some(0x11));
        assert_eq!(frame.channel(1, 101), Some(0x22));
        assert_eq!(frame.channel(1, 102), Some(0x33));
        assert_eq!(frame.channel(1, 103), Some(0x44));
        // Universe 4 is position 2, and this one ends exactly on channel 512.
        assert_eq!(frame.channel(2, 509), Some(0x55));
        assert_eq!(frame.channel(2, 510), Some(0x66));
        assert_eq!(frame.channel(2, 511), Some(0x77));
        assert_eq!(frame.channel(2, 512), Some(0x88));
        // Universe 7 is patched but carries nothing.
        assert!(frame.universe(0).unwrap().iter().all(|&byte| byte == 0));
    }

    #[test]
    fn channels_no_attribute_claims_are_left_alone() {
        // The encoder writes the patch, not the frame: a channel nothing is
        // patched to keeps whatever it had.
        let head = moving_head_16();
        let layout = layout(&[1]);
        let channels =
            channels_of(&layout, &[(&fixture(1, "test.movinghead16", 1, 10), &head)]).unwrap();
        let mut frame = DmxFrame::new(&layout);
        frame.fill(0x5A);
        channels.encode(&[0, 0], &mut frame);
        for channel in 1..=UNIVERSE_CHANNELS as u16 {
            let expected = if (10..=13).contains(&channel) {
                0
            } else {
                0x5A
            };
            assert_eq!(
                frame.channel(0, channel),
                Some(expected),
                "channel {channel}"
            );
        }
    }

    #[test]
    fn the_attribute_invert_flag_reverses_the_value_before_the_split() {
        let head = sized_fixture_type(
            "test.inverted",
            2,
            vec![{
                let mut def = attribute_at(AttributeType::Dimmer, 0, 0, Some(1));
                def.invert = true;
                def
            }],
        );
        let patched = fixture(1, "test.inverted", 1, 1);
        for (value, coarse, fine) in BOUNDARIES {
            let frame = encoded(&patched, &head, &[value]);
            let inverted = 65_535 - value;
            assert_eq!(frame.channel(0, 1), Some((inverted >> 8) as u8), "{value}");
            assert_eq!(
                frame.channel(0, 2),
                Some((inverted & 0xFF) as u8),
                "{value}"
            );
            // And it is genuinely the mirror of the un-inverted encoding.
            assert_eq!(coarse_byte(65_535 - value), (inverted >> 8) as u8);
            let _ = (coarse, fine);
        }
    }

    #[test]
    fn the_attribute_invert_and_the_fixture_invert_compose() {
        // docs/DMX_MERGE.md §5: the per-fixture flag is applied "on top of the
        // attribute-level invert", so a head hung upside down in a rig whose
        // profile already inverts pan comes out the right way round again.
        let plain = sized_fixture_type(
            "test.pan",
            2,
            vec![attribute_at(AttributeType::Pan, 0, 0, Some(1))],
        );
        let inverted = sized_fixture_type("test.pan", 2, {
            let mut def = attribute_at(AttributeType::Pan, 0, 0, Some(1));
            def.invert = true;
            vec![def]
        });

        for (attribute_invert, fixture_invert, expected) in [
            (false, false, 20_000u16),
            (true, false, 45_535),
            (false, true, 45_535),
            (true, true, 20_000),
        ] {
            let head = if attribute_invert { &inverted } else { &plain };
            let mut patched = fixture(1, "test.pan", 1, 1);
            patched.invert_pan = fixture_invert;
            let frame = encoded(&patched, head, &[20_000]);
            assert_eq!(
                (frame.channel(0, 1), frame.channel(0, 2)),
                (Some(coarse_byte(expected)), Some(fine_byte(expected))),
                "attribute {attribute_invert}, fixture {fixture_invert}"
            );
        }
    }

    #[test]
    fn the_fixture_inverts_reach_pan_and_tilt_and_nothing_else() {
        // A head hung upside down still dims the right way round.
        let head = fixture_type(
            "test.all",
            vec![
                attribute_at(AttributeType::Dimmer, 0, 0, None),
                attribute_at(AttributeType::Pan, 0, 1, None),
                attribute_at(AttributeType::Tilt, 0, 2, None),
            ],
        );
        let mut patched = fixture(1, "test.all", 1, 1);
        patched.invert_pan = true;
        patched.invert_tilt = true;
        let frame = encoded(&patched, &head, &[0x4000, 0x4000, 0x4000]);
        assert_eq!(frame.channel(0, 1), Some(0x40));
        assert_eq!(frame.channel(0, 2), Some(0xBF));
        assert_eq!(frame.channel(0, 3), Some(0xBF));
    }

    #[test]
    fn a_fixture_whose_footprint_runs_past_channel_512_is_rejected_at_patch_time() {
        // Not in the tick: `docs/DMX_MERGE.md` §5 puts this at patch time so the
        // tick can write without bounds checks beyond the slice guarantee.
        let head = moving_head_16();
        let layout = layout(&[1]);
        let patched = fixture(3, "test.movinghead16", 1, 510);
        assert_eq!(
            channels_of(&layout, &[(&patched, &head)]).unwrap_err(),
            PatchError::AddressOutOfRange {
                fixture: FixtureId::new(3),
                address: 510,
                footprint: 4,
            }
        );
        // One channel earlier it fits exactly.
        let patched = fixture(3, "test.movinghead16", 1, 509);
        assert!(channels_of(&layout, &[(&patched, &head)]).is_ok());
    }

    #[test]
    fn address_zero_and_an_empty_footprint_are_rejected_too() {
        let head = moving_head_16();
        let layout = layout(&[1]);
        let mut patched = fixture(1, "test.movinghead16", 1, 0);
        assert_eq!(
            channels_of(&layout, &[(&patched, &head)]).unwrap_err(),
            PatchError::AddressOutOfRange {
                fixture: FixtureId::new(1),
                address: 0,
                footprint: 4,
            }
        );
        patched.address = 1;
        let empty = sized_fixture_type("test.movinghead16", 0, head.attributes.clone());
        assert_eq!(
            channels_of(&layout, &[(&patched, &empty)]).unwrap_err(),
            PatchError::AddressOutOfRange {
                fixture: FixtureId::new(1),
                address: 1,
                footprint: 0,
            }
        );
    }

    #[test]
    fn a_fixture_in_a_universe_the_layout_does_not_carry_is_rejected_at_patch_time() {
        let head = moving_head();
        let layout = layout(&[1, 2]);
        let patched = fixture(9, "test.movinghead", 5, 1);
        assert_eq!(
            channels_of(&layout, &[(&patched, &head)]).unwrap_err(),
            PatchError::UniverseNotPatched {
                fixture: FixtureId::new(9),
                universe: UniverseId::new(5),
            }
        );
    }

    #[test]
    fn an_attribute_that_sits_outside_its_types_footprint_is_rejected() {
        // Otherwise a four-channel fixture at address 512 could write channel
        // 520 — which is a different fixture, or another universe.
        let layout = layout(&[1]);
        let patched = fixture(2, "test.wide", 1, 1);
        let coarse_outside = sized_fixture_type(
            "test.wide",
            2,
            vec![attribute_at(AttributeType::Pan, 0, 2, None)],
        );
        assert_eq!(
            channels_of(&layout, &[(&patched, &coarse_outside)]).unwrap_err(),
            PatchError::AttributeOutsideFootprint {
                fixture: FixtureId::new(2),
                attribute: AttributeType::Pan,
                offset: 2,
                footprint: 2,
            }
        );
        let fine_outside = sized_fixture_type(
            "test.wide",
            2,
            vec![attribute_at(AttributeType::Pan, 0, 0, Some(9))],
        );
        assert_eq!(
            channels_of(&layout, &[(&patched, &fine_outside)]).unwrap_err(),
            PatchError::AttributeOutsideFootprint {
                fixture: FixtureId::new(2),
                attribute: AttributeType::Pan,
                offset: 9,
                footprint: 2,
            }
        );
    }

    #[test]
    fn an_attribute_with_no_slot_in_the_merge_plan_is_rejected() {
        // The two plans must describe the same patch; a channel with no value
        // to put in it is a mismatch, not something to encode as zero.
        let head = moving_head();
        let other = fixture_type(
            "test.other",
            vec![attribute_at(AttributeType::Tilt, 0, 0, None)],
        );
        let layout = layout(&[1]);
        let patched = fixture(1, "test.movinghead", 1, 1);
        let plan = MergePlan::build([(FixtureId::new(1), &head)]).unwrap();
        assert_eq!(
            ChannelPlan::build(&plan, &layout, [(&patched, &other)]).unwrap_err(),
            PatchError::AttributeNotInPlan {
                fixture: FixtureId::new(1),
                attribute: AttributeType::Tilt,
            }
        );
    }

    #[test]
    fn one_slot_may_not_be_claimed_by_two_channel_targets() {
        // The same fixture patched twice would have the encoder write two
        // addresses from one value and the merge resolve one of them.
        let head = moving_head();
        let layout = layout(&[1]);
        let a = fixture(1, "test.movinghead", 1, 1);
        let b = fixture(1, "test.movinghead", 1, 20);
        let plan = MergePlan::build([(FixtureId::new(1), &head)]).unwrap();
        assert_eq!(
            ChannelPlan::build(&plan, &layout, [(&a, &head), (&b, &head)]).unwrap_err(),
            PatchError::DuplicateTarget {
                fixture: FixtureId::new(1),
                attribute: AttributeType::Dimmer,
            }
        );
    }

    #[test]
    fn two_fixtures_may_share_an_address_because_cloning_is_a_real_technique() {
        // Overlapping footprints are not rejected: patching two fixtures to one
        // address is how an operator clones a fixture onto a second one. The
        // result is deterministic — targets are written in slot order, so the
        // higher fixture number wins the shared channel.
        let head = moving_head();
        let layout = layout(&[1]);
        let one = fixture(1, "test.movinghead", 1, 1);
        let two = fixture(2, "test.movinghead", 1, 1);
        let channels = channels_of(&layout, &[(&one, &head), (&two, &head)]).unwrap();
        let mut frame = DmxFrame::new(&layout);
        channels.encode(&[0x1100, 0x2200, 0x3300, 0x4400], &mut frame);
        assert_eq!(frame.channel(0, 1), Some(0x33));
        assert_eq!(frame.channel(0, 2), Some(0x44));
    }

    #[test]
    fn targets_are_ordered_by_slot() {
        // The encode loop reads `values` in order, and a plan is a function of
        // the patch rather than of the order the patch was iterated in.
        let head = moving_head();
        let layout = layout(&[1]);
        let one = fixture(1, "test.movinghead", 1, 1);
        let seven = fixture(7, "test.movinghead", 1, 20);
        let forward = channels_of(&layout, &[(&one, &head), (&seven, &head)]).unwrap();
        let backward = channels_of(&layout, &[(&seven, &head), (&one, &head)]).unwrap();
        assert_eq!(forward.targets(), backward.targets());
        assert!(
            forward
                .targets()
                .windows(2)
                .all(|pair| pair[0].slot() < pair[1].slot())
        );
        assert_eq!(forward.target_count(), 4);
        assert_eq!(forward.slot_count(), 4);
        assert_eq!(forward.channel_count(), UNIVERSE_CHANNELS);
    }

    #[test]
    fn a_target_describes_the_channel_it_writes() {
        let head = moving_head_16();
        let layout = layout(&[1]);
        let mut patched = fixture(1, "test.movinghead16", 1, 5);
        patched.invert_pan = true;
        let channels = channels_of(&layout, &[(&patched, &head)]).unwrap();
        let dimmer = channels.targets().first().unwrap();
        assert_eq!(dimmer.slot(), 0);
        // Address 5 is index 4 of the universe, which is frame position 0.
        assert_eq!(dimmer.coarse(), 4);
        assert_eq!(dimmer.fine(), Some(5));
        assert!(dimmer.is_sixteen_bit());
        assert!(!dimmer.is_inverted());
        let pan = channels.targets().get(1).unwrap();
        assert!(pan.is_inverted());
    }

    #[test]
    fn an_empty_patch_is_a_valid_empty_channel_plan() {
        let layout = layout(&[1]);
        let plan = MergePlan::build(std::iter::empty()).unwrap();
        let channels = ChannelPlan::build(&plan, &layout, []).unwrap();
        assert_eq!(channels.target_count(), 0);
        assert!(channels.targets().is_empty());
        let mut frame = DmxFrame::new(&layout);
        channels.encode(&[], &mut frame);
        assert!(frame.channels().iter().all(|&byte| byte == 0));
    }

    #[test]
    fn encoding_into_a_frame_of_the_wrong_layout_writes_nothing_rather_than_panicking() {
        // A tick that panics costs a frame (CLAUDE.md). The bounds are already
        // guaranteed by patch-time validation, so this can only happen if a host
        // wires a plan to a publisher built from a different layout — which must
        // degrade, not crash.
        let head = moving_head();
        let three = layout(&[1, 2, 3]);
        let patched = fixture(1, "test.movinghead", 3, 1);
        let channels = channels_of(&three, &[(&patched, &head)]).unwrap();
        let mut frame = DmxFrame::new(&layout(&[1]));
        channels.encode(&[65_535, 65_535], &mut frame);
        assert!(frame.channels().iter().all(|&byte| byte == 0));
    }

    #[test]
    fn values_shorter_than_the_plan_encode_as_far_as_they_go() {
        let head = moving_head();
        let layout = layout(&[1]);
        let channels =
            channels_of(&layout, &[(&fixture(1, "test.movinghead", 1, 1), &head)]).unwrap();
        let mut frame = DmxFrame::new(&layout);
        channels.encode(&[65_535], &mut frame);
        assert_eq!(frame.channel(0, 1), Some(0xFF));
        assert_eq!(frame.channel(0, 2), Some(0));
    }

    #[test]
    fn a_rejected_patch_says_why_in_words() {
        // These reach an operator through the patch dialogue, so "invalid
        // patch" is not good enough.
        let errors = [
            (
                PatchError::Plan(MergeError::DuplicateFixture(FixtureId::new(4))),
                "fixture 4 is patched twice",
            ),
            (
                PatchError::UniverseNotPatched {
                    fixture: FixtureId::new(4),
                    universe: UniverseId::new(9),
                },
                "fixture 4 is patched into universe 9, which the frame layout does not carry",
            ),
            (
                PatchError::AddressOutOfRange {
                    fixture: FixtureId::new(4),
                    address: 510,
                    footprint: 8,
                },
                "fixture 4 at address 510 needs 8 channels, which runs past channel 512",
            ),
            (
                PatchError::AttributeOutsideFootprint {
                    fixture: FixtureId::new(4),
                    attribute: AttributeType::Pan,
                    offset: 9,
                    footprint: 4,
                },
                "fixture 4 puts the attribute Pan at offset 9 of a 4-channel footprint",
            ),
            (
                PatchError::AttributeNotInPlan {
                    fixture: FixtureId::new(4),
                    attribute: AttributeType::Tilt,
                },
                "fixture 4 defines the attribute Tilt, which the merge plan has no slot for",
            ),
            (
                PatchError::DuplicateTarget {
                    fixture: FixtureId::new(4),
                    attribute: AttributeType::Dimmer,
                },
                "fixture 4 has two channel targets for the attribute Dimmer",
            ),
            (
                PatchError::PlanMismatch {
                    plan: 12,
                    channels: 8,
                },
                "the merge plan has 12 slots and the channel plan was built for 8",
            ),
        ];
        for (error, text) in errors {
            assert_eq!(error.to_string(), text);
            let as_error: &dyn std::error::Error = &error;
            assert_eq!(as_error.to_string(), text);
        }
        // A merge error keeps its own type underneath.
        let wrapped = PatchError::Plan(MergeError::TooManySources(2_000));
        let as_error: &dyn std::error::Error = &wrapped;
        assert!(as_error.source().is_some());
        let unwrapped: &dyn std::error::Error = &PatchError::PlanMismatch {
            plan: 1,
            channels: 2,
        };
        assert!(unwrapped.source().is_none());
        assert_eq!(
            PatchError::from(MergeError::DuplicateFixture(FixtureId::new(1))),
            PatchError::Plan(MergeError::DuplicateFixture(FixtureId::new(1)))
        );
    }

    #[test]
    fn the_split_functions_are_the_specifications_arithmetic() {
        assert_eq!(coarse_byte(0xABCD), 0xAB);
        assert_eq!(fine_byte(0xABCD), 0xCD);
        assert_eq!(invert(0), 65_535);
        assert_eq!(invert(65_535), 0);
    }

    proptest! {
        #[test]
        fn a_split_value_reassembles_into_the_value_it_came_from(value: u16) {
            let rejoined = (u16::from(coarse_byte(value)) << 8) | u16::from(fine_byte(value));
            prop_assert_eq!(rejoined, value);
        }

        #[test]
        fn inverting_twice_is_the_identity(value: u16) {
            prop_assert_eq!(invert(invert(value)), value);
        }

        #[test]
        fn the_eight_bit_write_is_the_coarse_byte_of_the_sixteen_bit_one(value: u16) {
            // "Working in 16-bit internally regardless of the patched resolution"
            // (§5): the 8-bit case is the 16-bit case with the fine byte dropped,
            // not a separate scaling.
            let eight = fixture_type(
                "test.8",
                vec![attribute_at(AttributeType::Dimmer, 0, 0, None)],
            );
            let sixteen = sized_fixture_type(
                "test.16",
                2,
                vec![attribute_at(AttributeType::Dimmer, 0, 0, Some(1))],
            );
            let patched = fixture(1, "test.x", 1, 1);
            prop_assert_eq!(
                encoded(&patched, &eight, &[value]).channel(0, 1),
                encoded(&patched, &sixteen, &[value]).channel(0, 1)
            );
        }
    }

    prop_compose! {
        /// An arbitrary patched fixture, narrowed only where an unconstrained
        /// value would be rejected every time. `prism-domain`'s `Arbitrary`
        /// impls supply the rest.
        fn arbitrary_patch()(
            mut patched in any::<Fixture>(),
            universe in 1u32..=3,
            address in 0u16..=520,
            footprint in 0u16..=8,
            offsets in prop::collection::vec((0u16..=8, prop::option::of(0u16..=8)), 0..=4),
        ) -> (Fixture, FixtureType) {
            patched.universe = UniverseId::new(universe);
            patched.address = address;
            let attributes = offsets
                .into_iter()
                .zip(AttributeType::ALL)
                .map(|((coarse, fine), attribute)| attribute_at(attribute, 0, coarse, fine))
                .collect();
            (patched, sized_fixture_type("test.arb", footprint, attributes))
        }
    }

    proptest! {
        #[test]
        fn an_accepted_patch_writes_only_inside_its_own_footprint(
            (patched, fixture_type) in arbitrary_patch(),
        ) {
            let layout = layout(&[1, 2, 3]);
            let plan = MergePlan::build([(patched.id, &fixture_type)]).unwrap();
            let Ok(channels) = ChannelPlan::build(&plan, &layout, [(&patched, &fixture_type)])
            else {
                return Ok(());
            };
            let position = layout.index_of(patched.universe).unwrap();
            let base = position * UNIVERSE_CHANNELS + usize::from(patched.address - 1);
            let footprint = base + usize::from(fixture_type.footprint);
            for target in channels.targets() {
                prop_assert!((base..footprint).contains(&target.coarse()));
                if let Some(fine) = target.fine() {
                    prop_assert!((base..footprint).contains(&fine));
                }
            }

            // And what it writes lands in the frame, entirely inside the
            // fixture's own universe.
            let values = vec![0xFFFF; plan.slot_count()];
            let mut frame = DmxFrame::new(&layout);
            channels.encode(&values, &mut frame);
            for (index, byte) in frame.channels().iter().enumerate() {
                if *byte != 0 {
                    prop_assert!((base..footprint).contains(&index));
                }
            }
        }
    }
}
