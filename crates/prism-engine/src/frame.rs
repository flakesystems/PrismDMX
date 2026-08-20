//! The artefact the engine publishes once per tick: DMX channel data for every
//! patched universe.
//!
//! The layout is fixed before the tick starts. `ARCHITECTURE_SPEC.md` §3.1
//! requires every buffer to be sized up front from the patch, so a frame has no
//! growable field and no operation on it can allocate.
//!
//! Channel numbers are 1-based, matching `Fixture::address` and what an operator
//! reads off a fixture's display. The internal slices are 0-based; the
//! conversion happens here, once.

use core::fmt;

use prism_domain::{CHANNELS_PER_UNIVERSE, UniverseId};

/// Channels in one DMX universe, as a `usize` for slicing.
pub const UNIVERSE_CHANNELS: usize = CHANNELS_PER_UNIVERSE as usize;

/// Upper bound on universes in one frame layout.
///
/// Not a product limit — the design target is 64 universes and the number here
/// is far above it. It exists so that a corrupt patch turns into a rejected
/// layout rather than a multi-gigabyte allocation.
pub const MAX_UNIVERSES: usize = 1024;

/// Why a set of universes cannot be used as a frame layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    /// No universes were given. An engine with nothing to output is a
    /// configuration mistake, not a valid idle state.
    Empty,
    /// The same universe appeared twice, which would give two frame positions
    /// the same address on the wire.
    Duplicate(UniverseId),
    /// More universes than [`MAX_UNIVERSES`].
    TooMany(usize),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "a frame layout needs at least one universe"),
            Self::Duplicate(id) => write!(f, "universe {id} appears twice in the layout"),
            Self::TooMany(count) => {
                write!(f, "{count} universes exceeds the limit of {MAX_UNIVERSES}")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

/// Which universes a frame carries, and in what order.
///
/// Fixed for the lifetime of the engine: it comes from the patch, before the
/// tick starts. Publisher and subscribers share one of these, so a driver can
/// map a frame position back to the universe number it has to put on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameLayout {
    universes: Box<[UniverseId]>,
}

impl FrameLayout {
    /// Builds a layout from the patched universes, in patch order.
    ///
    /// # Errors
    ///
    /// [`LayoutError`] if the set is empty, contains a duplicate, or is
    /// implausibly large.
    pub fn new(universes: impl IntoIterator<Item = UniverseId>) -> Result<Self, LayoutError> {
        let universes: Vec<UniverseId> = universes.into_iter().collect();
        if universes.is_empty() {
            return Err(LayoutError::Empty);
        }
        if universes.len() > MAX_UNIVERSES {
            return Err(LayoutError::TooMany(universes.len()));
        }
        let mut seen = std::collections::BTreeSet::new();
        for id in &universes {
            if !seen.insert(*id) {
                return Err(LayoutError::Duplicate(*id));
            }
        }
        Ok(Self {
            universes: universes.into_boxed_slice(),
        })
    }

    /// How many universes a frame of this layout carries.
    #[must_use]
    pub const fn universe_count(&self) -> usize {
        self.universes.len()
    }

    /// The universe numbers, in frame order.
    #[must_use]
    pub const fn universes(&self) -> &[UniverseId] {
        &self.universes
    }

    /// Frame position of a universe number, if it is patched at all.
    #[must_use]
    pub fn index_of(&self, universe: UniverseId) -> Option<usize> {
        self.universes.iter().position(|id| *id == universe)
    }

    /// Total channels in a frame of this layout.
    #[must_use]
    pub const fn channel_count(&self) -> usize {
        self.universes.len() * UNIVERSE_CHANNELS
    }
}

/// One tick's output: every channel of every patched universe.
///
/// The buffer is allocated once, from a [`FrameLayout`], and never resized —
/// which is what lets the tick write one without touching the allocator.
#[derive(Clone, PartialEq, Eq)]
pub struct DmxFrame {
    sequence: u64,
    channels: Box<[u8]>,
}

impl DmxFrame {
    /// A blacked-out frame for the given layout.
    #[must_use]
    pub fn new(layout: &FrameLayout) -> Self {
        Self {
            sequence: 0,
            channels: vec![0; layout.channel_count()].into_boxed_slice(),
        }
    }

    /// The publisher's frame counter. Strictly increasing, so a driver can tell
    /// a new frame from a repeat of the last one.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn set_sequence(&mut self, sequence: u64) {
        self.sequence = sequence;
    }

    /// Every channel of every universe, back to back.
    #[must_use]
    pub const fn channels(&self) -> &[u8] {
        &self.channels
    }

    pub(crate) const fn channels_mut(&mut self) -> &mut [u8] {
        &mut self.channels
    }

    /// How many universes this frame carries.
    ///
    /// `as_chunks` rather than `chunks_exact`, here and in the two below,
    /// because the chunk size is a constant: it is one index rather than a
    /// walk, and the array type says the length is 512 without anybody having
    /// to check. Rust 1.98's `clippy::chunks_exact_to_as_chunks` is what asked.
    #[must_use]
    pub fn universe_count(&self) -> usize {
        self.channels.as_chunks::<UNIVERSE_CHANNELS>().0.len()
    }

    /// The 512 channels of one universe, by frame position.
    #[must_use]
    pub fn universe(&self, index: usize) -> Option<&[u8]> {
        self.channels
            .as_chunks::<UNIVERSE_CHANNELS>()
            .0
            .get(index)
            .map(|universe| universe.as_slice())
    }

    /// The 512 channels of one universe, by frame position, for writing.
    pub fn universe_mut(&mut self, index: usize) -> Option<&mut [u8]> {
        self.channels
            .as_chunks_mut::<UNIVERSE_CHANNELS>()
            .0
            .get_mut(index)
            .map(|universe| universe.as_mut_slice())
    }

    /// One channel, addressed the way an operator addresses it: `1..=512`.
    #[must_use]
    pub fn channel(&self, universe: usize, channel: u16) -> Option<u8> {
        let offset = usize::from(channel.checked_sub(1)?);
        self.universe(universe)?.get(offset).copied()
    }

    /// Writes one channel. Returns `false` if the address does not exist, which
    /// is the tick's answer to an out-of-range write: ignore it and keep going.
    pub fn set_channel(&mut self, universe: usize, channel: u16, value: u8) -> bool {
        let Some(offset) = channel.checked_sub(1) else {
            return false;
        };
        match self
            .universe_mut(universe)
            .and_then(|slots| slots.get_mut(usize::from(offset)))
        {
            Some(slot) => {
                *slot = value;
                true
            }
            None => false,
        }
    }

    /// Sets every channel of every universe to `value`.
    pub fn fill(&mut self, value: u8) {
        self.channels.fill(value);
    }

    /// Sets every channel to zero.
    pub fn blackout(&mut self) {
        self.fill(0);
    }
}

impl fmt::Debug for DmxFrame {
    /// Deliberately does not print the channel data: 32 KB of bytes in a panic
    /// message or a log line helps nobody.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DmxFrame")
            .field("sequence", &self.sequence)
            .field("universes", &self.universe_count())
            .field("channels", &self.channels.len())
            .finish()
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{DmxFrame, FrameLayout, LayoutError, MAX_UNIVERSES, UNIVERSE_CHANNELS};
    use prism_domain::UniverseId;

    fn layout(ids: &[u32]) -> FrameLayout {
        FrameLayout::new(ids.iter().copied().map(UniverseId::new)).unwrap()
    }

    #[test]
    fn a_layout_needs_at_least_one_universe() {
        assert_eq!(FrameLayout::new([]).unwrap_err(), LayoutError::Empty);
    }

    #[test]
    fn a_layout_rejects_a_universe_patched_twice() {
        // Two entries for the same universe would give the merge two frames to
        // write and the driver an arbitrary one to read.
        let ids = [1, 7, 1].map(UniverseId::new);
        assert_eq!(
            FrameLayout::new(ids).unwrap_err(),
            LayoutError::Duplicate(UniverseId::new(1))
        );
    }

    #[test]
    fn a_layout_refuses_an_absurd_universe_count() {
        let ids = (0..=MAX_UNIVERSES as u32).map(UniverseId::new);
        assert_eq!(
            FrameLayout::new(ids).unwrap_err(),
            LayoutError::TooMany(MAX_UNIVERSES + 1)
        );
    }

    #[test]
    fn a_layout_maps_universe_numbers_to_frame_positions() {
        let layout = layout(&[3, 1, 9]);
        assert_eq!(layout.universe_count(), 3);
        assert_eq!(layout.channel_count(), 3 * UNIVERSE_CHANNELS);
        // Patch order is preserved: position 0 is universe 3, not universe 1.
        assert_eq!(layout.universes()[0], UniverseId::new(3));
        assert_eq!(layout.index_of(UniverseId::new(9)), Some(2));
        assert_eq!(layout.index_of(UniverseId::new(2)), None);
    }

    #[test]
    fn a_new_frame_is_blacked_out() {
        let frame = DmxFrame::new(&layout(&[1, 2]));
        assert_eq!(frame.sequence(), 0);
        assert_eq!(frame.channels().len(), 2 * UNIVERSE_CHANNELS);
        assert!(frame.channels().iter().all(|&value| value == 0));
    }

    #[test]
    fn channels_are_addressed_from_one() {
        let mut frame = DmxFrame::new(&layout(&[1, 2]));
        assert!(frame.set_channel(0, 1, 255));
        assert!(frame.set_channel(1, 512, 7));
        assert_eq!(frame.channel(0, 1), Some(255));
        assert_eq!(frame.channel(1, 512), Some(7));
        assert_eq!(frame.channels()[0], 255);
        assert_eq!(frame.channels()[2 * UNIVERSE_CHANNELS - 1], 7);
        // Channel 0 does not exist in DMX, and neither does 513.
        assert_eq!(frame.channel(0, 0), None);
        assert_eq!(frame.channel(0, 513), None);
        assert!(!frame.set_channel(0, 0, 1));
        assert!(!frame.set_channel(0, 513, 1));
        // Neither does a universe outside the layout.
        assert_eq!(frame.channel(2, 1), None);
        assert!(!frame.set_channel(2, 1, 1));
    }

    #[test]
    fn universes_are_disjoint_slices_of_the_frame() {
        let mut frame = DmxFrame::new(&layout(&[1, 2, 3]));
        for index in 0..3 {
            let universe = frame.universe_mut(index).unwrap();
            assert_eq!(universe.len(), UNIVERSE_CHANNELS);
            universe.fill(index as u8 + 1);
        }
        assert_eq!(frame.universe(0).unwrap(), [1u8; UNIVERSE_CHANNELS]);
        assert_eq!(frame.universe(1).unwrap(), [2u8; UNIVERSE_CHANNELS]);
        assert_eq!(frame.universe(2).unwrap(), [3u8; UNIVERSE_CHANNELS]);
        assert!(frame.universe(3).is_none());
        assert!(frame.universe_mut(3).is_none());
    }

    #[test]
    fn a_frame_can_be_blacked_out_in_place() {
        let mut frame = DmxFrame::new(&layout(&[1]));
        frame.fill(200);
        assert!(frame.channels().iter().all(|&value| value == 200));
        frame.blackout();
        assert!(frame.channels().iter().all(|&value| value == 0));
    }

    #[test]
    fn a_frame_prints_its_shape_rather_than_its_bytes() {
        // 32 KB of channel data in a panic message helps nobody.
        let frame = DmxFrame::new(&layout(&[1, 2]));
        assert_eq!(
            format!("{frame:?}"),
            "DmxFrame { sequence: 0, universes: 2, channels: 1024 }"
        );
    }

    #[test]
    fn a_rejected_layout_says_why_in_words() {
        // These reach an operator through the patch dialogue, so "invalid
        // layout" is not good enough.
        let errors = [
            (
                LayoutError::Empty,
                "a frame layout needs at least one universe",
            ),
            (
                LayoutError::Duplicate(UniverseId::new(4)),
                "universe 4 appears twice in the layout",
            ),
            (
                LayoutError::TooMany(2000),
                "2000 universes exceeds the limit of 1024",
            ),
        ];
        for (error, text) in errors {
            assert_eq!(error.to_string(), text);
            let as_error: &dyn std::error::Error = &error;
            assert_eq!(as_error.to_string(), text);
        }
    }

    #[test]
    fn a_frame_knows_how_many_universes_it_carries() {
        assert_eq!(DmxFrame::new(&layout(&[1, 2, 3])).universe_count(), 3);
        assert_eq!(DmxFrame::new(&layout(&[7])).universe_count(), 1);
    }

    #[test]
    fn frames_of_the_same_layout_compare_by_content() {
        let layout = layout(&[1]);
        let mut a = DmxFrame::new(&layout);
        let b = DmxFrame::new(&layout);
        assert_eq!(a, b);
        assert!(a.set_channel(0, 1, 1));
        assert_ne!(a, b);
    }
}
