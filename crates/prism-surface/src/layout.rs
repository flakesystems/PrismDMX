//! Where every control of the X-Touch sits on its panel — S59.
//!
//! # A picture is the other way of finding a key
//!
//! The Controls panel is a list of actions, and a list is the right shape for
//! the question an operator usually has: *I want Go on the selected executor,
//! which key is it on?* It is the wrong shape for the other one, which comes up
//! exactly once per desk and matters for a whole evening afterwards: *which keys
//! are still free, and which part of the panel am I filling up?* The owner asked
//! for both (2026-09-20), and this module is what the second one is drawn from.
//!
//! # Why the layout is here and not in the interface
//!
//! It is a fact about a **device**, which makes it the device profile's — the
//! same rule `McuProfile::permanent` and `RESERVED_BUTTONS` already follow, and
//! the same reasoning: a browser has no business knowing what a desk looks like,
//! only how to draw what it is told. A second surface brings its own layout
//! rather than waiting for the interface to learn about it.
//!
//! # These are proportions, not millimetres
//!
//! The owner asked for a drawing to scale, so the X-Touch is recognisably
//! itself rather than a grid of boxes. What that needs is the **shape** of the
//! panel — eight strips down the left half, the button matrix on the right, the
//! jog wheel at the bottom — and the right relative sizes. The units are this
//! module's own and the interface scales them to whatever room it has.
//!
//! The sections are `XTouch.txt`'s, which is the owner's own description of the
//! device: Encoder Assign, View, Function, Modify, Automation, Utility,
//! Transport and Selection. A key in the wrong section would be a drawing that
//! lies about a real panel, so the grouping is what this table is careful about.

use prism_domain::{ControlBox, ControlShape, GlobalButton, PanelLayout, StripButton};

use crate::binding::BoundControl;

/// The X-Touch's panel, in the units this module draws in.
///
/// A little wider than it is tall, which is the device: 430 mm by about 265 mm
/// of front panel, plus the room the jog wheel needs at the bottom.
pub const X_TOUCH_PANEL: PanelLayout = PanelLayout {
    width: 1000,
    height: 520,
    strips: 8,
    strip_pitch: 58,
};

/// A key: the size every button on this panel is drawn at.
const KEY_W: u16 = 46;
const KEY_H: u16 = 22;

/// The left edge of the right-hand button matrix, and the pitch of its columns.
const PANEL_X: u16 = 540;
const COL: u16 = 56;

/// The vertical pitch of its rows.
const ROW: u16 = 28;

/// A key at column `c`, row `r` of the right-hand matrix.
const fn key(c: u16, r: u16) -> ControlBox {
    ControlBox {
        x: PANEL_X + c * COL,
        y: 16 + r * ROW,
        w: KEY_W,
        h: KEY_H,
        shape: ControlShape::Key,
    }
}

/// A key at an outright position, for the clusters a grid cannot hold.
const fn at(x: u16, y: u16) -> ControlBox {
    ControlBox {
        x,
        y,
        w: KEY_W,
        h: KEY_H,
        shape: ControlShape::Key,
    }
}

/// Every panel button and where it is, by section.
///
/// Written out rather than computed from [`GlobalButton::ALL`]'s order: the
/// order is the note map's and the panel is not laid out in it. A table that
/// derived positions from an enum would put the Automation row wherever the
/// enum happened to list it.
const PANEL: [(GlobalButton, ControlBox); 64] = [
    // Flip, which belongs to the **main fader** rather than to the matrix: it
    // sits directly above it on the device, between the eighth strip and the
    // right-hand panel.
    (GlobalButton::Flip, at(478, 176)),
    // Encoder Assign — six keys, two rows of three at the top left of the
    // matrix, which is where they are on the device.
    (GlobalButton::AssignTrack, key(0, 0)),
    (GlobalButton::AssignSend, key(1, 0)),
    (GlobalButton::AssignPan, key(2, 0)),
    (GlobalButton::AssignPlugin, key(3, 0)),
    (GlobalButton::AssignEq, key(4, 0)),
    (GlobalButton::AssignInstrument, key(5, 0)),
    // The display block, top right: the two keys under the seven-segment
    // display. Both are keys nothing is bound to — one reserved, one lampless.
    (GlobalButton::NameValue, key(7, 0)),
    (GlobalButton::SmpteBeats, key(7, 1)),
    // View — Global View and its eight, across two rows.
    (GlobalButton::GlobalView, key(0, 1)),
    (GlobalButton::ViewMidiTracks, key(1, 1)),
    (GlobalButton::ViewInputs, key(2, 1)),
    (GlobalButton::ViewAudioTracks, key(3, 1)),
    (GlobalButton::ViewAudioInstruments, key(4, 1)),
    (GlobalButton::ViewAux, key(0, 2)),
    (GlobalButton::ViewBusses, key(1, 2)),
    (GlobalButton::ViewOutputs, key(2, 2)),
    (GlobalButton::ViewUser, key(3, 2)),
    // Function — F1 to F8 in one row of eight, as on the device.
    (GlobalButton::F1, key(0, 4)),
    (GlobalButton::F2, key(1, 4)),
    (GlobalButton::F3, key(2, 4)),
    (GlobalButton::F4, key(3, 4)),
    (GlobalButton::F5, key(4, 4)),
    (GlobalButton::F6, key(5, 4)),
    (GlobalButton::F7, key(6, 4)),
    (GlobalButton::F8, key(7, 4)),
    // Modify — the four modifier keys.
    (GlobalButton::ModShift, key(0, 6)),
    (GlobalButton::ModOption, key(1, 6)),
    (GlobalButton::ModControl, key(2, 6)),
    (GlobalButton::ModAlt, key(3, 6)),
    // Automation — six, beside them.
    (GlobalButton::AutoRead, key(4, 6)),
    (GlobalButton::AutoWrite, key(5, 6)),
    (GlobalButton::AutoTrim, key(6, 6)),
    (GlobalButton::AutoTouch, key(7, 6)),
    (GlobalButton::AutoLatch, key(4, 7)),
    (GlobalButton::AutoGroup, key(5, 7)),
    // Utility — Save, Undo, Cancel, Enter.
    (GlobalButton::Save, key(0, 7)),
    (GlobalButton::Undo, key(1, 7)),
    (GlobalButton::Cancel, key(2, 7)),
    (GlobalButton::Enter, key(3, 7)),
    // Transport — the upper seven and the lower five, as two rows.
    (GlobalButton::Markers, key(0, 9)),
    (GlobalButton::Nudge, key(1, 9)),
    (GlobalButton::Cycle, key(2, 9)),
    (GlobalButton::Drop, key(3, 9)),
    (GlobalButton::Replace, key(4, 9)),
    (GlobalButton::Click, key(5, 9)),
    (GlobalButton::SoloClear, key(6, 9)),
    (GlobalButton::Rewind, key(0, 10)),
    (GlobalButton::FastForward, key(1, 10)),
    (GlobalButton::Stop, key(2, 10)),
    (GlobalButton::Play, key(3, 10)),
    (GlobalButton::Record, key(4, 10)),
    // Selection — the two bank pairs, and then the cluster around the wheel.
    (GlobalButton::BankLeft, key(0, 12)),
    (GlobalButton::BankRight, key(1, 12)),
    (GlobalButton::ChannelLeft, key(2, 12)),
    (GlobalButton::ChannelRight, key(3, 12)),
    // The two foot-switch jacks. They are on the **back** of the device, so
    // there is no honest place for them on a drawing of the front; they are put
    // at the end of the Selection row because an operator looking for them
    // wants to find them somewhere, and this is the row that is about what is
    // under the hand rather than what is under the eye.
    (GlobalButton::FootSwitch1, key(6, 12)),
    (GlobalButton::FootSwitch2, key(7, 12)),
    // The cursor diamond, with Zoom in the middle of it — which is where it is
    // on the device, and the reason §4.1 says PrismDMX does not make Zoom a
    // modifier for its neighbours. Beside the jog wheel, at the bottom right.
    (GlobalButton::Scrub, at(832, 392)),
    (GlobalButton::CursorUp, at(776, 392)),
    (GlobalButton::CursorLeft, at(720, 420)),
    (GlobalButton::Zoom, at(776, 420)),
    (GlobalButton::CursorRight, at(832, 420)),
    (GlobalButton::CursorDown, at(776, 448)),
];

/// The leftmost strip's controls. A drawing repeats the column
/// [`PanelLayout::strips`] times at [`PanelLayout::strip_pitch`].
const STRIP_X: u16 = 16;

/// Where a control is on the X-Touch, or `None` for one this table has no
/// place for.
///
/// `None` cannot happen for a control [`BoundControl::all`] lists, and
/// `every_control_has_a_place` is what holds that — but it is an `Option`
/// rather than a panic, because this crate does not panic outside its tests and
/// a missing box costs a drawing one key rather than a desk its start-up.
#[must_use]
pub fn box_of(control: BoundControl) -> Option<ControlBox> {
    Some(match control {
        BoundControl::StripEncoder => ControlBox {
            x: STRIP_X + 6,
            y: 16,
            w: 34,
            h: 34,
            shape: ControlShape::Knob,
        },
        BoundControl::StripButton { button } => ControlBox {
            x: STRIP_X,
            y: 92 + strip_row(button) * ROW,
            w: KEY_W,
            h: KEY_H,
            shape: ControlShape::Key,
        },
        BoundControl::StripFader => ControlBox {
            x: STRIP_X + 10,
            y: 214,
            w: 26,
            h: 210,
            shape: ControlShape::Fader,
        },
        BoundControl::MainFader => ControlBox {
            x: 482,
            y: 214,
            w: 26,
            h: 210,
            shape: ControlShape::Fader,
        },
        BoundControl::Jog => ControlBox {
            x: 540,
            y: 390,
            w: 110,
            h: 110,
            shape: ControlShape::Wheel,
        },
        BoundControl::Global { button } => PANEL
            .into_iter()
            .find(|(candidate, _)| *candidate == button)
            .map(|(_, geometry)| geometry)?,
    })
}

/// Which row of the strip a key is on, from the top.
///
/// §2.1's order is Rec, Solo, Mute, Select; the V-Pot push is not a row of its
/// own because it *is* the encoder, so it shares the knob's place.
const fn strip_row(button: StripButton) -> u16 {
    match button {
        StripButton::Rec => 0,
        StripButton::Solo => 1,
        StripButton::Mute => 2,
        StripButton::Select => 3,
        // The push of the V-Pot, drawn where the V-Pot is.
        StripButton::VPotPush => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{PANEL, X_TOUCH_PANEL, box_of};
    use crate::binding::BoundControl;
    use prism_domain::{ControlShape, GlobalButton, StripButton};

    /// **Every control has a place**, which is what lets the drawing be drawn
    /// from the binding table rather than from a second list of what exists.
    #[test]
    fn every_control_has_a_place() {
        for control in BoundControl::all() {
            assert!(
                box_of(control).is_some(),
                "{control} has nowhere to be drawn"
            );
        }
    }

    /// And every panel key appears once: a key listed twice would be drawn
    /// twice and a key listed nowhere would be missing, and neither is loud.
    #[test]
    fn the_panel_lists_every_button_once() {
        assert_eq!(PANEL.len(), GlobalButton::ALL.len());
        for button in GlobalButton::ALL {
            let found = PANEL
                .iter()
                .filter(|(candidate, _)| *candidate == button)
                .count();
            assert_eq!(found, 1, "Global.{button} appears {found} times");
        }
    }

    /// Nothing is drawn off the edge of the panel it is drawn on.
    #[test]
    fn everything_fits_on_the_panel() {
        for control in BoundControl::all() {
            let placed = box_of(control).expect("every control has a place");
            assert!(
                placed.x + placed.w <= X_TOUCH_PANEL.width,
                "{control} runs off the right edge"
            );
            assert!(
                placed.y + placed.h <= X_TOUCH_PANEL.height,
                "{control} runs off the bottom"
            );
        }
        // And the eight strips fit beside the main fader, which is the one
        // measurement the strip pitch has to agree with.
        let strip = box_of(BoundControl::StripFader).expect("the strip fader has a place");
        let main = box_of(BoundControl::MainFader).expect("the main fader has a place");
        let rightmost =
            strip.x + u16::from(X_TOUCH_PANEL.strips - 1) * X_TOUCH_PANEL.strip_pitch + strip.w;
        assert!(
            rightmost <= main.x,
            "the eighth strip at {rightmost} overlaps the main fader at {}",
            main.x
        );
    }

    /// No two panel keys overlap. A drawing where two keys sit on top of each
    /// other is one an operator cannot click.
    #[test]
    fn no_two_keys_are_in_the_same_place() {
        for (one, first) in PANEL {
            for (two, second) in PANEL {
                if one == two {
                    continue;
                }
                let apart = first.x + first.w <= second.x
                    || second.x + second.w <= first.x
                    || first.y + first.h <= second.y
                    || second.y + second.h <= first.y;
                assert!(apart, "Global.{one} and Global.{two} overlap");
            }
        }
    }

    /// The shapes are what a drawing needs to tell the controls apart.
    #[test]
    fn a_fader_is_a_fader_and_the_wheel_is_a_wheel() {
        let shape = |control| box_of(control).map(|placed| placed.shape);
        assert_eq!(shape(BoundControl::StripFader), Some(ControlShape::Fader));
        assert_eq!(shape(BoundControl::MainFader), Some(ControlShape::Fader));
        assert_eq!(shape(BoundControl::StripEncoder), Some(ControlShape::Knob));
        assert_eq!(shape(BoundControl::Jog), Some(ControlShape::Wheel));
        assert_eq!(
            shape(BoundControl::Global {
                button: GlobalButton::F1
            }),
            Some(ControlShape::Key)
        );
        assert_eq!(
            shape(BoundControl::StripButton {
                button: StripButton::Solo
            }),
            Some(ControlShape::Key)
        );
    }
}
