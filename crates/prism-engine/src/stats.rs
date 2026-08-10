//! What the tick records about itself.
//!
//! The point of these numbers is that the exit criteria for the engine are
//! stated as percentiles, and a percentile judged by eye is not a measurement.
//! Recording happens inside the tick, so the histogram is a fixed array sized at
//! construction: no allocation, no locks, no growth.
//!
//! The histogram is linear rather than logarithmic because the interesting range
//! is small and known — a tick period is 22.7 ms and the budget is 2 ms — and a
//! linear bucket has an exact, reportable width.

use std::time::Duration;

/// A fixed-bucket histogram of tick timings.
///
/// Linear buckets, because the range of interest is one tick period and the
/// budget inside it is 2 ms: a logarithmic scale would give resolution where it
/// is not needed and lose it where it is. A percentile is reported as the
/// *upper* bound of the bucket it lands in, so a reported figure is never
/// better than the truth.
#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: Box<[u64]>,
    count: u64,
    overflow: u64,
    max: Duration,
}

impl Histogram {
    /// Width of one bucket.
    pub const RESOLUTION: Duration = Duration::from_micros(100);

    /// Number of buckets.
    pub const BUCKETS: usize = 256;

    /// Largest value that still lands in a bucket — one tick period and a bit,
    /// which is all that is needed: anything longer means a missed tick, and
    /// those are counted separately.
    pub const RANGE: Duration = Duration::from_micros(100 * Self::BUCKETS as u64);

    /// An empty histogram. Allocates once, here, and never again.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: vec![0; Self::BUCKETS].into_boxed_slice(),
            count: 0,
            overflow: 0,
            max: Duration::ZERO,
        }
    }

    /// Records one sample. Called from inside the tick: no allocation, no lock.
    pub fn record(&mut self, value: Duration) {
        self.count += 1;
        if value > self.max {
            self.max = value;
        }
        let bucket = value
            .as_micros()
            .div_euclid(Self::RESOLUTION.as_micros())
            .try_into()
            .ok()
            .and_then(|index: usize| self.buckets.get_mut(index));
        match bucket {
            Some(bucket) => *bucket += 1,
            None => self.overflow += 1,
        }
    }

    /// How many samples have been recorded.
    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// How many of them were larger than [`RANGE`](Self::RANGE).
    #[must_use]
    pub const fn overflowed(&self) -> u64 {
        self.overflow
    }

    /// The largest sample, exactly — not rounded to a bucket.
    #[must_use]
    pub const fn max(&self) -> Duration {
        self.max
    }

    /// The value below which `p` of the samples fall, rounded up to the end of
    /// its bucket. `p` outside `0..=1` is clamped; `NaN` is treated as 0.
    ///
    /// A percentile that lands in the overflow bucket cannot be placed, so the
    /// measured maximum is returned instead — an over-estimate, never an
    /// under-estimate.
    #[must_use]
    pub fn percentile(&self, p: f64) -> Duration {
        if self.count == 0 {
            return Duration::ZERO;
        }
        let p = if p.is_nan() { 0.0 } else { p.clamp(0.0, 1.0) };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 0..=1 times a u64 count, then rounded up"
        )]
        let rank = ((p * self.count as f64).ceil() as u64).max(1);
        let mut seen = 0;
        for (index, samples) in self.buckets.iter().enumerate() {
            seen += samples;
            if seen >= rank {
                let upper = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1);
                return Self::RESOLUTION * upper;
            }
        }
        self.max
    }

    /// Forgets every sample.
    pub fn reset(&mut self) {
        self.buckets.fill(0);
        self.count = 0;
        self.overflow = 0;
        self.max = Duration::ZERO;
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// What one run of the tick loop recorded about itself.
#[derive(Debug, Clone)]
pub struct TickStats {
    /// Ticks executed.
    pub ticks: u64,
    /// Tick slots that were skipped because the previous tick overran. The
    /// exit criterion for this session is that this stays at zero.
    pub missed: u64,
    /// Ticks whose body panicked and whose frame was therefore not published.
    pub panics: u64,
    /// Commands drained from the queue.
    pub commands: u64,
    /// How late each tick started, measured against its own deadline.
    pub jitter: Histogram,
}

impl TickStats {
    /// Zeroed statistics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ticks: 0,
            missed: 0,
            panics: 0,
            commands: 0,
            jitter: Histogram::new(),
        }
    }

    /// Discards everything recorded so far.
    ///
    /// Every measured criterion in this session is stated "after warm-up", and
    /// the first ticks of a run are exactly what warm-up means: pages not yet
    /// faulted in, caches cold, the timer not yet at its final resolution.
    pub fn reset(&mut self) {
        self.ticks = 0;
        self.missed = 0;
        self.panics = 0;
        self.commands = 0;
        self.jitter.reset();
    }
}

impl Default for TickStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::{Histogram, TickStats};
    use std::time::Duration;

    #[test]
    fn an_empty_histogram_reports_zero_rather_than_guessing() {
        let histogram = Histogram::new();
        assert_eq!(histogram.count(), 0);
        assert_eq!(histogram.max(), Duration::ZERO);
        assert_eq!(histogram.percentile(0.999), Duration::ZERO);
    }

    #[test]
    fn a_percentile_is_reported_as_the_upper_bound_of_its_bucket() {
        // 1000 samples: 998 at 50 us, two at 5 ms. Nearest-rank puts p99.9 at
        // the 999th ordered sample, which is one of the 5 ms pair - and the
        // answer must not flatter it by rounding down to where the bucket
        // starts.
        let mut histogram = Histogram::new();
        for _ in 0..998 {
            histogram.record(Duration::from_micros(50));
        }
        histogram.record(Duration::from_millis(5));
        histogram.record(Duration::from_millis(5));
        assert_eq!(histogram.count(), 1000);
        assert_eq!(histogram.percentile(0.5), Histogram::RESOLUTION);
        assert_eq!(histogram.percentile(0.999), Duration::from_micros(5_100));
        assert_eq!(histogram.max(), Duration::from_millis(5));
    }

    #[test]
    fn percentiles_are_clamped_to_a_probability() {
        let mut histogram = Histogram::new();
        histogram.record(Duration::from_micros(10));
        assert_eq!(histogram.percentile(-1.0), Histogram::RESOLUTION);
        assert_eq!(histogram.percentile(2.0), Histogram::RESOLUTION);
        assert_eq!(histogram.percentile(f64::NAN), Histogram::RESOLUTION);
    }

    #[test]
    fn a_sample_beyond_the_last_bucket_is_counted_not_discarded() {
        let mut histogram = Histogram::new();
        histogram.record(Duration::from_secs(1));
        assert_eq!(histogram.count(), 1);
        assert_eq!(histogram.overflowed(), 1);
        assert_eq!(histogram.max(), Duration::from_secs(1));
        // An overflowing sample cannot be placed in a bucket, so the honest
        // answer for a percentile that lands on it is the measured maximum.
        assert_eq!(histogram.percentile(0.999), Duration::from_secs(1));
    }

    #[test]
    fn the_bucket_range_covers_a_whole_tick_period() {
        // A sample larger than a tick period means a tick was missed, and that
        // is counted separately - so the histogram only has to resolve inside
        // one period.
        assert!(Histogram::RANGE >= crate::TICK_PERIOD);
        assert_eq!(Histogram::RESOLUTION, Duration::from_micros(100));
    }

    #[test]
    fn fresh_statistics_are_all_zero() {
        let stats = TickStats::new();
        assert_eq!(stats.ticks, 0);
        assert_eq!(stats.missed, 0);
        assert_eq!(stats.panics, 0);
        assert_eq!(stats.commands, 0);
        assert_eq!(stats.jitter.count(), 0);
    }

    #[test]
    fn the_defaults_are_the_empty_values() {
        assert_eq!(Histogram::default().count(), 0);
        assert_eq!(TickStats::default().ticks, 0);
        // Clone is what lets a run's figures outlive the engine that made them.
        let mut stats = TickStats::new();
        stats.jitter.record(Duration::from_micros(250));
        let copy = stats.clone();
        assert_eq!(copy.jitter.count(), 1);
        assert_eq!(copy.jitter.max(), Duration::from_micros(250));
        assert!(format!("{copy:?}").contains("ticks"));
    }

    #[test]
    fn statistics_can_be_reset_after_warm_up() {
        // Every measured criterion in this session is stated "after warm-up",
        // so discarding the first ticks has to be a supported operation rather
        // than arithmetic done afterwards by hand.
        let mut stats = TickStats::new();
        stats.ticks = 7;
        stats.missed = 1;
        stats.panics = 2;
        stats.commands = 3;
        stats.jitter.record(Duration::from_millis(1));
        stats.reset();
        assert_eq!(stats.ticks, 0);
        assert_eq!(stats.missed, 0);
        assert_eq!(stats.panics, 0);
        assert_eq!(stats.commands, 0);
        assert_eq!(stats.jitter.count(), 0);
        assert_eq!(stats.jitter.max(), Duration::ZERO);
    }
}
