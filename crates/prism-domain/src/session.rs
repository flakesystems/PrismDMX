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

use crate::{ExecutorId, FeatureGroup, JsonValue, SessionId, ViewId, WindowInstanceId};

/// The kinds of window the canvas can hold.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum WindowType {
    /// Live values per fixture and attribute.
    #[default]
    FixtureSheet,
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
        ExecutorId, FeatureGroup, JsonValue, Session, SessionId, View, ViewId, WindowInstance,
        WindowInstanceId, WindowType,
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
                "encoderBank",
                "executorPage",
                "focusedWindow",
                "id",
                "name",
                "openWindows",
                "programmerPage",
                "programmerParamIndex",
                "selectedExecutor",
            ]
        );
    }

    #[test]
    fn nothing_is_focused_or_selected_in_a_fresh_session() {
        let fresh = Session::new(SessionId::new(1), "Main");
        assert_eq!(fresh.focused_window, None);
        assert_eq!(fresh.selected_executor, None);
        assert_eq!(fresh.encoder_bank, FeatureGroup::Dimmer);
        assert!(fresh.open_windows.is_empty());
        assert!(fresh.command_line.is_empty());
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
    fn all_ten_window_types_exist() {
        let cfg = Config::new();
        assert_eq!(
            WindowType::inline(&cfg),
            "\"FixtureSheet\" | \"SequenceSheet\" | \"Groups\" | \"Viewer3D\" \
             | \"PhaserEditor\" | \"ClockViewer\" | \"CueViewer\" | \"PresetPool\" \
             | \"Patch\" | \"Settings\""
        );
    }
}
