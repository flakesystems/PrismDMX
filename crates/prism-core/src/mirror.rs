//! The other end of a delta: a client's mirror of the daemon's state.
//!
//! `docs/IPC_PROTOCOL.md` §6 promises that "a client that has applied every
//! delta since its snapshot holds state identical to the daemon's". That is a
//! claim about two pieces of code — the generator here and an applier over
//! there — and it can only be checked by having both. [`JsonMirror`] is the
//! applier: RFC 6902 over [`JsonValue`]. [`ShowMirror`] and [`SessionMirror`]
//! are that engine plus the knowledge of which delta belongs to which document
//! — the `Snapshot` of §4.1 carries the show and the session as two documents,
//! and each has its own patch delta.
//!
//! It is not test scaffolding. The Web Remote and any Rust client mirror the
//! daemon exactly this way, and `tests/delta_round_trip.rs` is what holds the
//! generator to its promise, for both halves.
//!
//! # A failed operation is a bug, and says so
//!
//! Clients apply deltas without validating them, because the daemon has already
//! decided. So an operation that does not fit the mirror means the two have
//! already diverged, and the honest response is to say which operation failed
//! and re-snapshot — not to guess. [`ShowMirror::apply_all`] therefore stops at
//! the first failure and leaves the earlier operations applied: an
//! all-or-nothing batch would need a copy of the whole show per delta to
//! protect a case in which the mirror is already wrong.

use core::fmt;

use prism_domain::{Delta, JsonPatchOp, JsonValue};

/// Why an operation could not be applied to the mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorError {
    /// Not a JSON Pointer: RFC 6901 wants the empty string or a string
    /// beginning with `/`.
    MalformedPointer(String),
    /// Nothing at that path.
    NoSuchPath(String),
    /// An array was addressed with something that is not an index.
    NotAnIndex {
        /// The pointer.
        path: String,
        /// The token that is not a number.
        token: String,
    },
    /// An array index past the end.
    IndexOutOfRange {
        /// The pointer.
        path: String,
        /// The index asked for.
        index: usize,
    },
    /// A `test` operation found something else.
    TestFailed {
        /// The pointer.
        path: String,
    },
    /// A `move` or `copy` whose target is inside its own source, which RFC 6902
    /// forbids because the result is not defined.
    MoveIntoSelf {
        /// The source pointer.
        from: String,
        /// The target pointer.
        path: String,
    },
}

impl fmt::Display for MirrorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedPointer(path) => write!(f, "{path:?} is not a JSON Pointer"),
            Self::NoSuchPath(path) => write!(f, "nothing at {path:?}"),
            Self::NotAnIndex { path, token } => {
                write!(f, "{path:?} indexes an array with {token:?}")
            }
            Self::IndexOutOfRange { path, index } => {
                write!(f, "{path:?} indexes element {index} past the end")
            }
            Self::TestFailed { path } => write!(f, "the value at {path:?} is not the one tested"),
            Self::MoveIntoSelf { from, path } => {
                write!(f, "{path:?} is inside {from:?}, so the move is undefined")
            }
        }
    }
}

impl core::error::Error for MirrorError {}

/// A client's copy of the show, kept in step by deltas alone.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowMirror {
    inner: JsonMirror,
}

impl ShowMirror {
    /// A mirror of a snapshot.
    #[must_use]
    pub const fn new(root: JsonValue) -> Self {
        Self {
            inner: JsonMirror::new(root),
        }
    }

    /// What the mirror currently holds.
    #[must_use]
    pub const fn value(&self) -> &JsonValue {
        self.inner.value()
    }

    /// The mirrored document, consuming the mirror.
    #[must_use]
    pub fn into_value(self) -> JsonValue {
        self.inner.into_value()
    }

    /// Applies one delta.
    ///
    /// `ShowPatch` is applied as JSON Patch and `ExecutorState` writes the two
    /// fields it carries — the protocol gives running executors their own delta
    /// so a client does not have to diff the show to draw a moving executor
    /// bar, and a mirror that ignored it would drift on exactly those fields.
    /// Every other delta describes something that is not show state
    /// (`SessionPatch`, which is [`SessionMirror`]'s, `ProgrammerChanged`,
    /// `OutputsChanged` — S33's rig, which belongs to the *machine* and must
    /// never be written into a document a client thinks is the show —
    /// `OutputHealth`, `DirtyFlag`, `Notice`) and is ignored here.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if an operation does not fit the document.
    pub fn apply_delta(&mut self, delta: &Delta) -> Result<(), MirrorError> {
        match delta {
            Delta::ShowPatch { ops } => self.apply_all(ops),
            Delta::PlaybackState {
                playback,
                is_active,
                cue_index,
            } => {
                // Which collection the two fields live in is the playback's own
                // answer (S40): an executor's are on its row of the grid, and a
                // cue list playing on no fader keeps them on the sequence.
                let base = match playback {
                    prism_domain::PlaybackId::Executor { executor_id } => {
                        crate::show::pointer(crate::show::EXECUTORS, &executor_id.to_string())
                    }
                    prism_domain::PlaybackId::Sequence { sequence_id } => {
                        crate::show::pointer(crate::show::SEQUENCES, &sequence_id.to_string())
                    }
                };
                self.apply(&JsonPatchOp::Replace {
                    path: format!("{base}/isActive"),
                    value: JsonValue::Bool(*is_active),
                })?;
                self.apply(&JsonPatchOp::Replace {
                    path: format!("{base}/currentCueIndex"),
                    value: cue_index
                        .map_or(JsonValue::Null, |index| JsonValue::Int(i64::from(index))),
                })
            }
            Delta::SessionPatch { .. }
            | Delta::ProgrammerChanged { .. }
            | Delta::OutputsChanged { .. }
            | Delta::OutputHealth { .. }
            | Delta::DirtyFlag { .. }
            | Delta::Notice { .. } => Ok(()),
        }
    }

    /// Applies operations in order, stopping at the first that does not fit.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] for the operation that failed. Earlier operations stay
    /// applied — see the module documentation.
    pub fn apply_all(&mut self, ops: &[JsonPatchOp]) -> Result<(), MirrorError> {
        self.inner.apply_all(ops)
    }

    /// Applies one RFC 6902 operation.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if the operation does not fit the document.
    pub fn apply(&mut self, op: &JsonPatchOp) -> Result<(), MirrorError> {
        self.inner.apply(op)
    }

    /// The value at a pointer.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if the pointer is malformed or names nothing.
    pub fn get(&self, path: &str) -> Result<&JsonValue, MirrorError> {
        self.inner.get(path)
    }
}

/// A client's copy of the **session**, kept in step by deltas alone.
///
/// The other half of D11: a client that has applied every `SessionPatch` since
/// its snapshot shows the view, the windows, the page and the selection the
/// daemon holds — which is what makes a view switched from the X-Touch appear
/// on every screen at once. The document is
/// [`SessionState::to_json`](crate::SessionState::to_json).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionMirror {
    inner: JsonMirror,
}

impl SessionMirror {
    /// A mirror of a snapshot.
    #[must_use]
    pub const fn new(root: JsonValue) -> Self {
        Self {
            inner: JsonMirror::new(root),
        }
    }

    /// What the mirror currently holds.
    #[must_use]
    pub const fn value(&self) -> &JsonValue {
        self.inner.value()
    }

    /// The mirrored document, consuming the mirror.
    #[must_use]
    pub fn into_value(self) -> JsonValue {
        self.inner.into_value()
    }

    /// Applies one delta.
    ///
    /// `SessionPatch` is applied as JSON Patch; every other delta describes
    /// something that is not session state and is ignored, exactly as
    /// [`ShowMirror`] ignores this one.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if an operation does not fit the document.
    pub fn apply_delta(&mut self, delta: &Delta) -> Result<(), MirrorError> {
        match delta {
            Delta::SessionPatch { ops } => self.apply_all(ops),
            Delta::ShowPatch { .. }
            | Delta::PlaybackState { .. }
            | Delta::ProgrammerChanged { .. }
            | Delta::OutputsChanged { .. }
            | Delta::OutputHealth { .. }
            | Delta::DirtyFlag { .. }
            | Delta::Notice { .. } => Ok(()),
        }
    }

    /// Applies operations in order, stopping at the first that does not fit.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] for the operation that failed.
    pub fn apply_all(&mut self, ops: &[JsonPatchOp]) -> Result<(), MirrorError> {
        self.inner.apply_all(ops)
    }

    /// Applies one RFC 6902 operation.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if the operation does not fit the document.
    pub fn apply(&mut self, op: &JsonPatchOp) -> Result<(), MirrorError> {
        self.inner.apply(op)
    }

    /// The value at a pointer.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if the pointer is malformed or names nothing.
    pub fn get(&self, path: &str) -> Result<&JsonValue, MirrorError> {
        self.inner.get(path)
    }
}

/// RFC 6902 over a [`JsonValue`]: the engine both mirrors are made of.
///
/// Public because a client with a document of its own — the Web Remote's
/// outputs and health, say — needs the same applier, and because two copies of
/// a JSON Patch implementation is exactly the kind of duplication that drifts.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonMirror {
    root: JsonValue,
}

impl JsonMirror {
    /// A mirror of a snapshot.
    #[must_use]
    pub const fn new(root: JsonValue) -> Self {
        Self { root }
    }

    /// What the mirror currently holds.
    #[must_use]
    pub const fn value(&self) -> &JsonValue {
        &self.root
    }

    /// The mirrored document, consuming the mirror.
    #[must_use]
    pub fn into_value(self) -> JsonValue {
        self.root
    }

    /// Applies operations in order, stopping at the first that does not fit.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] for the operation that failed. Earlier operations stay
    /// applied — see the module documentation.
    pub fn apply_all(&mut self, ops: &[JsonPatchOp]) -> Result<(), MirrorError> {
        for op in ops {
            self.apply(op)?;
        }
        Ok(())
    }

    /// Applies one RFC 6902 operation.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if the operation does not fit the document.
    pub fn apply(&mut self, op: &JsonPatchOp) -> Result<(), MirrorError> {
        match op {
            JsonPatchOp::Add { path, value } => self.add(path, value.clone()),
            JsonPatchOp::Remove { path } => self.remove(path).map(drop),
            JsonPatchOp::Replace { path, value } => self.replace(path, value.clone()),
            JsonPatchOp::Move { from, path } => {
                if is_inside(from, path) {
                    return Err(MirrorError::MoveIntoSelf {
                        from: from.clone(),
                        path: path.clone(),
                    });
                }
                let value = self.remove(from)?;
                self.add(path, value)
            }
            JsonPatchOp::Copy { from, path } => {
                let value = self.get(from)?.clone();
                self.add(path, value)
            }
            JsonPatchOp::Test { path, value } => {
                if self.get(path)? == value {
                    Ok(())
                } else {
                    Err(MirrorError::TestFailed { path: path.clone() })
                }
            }
        }
    }

    /// The value at a pointer.
    ///
    /// # Errors
    ///
    /// [`MirrorError`] if the pointer is malformed or names nothing.
    pub fn get(&self, path: &str) -> Result<&JsonValue, MirrorError> {
        let tokens = tokens(path)?;
        let mut node = &self.root;
        for token in &tokens {
            node = match node {
                JsonValue::Object(members) => members
                    .get(token)
                    .ok_or_else(|| MirrorError::NoSuchPath(path.to_owned()))?,
                JsonValue::Array(elements) => elements
                    .get(index(path, token)?)
                    .ok_or_else(|| MirrorError::NoSuchPath(path.to_owned()))?,
                _ => return Err(MirrorError::NoSuchPath(path.to_owned())),
            };
        }
        Ok(node)
    }

    fn add(&mut self, path: &str, value: JsonValue) -> Result<(), MirrorError> {
        let tokens = tokens(path)?;
        let Some((last, parents)) = tokens.split_last() else {
            self.root = value;
            return Ok(());
        };
        match self.parent(path, parents)? {
            JsonValue::Object(members) => {
                members.insert(last.clone(), value);
                Ok(())
            }
            JsonValue::Array(elements) => {
                let position = if last == "-" {
                    elements.len()
                } else {
                    index(path, last)?
                };
                if position > elements.len() {
                    return Err(MirrorError::IndexOutOfRange {
                        path: path.to_owned(),
                        index: position,
                    });
                }
                elements.insert(position, value);
                Ok(())
            }
            _ => Err(MirrorError::NoSuchPath(path.to_owned())),
        }
    }

    fn remove(&mut self, path: &str) -> Result<JsonValue, MirrorError> {
        let tokens = tokens(path)?;
        let Some((last, parents)) = tokens.split_last() else {
            return Err(MirrorError::NoSuchPath(path.to_owned()));
        };
        match self.parent(path, parents)? {
            JsonValue::Object(members) => members
                .remove(last)
                .ok_or_else(|| MirrorError::NoSuchPath(path.to_owned())),
            JsonValue::Array(elements) => {
                let position = index(path, last)?;
                if position >= elements.len() {
                    return Err(MirrorError::IndexOutOfRange {
                        path: path.to_owned(),
                        index: position,
                    });
                }
                Ok(elements.remove(position))
            }
            _ => Err(MirrorError::NoSuchPath(path.to_owned())),
        }
    }

    fn replace(&mut self, path: &str, value: JsonValue) -> Result<(), MirrorError> {
        // RFC 6902: replace is only defined where something already is.
        self.get(path)?;
        self.add(path, value)
    }

    /// The container a pointer's last token lives in.
    fn parent(&mut self, path: &str, parents: &[String]) -> Result<&mut JsonValue, MirrorError> {
        let mut node = &mut self.root;
        for token in parents {
            node = match node {
                JsonValue::Object(members) => members
                    .get_mut(token)
                    .ok_or_else(|| MirrorError::NoSuchPath(path.to_owned()))?,
                JsonValue::Array(elements) => {
                    let position = index(path, token)?;
                    elements
                        .get_mut(position)
                        .ok_or_else(|| MirrorError::NoSuchPath(path.to_owned()))?
                }
                _ => return Err(MirrorError::NoSuchPath(path.to_owned())),
            };
        }
        Ok(node)
    }
}

/// Splits a JSON Pointer into its unescaped reference tokens (RFC 6901 §3).
fn tokens(path: &str) -> Result<Vec<String>, MirrorError> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    if !path.starts_with('/') {
        return Err(MirrorError::MalformedPointer(path.to_owned()));
    }
    // `~1` before `~0`, or an escaped `~1` would be turned into a separator.
    Ok(path
        .split('/')
        .skip(1)
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect())
}

/// A reference token used as an array index.
fn index(path: &str, token: &str) -> Result<usize, MirrorError> {
    // RFC 6901: an index is either "0" or digits not starting with zero, so
    // "01" and "+1" are not indexes. `parse` accepts "+1", hence the check.
    if token.starts_with('+') || (token.len() > 1 && token.starts_with('0')) {
        return Err(MirrorError::NotAnIndex {
            path: path.to_owned(),
            token: token.to_owned(),
        });
    }
    token.parse().map_err(|_| MirrorError::NotAnIndex {
        path: path.to_owned(),
        token: token.to_owned(),
    })
}

/// Whether `path` names something inside `from`.
fn is_inside(from: &str, path: &str) -> bool {
    path.strip_prefix(from)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::{MirrorError, SessionMirror, ShowMirror};
    use prism_domain::{Delta, ExecutorId, JsonPatchOp, JsonValue, PlaybackId};
    use std::collections::BTreeMap;

    fn document() -> JsonValue {
        JsonValue::Object(BTreeMap::from([
            (
                "fixtures".to_owned(),
                JsonValue::Object(BTreeMap::from([(
                    "1".to_owned(),
                    JsonValue::Object(BTreeMap::from([(
                        "name".to_owned(),
                        JsonValue::String("Front".to_owned()),
                    )])),
                )])),
            ),
            (
                "cues".to_owned(),
                JsonValue::Array(vec![JsonValue::Int(1), JsonValue::Int(2)]),
            ),
        ]))
    }

    fn mirror() -> ShowMirror {
        ShowMirror::new(document())
    }

    #[test]
    fn add_inserts_an_object_member() {
        let mut mirror = mirror();
        mirror
            .apply(&JsonPatchOp::Add {
                path: "/fixtures/2".to_owned(),
                value: JsonValue::Int(7),
            })
            .unwrap();
        assert_eq!(mirror.get("/fixtures/2").unwrap(), &JsonValue::Int(7));
    }

    #[test]
    fn add_inserts_into_an_array_and_appends_with_a_dash() {
        let mut mirror = mirror();
        mirror
            .apply(&JsonPatchOp::Add {
                path: "/cues/0".to_owned(),
                value: JsonValue::Int(0),
            })
            .unwrap();
        mirror
            .apply(&JsonPatchOp::Add {
                path: "/cues/-".to_owned(),
                value: JsonValue::Int(3),
            })
            .unwrap();
        assert_eq!(
            mirror.get("/cues").unwrap(),
            &JsonValue::Array(vec![
                JsonValue::Int(0),
                JsonValue::Int(1),
                JsonValue::Int(2),
                JsonValue::Int(3),
            ])
        );
    }

    #[test]
    fn an_empty_pointer_names_the_whole_document() {
        let mut mirror = mirror();
        mirror
            .apply(&JsonPatchOp::Add {
                path: String::new(),
                value: JsonValue::Null,
            })
            .unwrap();
        assert_eq!(mirror.into_value(), JsonValue::Null);
    }

    #[test]
    fn remove_takes_a_member_and_an_element() {
        let mut mirror = mirror();
        mirror
            .apply(&JsonPatchOp::Remove {
                path: "/fixtures/1".to_owned(),
            })
            .unwrap();
        assert_eq!(
            mirror.apply(&JsonPatchOp::Remove {
                path: "/fixtures/1".to_owned()
            }),
            Err(MirrorError::NoSuchPath("/fixtures/1".to_owned()))
        );
        mirror
            .apply(&JsonPatchOp::Remove {
                path: "/cues/0".to_owned(),
            })
            .unwrap();
        assert_eq!(
            mirror.get("/cues").unwrap(),
            &JsonValue::Array(vec![JsonValue::Int(2)])
        );
    }

    #[test]
    fn replace_needs_something_to_replace() {
        let mut mirror = mirror();
        assert_eq!(
            mirror.apply(&JsonPatchOp::Replace {
                path: "/fixtures/9".to_owned(),
                value: JsonValue::Null,
            }),
            Err(MirrorError::NoSuchPath("/fixtures/9".to_owned()))
        );
        mirror
            .apply(&JsonPatchOp::Replace {
                path: "/fixtures/1/name".to_owned(),
                value: JsonValue::String("Back".to_owned()),
            })
            .unwrap();
        assert_eq!(
            mirror.get("/fixtures/1/name").unwrap(),
            &JsonValue::String("Back".to_owned())
        );
    }

    #[test]
    fn move_and_copy_carry_a_value_across() {
        let mut mirror = mirror();
        mirror
            .apply(&JsonPatchOp::Copy {
                from: "/fixtures/1".to_owned(),
                path: "/fixtures/2".to_owned(),
            })
            .unwrap();
        mirror
            .apply(&JsonPatchOp::Move {
                from: "/fixtures/1".to_owned(),
                path: "/fixtures/3".to_owned(),
            })
            .unwrap();
        assert_eq!(
            mirror.get("/fixtures/2/name").unwrap(),
            &JsonValue::String("Front".to_owned())
        );
        assert_eq!(
            mirror.get("/fixtures/3/name").unwrap(),
            &JsonValue::String("Front".to_owned())
        );
        assert!(mirror.get("/fixtures/1").is_err());
    }

    #[test]
    fn a_move_into_its_own_source_is_refused() {
        let mut mirror = mirror();
        assert_eq!(
            mirror.apply(&JsonPatchOp::Move {
                from: "/fixtures".to_owned(),
                path: "/fixtures/1".to_owned(),
            }),
            Err(MirrorError::MoveIntoSelf {
                from: "/fixtures".to_owned(),
                path: "/fixtures/1".to_owned(),
            })
        );
        // A prefix that is not a path segment boundary is a different path.
        assert!(super::is_inside("/fixtures", "/fixtures"));
        assert!(!super::is_inside("/fixtures", "/fixturesheet"));
    }

    #[test]
    fn test_compares_and_does_not_write() {
        let mut mirror = mirror();
        mirror
            .apply(&JsonPatchOp::Test {
                path: "/fixtures/1/name".to_owned(),
                value: JsonValue::String("Front".to_owned()),
            })
            .unwrap();
        assert_eq!(
            mirror.apply(&JsonPatchOp::Test {
                path: "/fixtures/1/name".to_owned(),
                value: JsonValue::String("Back".to_owned()),
            }),
            Err(MirrorError::TestFailed {
                path: "/fixtures/1/name".to_owned()
            })
        );
    }

    #[test]
    fn a_pointer_has_to_be_a_pointer() {
        let mut mirror = mirror();
        assert_eq!(
            mirror.apply(&JsonPatchOp::Remove {
                path: "fixtures/1".to_owned()
            }),
            Err(MirrorError::MalformedPointer("fixtures/1".to_owned()))
        );
        assert_eq!(
            mirror.apply(&JsonPatchOp::Remove {
                path: String::new()
            }),
            Err(MirrorError::NoSuchPath(String::new()))
        );
    }

    #[test]
    fn an_array_is_addressed_by_index_and_nothing_else() {
        let mut mirror = mirror();
        for token in ["a", "01", "+1"] {
            assert_eq!(
                mirror.apply(&JsonPatchOp::Remove {
                    path: format!("/cues/{token}")
                }),
                Err(MirrorError::NotAnIndex {
                    path: format!("/cues/{token}"),
                    token: token.to_owned(),
                })
            );
        }
        assert_eq!(
            mirror.apply(&JsonPatchOp::Remove {
                path: "/cues/9".to_owned()
            }),
            Err(MirrorError::IndexOutOfRange {
                path: "/cues/9".to_owned(),
                index: 9,
            })
        );
        assert_eq!(
            mirror.apply(&JsonPatchOp::Add {
                path: "/cues/9".to_owned(),
                value: JsonValue::Null,
            }),
            Err(MirrorError::IndexOutOfRange {
                path: "/cues/9".to_owned(),
                index: 9,
            })
        );
    }

    #[test]
    fn a_pointer_reads_through_arrays_as_well_as_objects() {
        let mirror = mirror();
        assert_eq!(mirror.get("/cues/1").unwrap(), &JsonValue::Int(2));
        assert_eq!(
            mirror.get("/cues/9"),
            Err(MirrorError::NoSuchPath("/cues/9".to_owned()))
        );
        assert_eq!(
            mirror.get("/cues/x"),
            Err(MirrorError::NotAnIndex {
                path: "/cues/x".to_owned(),
                token: "x".to_owned(),
            })
        );
        // The whole document, which RFC 6901 spells as the empty pointer.
        assert_eq!(mirror.get("").unwrap(), mirror.value());
    }

    #[test]
    fn a_pointer_through_a_leaf_names_nothing() {
        let mut mirror = mirror();
        assert_eq!(
            mirror.apply(&JsonPatchOp::Add {
                path: "/fixtures/1/name/deeper/still".to_owned(),
                value: JsonValue::Null,
            }),
            Err(MirrorError::NoSuchPath(
                "/fixtures/1/name/deeper/still".to_owned()
            ))
        );
        assert_eq!(
            mirror.get("/fixtures/1/name/deeper"),
            Err(MirrorError::NoSuchPath(
                "/fixtures/1/name/deeper".to_owned()
            ))
        );
        assert_eq!(
            mirror.apply(&JsonPatchOp::Add {
                path: "/fixtures/1/name/deeper".to_owned(),
                value: JsonValue::Null,
            }),
            Err(MirrorError::NoSuchPath(
                "/fixtures/1/name/deeper".to_owned()
            ))
        );
        assert_eq!(
            mirror.apply(&JsonPatchOp::Remove {
                path: "/fixtures/1/name/deeper".to_owned(),
            }),
            Err(MirrorError::NoSuchPath(
                "/fixtures/1/name/deeper".to_owned()
            ))
        );
        assert_eq!(
            mirror.apply(&JsonPatchOp::Add {
                path: "/cues/0/deeper".to_owned(),
                value: JsonValue::Null,
            }),
            Err(MirrorError::NoSuchPath("/cues/0/deeper".to_owned()))
        );
        assert_eq!(
            mirror.apply(&JsonPatchOp::Add {
                path: "/nothing/here".to_owned(),
                value: JsonValue::Null,
            }),
            Err(MirrorError::NoSuchPath("/nothing/here".to_owned()))
        );
        assert_eq!(
            mirror.apply(&JsonPatchOp::Add {
                path: "/cues/7/here".to_owned(),
                value: JsonValue::Null,
            }),
            Err(MirrorError::NoSuchPath("/cues/7/here".to_owned()))
        );
    }

    #[test]
    fn a_batch_stops_at_the_operation_that_does_not_fit() {
        let mut mirror = mirror();
        let error = mirror
            .apply_all(&[
                JsonPatchOp::Add {
                    path: "/fixtures/2".to_owned(),
                    value: JsonValue::Int(2),
                },
                JsonPatchOp::Remove {
                    path: "/nothing".to_owned(),
                },
                JsonPatchOp::Add {
                    path: "/fixtures/3".to_owned(),
                    value: JsonValue::Int(3),
                },
            ])
            .unwrap_err();
        assert_eq!(error, MirrorError::NoSuchPath("/nothing".to_owned()));
        assert!(mirror.get("/fixtures/2").is_ok());
        assert!(mirror.get("/fixtures/3").is_err());
    }

    #[test]
    fn escaped_tokens_come_back_as_themselves() {
        let mut mirror = ShowMirror::new(JsonValue::Object(BTreeMap::new()));
        mirror
            .apply(&JsonPatchOp::Add {
                path: "/a~1b~0c".to_owned(),
                value: JsonValue::Int(1),
            })
            .unwrap();
        assert_eq!(
            mirror.value(),
            &JsonValue::Object(BTreeMap::from([("a/b~c".to_owned(), JsonValue::Int(1))]))
        );
    }

    #[test]
    fn executor_state_writes_the_two_fields_it_carries() {
        let mut mirror = ShowMirror::new(JsonValue::Object(BTreeMap::from([(
            "executors".to_owned(),
            JsonValue::Object(BTreeMap::from([(
                "3".to_owned(),
                JsonValue::Object(BTreeMap::from([
                    ("isActive".to_owned(), JsonValue::Bool(false)),
                    ("currentCueIndex".to_owned(), JsonValue::Null),
                ])),
            )])),
        )])));
        mirror
            .apply_delta(&Delta::PlaybackState {
                playback: PlaybackId::of_executor(ExecutorId::new(3)),
                is_active: true,
                cue_index: Some(2),
            })
            .unwrap();
        assert_eq!(
            mirror.get("/executors/3/isActive").unwrap(),
            &JsonValue::Bool(true)
        );
        assert_eq!(
            mirror.get("/executors/3/currentCueIndex").unwrap(),
            &JsonValue::Int(2)
        );

        mirror
            .apply_delta(&Delta::PlaybackState {
                playback: PlaybackId::of_executor(ExecutorId::new(3)),
                is_active: false,
                cue_index: None,
            })
            .unwrap();
        assert_eq!(
            mirror.get("/executors/3/currentCueIndex").unwrap(),
            &JsonValue::Null
        );
    }

    #[test]
    fn the_deltas_that_are_not_show_state_leave_the_mirror_alone() {
        let mut mirror = mirror();
        let before = mirror.value().clone();
        for delta in [
            Delta::SessionPatch { ops: vec![] },
            Delta::ProgrammerChanged {
                state: prism_domain::ProgrammerState::default(),
            },
            Delta::OutputHealth {
                output_id: prism_domain::OutputId::new(1),
                health: prism_domain::OutputHealth::Ok,
            },
            Delta::DirtyFlag {
                unsaved_changes: true,
            },
            Delta::Notice {
                level: prism_domain::NoticeLevel::Warn,
                message: "overlap".to_owned(),
            },
        ] {
            mirror.apply_delta(&delta).unwrap();
        }
        assert_eq!(mirror.value(), &before);
    }

    #[test]
    fn a_show_patch_delta_is_applied_as_operations() {
        let mut mirror = mirror();
        mirror
            .apply_delta(&Delta::ShowPatch {
                ops: vec![JsonPatchOp::Replace {
                    path: "/fixtures/1/name".to_owned(),
                    value: JsonValue::String("Side".to_owned()),
                }],
            })
            .unwrap();
        assert_eq!(
            mirror.get("/fixtures/1/name").unwrap(),
            &JsonValue::String("Side".to_owned())
        );
    }

    #[test]
    fn every_failure_says_something_readable() {
        for error in [
            MirrorError::MalformedPointer("x".to_owned()),
            MirrorError::NoSuchPath("/x".to_owned()),
            MirrorError::NotAnIndex {
                path: "/x/a".to_owned(),
                token: "a".to_owned(),
            },
            MirrorError::IndexOutOfRange {
                path: "/x/9".to_owned(),
                index: 9,
            },
            MirrorError::TestFailed {
                path: "/x".to_owned(),
            },
            MirrorError::MoveIntoSelf {
                from: "/x".to_owned(),
                path: "/x/y".to_owned(),
            },
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
        }
    }

    #[test]
    fn a_session_mirror_is_the_same_applier_over_the_session_document() {
        // The session's own document, and every method a client calls on it.
        let mut mirror = SessionMirror::new(JsonValue::Object(BTreeMap::from([
            (
                "session".to_owned(),
                JsonValue::Object(BTreeMap::from([(
                    "executorPage".to_owned(),
                    JsonValue::Int(0),
                )])),
            ),
            ("views".to_owned(), JsonValue::Object(BTreeMap::new())),
        ])));

        mirror
            .apply(&JsonPatchOp::Replace {
                path: "/session/executorPage".to_owned(),
                value: JsonValue::Int(3),
            })
            .unwrap();
        assert_eq!(
            mirror.get("/session/executorPage").unwrap(),
            &JsonValue::Int(3)
        );
        assert_eq!(
            mirror.get("/session/nothing"),
            Err(MirrorError::NoSuchPath("/session/nothing".to_owned()))
        );

        mirror
            .apply_delta(&Delta::SessionPatch {
                ops: vec![JsonPatchOp::Add {
                    path: "/views/2".to_owned(),
                    value: JsonValue::String("Programming".to_owned()),
                }],
            })
            .unwrap();
        // A delta belonging to the other document changes nothing here, and an
        // operation that does not fit is still reported.
        mirror
            .apply_delta(&Delta::ShowPatch {
                ops: vec![JsonPatchOp::Remove {
                    path: "/session".to_owned(),
                }],
            })
            .unwrap();
        assert_eq!(
            mirror.apply_delta(&Delta::SessionPatch {
                ops: vec![JsonPatchOp::Replace {
                    path: "/views/9".to_owned(),
                    value: JsonValue::Null,
                }],
            }),
            Err(MirrorError::NoSuchPath("/views/9".to_owned()))
        );

        assert_eq!(mirror.value(), &mirror.clone().into_value());
    }
}
