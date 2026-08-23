//! Session state — the operating state the console shares with every client.
//!
//! `ARCHITECTURE_SPEC.md` §4 and decision **D11**: the active view, the open
//! windows, the executor page and the selection live in the daemon, not in the
//! UI. That is what lets the X-Touch switch a view while no client is running,
//! and what makes a UI restart an ordinary reconnect.
//!
//! §4.2 lists what deliberately does *not* live here — monitor assignment,
//! scroll position, camera, zoom. They are legitimately per-screen.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    CueEdit, ExecutorId, FeatureGroup, JsonValue, SequenceId, SessionId, ViewId, WindowInstanceId,
};

/// The kinds of window the canvas can hold.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
pub enum WindowType {
    /// Live values per fixture and attribute.
    #[default]
    FixtureSheet,
    /// The DMX output itself, universe by universe and channel by channel.
    ///
    /// **Added in S25**, and the reason is worth writing down: S24 built the
    /// level view — the telemetry canvas — and the ten window types
    /// `ARCHITECTURE_SPEC.md` §6 listed had no name for it. A
    /// [`FixtureSheet`](Self::FixtureSheet) shows what the *fixtures* are set
    /// to, which is the show's view of the rig; this shows what is going down
    /// the *cable*, which is the only view that can disagree with it. A console
    /// has both, and an operator patching a rig needs the second one.
    DmxSheet,
    /// Cues of a sequence.
    SequenceSheet,
    /// Group pool.
    Groups,
    /// 3D stage view.
    Viewer3D,
    /// Phaser (effect) editor.
    PhaserEditor,
    /// Clocks and timecode.
    ClockViewer,
    /// The cue currently running on an executor.
    CueViewer,
    /// A preset pool; which pool is in `params`.
    PresetPool,
    /// The patch.
    Patch,
    /// Settings.
    Settings,
}

/// One open window on the canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct WindowInstance {
    /// Identifies this window within the session.
    pub instance_id: WindowInstanceId,
    /// What the window shows.
    ///
    /// Named `window_type` in Rust because `type` is a keyword; the wire name
    /// stays `type`, as specified.
    #[serde(rename = "type")]
    pub window_type: WindowType,
    /// Left edge on the canvas.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub x: f64,
    /// Top edge on the canvas.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub y: f64,
    /// Width.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub w: f64,
    /// Height.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::finite_f64()")
    )]
    pub h: f64,
    /// Window-specific parameters, e.g. which preset pool is shown.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_map(2)")
    )]
    pub params: BTreeMap<String, JsonValue>,
}

/// A stored canvas layout, selected from the View Selector Bar or the console.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// View number.
    pub id: ViewId,
    /// Operator-facing name.
    pub name: String,
    /// The windows this view restores.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub windows: Vec<WindowInstance>,
}

/// Operating state shared by every attached client (**D11**).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Session number. V1 has exactly one.
    pub id: SessionId,
    /// Session name. V1 uses "Main".
    pub name: String,
    /// The canvas layout currently shown.
    pub active_view_id: ViewId,
    /// Windows currently open, in stacking order.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub open_windows: Vec<WindowInstance>,
    /// The focused window, if any.
    pub focused_window: Option<WindowInstanceId>,
    /// Executor page shown on the fader bank.
    pub executor_page: u32,
    /// The executor the main fader, Flip and transport act on.
    pub selected_executor: Option<ExecutorId>,
    /// The cue list a store with no sequence named goes into (S39).
    ///
    /// **S39's decision.** Until it existed the sheets followed
    /// `selected_executor`'s sequence and S28 marked the assumption rather than
    /// making it permanent; `ARCHITECTURE_SPEC.md` §4.4 named this session as
    /// the one that would settle it. It is settled by adding the field, because
    /// the alternative cannot answer `Store Cue 5` typed with no executor
    /// selected, and cannot reach a cue list nobody has put on a fader.
    ///
    /// Deliberately **not** coupled to `selected_executor`: an operator
    /// programming cue list 7 while executor 3 plays the show is the ordinary
    /// case on a console, not the edge case, and a desk that moved this every
    /// time a fader was selected would store into whatever was last touched.
    ///
    /// `#[serde(default)]` because a `.prism` file keeps each session as an
    /// opaque MessagePack blob (S15), so a file written before this field
    /// existed does not carry it — the rule `Executor::speed` found in S34.
    #[serde(default)]
    pub selected_sequence: Option<SequenceId>,
    /// Which cue the programmer is editing, and whether it has moved since
    /// (S39) — the state an Update key blinks on. See [`CueEdit`].
    ///
    /// `#[serde(default)]` for [`Self::selected_sequence`]'s reason. Reopening a
    /// show is not resuming an edit: the programmer is deliberately not in the
    /// file either (`prism_core::Programmer`), so an `editingCue` restored
    /// beside an empty programmer would blink an Update that had nothing to put
    /// back.
    #[serde(default)]
    pub editing_cue: Option<CueEdit>,
    /// The encoder bank currently selected.
    pub encoder_bank: FeatureGroup,
    /// Programmer page (Zoom up/down on the console).
    pub programmer_page: u32,
    /// Which parameter the jog wheel turns (Zoom left/right).
    pub programmer_param_index: u32,
    /// Contents of the command line.
    pub command_line: String,
}

impl Session {
    /// A session with nothing open, nothing selected and view 1 active.
    #[must_use]
    pub fn new(id: SessionId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            active_view_id: ViewId::new(1),
            open_windows: Vec::new(),
            focused_window: None,
            executor_page: 0,
            selected_executor: None,
            selected_sequence: None,
            editing_cue: None,
            encoder_bank: FeatureGroup::Dimmer,
            programmer_page: 0,
            programmer_param_index: 0,
            command_line: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CueEdit, ExecutorId, FeatureGroup, JsonValue, SequenceId, Session, SessionId, View, ViewId,
        WindowInstance, WindowInstanceId, WindowType,
    };
    use std::collections::BTreeMap;
    use ts_rs::{Config, TS};

    fn window() -> WindowInstance {
        WindowInstance {
            instance_id: WindowInstanceId::new(1),
            window_type: WindowType::PresetPool,
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 300.0,
            params: BTreeMap::from([("pool".to_owned(), JsonValue::String("Color".to_owned()))]),
        }
    }

    fn session() -> Session {
        Session {
            id: SessionId::new(1),
            name: "Main".to_owned(),
            active_view_id: ViewId::new(3),
            open_windows: vec![window()],
            focused_window: Some(WindowInstanceId::new(1)),
            executor_page: 0,
            selected_executor: Some(ExecutorId::new(2)),
            selected_sequence: Some(SequenceId::new(7)),
            editing_cue: None,
            encoder_bank: FeatureGroup::Dimmer,
            programmer_page: 0,
            programmer_param_index: 0,
            command_line: String::new(),
        }
    }

    #[test]
    fn window_instance_keeps_the_reserved_field_name_type() {
        let json = serde_json::to_value(window()).unwrap();
        assert_eq!(json["type"], "PresetPool");
        assert_eq!(json["instanceId"], 1);
        assert_eq!(json["params"]["pool"], "Color");
    }

    #[test]
    fn session_matches_the_wire_shape() {
        let json = serde_json::to_value(session()).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "activeViewId",
                "commandLine",
                "editingCue",
                "encoderBank",
                "executorPage",
                "focusedWindow",
                "id",
                "name",
                "openWindows",
                "programmerPage",
                "programmerParamIndex",
                "selectedExecutor",
                "selectedSequence",
            ]
        );
    }

    #[test]
    fn nothing_is_focused_or_selected_in_a_fresh_session() {
        let fresh = Session::new(SessionId::new(1), "Main");
        assert_eq!(fresh.focused_window, None);
        assert_eq!(fresh.selected_executor, None);
        // S39's two, and both start empty: a desk that opened on a cue list
        // nobody chose would store into it.
        assert_eq!(fresh.selected_sequence, None);
        assert_eq!(fresh.editing_cue, None);
        assert_eq!(fresh.encoder_bank, FeatureGroup::Dimmer);
        assert!(fresh.open_windows.is_empty());
        assert!(fresh.command_line.is_empty());
    }

    /// **A session written before S39 reads back with neither field**, which is
    /// how an opaque-document schema grows: `.prism` keeps the session as a
    /// MessagePack blob (S15), so a field added later is one older files do not
    /// carry. `Executor::speed` found this in S34 within a minute.
    #[test]
    fn a_session_written_before_s39_opens_with_nothing_selected_and_nothing_edited() {
        let mut json = serde_json::to_value(session()).unwrap();
        let object = json.as_object_mut().unwrap();
        object.remove("selectedSequence");
        object.remove("editingCue");
        let read: Session = serde_json::from_value(json).unwrap();
        assert_eq!(read.selected_sequence, None);
        assert_eq!(read.editing_cue, None);
        assert_eq!(read.selected_executor, Some(ExecutorId::new(2)));
    }

    /// The update state is *which cue* and *whether it has moved*, and both
    /// travel.
    #[test]
    fn a_cue_edit_names_the_cue_and_says_whether_it_has_moved() {
        let edit = CueEdit {
            sequence_id: SequenceId::new(3),
            cue_number: "1.5".to_owned(),
            modified: true,
        };
        assert_eq!(
            serde_json::to_string(&edit).unwrap(),
            r#"{"sequenceId":3,"cueNumber":"1.5","modified":true}"#
        );
        let session = Session {
            editing_cue: Some(edit.clone()),
            ..session()
        };
        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["editingCue"]["cueNumber"], "1.5");
        assert_eq!(
            serde_json::from_value::<Session>(json).unwrap().editing_cue,
            Some(edit)
        );
    }

    #[test]
    fn a_view_is_a_named_set_of_windows() {
        let view = View {
            id: ViewId::new(3),
            name: "Programming".to_owned(),
            windows: vec![window()],
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["id"], 3);
        assert_eq!(json["windows"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn the_dmx_sheet_is_a_window_like_any_other() {
        // S25 added it, and a variant no test ever constructs is a variant
        // whose wire name nobody has checked. `DmxSheet` is what the level
        // view lives in, so this is the name `openWindows` will carry in every
        // saved show that has one.
        let window = WindowInstance {
            window_type: WindowType::DmxSheet,
            ..window()
        };
        let json = serde_json::to_value(&window).unwrap();
        assert_eq!(json["type"], "DmxSheet");
        assert_eq!(
            serde_json::from_value::<WindowInstance>(json).unwrap(),
            window
        );
    }

    #[test]
    fn all_eleven_window_types_exist() {
        // `ARCHITECTURE_SPEC.md` §6's ten, and `DmxSheet` — S25's, for the level
        // view S24 built and none of the ten named.
        let cfg = Config::new();
        assert_eq!(
            WindowType::inline(&cfg),
            "\"FixtureSheet\" | \"DmxSheet\" | \"SequenceSheet\" | \"Groups\" \
             | \"Viewer3D\" | \"PhaserEditor\" | \"ClockViewer\" | \"CueViewer\" \
             | \"PresetPool\" | \"Patch\" | \"Settings\""
        );
    }
}

impl WindowType {
    /// Every kind of window, so a test — and the control editor's chooser —
    /// walks the whole set rather than the ones somebody remembered.
    ///
    /// Added in S38, which needed the list twice: an *open this window* binding
    /// offers it at run time, and it is this enum's proptest strategy
    /// (`crate::arb::arbitrary_from_list`).
    pub const ALL: [Self; 11] = [
        Self::FixtureSheet,
        Self::DmxSheet,
        Self::SequenceSheet,
        Self::Groups,
        Self::Viewer3D,
        Self::PhaserEditor,
        Self::ClockViewer,
        Self::CueViewer,
        Self::PresetPool,
        Self::Patch,
        Self::Settings,
    ];
}

#[cfg(any(test, feature = "proptest"))]
crate::arb::arbitrary_from_list!(WindowType);
