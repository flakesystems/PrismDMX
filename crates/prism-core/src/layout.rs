//! Where a window goes on the canvas, and where it may not — S43.
//!
//! # Why the daemon lays the canvas out
//!
//! The owner's punch list (B10) asks for two things: a new window should find a
//! free place rather than land on top of the last one, and dragging one should
//! not be able to bury another.
//!
//! Both are arithmetic a client could do, and a client must not. `OpenWindow`
//! carries no geometry and never has (`ARCHITECTURE_SPEC.md` §4.4: the console
//! opens a window, it does not lay one out), and S25 recorded the reason a
//! client may not fill the gap in as many words — *a client that offset its own
//! new windows would be writing session state on its own initiative, and two
//! clients doing it would race*. The layout is the session's (§4.1), so
//! whichever process owns the session owns the arithmetic. S25 predicted the fix
//! would be "geometry on `OpenWindow` or a cascade in the daemon"; this is the
//! second, and it is the better half of the prediction because it needs no new
//! field on a command an X-Touch key sends.
//!
//! # The two rules
//!
//! **Opening**: the first free place, searched from the top left, at the default
//! size — and if nothing of that size fits, at successively smaller sizes down
//! to [`prism_domain::MIN_WINDOW_WIDTH`] by
//! [`prism_domain::MIN_WINDOW_HEIGHT`]. Only when even that will not fit is the
//! window refused, and then it is refused out loud
//! ([`crate::SessionError::NoRoomOnTheCanvas`]) rather than opened underneath
//! something.
//!
//! **Moving**: a placement is accepted when it overlaps **no more windows than
//! the one it replaces**. The obvious rule — refuse anything that overlaps —
//! would be a trap: every layout written before this session has its windows
//! stacked at the origin, and under that rule not one of them could ever be
//! dragged apart again. *Overlap may only shrink* lets a stacked layout be taken
//! apart, never lets a clean one be spoiled, and needs nothing said about which
//! shows are old.
//!
//! A refused move is **not an error**. A drag sends a placement every 33 ms
//! (S25), so a refusal would be a stream of notices for an ordinary gesture;
//! instead the placement simply changes nothing, the client drops its local
//! rectangle when the button comes up, and the window is back where the daemon
//! has it. That is what *blocked* looks like to an operator.

use prism_domain::{
    CANVAS_HEIGHT, CANVAS_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, WindowInstance,
};

/// A rectangle in canvas units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Rect {
    /// Whether two rectangles share any area at all.
    ///
    /// Touching edges do not overlap: two windows side by side at
    /// `x = 0, w = 640` and `x = 640` are a tiled layout, not a collision, and
    /// the placement search below produces exactly that pair.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }

    /// Whether this rectangle is inside the canvas.
    #[must_use]
    pub fn on_canvas(&self) -> bool {
        self.x >= 0.0
            && self.y >= 0.0
            && self.x + self.w <= CANVAS_WIDTH
            && self.y + self.h <= CANVAS_HEIGHT
    }

    /// The rectangle of an open window.
    #[must_use]
    pub fn of(window: &WindowInstance) -> Self {
        Self {
            x: window.x,
            y: window.y,
            w: window.w,
            h: window.h,
        }
    }
}

/// The sizes a placement search tries, largest first, as fractions of the
/// default.
///
/// Three steps rather than a continuous shrink: an operator opening a fifth
/// window on a full canvas wants *a window they can read*, and a search that
/// crept down a pixel at a time would find a sliver. The last step is the floor
/// itself, so the final answer is always the smallest window this interface
/// allows rather than an arbitrary fraction.
const SHRINK_STEPS: [f64; 3] = [1.0, 0.75, 0.5];

/// Where to put a new window of `w` × `h`, given what is already open.
///
/// Answers `None` when nothing of any allowed size fits.
///
/// # The search
///
/// Candidate corners are the origin and the bottom-right corners of what is
/// already there — every position where a window could sit flush against
/// something, which is the only kind of position worth trying: any free spot can
/// be slid up and left until it touches, so a rectangle that fits anywhere fits
/// at one of these. Sorted top-to-bottom then left-to-right, so a canvas fills
/// the way a person reads.
#[must_use]
pub fn free_place(taken: &[Rect], w: f64, h: f64) -> Option<Rect> {
    for step in SHRINK_STEPS {
        // `clamp` is safe here because both bounds are `const` and finite and
        // the floor is below the ceiling — the two things it panics on.
        let width = (w * step).clamp(MIN_WINDOW_WIDTH, CANVAS_WIDTH);
        let height = (h * step).clamp(MIN_WINDOW_HEIGHT, CANVAS_HEIGHT);
        if let Some(place) = fit(taken, width, height) {
            return Some(place);
        }
    }
    fit(taken, MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)
}

/// One size, tried at every candidate corner.
fn fit(taken: &[Rect], w: f64, h: f64) -> Option<Rect> {
    let mut xs: Vec<f64> = vec![0.0];
    let mut ys: Vec<f64> = vec![0.0];
    for rect in taken {
        xs.push(rect.x + rect.w);
        ys.push(rect.y + rect.h);
    }
    sort_unique(&mut xs);
    sort_unique(&mut ys);
    for y in &ys {
        for x in &xs {
            let candidate = Rect { x: *x, y: *y, w, h };
            if candidate.on_canvas() && !taken.iter().any(|rect| rect.overlaps(&candidate)) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Sorts ascending and drops repeats, so a candidate is tried once.
///
/// The values are finite by construction — they come from a `WindowInstance`,
/// whose four coordinates are guarded at the decoder — so a total order exists
/// and `partial_cmp` cannot answer `None` here.
fn sort_unique(values: &mut Vec<f64>) {
    values.sort_by(|left, right| {
        left.partial_cmp(right)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    values.dedup_by(|left, right| (*left - *right).abs() < f64::EPSILON);
}

/// Whether a window may be moved from `from` to `to`, given its neighbours.
///
/// `neighbours` is every *other* open window. See the module documentation for
/// why the rule is *overlap may only shrink* rather than *no overlap*.
#[must_use]
pub fn may_place(from: &Rect, to: &Rect, neighbours: &[Rect]) -> bool {
    neighbours
        .iter()
        .all(|rect| !to.overlaps(rect) || from.overlaps(rect))
}

#[cfg(test)]
mod tests {
    use super::{Rect, free_place, may_place};
    use prism_domain::{CANVAS_HEIGHT, CANVAS_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH};

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn the_first_window_goes_in_the_corner() {
        assert_eq!(
            free_place(&[], 640.0, 480.0),
            Some(rect(0.0, 0.0, 640.0, 480.0))
        );
    }

    #[test]
    fn the_next_window_goes_beside_the_last_and_touches_it() {
        let first = rect(0.0, 0.0, 640.0, 480.0);
        // Flush against the first, not overlapping it — touching edges are a
        // tiled layout rather than a collision.
        assert_eq!(
            free_place(&[first], 640.0, 480.0),
            Some(rect(640.0, 0.0, 640.0, 480.0))
        );
    }

    #[test]
    fn a_row_that_is_full_wraps_to_the_next_one() {
        let row: Vec<Rect> = (0..3)
            .map(|n| rect(f64::from(n) * 640.0, 0.0, 640.0, 480.0))
            .collect();
        assert_eq!(
            free_place(&row, 640.0, 480.0),
            Some(rect(0.0, 480.0, 640.0, 480.0))
        );
    }

    #[test]
    fn a_canvas_with_no_room_shrinks_the_window_rather_than_refusing_it() {
        // Everything but a 300-unit strip along the bottom is taken. A default
        // window is 480 tall and will not go there; three quarters of one is
        // 360 and will not either; half of one is 320 × 240 and fits.
        let most = rect(0.0, 0.0, CANVAS_WIDTH, CANVAS_HEIGHT - 300.0);
        let place = free_place(&[most], 640.0, 480.0).expect("the strip along the bottom");
        assert!(place.on_canvas());
        assert!(!most.overlaps(&place));
        assert_eq!((place.w, place.h), (320.0, 240.0));
    }

    #[test]
    fn six_default_windows_fill_the_canvas_and_the_seventh_is_refused() {
        // Three across by two down is 1920 × 960, and what is left is 120 tall
        // — under the floor. This is the state `NoRoomOnTheCanvas` exists for.
        let full: Vec<Rect> = (0..6)
            .map(|n| {
                rect(
                    f64::from(n % 3) * 640.0,
                    f64::from(n / 3) * 480.0,
                    640.0,
                    480.0,
                )
            })
            .collect();
        assert_eq!(free_place(&full, 640.0, 480.0), None);
    }

    #[test]
    fn a_canvas_with_genuinely_no_room_answers_nothing() {
        let whole = rect(0.0, 0.0, CANVAS_WIDTH, CANVAS_HEIGHT);
        assert_eq!(free_place(&[whole], 640.0, 480.0), None);
    }

    #[test]
    fn the_floor_is_the_last_size_tried() {
        // A canvas with one narrow strip left: only the smallest window fits.
        let most = rect(0.0, 0.0, CANVAS_WIDTH, CANVAS_HEIGHT - MIN_WINDOW_HEIGHT);
        let place = free_place(&[most], 640.0, 480.0).expect("the strip along the bottom");
        assert_eq!(place.h, MIN_WINDOW_HEIGHT);
        assert!(place.w >= MIN_WINDOW_WIDTH);
    }

    #[test]
    fn a_move_that_lands_clear_is_allowed() {
        let neighbour = rect(640.0, 0.0, 640.0, 480.0);
        let from = rect(0.0, 0.0, 640.0, 480.0);
        assert!(may_place(
            &from,
            &rect(0.0, 480.0, 640.0, 480.0),
            &[neighbour]
        ));
    }

    #[test]
    fn a_move_that_buries_a_neighbour_is_refused() {
        let neighbour = rect(640.0, 0.0, 640.0, 480.0);
        let from = rect(0.0, 0.0, 640.0, 480.0);
        assert!(!may_place(
            &from,
            &rect(700.0, 0.0, 640.0, 480.0),
            &[neighbour]
        ));
    }

    /// **The rule that lets an old layout be taken apart.** Every show written
    /// before S43 has its windows stacked at the origin, because `OpenWindow`
    /// used to place them all there. Under a plain *no overlap* rule none of
    /// them could ever be moved again — every position overlaps the pile.
    #[test]
    fn a_window_already_buried_can_still_be_dragged_out() {
        let buried = rect(0.0, 0.0, 640.0, 480.0);
        let on_top_of_it = rect(0.0, 0.0, 640.0, 480.0);
        // Still overlapping, but less of it: allowed, because the overlap set
        // has not grown.
        assert!(may_place(
            &buried,
            &rect(100.0, 0.0, 640.0, 480.0),
            &[on_top_of_it]
        ));
        // And all the way out: allowed.
        assert!(may_place(
            &buried,
            &rect(640.0, 0.0, 640.0, 480.0),
            &[on_top_of_it]
        ));
    }

    #[test]
    fn a_window_may_not_pick_up_a_second_neighbour_while_escaping_the_first() {
        let first = rect(0.0, 0.0, 640.0, 480.0);
        let second = rect(1000.0, 0.0, 640.0, 480.0);
        let from = rect(0.0, 0.0, 640.0, 480.0);
        assert!(!may_place(
            &from,
            &rect(1000.0, 0.0, 640.0, 480.0),
            &[first, second]
        ));
    }
}
