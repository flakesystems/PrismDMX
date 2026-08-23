//! Session state: the operating state the console shares with every client.
//!
//! `ARCHITECTURE_SPEC.md` §4 and decision **D11**. The X-Touch does not
//! remote-control the user interface; it changes state *here*, in the daemon,
//! and the daemon broadcasts a `Delta::SessionPatch` to every attached client.
//! That is what makes a view switched from the console while the UI was closed
//! simply part of the snapshot the UI receives when it comes back — and it is
//! why this state exists and goes on existing when no client is connected at
//! all.
//!
//! [`SessionState::apply`] is [`Show::apply`](crate::Show::apply) again, for the
//! other half of the protocol: it validates, it applies, it answers with the
//! deltas that describe what changed, and a rejection leaves the session
//! byte-identical. The two appliers are exhaustive over
//! `prism_domain::Command` in opposite directions, so a command added to the
//! protocol is a compile error in both rather than a silent rejection in
//! either. `Command::is_session_command` is the predicate the daemon routes on
//! and [`crate::ShowFile::apply`] is that routing, written down.
//!
//! # Where the views live, and why it is here
//!
//! A [`View`] is a stored canvas layout; `open_windows` is the canvas as it is
//! now. `StoreView` writes the one into the other and `SelectView` loads it
//! back. Which of the show and the session owns the library of them is a
//! decision this session had to make, and three things point the same way:
//!
//! - `docs/IPC_PROTOCOL.md` §6 annotates `SessionPatch` with "views, windows,
//!   pages, selection". A view stored into the show would travel as a
//!   `ShowPatch`, which contradicts both that line and the exit criterion that
//!   every §4.4 command emits a `SessionPatch`.
//! - `Command::is_undoable` already excludes every session command from the
//!   Oops journal (S14). A view stored into the show would be a show edit the
//!   journal deliberately cannot undo, which is exactly the kind of asymmetry
//!   that turns into a bug report about undo.
//! - §4.4's closing note gives the reason multi-session exists at all: "two
//!   operators can work with independent views". Independent views need a view
//!   library per session.
//!
//! # What deliberately is not here
//!
//! §4.2's list — monitor assignment of a window, scroll position, hover and
//! drag state, the 3D viewer camera, the UI zoom level. They are legitimately
//! different per screen and would be actively annoying as shared state. Their
//! absence is structural: the session is `prism_domain::Session` plus the views
//! it selects between, and neither type has a field for any of them.
//!
//! The one place that absence is not structural is
//! [`WindowInstance::params`](prism_domain::WindowInstance::params), which is an
//! open bag by design — it is how a preset pool window says which pool it
//! shows. A client could put its scroll position in there and it would become
//! shared state. So the bag is checked at the door: see
//! [`SessionError::ClientLocalParam`].

use core::fmt;
use std::collections::BTreeMap;

use prism_domain::{
    Command, CueEdit, Delta, EXECUTORS_PER_PAGE, ExecutorId, FeatureGroup, JsonPatchOp, JsonValue,
    ObjectRef, OverwriteMode, ParamDirection, SequenceId, Session, SessionId, View, ViewId,
    WindowInstance, WindowInstanceId, WindowType,
};
use serde::{Deserialize, Serialize};

use crate::command::Applied;
use crate::show::{pointer, project};

/// Wire name of the session itself within the session document.
pub(crate) const SESSION: &str = "session";
/// Wire name of the stored views.
pub(crate) const VIEWS: &str = "views";

/// Wire name of `Session::active_view_id`.
const ACTIVE_VIEW_ID: &str = "activeViewId";
/// Wire name of `Session::open_windows`.
const OPEN_WINDOWS: &str = "openWindows";
/// Wire name of `Session::focused_window`.
const FOCUSED_WINDOW: &str = "focusedWindow";
/// Wire name of `Session::executor_page`.
const EXECUTOR_PAGE: &str = "executorPage";
/// Wire name of `Session::selected_executor`.
const SELECTED_EXECUTOR: &str = "selectedExecutor";
/// Wire name of `Session::selected_sequence`.
const SELECTED_SEQUENCE: &str = "selectedSequence";
/// Wire name of `Session::editing_cue`.
const EDITING_CUE: &str = "editingCue";
/// Wire name of `Session::encoder_bank`.
const ENCODER_BANK: &str = "encoderBank";
/// Wire name of `Session::programmer_page`.
const PROGRAMMER_PAGE: &str = "programmerPage";
/// Wire name of `Session::programmer_param_index`.
const PROGRAMMER_PARAM_INDEX: &str = "programmerParamIndex";
/// Wire name of `Session::command_line`.
const COMMAND_LINE: &str = "commandLine";

/// Where a window opened from the console appears, and how big it is.
///
/// `OpenWindow` carries no geometry — §4.4's command opens a window, it does not
/// lay one out — and the daemon has no screen to centre one on, so it has to
/// choose. These are canvas units, which the client scales; a window is moved
/// afterwards through [`SessionState::place_window`].
const DEFAULT_WINDOW: (f64, f64, f64, f64) = (0.0, 0.0, 640.0, 480.0);

/// The words §4.2 keeps client-local, as they would appear in a parameter key.
///
/// Matched as a substring of the lower-cased key, so `scrollTop`,
/// `cameraPosition` and `uiZoom` are all caught. It is a deny-list and it is
/// therefore not a proof — a client determined to share its scroll position can
/// call it something else — but it turns the one open bag in the session from
/// an invitation into a refusal, and the refusal names the rule.
const CLIENT_LOCAL_PARAM_KEYS: [&str; 7] = [
    "monitor", "screen", "scroll", "hover", "drag", "camera", "zoom",
];

/// Why a session edit was refused.
///
/// Every variant leaves the session exactly as it was: an edit is validated and
/// encoded against a copy, and the copy is only swapped in once the operations
/// describing it exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// A view number that has never been stored.
    UnknownView(ViewId),
    /// The only stored view cannot be deleted.
    ///
    /// `activeViewId` names a view from the first moment ([`SessionState::new`])
    /// and [`crate::ShowStore`] refuses a file whose active view is not stored.
    /// A session with no views at all could satisfy neither, so the last one is
    /// a floor rather than something the caller has to remember.
    LastView(ViewId),
    /// A window number that is not open.
    UnknownWindow(WindowInstanceId),
    /// An executor page whose slots cannot all be addressed.
    ///
    /// `ExecutorId::from_page_and_slot` saturates rather than wrapping (S1), so
    /// a page this large does not have eight distinct executors on it — every
    /// slot would name the same one.
    ExecutorPageOutOfRange {
        /// The page asked for.
        page: u32,
    },
    /// A window parameter naming something `ARCHITECTURE_SPEC.md` §4.2 keeps
    /// client-local.
    ClientLocalParam {
        /// The offending key.
        key: String,
    },
    /// Every window number is taken. Reachable only from a hand-edited file
    /// that already numbers a window `u32::MAX`.
    NoWindowNumberLeft,
    /// A show command reached the session applier. `Command::is_session_command`
    /// is the predicate that routes them apart.
    NotASessionCommand,
    /// The session could not be projected into JSON — a non-finite float that
    /// reached a window's geometry, which `prism-domain` refuses in both
    /// directions (S1).
    NotRepresentable(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownView(id) => write!(f, "no view {id} has been stored"),
            Self::LastView(id) => write!(
                f,
                "view {id} is the only one stored, and the session must always have one"
            ),
            Self::UnknownWindow(id) => write!(f, "window {id} is not open"),
            Self::ExecutorPageOutOfRange { page } => write!(
                f,
                "executor page {page} is out of range: its slots would not be {EXECUTORS_PER_PAGE} \
                 different executors"
            ),
            Self::ClientLocalParam { key } => write!(
                f,
                "window parameter {key:?} is client-local state, which the session does not carry"
            ),
            Self::NoWindowNumberLeft => write!(f, "no window number is free"),
            Self::NotASessionCommand => write!(f, "this is a show command, not a session command"),
            Self::NotRepresentable(reason) => {
                write!(f, "the session cannot be encoded: {reason}")
            }
        }
    }
}

impl core::error::Error for SessionError {}

/// The operating state of one session, and the views it selects between.
///
/// This is the document a `Delta::SessionPatch` points into: `/session/...` is
/// the §4.1 state and `/views/<number>` is one stored layout. A client mirrors
/// it with [`SessionMirror`](crate::SessionMirror).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    /// The §4.1 state, exactly as `ARCHITECTURE_SPEC.md` writes it.
    session: Session,
    /// The stored layouts, keyed by view number.
    views: BTreeMap<ViewId, View>,
    /// Whether a stored view has not been written to disk. Not session content:
    /// it describes this run of the daemon, like `Show`'s own flag.
    #[serde(skip)]
    dirty: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    /// V1's session: one session called "Main", one empty view, nothing open.
    ///
    /// The view exists so that `activeViewId` names something from the first
    /// moment. A session whose active view is not a view is a dangling
    /// reference, and the console can reach `SelectView` before anything has
    /// ever been stored.
    #[must_use]
    pub fn new() -> Self {
        let session = Session::new(SessionId::new(1), "Main");
        let view = View {
            id: session.active_view_id,
            name: "View 1".to_owned(),
            windows: Vec::new(),
        };
        Self {
            views: BTreeMap::from([(view.id, view)]),
            session,
            dirty: false,
        }
    }

    /// The session a `.prism` file's rows add up to.
    ///
    /// The caller has checked the two invariants this type is otherwise
    /// responsible for — that the active view exists and that the focused
    /// window is open — because they are what a *file* can be wrong about, and
    /// [`crate::ShowStore`] is what has the row to name in the complaint.
    pub(crate) const fn from_parts(session: Session, views: BTreeMap<ViewId, View>) -> Self {
        Self {
            session,
            views,
            // Not session content: a session that has just been read has
            // nothing unstored in it.
            dirty: false,
        }
    }

    // -- queries ----------------------------------------------------------

    /// The §4.1 state itself.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// The stored views, in number order.
    pub fn views(&self) -> impl Iterator<Item = &View> {
        self.views.values()
    }

    /// One stored view.
    #[must_use]
    pub fn view(&self, id: ViewId) -> Option<&View> {
        self.views.get(&id)
    }

    /// One open window.
    #[must_use]
    pub fn window(&self, id: WindowInstanceId) -> Option<&WindowInstance> {
        self.session
            .open_windows
            .iter()
            .find(|window| window.instance_id == id)
    }

    /// Whether a view has been stored since the last save.
    ///
    /// Only [`SessionState::store_view`] sets this. Paging the fader bank,
    /// opening a window or moving the encoder bank is *operating* the desk, and
    /// a Save LED that lit because somebody paged the faders would teach an
    /// operator to ignore it — the same reasoning S11 applied to an executor
    /// advancing a cue. Storing a view is the one §4.4 command that is
    /// authoring.
    ///
    /// The LED itself is [`ShowFile::is_dirty`](crate::ShowFile::is_dirty):
    /// one lamp, two sources.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Records that the session has been written to disk.
    ///
    /// Returns whether the flag actually changed.
    pub const fn mark_saved(&mut self) -> bool {
        let changed = self.dirty;
        self.dirty = false;
        changed
    }

    /// The session as JSON: the document `SessionPatch` operations point into.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] if a value cannot be encoded.
    pub fn to_json(&self) -> Result<JsonValue, SessionError> {
        project(self).map_err(SessionError::NotRepresentable)
    }

    // -- the applier ------------------------------------------------------

    /// Validates a session command and applies it.
    ///
    /// A command that changes nothing produces no delta at all rather than an
    /// empty one: a `SessionPatch` with no operations is a broadcast to every
    /// client that says nothing. Turning the jog wheel left at the first
    /// parameter is the ordinary case.
    ///
    /// # Errors
    ///
    /// [`SessionError`] if the command cannot be applied. **The session is
    /// unchanged after any error**, and `tests/session_commands.rs` asserts it
    /// on the serialised bytes rather than on the claim.
    pub fn apply(&mut self, command: &Command) -> Result<Applied, SessionError> {
        let ops = match command {
            Command::SelectView { view_id } => self.select_view(*view_id)?,
            Command::StoreView { view_id, name } => self.store_view(*view_id, name)?,
            // S40's four generic verbs, for the one of their six targets that
            // is session state. `Command::is_session_command` reads the target
            // and routes them here; anything else in them is the show's, and
            // reaching this applier with one is the mirror image of a show
            // command arriving here at all.
            Command::Delete {
                target: ObjectRef::View { view_id },
            } => self.delete_view(*view_id)?,
            Command::Label {
                target: ObjectRef::View { view_id },
                name,
            } => self.rename_view(*view_id, name)?,
            Command::Copy {
                from: ObjectRef::View { view_id: from },
                to: ObjectRef::View { view_id: to },
                mode,
            } => self.copy_view(*from, *to, *mode)?,
            Command::Move {
                from: ObjectRef::View { view_id: from },
                to: ObjectRef::View { view_id: to },
                ..
            } => self.move_view(*from, *to)?,
            Command::OpenWindow { window, params } => self.open_window(*window, params.as_ref())?,
            Command::CloseWindow { instance_id } => self.close_window(*instance_id)?,
            Command::FocusWindow { instance_id } => self.focus_window(*instance_id)?,
            Command::PlaceWindow {
                instance_id,
                x,
                y,
                w,
                h,
            } => self.place_window(*instance_id, *x, *y, *w, *h)?,
            Command::SetExecutorPage { page } => self.set_executor_page(*page)?,
            Command::SelectExecutor { executor_id } => self.select_executor(Some(*executor_id))?,
            Command::SelectSequence { sequence_id } => self.select_sequence(Some(*sequence_id))?,
            Command::SetEncoderBank { group } => self.set_encoder_bank(*group)?,
            Command::SetProgrammerPage { page } => self.set_programmer_page(*page)?,
            Command::SelectProgrammerParam { direction } => {
                self.select_programmer_param(*direction)?
            }
            Command::CommandLineInput { text } => self.set_command_line(text)?,
            // The twenty-four show commands, named rather than caught by a wildcard,
            // so this match is exhaustive and a command added to the protocol
            // is a compile error here as well as in `Show::apply`.
            Command::SelectFixtures { .. }
            | Command::SelectGroup { .. }
            | Command::SetAttribute { .. }
            | Command::ApplyPreset { .. }
            | Command::ClearProgrammer
            | Command::StoreCue { .. }
            | Command::StoreSequence { .. }
            | Command::EditCue { .. }
            | Command::Update
            | Command::ExecutorGo { .. }
            | Command::ExecutorOff { .. }
            | Command::ExecutorButton { .. }
            | Command::SetExecutorMaster { .. }
            | Command::PatchFixture { .. }
            | Command::UnpatchFixture { .. }
            | Command::RenumberFixture { .. }
            | Command::EmbedFixtureType { .. }
            | Command::Oops
            | Command::Redo
            | Command::StorePreset { .. }
            | Command::StoreGroup { .. }
            | Command::ExecutorOn { .. }
            | Command::Goto { .. }
            | Command::Delete { .. }
            | Command::Copy { .. }
            | Command::Move { .. }
            | Command::Label { .. }
            | Command::Color { .. }
            | Command::SetCueProperty { .. }
            | Command::AssignExecutor { .. }
            | Command::SaveShow
            | Command::SaveShowAs { .. }
            | Command::OpenShow { .. }
            | Command::NewShow { .. }
            | Command::ExportShow { .. }
            | Command::ImportShow { .. } => return Err(SessionError::NotASessionCommand),
            // S33's four: this machine's rig, which is neither the show's nor
            // the session's. `crate::outputs` has the argument, and
            // `Command::is_machine_command` is what a daemon routes on.
            Command::AddOutput { .. }
            | Command::ConfigureOutput { .. }
            | Command::RemoveOutput { .. }
            | Command::SetOutputEnabled { .. }
            | Command::SetSurfacePort { .. }
            | Command::ConfigureMachine { .. } => {
                return Err(SessionError::NotASessionCommand);
            }
        };
        Ok(if ops.is_empty() {
            Applied::default()
        } else {
            Applied {
                deltas: vec![Delta::SessionPatch { ops }],
                effects: Vec::new(),
            }
        })
    }

    // -- edits ------------------------------------------------------------

    /// Loads a stored view onto the canvas.
    ///
    /// Always loads, even when the view is already active: pressing the view
    /// button again is how an operator gets their layout back after moving
    /// things around, and `activeViewId` names the view last *selected* rather
    /// than a promise that the canvas still equals it.
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownView`] if no such view has been stored.
    pub fn select_view(&mut self, id: ViewId) -> Result<Vec<JsonPatchOp>, SessionError> {
        let Some(view) = self.views.get(&id) else {
            return Err(SessionError::UnknownView(id));
        };
        let windows = view.windows.clone();
        let mut next = self.session.clone();
        next.active_view_id = id;
        next.focused_window = windows.last().map(|window| window.instance_id);
        next.open_windows = windows;
        self.commit(next)
    }

    /// Stores the current canvas as a view.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] if a window cannot be encoded.
    pub fn store_view(&mut self, id: ViewId, name: &str) -> Result<Vec<JsonPatchOp>, SessionError> {
        let view = View {
            id,
            name: name.to_owned(),
            windows: self.session.open_windows.clone(),
        };
        if self.views.get(&id) == Some(&view) {
            return Ok(Vec::new());
        }
        let existed = self.views.contains_key(&id);
        let op = put(pointer(VIEWS, &id.to_string()), &view, existed)?;
        self.views.insert(id, view);
        self.dirty = true;
        Ok(vec![op])
    }

    /// Renames a stored view, leaving its windows exactly as they are.
    ///
    /// Deliberately not [`SessionState::store_view`] with the old name: storing
    /// overwrites the layout with whatever is on the canvas now, and an operator
    /// correcting a typo has not asked for that.
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownView`] if no such view has been stored.
    pub fn rename_view(
        &mut self,
        id: ViewId,
        name: &str,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let Some(view) = self.views.get(&id) else {
            return Err(SessionError::UnknownView(id));
        };
        if view.name == name {
            return Ok(Vec::new());
        }
        let renamed = View {
            id,
            name: name.to_owned(),
            windows: view.windows.clone(),
        };
        let op = put(pointer(VIEWS, &id.to_string()), &renamed, true)?;
        self.views.insert(id, renamed);
        self.dirty = true;
        Ok(vec![op])
    }

    /// Deletes a stored view.
    ///
    /// # What the canvas shows afterwards is decided here
    ///
    /// Deleting the **active** view leaves `activeViewId` naming something that
    /// no longer exists, and that has to be resolved by the daemon rather than
    /// by whichever client happened to send the command — otherwise two screens
    /// would answer it differently. The rule: the neighbour along the bar, the
    /// one **before** it if there is one and the one after it otherwise, is
    /// selected exactly as [`SessionState::select_view`] would have selected it.
    /// So the canvas is a stored layout and never an arbitrary leftover.
    ///
    /// Deleting a view that is *not* active leaves the canvas alone.
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownView`] if no such view has been stored, or
    /// [`SessionError::LastView`] if it is the only one.
    pub fn delete_view(&mut self, id: ViewId) -> Result<Vec<JsonPatchOp>, SessionError> {
        if !self.views.contains_key(&id) {
            return Err(SessionError::UnknownView(id));
        }
        if self.views.len() == 1 {
            return Err(SessionError::LastView(id));
        }
        // The successor is chosen before anything is removed, so a failure to
        // encode leaves the session untouched — the rule `commit` exists for.
        let successor = (self.session.active_view_id == id).then(|| {
            self.neighbour(id, ParamDirection::Prev)
                .or_else(|| self.neighbour(id, ParamDirection::Next))
        });
        let mut ops = vec![JsonPatchOp::Remove {
            path: pointer(VIEWS, &id.to_string()),
        }];
        if let Some(Some(next_id)) = successor {
            // `select_view` reads `self.views`, so the removal happens first —
            // and the view being selected is not the one being removed.
            let windows = self.views.get(&next_id).map(|view| view.windows.clone());
            self.views.remove(&id);
            if let Some(windows) = windows {
                let mut next = self.session.clone();
                next.active_view_id = next_id;
                next.focused_window = windows.last().map(|window| window.instance_id);
                next.open_windows = windows;
                ops.extend(self.commit(next)?);
            }
        } else {
            self.views.remove(&id);
        }
        self.dirty = true;
        Ok(ops)
    }

    /// Copies one stored view onto another number — S40's `Copy View 1 View 4`.
    ///
    /// `Merge` adds the source's windows to the destination's and `Override`
    /// replaces them. A destination that does not exist is created and takes the
    /// source's name; one that exists keeps its own, because a copy onto a view
    /// somebody has named is not a rename — that is `Label`'s.
    ///
    /// **Merging two layouts is a union of windows and nothing cleverer.** Two
    /// windows of the same type on one canvas is an ordinary thing to want (two
    /// preset pools, two sheets), so a merge does not try to match them up; what
    /// it does do is give every incoming window a **fresh instance number**, or
    /// the two layouts would disagree about which window is which.
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownView`] for a source that is not stored, or
    /// [`SessionError::NotRepresentable`].
    pub fn copy_view(
        &mut self,
        from: ViewId,
        to: ViewId,
        mode: OverwriteMode,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let Some(source) = self.views.get(&from).cloned() else {
            return Err(SessionError::UnknownView(from));
        };
        if from == to {
            return Ok(Vec::new());
        }
        let existing = self.views.get(&to).cloned();
        let windows = match (&existing, mode) {
            (Some(existing), OverwriteMode::Merge) => {
                let mut windows = existing.windows.clone();
                let mut next = windows
                    .iter()
                    .map(|window| window.instance_id.get())
                    .max()
                    .map_or(1, |highest| highest.saturating_add(1));
                for window in &source.windows {
                    windows.push(WindowInstance {
                        instance_id: WindowInstanceId::new(next),
                        ..window.clone()
                    });
                    next = next.saturating_add(1);
                }
                windows
            }
            _ => source.windows.clone(),
        };
        let view = View {
            id: to,
            name: existing
                .as_ref()
                .map_or_else(|| source.name.clone(), |view| view.name.clone()),
            windows,
        };
        let op = put(pointer(VIEWS, &to.to_string()), &view, existing.is_some())?;
        self.views.insert(to, view);
        self.dirty = true;
        Ok(vec![op])
    }

    /// Exchanges two stored views — S40's `Move View 1 View 3`.
    ///
    /// **The contents swap and the numbers stay put**, which is S35's decision
    /// carried forward: a view library's order *is* its numbers, because `views`
    /// is keyed by number, the View Selector Bar draws in number order and
    /// `Channel ◀▶` steps from one number to the next. Recording an order beside
    /// the numbers would give two things that can disagree, and the disagreement
    /// an operator would meet is the console stepping to a view other than the
    /// one drawn next.
    ///
    /// The price is the one S35 named and accepted: after a move, `SelectView 3`
    /// names a different layout, and an F-key bound to a view number reaches
    /// whatever now sits in that place.
    ///
    /// > **It was relative until S40** (`Prev`/`Next`), and one absolute form
    /// > covers both: the bar knows its neighbour's number and writes the line,
    /// > which is `ARCHITECTURE_SPEC.md` §4.5 entire. Two commands for one act
    /// > would have been the second grammar S40 exists to remove.
    ///
    /// A destination that is not stored is not an error: the source takes that
    /// number and leaves the one it came from, which is a *move* rather than a
    /// swap and is what an operator asking for an empty place means.
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownView`] if the source has not been stored.
    pub fn move_view(
        &mut self,
        id: ViewId,
        other: ViewId,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        if !self.views.contains_key(&id) {
            return Err(SessionError::UnknownView(id));
        }
        if id == other {
            return Ok(Vec::new());
        }
        if !self.views.contains_key(&other) {
            let Some(here) = self.views.get(&id).cloned() else {
                return Err(SessionError::UnknownView(id));
            };
            let moved = View { id: other, ..here };
            let mut ops = vec![put(pointer(VIEWS, &other.to_string()), &moved, false)?];
            self.views.insert(other, moved);
            ops.extend(self.delete_view(id)?);
            return Ok(ops);
        }
        // Both are known to exist, so the two clones below cannot fail; they are
        // taken before anything is written for `commit`'s reason.
        let (Some(here), Some(there)) = (self.views.get(&id), self.views.get(&other)) else {
            return Err(SessionError::UnknownView(other));
        };
        let moved = View {
            id: other,
            name: here.name.clone(),
            windows: here.windows.clone(),
        };
        let displaced = View {
            id,
            name: there.name.clone(),
            windows: there.windows.clone(),
        };
        let mut ops = vec![
            put(pointer(VIEWS, &id.to_string()), &displaced, true)?,
            put(pointer(VIEWS, &other.to_string()), &moved, true)?,
        ];
        self.views.insert(id, displaced);
        self.views.insert(other, moved);

        // The active view is a number, and one of these two views has just
        // changed its number. Following the view is what keeps the canvas the
        // operator is looking at under the button that is lit.
        let active = self.session.active_view_id;
        if active == id || active == other {
            let mut next = self.session.clone();
            next.active_view_id = if active == id { other } else { id };
            ops.extend(self.commit(next)?);
        }
        self.dirty = true;
        Ok(ops)
    }

    /// The stored view one place along the bar from `id`, if there is one.
    ///
    /// The **only** place the order of the bar is expressed, so that
    /// `MoveView`, `DeleteView`'s choice of successor and `prismd`'s
    /// `context_of` — which is what `Channel ◀▶` steps — cannot disagree about
    /// what *next* means. Written by comparing numbers rather than by index so
    /// that an `activeViewId` naming a view that is not stored, which a
    /// hand-edited file can produce, still has neighbours.
    fn neighbour(&self, id: ViewId, direction: ParamDirection) -> Option<ViewId> {
        match direction {
            ParamDirection::Prev => self.views.keys().rev().find(|key| **key < id).copied(),
            ParamDirection::Next => self.views.keys().find(|key| **key > id).copied(),
        }
    }

    /// Opens a window on the canvas and focuses it.
    ///
    /// The new window is the last of `open_windows`, which is the front of the
    /// stacking order.
    ///
    /// # Errors
    ///
    /// [`SessionError::ClientLocalParam`] if a parameter names something §4.2
    /// keeps client-local, [`SessionError::NoWindowNumberLeft`] if no number is
    /// free, or [`SessionError::NotRepresentable`].
    pub fn open_window(
        &mut self,
        window: WindowType,
        params: Option<&BTreeMap<String, JsonValue>>,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let params = params.cloned().unwrap_or_default();
        for key in params.keys() {
            check_param_key(key)?;
        }
        let (x, y, w, h) = DEFAULT_WINDOW;
        let instance = WindowInstance {
            instance_id: self.next_instance_id()?,
            window_type: window,
            x,
            y,
            w,
            h,
            params,
        };
        let mut next = self.session.clone();
        next.focused_window = Some(instance.instance_id);
        next.open_windows.push(instance);
        self.commit(next)
    }

    /// Closes an open window.
    ///
    /// The focus falls to whatever is left in front, so the invariant that the
    /// focused window is an open one holds without the caller doing anything.
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownWindow`] if it is not open.
    pub fn close_window(&mut self, id: WindowInstanceId) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        let Some(position) = next
            .open_windows
            .iter()
            .position(|window| window.instance_id == id)
        else {
            return Err(SessionError::UnknownWindow(id));
        };
        next.open_windows.remove(position);
        if next.focused_window == Some(id) {
            next.focused_window = next.open_windows.last().map(|window| window.instance_id);
        }
        self.commit(next)
    }

    /// Brings a window to the front and focuses it.
    ///
    /// `open_windows` is in stacking order, so "to the front" is "to the end".
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownWindow`] if it is not open.
    pub fn focus_window(&mut self, id: WindowInstanceId) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        let Some(position) = next
            .open_windows
            .iter()
            .position(|window| window.instance_id == id)
        else {
            return Err(SessionError::UnknownWindow(id));
        };
        let window = next.open_windows.remove(position);
        next.open_windows.push(window);
        next.focused_window = Some(id);
        self.commit(next)
    }

    /// Moves and resizes an open window.
    ///
    /// There is no §4.4 command for this: the console opens and closes windows,
    /// it does not drag them. Position and size are session state all the same
    /// (§4.1), so dragging a window on one screen has to move it on every
    /// other. **S25 took the consequence the note here used to predict:**
    /// `Command::PlaceWindow` now exists, `docs/IPC_PROTOCOL.md` §5 carries it,
    /// and the canvas reaches this method through the same door every other
    /// client command uses rather than keeping the position to itself.
    ///
    /// # Errors
    ///
    /// [`SessionError::UnknownWindow`] if it is not open, or
    /// [`SessionError::NotRepresentable`] for a non-finite coordinate — which
    /// is refused before anything is written.
    pub fn place_window(
        &mut self,
        id: WindowInstanceId,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        let Some(window) = next
            .open_windows
            .iter_mut()
            .find(|window| window.instance_id == id)
        else {
            return Err(SessionError::UnknownWindow(id));
        };
        window.x = x;
        window.y = y;
        window.w = w;
        window.h = h;
        self.commit(next)
    }

    /// Pages the fader bank.
    ///
    /// # Errors
    ///
    /// [`SessionError::ExecutorPageOutOfRange`] for a page whose eight slots are
    /// not eight executors. `SetExecutorPage` carries an arbitrary `u32` from
    /// any client and `ExecutorId::from_page_and_slot` saturates, so the top of
    /// the range folds every slot onto one executor — which is a fader bank
    /// where all eight faders drive the same thing.
    pub fn set_executor_page(&mut self, page: u32) -> Result<Vec<JsonPatchOp>, SessionError> {
        // The saturation shows up as the last slot's executor no longer being
        // on the page that was asked for.
        if ExecutorId::from_page_and_slot(page, EXECUTORS_PER_PAGE - 1).page() != page {
            return Err(SessionError::ExecutorPageOutOfRange { page });
        }
        let mut next = self.session.clone();
        next.executor_page = page;
        self.commit(next)
    }

    /// Selects the executor the main fader, Flip and transport act on, or
    /// nothing.
    ///
    /// Deliberately **not** validated against the show, and deliberately not
    /// coupled to [`SessionState::set_executor_page`]. Selecting an empty
    /// executor slot is how an operator says where the next sequence goes, and
    /// an executor bar showing another page must be able to select from it.
    /// `Option` is here for the daemon: an executor deleted from the show
    /// leaves the selection naming nothing rather than naming a ghost.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn select_executor(
        &mut self,
        executor: Option<ExecutorId>,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        next.selected_executor = executor;
        self.commit(next)
    }

    /// Selects the cue list a store with no sequence named goes into, or
    /// nothing — S39.
    ///
    /// Deliberately **not** validated against the show and deliberately not
    /// coupled to [`Self::select_executor`], for that method's two reasons
    /// repeated one field along: this applier has no show, and an operator
    /// programming cue list 7 while executor 3 plays the show is the ordinary
    /// case. `Option` is here for the daemon, so a sequence deleted from the
    /// show can leave the selection naming nothing rather than naming a ghost.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn select_sequence(
        &mut self,
        sequence: Option<SequenceId>,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        next.selected_sequence = sequence;
        self.commit(next)
    }

    /// Records which cue the programmer is editing, or that it is editing none
    /// — S39's update state.
    ///
    /// **There is no command for this**, and that is the point: it is not
    /// something an operator sets, it is what `Command::EditCue`,
    /// `Command::Update`, `Command::ClearProgrammer` and `Command::DeleteCue`
    /// leave behind. [`crate::ShowFile`] is the one caller, because it is the
    /// only place that sees the programmer and the session at once — the same
    /// door the jog wheel's parameter index goes through (S13).
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn set_editing_cue(
        &mut self,
        editing: Option<CueEdit>,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        next.editing_cue = editing;
        self.commit(next)
    }

    /// Switches the encoder bank.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn set_encoder_bank(
        &mut self,
        group: FeatureGroup,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        next.encoder_bank = group;
        self.commit(next)
    }

    /// Pages the programmer.
    ///
    /// Any page is accepted, unlike the executor page: this number is a display
    /// offset into the parameters of the current selection and nothing is
    /// derived from it by arithmetic that could fold two pages into one. What
    /// is on a page at all is the programmer's question, and the programmer is
    /// S13.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn set_programmer_page(&mut self, page: u32) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        next.programmer_page = page;
        self.commit(next)
    }

    /// Moves the parameter the jog wheel turns.
    ///
    /// Saturating at zero, and a jog wheel turned past the end is a no-op
    /// rather than a refusal: the operator turned a wheel, and an error on the
    /// console for that would be noise. How many parameters there actually are
    /// is the programmer's question (S13), so the upper end is not bounded
    /// here.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn select_programmer_param(
        &mut self,
        direction: ParamDirection,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let index = match direction {
            ParamDirection::Prev => self.session.programmer_param_index.saturating_sub(1),
            ParamDirection::Next => self.session.programmer_param_index.saturating_add(1),
        };
        self.set_programmer_param_index(index)
    }

    /// Sets the parameter the jog wheel turns.
    ///
    /// **S13 requirement:** a change of selection changes what the parameters
    /// are, so the programmer resets this through here rather than leaving the
    /// wheel pointing at a parameter the new selection does not have.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn set_programmer_param_index(
        &mut self,
        index: u32,
    ) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        next.programmer_param_index = index;
        self.commit(next)
    }

    /// Replaces the contents of the command line.
    ///
    /// The command carries the **whole line**, not a keystroke. §4.1 defines the
    /// field as the contents of the console line, and an append-only reading
    /// would leave no way to backspace or to clear it. **S19/S26 requirement:**
    /// the console and the command line widget send the line as it now reads,
    /// and clearing it is `CommandLineInput { text: "" }`.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only.
    pub fn set_command_line(&mut self, text: &str) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut next = self.session.clone();
        next.command_line = text.to_owned();
        self.commit(next)
    }

    // -- internals --------------------------------------------------------

    /// Encodes what changed between the session and its successor, then swaps
    /// the successor in.
    ///
    /// This is where "validate and encode before you write" lives for the whole
    /// module: every edit builds its successor beside the current state and
    /// hands it here, so an operation that cannot be encoded — S11's finding —
    /// leaves the session untouched. It is also where change detection lives, so
    /// no edit has to remember to check whether it changed anything.
    ///
    /// `id` and `name` are not compared: nothing in V1 changes them, and a
    /// multi-session daemon would add a session rather than rename one.
    fn commit(&mut self, next: Session) -> Result<Vec<JsonPatchOp>, SessionError> {
        let mut ops = Vec::new();
        let current = &self.session;
        if next.active_view_id != current.active_view_id {
            ops.push(replace(ACTIVE_VIEW_ID, &next.active_view_id)?);
        }
        if next.open_windows != current.open_windows {
            ops.push(replace(OPEN_WINDOWS, &next.open_windows)?);
        }
        if next.focused_window != current.focused_window {
            ops.push(replace(FOCUSED_WINDOW, &next.focused_window)?);
        }
        if next.executor_page != current.executor_page {
            ops.push(replace(EXECUTOR_PAGE, &next.executor_page)?);
        }
        if next.selected_executor != current.selected_executor {
            ops.push(replace(SELECTED_EXECUTOR, &next.selected_executor)?);
        }
        if next.selected_sequence != current.selected_sequence {
            ops.push(replace(SELECTED_SEQUENCE, &next.selected_sequence)?);
        }
        if next.editing_cue != current.editing_cue {
            ops.push(replace(EDITING_CUE, &next.editing_cue)?);
        }
        if next.encoder_bank != current.encoder_bank {
            ops.push(replace(ENCODER_BANK, &next.encoder_bank)?);
        }
        if next.programmer_page != current.programmer_page {
            ops.push(replace(PROGRAMMER_PAGE, &next.programmer_page)?);
        }
        if next.programmer_param_index != current.programmer_param_index {
            let index = &next.programmer_param_index;
            ops.push(replace(PROGRAMMER_PARAM_INDEX, index)?);
        }
        if next.command_line != current.command_line {
            ops.push(replace(COMMAND_LINE, &next.command_line)?);
        }
        self.session = next;
        Ok(ops)
    }

    /// A window number nothing else is using.
    ///
    /// One past the highest number in use anywhere — the canvas *and* every
    /// stored view, so loading a view can never collide with a window that is
    /// already open, and a number a view still remembers is never handed out
    /// again.
    ///
    /// A number that **no** view remembers is reused once its window closes,
    /// and that is a deliberate trade rather than an oversight: the alternative
    /// is a counter that has to be serialised, restored and kept above the
    /// content it numbers, to defend against a client sending `CloseWindow` for
    /// a window that closed *and* whose number was reissued before the message
    /// arrived. The cost of that race is the wrong window closing, which the
    /// operator sees and can undo by opening it again.
    fn next_instance_id(&self) -> Result<WindowInstanceId, SessionError> {
        self.session
            .open_windows
            .iter()
            .chain(self.views.values().flat_map(|view| view.windows.iter()))
            .map(|window| window.instance_id.get())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .map(WindowInstanceId::new)
            .ok_or(SessionError::NoWindowNumberLeft)
    }
}

/// One `replace` on a member of the session.
fn replace(field: &str, value: &impl Serialize) -> Result<JsonPatchOp, SessionError> {
    Ok(JsonPatchOp::Replace {
        path: format!("/{SESSION}/{field}"),
        value: project(value).map_err(SessionError::NotRepresentable)?,
    })
}

/// `add` for a view that is new, `replace` for one that was already stored.
fn put(path: String, value: &impl Serialize, existed: bool) -> Result<JsonPatchOp, SessionError> {
    let value = project(value).map_err(SessionError::NotRepresentable)?;
    Ok(if existed {
        JsonPatchOp::Replace { path, value }
    } else {
        JsonPatchOp::Add { path, value }
    })
}

/// Refuses a window parameter that is really client-local state.
fn check_param_key(key: &str) -> Result<(), SessionError> {
    let lowered = key.to_ascii_lowercase();
    if CLIENT_LOCAL_PARAM_KEYS
        .iter()
        .any(|forbidden| lowered.contains(forbidden))
    {
        return Err(SessionError::ClientLocalParam {
            key: key.to_owned(),
        });
    }
    Ok(())
}

/// The operations a `Delta::SessionPatch` in `deltas` carries, for tests and for
/// callers that want to feed a mirror without matching on the delta.
#[must_use]
pub fn session_patch_ops(deltas: &[Delta]) -> Vec<JsonPatchOp> {
    deltas
        .iter()
        .filter_map(|delta| match delta {
            Delta::SessionPatch { ops } => Some(ops.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{SessionError, SessionState, session_patch_ops};
    use prism_domain::{
        Command, Delta, EXECUTORS_PER_PAGE, ExecutorId, FeatureGroup, JsonPatchOp, JsonValue,
        OverwriteMode, ParamDirection, Session, SessionId, ViewId, WindowInstanceId, WindowType,
    };
    use std::collections::BTreeMap;

    /// Two windows open, the first of them stored as view 2.
    fn session() -> SessionState {
        let mut session = SessionState::new();
        session.open_window(WindowType::FixtureSheet, None).unwrap();
        session.store_view(ViewId::new(2), "Programming").unwrap();
        session.open_window(WindowType::Patch, None).unwrap();
        session.mark_saved();
        session
    }

    #[test]
    fn a_fresh_session_is_main_with_one_empty_view_active() {
        let fresh = SessionState::new();
        assert_eq!(fresh.session().id, SessionId::new(1));
        assert_eq!(fresh.session().name, "Main");
        assert_eq!(fresh.views().count(), 1);
        // The active view exists from the first moment, so `SelectView` on it
        // is a reset of the canvas rather than an error.
        assert!(fresh.view(fresh.session().active_view_id).is_some());
        assert!(!fresh.is_dirty());
    }

    #[test]
    fn the_field_names_the_operations_use_are_the_wire_names() {
        // A pointer into a member the session does not have would be an
        // operation no mirror could apply, and the only thing that would notice
        // is the round trip. This notices first.
        let wire = serde_json::to_value(Session::new(SessionId::new(1), "Main")).unwrap();
        let members = wire.as_object().unwrap();
        for field in [
            super::ACTIVE_VIEW_ID,
            super::OPEN_WINDOWS,
            super::FOCUSED_WINDOW,
            super::EXECUTOR_PAGE,
            super::SELECTED_EXECUTOR,
            super::ENCODER_BANK,
            super::PROGRAMMER_PAGE,
            super::PROGRAMMER_PARAM_INDEX,
            super::COMMAND_LINE,
        ] {
            assert!(members.contains_key(field), "{field} is not a member");
        }
    }

    #[test]
    fn an_edit_that_changes_nothing_says_nothing() {
        let mut session = session();
        // The same value again, for each kind of member.
        assert!(
            session
                .set_encoder_bank(session.session().encoder_bank)
                .unwrap()
                .is_empty()
        );
        assert!(session.set_executor_page(0).unwrap().is_empty());
        assert!(session.set_programmer_page(0).unwrap().is_empty());
        assert!(session.set_command_line("").unwrap().is_empty());
        assert!(session.select_executor(None).unwrap().is_empty());
        // A jog wheel turned left at the first parameter.
        assert!(
            session
                .select_programmer_param(ParamDirection::Prev)
                .unwrap()
                .is_empty()
        );
        // And storing a view whose contents are already stored.
        session.store_view(ViewId::new(2), "Programming").unwrap();
        let before = rmp_serde::to_vec_named(&session).unwrap();
        assert!(
            session
                .store_view(ViewId::new(2), "Programming")
                .unwrap()
                .is_empty()
        );
        assert_eq!(rmp_serde::to_vec_named(&session).unwrap(), before);
        assert!(
            session
                .apply(&Command::SetExecutorPage { page: 0 })
                .unwrap()
                .deltas
                .is_empty()
        );
    }

    #[test]
    fn an_edit_reports_the_members_it_changed_and_no_others() {
        let mut session = session();
        let ops = session.set_executor_page(3).unwrap();
        assert_eq!(
            ops,
            vec![JsonPatchOp::Replace {
                path: "/session/executorPage".to_owned(),
                value: JsonValue::Int(3),
            }]
        );
        // Opening a window moves the list and the focus, and nothing else.
        let ops = session.open_window(WindowType::Groups, None).unwrap();
        let windows: JsonValue =
            serde_json::from_value(serde_json::to_value(&session.session().open_windows).unwrap())
                .unwrap();
        assert_eq!(
            ops,
            vec![
                JsonPatchOp::Replace {
                    path: "/session/openWindows".to_owned(),
                    value: windows,
                },
                JsonPatchOp::Replace {
                    path: "/session/focusedWindow".to_owned(),
                    value: JsonValue::Int(3),
                },
            ]
        );
    }

    #[test]
    fn a_view_is_the_canvas_as_it_was_when_it_was_stored() {
        let mut session = session();
        assert_eq!(session.session().open_windows.len(), 2);
        // View 2 was stored when only the fixture sheet was open.
        session.select_view(ViewId::new(2)).unwrap();
        assert_eq!(session.session().open_windows.len(), 1);
        assert_eq!(
            session.session().open_windows[0].window_type,
            WindowType::FixtureSheet
        );
        assert_eq!(session.session().active_view_id, ViewId::new(2));
        // The focus follows the layout rather than pointing at a closed window.
        assert_eq!(
            session.session().focused_window,
            Some(WindowInstanceId::new(1))
        );

        // Selecting it again after a change is how a layout is restored.
        session.open_window(WindowType::Settings, None).unwrap();
        assert_eq!(session.session().open_windows.len(), 2);
        assert!(!session.select_view(ViewId::new(2)).unwrap().is_empty());
        assert_eq!(session.session().open_windows.len(), 1);
    }

    /// Renaming changes the name and **nothing else** — which is the whole
    /// reason it is not `StoreView` with the old name.
    #[test]
    fn renaming_a_view_leaves_its_windows_alone() {
        let mut session = session();
        // The canvas has two windows; view 2 was stored holding one.
        assert_eq!(session.session().open_windows.len(), 2);
        let before = session.view(ViewId::new(2)).unwrap().windows.clone();

        let ops = session.rename_view(ViewId::new(2), "Busking").unwrap();
        let view = session.view(ViewId::new(2)).unwrap();
        assert_eq!(view.name, "Busking");
        assert_eq!(view.windows, before);
        assert_eq!(view.windows.len(), 1);
        // One operation, on the view and not on the session.
        assert_eq!(ops.len(), 1);
        assert!(matches!(
            &ops[0],
            JsonPatchOp::Replace { path, .. } if path == "/views/2"
        ));
        assert!(session.is_dirty());
    }

    #[test]
    fn renaming_a_view_to_the_name_it_has_is_not_a_change() {
        let mut session = session();
        let before = rmp_serde::to_vec_named(&session).unwrap();
        assert!(
            session
                .rename_view(ViewId::new(2), "Programming")
                .unwrap()
                .is_empty()
        );
        assert_eq!(rmp_serde::to_vec_named(&session).unwrap(), before);
        assert!(!session.is_dirty());
    }

    #[test]
    fn a_view_that_was_never_stored_cannot_be_renamed_deleted_or_moved() {
        let mut session = session();
        let missing = ViewId::new(9);
        assert_eq!(
            session.rename_view(missing, "Nope"),
            Err(SessionError::UnknownView(missing))
        );
        assert_eq!(
            session.delete_view(missing),
            Err(SessionError::UnknownView(missing))
        );
        assert_eq!(
            session.move_view(missing, ViewId::new(2)),
            Err(SessionError::UnknownView(missing))
        );
        assert!(!session.is_dirty());
    }

    #[test]
    fn the_last_view_cannot_be_deleted() {
        let mut fresh = SessionState::new();
        let only = fresh.session().active_view_id;
        assert_eq!(fresh.views().count(), 1);
        let before = rmp_serde::to_vec_named(&fresh).unwrap();

        assert_eq!(fresh.delete_view(only), Err(SessionError::LastView(only)));

        // Refused and byte-identical, which is this module's rule for every
        // rejection — and the invariant `ShowStore` relies on still holds.
        assert_eq!(rmp_serde::to_vec_named(&fresh).unwrap(), before);
        assert_eq!(fresh.views().count(), 1);
        assert!(fresh.view(fresh.session().active_view_id).is_some());
        assert!(!fresh.is_dirty());
    }

    #[test]
    fn deleting_a_view_that_is_not_active_leaves_the_canvas_alone() {
        let mut session = session();
        session.store_view(ViewId::new(5), "Busking").unwrap();
        let canvas = session.session().open_windows.clone();
        let active = session.session().active_view_id;

        let ops = session.delete_view(ViewId::new(5)).unwrap();
        assert!(session.view(ViewId::new(5)).is_none());
        assert_eq!(session.session().open_windows, canvas);
        assert_eq!(session.session().active_view_id, active);
        // The removal, and nothing on the session at all.
        assert_eq!(
            ops,
            vec![JsonPatchOp::Remove {
                path: "/views/5".to_owned()
            }]
        );
    }

    /// **The exit criterion**: the canvas after deleting the active view is the
    /// daemon's answer, and it is a *stored layout* rather than whatever
    /// happened to be open.
    #[test]
    fn deleting_the_active_view_selects_the_one_before_it() {
        let mut session = session();
        // Views 1 and 2 exist; 1 was stored empty, 2 holds the fixture sheet.
        session.select_view(ViewId::new(2)).unwrap();
        assert_eq!(session.session().active_view_id, ViewId::new(2));
        assert_eq!(session.session().open_windows.len(), 1);

        session.delete_view(ViewId::new(2)).unwrap();
        assert!(session.view(ViewId::new(2)).is_none());
        // View 1 is the neighbour before, and it was stored with nothing open.
        assert_eq!(session.session().active_view_id, ViewId::new(1));
        assert!(session.session().open_windows.is_empty());
        assert_eq!(session.session().focused_window, None);
    }

    #[test]
    fn deleting_the_first_view_falls_forward_to_the_next_one() {
        let mut session = session();
        // Active is view 1, and view 2 is the only other one.
        assert_eq!(session.session().active_view_id, ViewId::new(1));

        session.delete_view(ViewId::new(1)).unwrap();
        assert_eq!(session.session().active_view_id, ViewId::new(2));
        // View 2's layout, loaded — not the two windows that were on the canvas.
        assert_eq!(session.session().open_windows.len(), 1);
        assert_eq!(
            session.session().open_windows[0].window_type,
            WindowType::FixtureSheet
        );
    }

    /// **The ordering decision, asserted.** Moving exchanges the numbers, so
    /// the drawn order and the numbers are the same one thing — and
    /// `SelectView 2` afterwards names the layout that moved into place.
    #[test]
    fn moving_a_view_exchanges_its_number_with_its_neighbour() {
        let mut session = session();
        assert_eq!(
            session
                .views()
                .map(|view| (view.id.get(), view.name.clone()))
                .collect::<Vec<_>>(),
            vec![(1, "View 1".to_owned()), (2, "Programming".to_owned())]
        );
        let programming = session.view(ViewId::new(2)).unwrap().windows.clone();

        let ops = session.move_view(ViewId::new(2), ViewId::new(1)).unwrap();

        assert_eq!(
            session
                .views()
                .map(|view| (view.id.get(), view.name.clone()))
                .collect::<Vec<_>>(),
            vec![(1, "Programming".to_owned()), (2, "View 1".to_owned())]
        );
        // The layout travelled with the name, not with the number.
        assert_eq!(session.view(ViewId::new(1)).unwrap().windows, programming);
        // Both views were rewritten; neither was added or removed. The third
        // operation is `activeViewId` following the view it names — view 1 was
        // active, and view 1's layout is now numbered 2.
        let paths: Vec<&str> = ops
            .iter()
            .map(|op| match op {
                JsonPatchOp::Replace { path, .. } => path.as_str(),
                _ => panic!("a move adds and removes nothing: {op:?}"),
            })
            .collect();
        // The view that was asked to move is written first, then the one it
        // displaced. Both are `Replace` on distinct paths, so a mirror applying
        // them in either order lands in the same place.
        assert_eq!(paths, vec!["/views/2", "/views/1", "/session/activeViewId"]);
        assert!(session.is_dirty());
    }

    /// The active view is a *number*, and a move changes numbers. Following the
    /// view is what keeps the lit button under the canvas being looked at.
    #[test]
    fn moving_the_active_view_keeps_it_active() {
        let mut session = session();
        session.select_view(ViewId::new(2)).unwrap();
        let windows = session.session().open_windows.clone();

        session.move_view(ViewId::new(2), ViewId::new(1)).unwrap();

        assert_eq!(session.session().active_view_id, ViewId::new(1));
        assert_eq!(session.view(ViewId::new(1)).unwrap().name, "Programming");
        // The canvas did not move: only the number under it did.
        assert_eq!(session.session().open_windows, windows);
    }

    /// The other half: the view the active one was swapped *with* also changes
    /// number, and the active id has to follow that too.
    #[test]
    fn moving_a_view_onto_the_active_one_carries_the_active_number_back() {
        let mut session = session();
        // Active is view 1. Moving view 2 onto it makes view 1's layout view 2.
        assert_eq!(session.session().active_view_id, ViewId::new(1));
        let windows = session.session().open_windows.clone();

        session.move_view(ViewId::new(2), ViewId::new(1)).unwrap();

        assert_eq!(session.session().active_view_id, ViewId::new(2));
        assert_eq!(session.view(ViewId::new(2)).unwrap().name, "View 1");
        assert_eq!(session.session().open_windows, windows);
    }

    /// **A move onto a number nobody has stored is a move, not a no-op** (S40).
    /// The relative form this replaced had nowhere to go at the end of the bar;
    /// the absolute one always has somewhere to go, so what has to be true
    /// instead is that the view arrives there and leaves nothing behind.
    #[test]
    fn a_view_moved_onto_an_empty_number_takes_that_place_and_leaves_none() {
        let mut session = session();
        let name = session.view(ViewId::new(1)).unwrap().name.clone();

        let ops = session.move_view(ViewId::new(1), ViewId::new(9)).unwrap();

        assert!(!ops.is_empty());
        assert!(session.view(ViewId::new(1)).is_none());
        assert_eq!(session.view(ViewId::new(9)).unwrap().name, name);
    }

    /// A move of a view onto itself changes nothing at all, which is the only
    /// no-op the absolute form has.
    #[test]
    fn a_view_moved_onto_itself_does_nothing() {
        let mut session = session();
        let before = rmp_serde::to_vec_named(&session).unwrap();
        assert!(
            session
                .move_view(ViewId::new(1), ViewId::new(1))
                .unwrap()
                .is_empty()
        );
        assert_eq!(rmp_serde::to_vec_named(&session).unwrap(), before);
        assert!(!session.is_dirty());
    }

    /// Views are **not** consecutive, so a move must step to the next stored
    /// number rather than to `id ± 1`.
    #[test]
    fn moving_steps_to_the_next_stored_number_and_not_the_next_integer() {
        let mut session = session();
        session.store_view(ViewId::new(9), "Busking").unwrap();

        session.move_view(ViewId::new(9), ViewId::new(2)).unwrap();

        assert_eq!(
            session
                .views()
                .map(|view| (view.id.get(), view.name.clone()))
                .collect::<Vec<_>>(),
            vec![
                (1, "View 1".to_owned()),
                (2, "Busking".to_owned()),
                (9, "Programming".to_owned()),
            ]
        );
    }

    #[test]
    fn storing_a_view_is_the_only_session_edit_that_lights_the_save_led() {
        let mut session = session();
        session.set_executor_page(2).unwrap();
        session.open_window(WindowType::Groups, None).unwrap();
        session.close_window(WindowInstanceId::new(1)).unwrap();
        session.select_view(ViewId::new(2)).unwrap();
        assert!(!session.is_dirty());

        session.store_view(ViewId::new(5), "Busking").unwrap();
        assert!(session.is_dirty());
        assert!(session.mark_saved());
        assert!(!session.mark_saved());
    }

    #[test]
    fn a_window_number_a_view_still_remembers_is_never_handed_out_again() {
        let mut session = session();
        // Windows 1 and 2 are open; view 2 remembers window 1.
        session.close_window(WindowInstanceId::new(2)).unwrap();
        session.close_window(WindowInstanceId::new(1)).unwrap();
        assert!(session.session().open_windows.is_empty());
        assert_eq!(session.session().focused_window, None);
        // With nothing open at all, the next number is still 2: window 1 is
        // free but view 2 names it, and loading that view has to be able to put
        // it back beside whatever else is open by then.
        session.open_window(WindowType::Groups, None).unwrap();
        assert_eq!(
            session.session().open_windows[0].instance_id,
            WindowInstanceId::new(2)
        );
        session.select_view(ViewId::new(2)).unwrap();
        assert_eq!(
            session.session().open_windows[0].instance_id,
            WindowInstanceId::new(1)
        );
    }

    #[test]
    fn focusing_a_window_brings_it_to_the_front() {
        let mut session = session();
        session.open_window(WindowType::Groups, None).unwrap();
        let order: Vec<u32> = session
            .session()
            .open_windows
            .iter()
            .map(|window| window.instance_id.get())
            .collect();
        assert_eq!(order, [1, 2, 3]);

        session.focus_window(WindowInstanceId::new(1)).unwrap();
        let order: Vec<u32> = session
            .session()
            .open_windows
            .iter()
            .map(|window| window.instance_id.get())
            .collect();
        assert_eq!(order, [2, 3, 1]);
        assert_eq!(
            session.session().focused_window,
            Some(WindowInstanceId::new(1))
        );
    }

    #[test]
    fn closing_the_focused_window_leaves_the_one_in_front_focused() {
        let mut session = session();
        assert_eq!(
            session.session().focused_window,
            Some(WindowInstanceId::new(2))
        );
        session.close_window(WindowInstanceId::new(2)).unwrap();
        assert_eq!(
            session.session().focused_window,
            Some(WindowInstanceId::new(1))
        );
        // Closing one that is not focused leaves the focus alone. The new
        // window takes the number 2 back: no view remembers it.
        session.open_window(WindowType::Groups, None).unwrap();
        session.close_window(WindowInstanceId::new(1)).unwrap();
        assert_eq!(
            session.session().focused_window,
            Some(WindowInstanceId::new(2))
        );
    }

    #[test]
    fn a_window_that_is_not_open_cannot_be_closed_focused_or_placed() {
        let mut session = session();
        let missing = WindowInstanceId::new(99);
        assert_eq!(
            session.close_window(missing),
            Err(SessionError::UnknownWindow(missing))
        );
        assert_eq!(
            session.focus_window(missing),
            Err(SessionError::UnknownWindow(missing))
        );
        assert_eq!(
            session.place_window(missing, 0.0, 0.0, 1.0, 1.0),
            Err(SessionError::UnknownWindow(missing))
        );
        assert_eq!(session.window(missing), None);
        assert!(session.window(WindowInstanceId::new(1)).is_some());
    }

    /// **A copy makes a view that was not there, and leaves the source alone.**
    ///
    /// The whole difference between a copy and a move, said on the session
    /// rather than in a comment (S40).
    #[test]
    fn a_view_copied_onto_a_free_number_appears_and_leaves_the_source() {
        let mut session = session();
        let windows = session.view(ViewId::new(1)).unwrap().windows.len();

        let ops = session
            .copy_view(ViewId::new(1), ViewId::new(9), OverwriteMode::Merge)
            .unwrap();

        assert_eq!(ops.len(), 1);
        assert_eq!(session.view(ViewId::new(9)).unwrap().windows.len(), windows);
        // A copy onto a free number takes the source's name, because there is no
        // name there to keep.
        assert_eq!(
            session.view(ViewId::new(9)).unwrap().name,
            session.view(ViewId::new(1)).unwrap().name
        );
        assert_eq!(session.view(ViewId::new(1)).unwrap().windows.len(), windows);
    }

    /// **Merging two layouts is a union of windows, each with a fresh number.**
    ///
    /// Two windows of the same type on one canvas is an ordinary thing to want,
    /// so a merge does not try to match them up; what it does do is renumber the
    /// incoming ones, or the two layouts would disagree about which window is
    /// which.
    #[test]
    fn merging_a_view_into_another_unions_the_windows_and_renumbers_them() {
        let mut session = session();
        session.store_view(ViewId::new(4), "Busking").unwrap();
        let first = session.view(ViewId::new(1)).unwrap().windows.len();
        let second = session.view(ViewId::new(4)).unwrap().windows.len();

        session
            .copy_view(ViewId::new(1), ViewId::new(4), OverwriteMode::Merge)
            .unwrap();

        let merged = session.view(ViewId::new(4)).unwrap();
        assert_eq!(merged.windows.len(), first + second);
        // Every instance number is its own, which is what a canvas needs to
        // address a window at all.
        let mut numbers: Vec<u32> = merged
            .windows
            .iter()
            .map(|window| window.instance_id.get())
            .collect();
        numbers.sort_unstable();
        let unique = numbers.len();
        numbers.dedup();
        assert_eq!(
            numbers.len(),
            unique,
            "two windows share an instance number"
        );
        // And the destination keeps its own name: a copy is not a rename.
        assert_eq!(merged.name, "Busking");
    }

    /// An Override replaces the layout rather than adding to it.
    #[test]
    fn overriding_a_view_replaces_its_windows() {
        let mut session = session();
        session.store_view(ViewId::new(4), "Busking").unwrap();
        let first = session.view(ViewId::new(1)).unwrap().windows.len();

        session
            .copy_view(ViewId::new(1), ViewId::new(4), OverwriteMode::Override)
            .unwrap();

        assert_eq!(session.view(ViewId::new(4)).unwrap().windows.len(), first);
    }

    /// A copy of a view that was never stored is refused, and a copy onto
    /// itself changes nothing at all.
    #[test]
    fn copying_a_view_that_is_not_there_is_refused_and_onto_itself_is_nothing() {
        let mut session = session();
        assert!(matches!(
            session.copy_view(ViewId::new(9), ViewId::new(2), OverwriteMode::Merge),
            Err(SessionError::UnknownView(_))
        ));
        assert!(
            session
                .copy_view(ViewId::new(1), ViewId::new(1), OverwriteMode::Merge)
                .unwrap()
                .is_empty()
        );
    }

    /// A move of a view that was never stored is refused.
    #[test]
    fn moving_a_view_that_is_not_there_is_refused() {
        let mut session = session();
        assert!(matches!(
            session.move_view(ViewId::new(9), ViewId::new(1)),
            Err(SessionError::UnknownView(_))
        ));
    }

    #[test]
    fn a_window_cannot_be_placed_somewhere_that_has_no_wire_form() {
        // The encoding is the last thing that can fail in an edit (S11), and it
        // has to fail before the session is written.
        let mut session = session();
        let before = rmp_serde::to_vec_named(&session).unwrap();
        assert!(matches!(
            session.place_window(WindowInstanceId::new(1), f64::NAN, 0.0, 1.0, 1.0),
            Err(SessionError::NotRepresentable(_))
        ));
        assert_eq!(rmp_serde::to_vec_named(&session).unwrap(), before);
        assert!(matches!(
            session.place_window(WindowInstanceId::new(1), 0.0, 0.0, f64::INFINITY, 1.0),
            Err(SessionError::NotRepresentable(_))
        ));
        assert_eq!(rmp_serde::to_vec_named(&session).unwrap(), before);
        // A place that can be encoded goes through.
        session
            .place_window(WindowInstanceId::new(1), 10.0, 20.0, 30.0, 40.0)
            .unwrap();
        let window = session.window(WindowInstanceId::new(1)).unwrap();
        assert_eq!(
            (window.x, window.y, window.w, window.h),
            (10.0, 20.0, 30.0, 40.0)
        );
    }

    #[test]
    fn the_last_page_whose_slots_are_eight_executors_is_accepted_and_the_next_is_not() {
        let mut session = session();
        let last = (u32::MAX - (EXECUTORS_PER_PAGE - 1)) / EXECUTORS_PER_PAGE;
        session.set_executor_page(last).unwrap();
        assert_eq!(session.session().executor_page, last);
        assert_eq!(
            ExecutorId::from_page_and_slot(last, EXECUTORS_PER_PAGE - 1).get(),
            u32::MAX
        );
        for page in [last + 1, u32::MAX] {
            assert_eq!(
                session.set_executor_page(page),
                Err(SessionError::ExecutorPageOutOfRange { page })
            );
        }
        assert_eq!(session.session().executor_page, last);
    }

    #[test]
    fn the_jog_wheel_saturates_at_both_ends_rather_than_wrapping() {
        let mut session = session();
        session
            .select_programmer_param(ParamDirection::Prev)
            .unwrap();
        assert_eq!(session.session().programmer_param_index, 0);
        session
            .select_programmer_param(ParamDirection::Next)
            .unwrap();
        assert_eq!(session.session().programmer_param_index, 1);
        session.set_programmer_param_index(u32::MAX).unwrap();
        session
            .select_programmer_param(ParamDirection::Next)
            .unwrap();
        assert_eq!(session.session().programmer_param_index, u32::MAX);
    }

    #[test]
    fn a_window_parameter_may_not_be_client_local_state() {
        let mut session = session();
        assert_eq!(
            session.open_window(
                WindowType::Viewer3D,
                Some(&BTreeMap::from([(
                    "cameraTarget".to_owned(),
                    JsonValue::Int(1)
                )])),
            ),
            Err(SessionError::ClientLocalParam {
                key: "cameraTarget".to_owned()
            })
        );
        assert_eq!(session.session().open_windows.len(), 2);
        // The bag is still a bag for what a window genuinely shows.
        session
            .open_window(
                WindowType::PresetPool,
                Some(&BTreeMap::from([(
                    "pool".to_owned(),
                    JsonValue::String("Color".to_owned()),
                )])),
            )
            .unwrap();
        assert_eq!(
            session.window(WindowInstanceId::new(3)).unwrap().params["pool"],
            JsonValue::String("Color".to_owned())
        );
    }

    #[test]
    fn there_is_a_window_number_after_which_no_window_can_be_opened() {
        // Only a hand-edited file reaches this, and it says so rather than
        // opening a second window with the same number.
        let mut session: SessionState = serde_json::from_str(
            r#"{"session":{"id":1,"name":"Main","activeViewId":1,"openWindows":[
                {"instanceId":4294967295,"type":"Patch","x":0.0,"y":0.0,"w":1.0,"h":1.0,
                 "params":{}}],
                "focusedWindow":null,"executorPage":0,"selectedExecutor":null,
                "encoderBank":"Dimmer","programmerPage":0,"programmerParamIndex":0,
                "commandLine":""},"views":{}}"#,
        )
        .unwrap();
        assert_eq!(
            session.open_window(WindowType::Groups, None),
            Err(SessionError::NoWindowNumberLeft)
        );
    }

    #[test]
    fn the_command_line_carries_the_whole_line() {
        let mut session = session();
        session.set_command_line("1 thru 4 at ").unwrap();
        // A backspace is the shorter line, not a second command.
        session.set_command_line("1 thru 4 at").unwrap();
        assert_eq!(session.session().command_line, "1 thru 4 at");
        session
            .apply(&Command::CommandLineInput {
                text: String::new(),
            })
            .unwrap();
        assert!(session.session().command_line.is_empty());
    }

    #[test]
    fn applying_a_command_answers_with_one_session_patch() {
        let mut session = session();
        let applied = session
            .apply(&Command::SetEncoderBank {
                group: FeatureGroup::Color,
            })
            .unwrap();
        assert_eq!(
            applied.deltas,
            vec![Delta::SessionPatch {
                ops: vec![JsonPatchOp::Replace {
                    path: "/session/encoderBank".to_owned(),
                    value: JsonValue::String("Color".to_owned()),
                }]
            }]
        );
        assert_eq!(session_patch_ops(&applied.deltas).len(), 1);
        assert!(
            session_patch_ops(&[Delta::DirtyFlag {
                unsaved_changes: true
            }])
            .is_empty()
        );
    }

    #[test]
    fn every_refusal_says_what_was_wrong() {
        for error in [
            SessionError::UnknownView(ViewId::new(3)),
            SessionError::UnknownWindow(WindowInstanceId::new(3)),
            SessionError::ExecutorPageOutOfRange { page: 9 },
            SessionError::ClientLocalParam {
                key: "scrollTop".to_owned(),
            },
            SessionError::NoWindowNumberLeft,
            SessionError::NotASessionCommand,
            SessionError::NotRepresentable("NaN".to_owned()),
        ] {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn the_default_session_is_a_new_one() {
        assert_eq!(SessionState::default(), SessionState::new());
    }
}
