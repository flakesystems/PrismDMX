//! What a key's lamp says — S59.
//!
//! # The change this module *is*
//!
//! Until S59 the surface's LEDs hung on the **hardware position**. `surface.rs`
//! lit the Select lamp of a strip because it was a strip's, and the Save lamp
//! because it was Save's, and those were the only two. That arrangement cannot
//! survive a desk whose keys are bound: a Save lamp on a key that has been given
//! to `Store` is a lamp that lies, and it lies in the dark, which is where an
//! operator reads it.
//!
//! So the lamp follows **what the key is bound to**. Every control the profile
//! names is asked the same question, and the answer is a property of its action
//! rather than of the note number underneath it.
//!
//! # The question is: would pressing this now lead anywhere
//!
//! The owner's rule, taken in two halves (`IMPLEMENTATION_PLAN.md` S59 §6):
//!
//! - **the grammar** — would the command line accept what this key writes? Only
//!   an *append* key has a real answer here, because a *run* or a *write* key
//!   throws the standing line away and starts a fresh one.
//!   `prism_core::console::accepts_next` is the whole of it.
//! - **the meaning** — would it *do* something? `Store` with an empty programmer
//!   stores nothing, and `Update` with no cue open updates nothing.
//!
//! Lit means both. Nine actions have a meaning to check and the rest have
//! none — and an action with no meaningful state is **dark**, deliberately:
//! *nur Tasten bei denen die Einstufung sinnvoll ist, andere bleiben dauerhaft
//! dunkel*. A lamp that is always on tells an operator nothing and costs them a
//! glance to find that out.
//!
//! # The one lamp that is a state rather than an offer
//!
//! `SelectExecutor` on a strip key stays what it has always been: lit while that
//! strip's cue list is **running**. It is not an answer to *would this do
//! something* — selecting is always possible — and it was not invented here. It
//! is the only place §4.1's own table asks for a lamp that reports rather than
//! offers, and it is worth keeping separate in the reading rather than bent into
//! the rule.

use prism_domain::{ConsoleKey, ExecutorId, ExecutorTarget, KeyShape, SequenceId, SurfaceAction};
use prism_surface::LedState;

use crate::core::Core;

/// What a control's lamp should be doing, given what it is bound to.
///
/// `strip` is the strip a control sits on, for the actions that mean *the
/// executor under this one* — the same answer [`prism_surface::Bindings`] needs
/// to resolve an [`ExecutorTarget::Strip`], and `None` for a panel key.
///
/// An unbound control is [`LedState::Off`]: a key that does nothing has no
/// business being lit, and the surface never lights its own lamps (see
/// `prism_surface::feedback`), so dark is also what it would be if nobody asked.
#[must_use]
pub fn lamp_of(action: Option<&SurfaceAction>, strip: Option<u8>, core: &Core) -> LedState {
    let Some(action) = action else {
        return LedState::Off;
    };
    if lit(action, strip, core) {
        LedState::On
    } else {
        LedState::Off
    }
}

/// Whether this action's key should be lit.
fn lit(action: &SurfaceAction, strip: Option<u8>, core: &Core) -> bool {
    match action {
        SurfaceAction::ConsoleWord { word } => word_is_lit(word, core),
        // The show's own Save lamp, §4.1: "lit while unsaved changes exist". It
        // has always been this and it is the model the rest of this module was
        // written from.
        SurfaceAction::SaveShow => core.file.is_dirty(),
        SurfaceAction::Oops => oops_is_lit(core),
        SurfaceAction::Redo => core.file.journal.redoable().is_some(),
        SurfaceAction::ClearProgrammer => clear_is_lit(core),
        // The playback row: lit when the executor it acts on has a cue list to
        // act on. An operator pressing Go on an empty executor has done nothing,
        // and the lamp is what says so before the press rather than after.
        SurfaceAction::ExecutorGo { target, .. }
        | SurfaceAction::ExecutorOff { target }
        | SurfaceAction::ExecutorOn { target } => sequence_of(*target, strip, core).is_some(),
        // The one reporting lamp — see the module documentation.
        SurfaceAction::SelectExecutor { target } => sequence_of(*target, strip, core)
            .and_then(|id| core.file.show.sequence(id))
            .is_some_and(|sequence| sequence.is_active),
        // Everything else has no state worth a lamp: a window is neither open
        // nor closed from a key's point of view, a view is always reachable, an
        // encoder bank is always switchable, and a fader has no lamp at all.
        _ => false,
    }
}

/// A key of the console keypad: the grammar, and then the meaning.
fn word_is_lit(word: &str, core: &Core) -> bool {
    let Some(key) = ConsoleKey::named(word) else {
        // A word this build does not have. The daemon is the one that wrote the
        // table, so this is a profile carried back from a newer build — dark
        // rather than lit, because a lamp is a promise.
        return false;
    };
    let line = &core.file.session.session().command_line;
    match key.shape {
        // Oops writes nothing, so it has no grammar half: it is lit when there
        // is a word to take off the line, or an edit to take back (B58).
        KeyShape::Oops => oops_is_lit(core),
        // An append key's whole answer is the grammar's. `Cue` is dark on an
        // empty line and lit after `Store`, which is exactly the sequence an
        // operator's hand makes.
        KeyShape::Append => prism_core::console::accepts_next(line, word),
        // A run or a write key starts a fresh line, so the grammar always says
        // yes and the meaning is all there is.
        KeyShape::Run | KeyShape::Write => meaning_of(word, core),
    }
}

/// The nine hand-written conditions of `IMPLEMENTATION_PLAN.md` S59 §6.
///
/// A word that is not here has no meaning to check and is lit, because its
/// shape already said the line would take it. That is the rule the right way
/// round: *dark* is for an action with no state at all, and every one of those
/// is caught by [`lit`]'s final arm rather than by a word being missing here.
fn meaning_of(word: &str, core: &Core) -> bool {
    match word.to_ascii_lowercase().as_str() {
        // Store with an empty programmer writes an empty cue.
        "store" => !core.file.programmer.state().values.is_empty(),
        // Update needs a cue open in the programmer — `ShowFile::apply` refuses
        // it with `NothingIsBeingEdited` otherwise, and this is that refusal
        // said before the press instead of after it.
        "update" => core.file.session.session().editing_cue.is_some(),
        // Full acts on the selection, and an empty selection is nothing to act
        // on.
        "full" => !core.file.programmer.state().selection.is_empty(),
        "clear" => clear_is_lit(core),
        _ => true,
    }
}

/// Whether Clear has anything left to take.
///
/// **The owner's correction of 2026-09-20**, and the better rule: the lamp
/// follows the **stage the machine stands on**, not whether the programmer is
/// empty. The two agree most of the time and part company at the end — the last
/// press of a three-stage Clear leaves a programmer that is already empty of
/// values while the stage still has the encoder bank and the page to take
/// (`ClearStage::All`). A lamp reading the values would go dark one press early
/// and an operator would stop pressing one press early.
fn clear_is_lit(core: &Core) -> bool {
    core.file.programmer.state().stage() != prism_domain::ClearStage::Nothing
}

/// Whether Oops would do anything — B58's two answers in one.
///
/// A word to take off the line, or, on an empty line, an edit to take back.
fn oops_is_lit(core: &Core) -> bool {
    !core.file.session.session().command_line.trim().is_empty()
        || core.file.journal.undoable().is_some()
}

/// The cue list the executor this action acts on is carrying, if any.
fn sequence_of(target: ExecutorTarget, strip: Option<u8>, core: &Core) -> Option<SequenceId> {
    let session = core.file.session.session();
    let id = match target {
        ExecutorTarget::Selected => session.selected_executor?,
        ExecutorTarget::Strip => {
            ExecutorId::from_page_and_slot(session.executor_page, u32::from(strip?))
        }
    };
    core.file
        .show
        .executor(id)
        .and_then(|executor| executor.sequence_id)
}
