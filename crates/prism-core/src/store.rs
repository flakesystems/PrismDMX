//! The `.prism` file: what a show looks like on the platter.
//!
//! Everything above this module lives in memory. A show that is closed is a
//! show that is gone until this module writes it down, and the promise it makes
//! is narrow and absolute: **what comes back is what was written, byte for
//! byte, or the read fails loudly.**
//!
//! # Why SQLite, and what it is actually for here
//!
//! A show file is small — a school's rig is a few hundred kilobytes — so it is
//! not query power that earns the dependency. It is the write. `IMPLEMENTATION_PLAN.md`
//! S15 asks for a file that survives the process being killed in the middle of
//! saving it, and a transaction over a write-ahead log is the only way to get
//! that without writing a journalling file format by hand. The whole show is
//! written inside **one** transaction, so a reader sees the show as it was
//! before the save or as it is after it, and never a mixture of the two.
//!
//! # One row per thing, not one blob per file
//!
//! Each pool is a table keyed by the number the operator uses — `fixture` by
//! fixture number, `sequence` by sequence number — with the entity itself in a
//! `document` column. Two reasons, in order of importance:
//!
//! - **A damaged row costs one sequence, not the show.** A single blob is a
//!   file that is either readable or not.
//! - The keys are the same keys `Show` holds its collections under, so a load
//!   is a walk over rows rather than a rebuild, and a later session that wants
//!   to write only what changed has somewhere to write it.
//!
//! The documents are **MessagePack**, not JSON, and S1's finding is the reason:
//! `serde_json` writes the shortest text that round-trips through a correctly
//! rounded parser and does not itself have one, so a fixture's position would
//! come back a unit in the last place away from where it was hung. MessagePack
//! is binary IEEE 754 and exact. JSON is what [`export_json`] is for, and that
//! function documents the difference rather than hiding it.
//!
//! # What is deliberately not in the file
//!
//! Three things, each decided in an earlier session and each with no table
//! here:
//!
//! - **The programmer** (S13). Restored from disk it would be an absolute
//!   override of every playback, applied to a rig the moment a show opened.
//! - **The Oops journal** (S14). A record is an assertion about the state of
//!   *this* run; against a file that may have been edited since, its inverse
//!   describes a show that no longer exists.
//! - **The desk identity, and with it the sACN CID** (S11). It belongs to the
//!   machine, and copying a show is exactly how a second machine comes to have
//!   one — see [`crate::MachineConfig`].
//!
//! [`ShowStore::load`] therefore clears the journal and the programmer rather
//! than relying on them being absent from the file: it replaces `show` and
//! `session` in place, and a `ShowFile` that kept the other two would keep them
//! from the show that was open before.
//!
//! # Versions
//!
//! `PRAGMA application_id` says the file is a PrismDMX show; `PRAGMA
//! user_version` says which schema it holds. [`ShowStore::open`] migrates a
//! file forward through [`MIGRATIONS`] and refuses one written by a newer
//! PrismDMX, because guessing at a schema nobody here has seen is how a show
//! gets quietly truncated.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use prism_domain::{
    Delta, Executor, ExecutorId, Fixture, FixtureType, Group, Preset, Sequence, SequenceId,
    Session, SessionId, View, ViewId,
};
use rusqlite::{Connection, OpenFlags, Transaction};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::command::Effect;
use crate::file::ShowFile;
use crate::programmer::Programmer;
use crate::session::SessionState;
use crate::show::Show;

/// Why a `.prism` file could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// SQLite refused: the file is not a database, the disk is full, another
    /// process holds the write lock.
    Database(String),
    /// The file system refused — creating, copying or removing a file.
    Io(String),
    /// A SQLite database that is not a PrismDMX show.
    NotAShowFile {
        /// The `application_id` the file actually carries.
        application_id: i32,
    },
    /// Written by a newer PrismDMX than this one.
    FutureVersion {
        /// The `user_version` in the file.
        found: u32,
        /// The newest schema this build knows.
        supported: u32,
    },
    /// A row that does not decode, or that disagrees with its own key.
    Damaged(String),
    /// The show could not be encoded. `prism-domain` refuses non-finite floats
    /// in both directions, so this is what a NaN that reached the model looks
    /// like on the way to the platter — and it happens **before** anything is
    /// written.
    NotRepresentable(String),
}

impl core::fmt::Display for StoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Database(reason) => write!(f, "the show file could not be used: {reason}"),
            Self::Io(reason) => write!(f, "the show file could not be reached: {reason}"),
            Self::NotAShowFile { application_id } => write!(
                f,
                "this is not a PrismDMX show file (application id {application_id:#010x})"
            ),
            Self::FutureVersion { found, supported } => write!(
                f,
                "this show file is version {found}; this PrismDMX reads up to version {supported}"
            ),
            Self::Damaged(reason) => write!(f, "the show file is damaged: {reason}"),
            Self::NotRepresentable(reason) => {
                write!(f, "the show cannot be written: {reason}")
            }
        }
    }
}

impl core::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

/// The schema, one statement block per version.
///
/// The index in this list plus one **is** the `user_version` it produces, so a
/// file at version *n* has had the first *n* of these applied to it and a new
/// version is an entry appended here. Nothing is ever edited in place: a
/// migration that changed retrospectively would mean two files claiming the
/// same version with different tables in them.
///
/// - **Version 1** is the show: the patch, the profiles it embeds and the pools
///   stored on top. It is what a PrismDMX before S12 would have written, and it
///   is the version `tests/fixtures/version-1.prism` is frozen at.
/// - **Version 2** adds the session (`ARCHITECTURE_SPEC.md` §4.1) and the views
///   it selects between. A version-1 file therefore opens in the session a
///   fresh desk starts in, which is the only honest answer: it never had one.
pub const MIGRATIONS: [&str; 2] = [
    "CREATE TABLE fixture_type (id TEXT PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;
     CREATE TABLE fixture      (id INTEGER PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;
     CREATE TABLE fixture_group(id INTEGER PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;
     CREATE TABLE preset       (id INTEGER PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;
     CREATE TABLE sequence     (id INTEGER PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;
     CREATE TABLE executor     (id INTEGER PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;",
    "CREATE TABLE session      (id INTEGER PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;
     CREATE TABLE stored_view  (id INTEGER PRIMARY KEY NOT NULL, document BLOB NOT NULL) STRICT;",
];

/// The show tables, in the order a load walks them.
const SHOW_TABLES: [&str; 6] = [
    "fixture_type",
    "fixture",
    "fixture_group",
    "preset",
    "sequence",
    "executor",
];

/// The session tables, added in version 2.
const SESSION_TABLES: [&str; 2] = ["session", "stored_view"];

/// A show file, open.
///
/// Holds one connection, which is the daemon's. A second `ShowStore` on the
/// same path is another writer, and SQLite will make one of them wait and then
/// fail — which is the behaviour a desk wants: two processes writing one show
/// is the failure `ARCHITECTURE_SPEC.md` §10.3 prevents structurally with a
/// lock file, and this is the backstop underneath it.
#[derive(Debug)]
pub struct ShowStore {
    path: PathBuf,
    connection: Connection,
}

impl ShowStore {
    /// The newest schema this build writes and reads.
    ///
    /// One more than the index of the last entry in [`MIGRATIONS`], because a
    /// version number that has to be kept in step with a list by hand is a
    /// version number that will not be.
    pub const FORMAT_VERSION: u32 = MIGRATIONS.len() as u32;

    /// `PRAGMA application_id` for a PrismDMX show: ASCII `PRSM`.
    ///
    /// SQLite's own recommendation for an application file format, and what
    /// tells a show file from any other database that happens to be called
    /// `.prism`.
    pub const APPLICATION_ID: i32 = 0x5052_534d;

    /// How long a write waits for another writer before giving up.
    ///
    /// Deliberately short. The daemon is the only writer of its own show, so a
    /// lock held by anybody else is a backup tool or a second daemon, and
    /// freezing the desk for five seconds while a Save LED stays lit is worse
    /// than saying so.
    pub const BUSY_TIMEOUT: Duration = Duration::from_millis(250);

    /// Opens a show file, creating it if it is not there and migrating it
    /// forward if it is older than this build.
    ///
    /// # Errors
    ///
    /// [`StoreError::Database`] if the path is not a SQLite database or cannot
    /// be opened, [`StoreError::NotAShowFile`] if it is a database that belongs
    /// to something else, [`StoreError::FutureVersion`] if it was written by a
    /// newer PrismDMX.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI,
        )?;
        connection.busy_timeout(Self::BUSY_TIMEOUT)?;
        // A write-ahead log is what makes a killed process survivable: a
        // committed transaction is durable the moment its commit record is in
        // the log, and an interrupted one leaves frames nobody will ever read.
        // `pragma_query_value` rather than `execute`, because this pragma
        // answers with the mode it ended up in.
        let mode: String =
            connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StoreError::Database(format!(
                "the file system under {} does not support a write-ahead log (journal mode {mode})",
                path.display()
            )));
        }
        // NORMAL is the documented pairing with WAL: a commit is durable
        // against a killed *process* without an fsync per transaction, and a
        // killed *machine* can lose the last commits. FULL would cost a disk
        // flush on every autosave for a guarantee a lighting desk on mains
        // power does not need.
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", true)?;

        let store = Self { path, connection };
        store.identify()?;
        store.migrate()?;
        Ok(store)
    }

    /// Where this file is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The schema version the file holds, which after [`Self::open`] is always
    /// [`Self::FORMAT_VERSION`].
    #[must_use]
    pub fn version(&self) -> u32 {
        self.user_version().unwrap_or(0)
    }

    /// Checks the file's own structure — SQLite's `integrity_check`.
    ///
    /// What an operator runs on a show that has been through a crash, and what
    /// the crash test asserts before it believes a word the file says.
    ///
    /// # Errors
    ///
    /// [`StoreError::Damaged`] with SQLite's report if the file is not intact.
    pub fn integrity_check(&self) -> Result<(), StoreError> {
        let report: String = self
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if report == "ok" {
            Ok(())
        } else {
            Err(StoreError::Damaged(report))
        }
    }

    // -- writing ----------------------------------------------------------

    /// Writes the show and the session, and marks the file saved.
    ///
    /// Returns the `Delta::DirtyFlag` to broadcast, which is empty when the
    /// lamp was already out: an LED cannot be turned off twice, and S11 made
    /// "the flag travels when it changes" a rule rather than a habit.
    ///
    /// **The order is S11's rule applied to the platter.** Everything is
    /// encoded first, then written inside one transaction, and
    /// [`ShowFile::mark_saved`] is called only once the commit has returned. A
    /// failed write therefore leaves the file on disk as it was, the state in
    /// memory byte-identical, and the Save LED lit — which is the truth, since
    /// nothing was saved.
    ///
    /// The answer also carries a `Delta::Notice` if the recovery copy could not
    /// be cleared away afterwards, which is a complaint about the tidying and
    /// not about the save.
    ///
    /// # Errors
    ///
    /// [`StoreError::NotRepresentable`] if the show cannot be encoded — before
    /// anything is written — or [`StoreError::Database`] if the write fails.
    pub fn save(&mut self, file: &mut ShowFile) -> Result<Vec<Delta>, StoreError> {
        let rows = Rows::encode(file)?;
        let transaction = self.connection.transaction()?;
        rows.write(&transaction)?;
        transaction.commit()?;
        // Only now: the bytes are on the platter.
        let changed = file.mark_saved();
        let mut deltas = Vec::new();
        if changed {
            deltas.push(Delta::DirtyFlag {
                unsaved_changes: false,
            });
        }
        // A recovery copy describes edits that were not in the file. They are
        // in it now, so it is no longer a rescue but a trap for whoever finds
        // it next — and if it cannot be removed, that is worth saying and is
        // **not** worth calling the save a failure. The show is on the platter;
        // reporting otherwise would light the Save LED over a file that is
        // written, which is the one thing the lamp must never do.
        if let Err(error) = self.discard_recovery() {
            deltas.push(Delta::Notice {
                level: prism_domain::NoticeLevel::Warn,
                message: format!(
                    "the show was saved, but the recovery copy could not be removed: {error}"
                ),
            });
        }
        Ok(deltas)
    }

    /// Where the autosave copy of this file lives: `aula.prism` →
    /// `aula.recovery.prism`.
    ///
    /// A `.prism` file like any other, so the daemon that finds one after a
    /// crash opens it with [`Self::open`] and shows the operator what it holds
    /// rather than having to understand a second format.
    #[must_use]
    pub fn recovery_path(&self) -> PathBuf {
        let stem = self
            .path
            .file_stem()
            .map_or_else(|| "show".to_owned(), |stem| stem.to_string_lossy().into());
        self.path.with_file_name(format!("{stem}.recovery.prism"))
    }

    /// Whether a recovery copy is sitting beside this file.
    #[must_use]
    pub fn has_recovery(&self) -> bool {
        self.recovery_path().is_file()
    }

    /// Writes the current state into the recovery copy — the autosave.
    ///
    /// **It does not mark the file saved**, and that is the whole point: the
    /// operator asked for the show to be written to *their* file, and until it
    /// is, the Save LED is telling the truth. What this buys is that a crash
    /// costs at most [`Autosave::INTERVAL`] of work instead of everything since
    /// the last save.
    ///
    /// # Errors
    ///
    /// As [`Self::save`].
    pub fn write_recovery(&self, file: &ShowFile) -> Result<(), StoreError> {
        let rows = Rows::encode(file)?;
        let path = self.recovery_path();
        // A fresh file every time rather than an incremental update: the copy
        // has one reader, and it is one that arrives after a crash.
        remove_database(&path)?;
        let mut copy = Self::open(&path)?;
        let transaction = copy.connection.transaction()?;
        rows.write(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    /// Removes the recovery copy, if there is one.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] if it exists and cannot be removed.
    pub fn discard_recovery(&self) -> Result<(), StoreError> {
        remove_database(&self.recovery_path())
    }

    // -- reading ----------------------------------------------------------

    /// Reads the file into `file`, replacing the show and the session.
    ///
    /// Returns what the engine has to rebuild, which after a load is
    /// everything: the patch is a different rig, the groups are different
    /// groups and every sequence is a different cue list.
    ///
    /// **The journal and the programmer are cleared**, and not as a
    /// convenience. Both describe the show that was open a moment ago: an
    /// [`crate::UndoRecord`] would put a fixture back into a patch it never
    /// belonged to, and a [`Programmer`] left holding values is an absolute
    /// override applied to a rig that has just changed underneath it.
    ///
    /// # Errors
    ///
    /// [`StoreError::Damaged`] if a row does not decode or disagrees with its
    /// own key, [`StoreError::Database`] if the file cannot be read.
    pub fn load(&self, file: &mut ShowFile) -> Result<Vec<Effect>, StoreError> {
        let read = self.read()?;
        file.show = read.show;
        file.session = read.session;
        file.programmer = Programmer::new();
        file.journal.clear();

        let mut effects = vec![Effect::Repatch, Effect::ReloadGroups];
        effects.extend(
            file.show
                .sequences()
                .map(|sequence| Effect::ReloadSequence(sequence.id)),
        );
        Ok(effects)
    }

    /// Moves the level and the rate a pre-S45 executor carried onto the cue
    /// list standing on it.
    ///
    /// **The half `#[serde(default)]` cannot do.** S45 took `masterLevel` and
    /// `speed` off `prism_domain::Executor` and put one of each on
    /// `prism_domain::Sequence`, because a playback is a cue list's and two
    /// executors on one list were moving two numbers (punch-list entry B18). A
    /// default on the sequence answers *what does a file that never had one
    /// say* — full, and unity — but it cannot answer *what did the operator
    /// leave the fader at*, because that number is in the executor's row.
    ///
    /// So the row is read twice: once as an `Executor`, which now ignores the
    /// two fields, and once as [`LegacyExecutorLevel`], which is only those two.
    /// A `.prism` file keeps each row as a MessagePack **map** (`to_vec_named`,
    /// S1), so a partial decode is an ordinary read rather than a trick.
    ///
    /// **The lowest-numbered executor wins** when two of them carry one list at
    /// different levels. There is no right answer — the file records a state
    /// this model says cannot exist — and *the first fader* is the one an
    /// operator would point at. The other fader jumps to it on the next paint,
    /// which is the whole point of the session.
    ///
    /// A file written by this build has neither field, so this reads nothing and
    /// changes nothing.
    fn carry_levels_onto_their_cue_lists(&self, show: &mut Show) -> Result<(), StoreError> {
        let legacy: BTreeMap<ExecutorId, LegacyExecutorLevel> =
            self.rows_by_id("executor", |row: &LegacyExecutorLevel| row.id)?;
        let mut done: BTreeSet<SequenceId> = BTreeSet::new();
        for (id, row) in legacy {
            let Some(sequence_id) = show.executor(id).and_then(|executor| executor.sequence_id)
            else {
                continue;
            };
            if !done.insert(sequence_id) {
                continue;
            }
            // The cue list is there — it is the one the executor names, and
            // `Show::from_parts` has already been given both pools — so a
            // refusal here is a file whose executor points at nothing, which is
            // `Show::issues`' complaint rather than a reason not to open it.
            if let Some(level) = row.master_level {
                let _ = show.set_sequence_master(sequence_id, level);
            }
            if let Some(speed) = row.speed {
                let _ = show.set_sequence_speed(sequence_id, speed);
            }
        }
        // Reading a file is not an edit, and the levels were already in it.
        show.mark_saved();
        Ok(())
    }

    /// Reads the file into a fresh [`ShowFile`].
    ///
    /// # Errors
    ///
    /// As [`Self::load`].
    pub fn read(&self) -> Result<ShowFile, StoreError> {
        let mut show = Show::from_parts(
            self.rows_by_text("fixture_type", |fixture_type: &FixtureType| {
                fixture_type.id.clone()
            })?,
            self.rows_by_id("fixture", |fixture: &Fixture| fixture.id)?,
            self.rows_by_id("fixture_group", |group: &Group| group.id)?,
            self.rows_by_id("preset", |preset: &Preset| preset.id)?,
            self.rows_by_id("sequence", |sequence: &Sequence| sequence.id)?,
            self.rows_by_id("executor", |executor: &Executor| executor.id)?,
        );
        self.carry_levels_onto_their_cue_lists(&mut show)?;
        let sessions: BTreeMap<SessionId, Session> =
            self.rows_by_id("session", |session: &Session| session.id)?;
        let views: BTreeMap<ViewId, View> =
            self.rows_by_id("stored_view", |view: &View| view.id)?;
        // One session, and `ARCHITECTURE_SPEC.md` §4.4 says why: "V1 has
        // exactly one session". A file holding several was written by a
        // PrismDMX this one does not know how to be.
        let session = match sessions.into_values().next() {
            Some(session) => {
                // The two invariants S12 holds the session to. A file is the
                // one place they can arrive broken — every edit in `session.rs`
                // maintains them — and an active view that is not a view is a
                // console with nothing on the canvas and no way to say so.
                if !views.contains_key(&session.active_view_id) {
                    return Err(StoreError::Damaged(format!(
                        "the session is in view {}, which the file does not have",
                        session.active_view_id
                    )));
                }
                if let Some(focused) = session.focused_window
                    && !session
                        .open_windows
                        .iter()
                        .any(|window| window.instance_id == focused)
                {
                    return Err(StoreError::Damaged(format!(
                        "the session focuses window {focused}, which is not open"
                    )));
                }
                SessionState::from_parts(session, views)
            }
            // A version-1 file, migrated: it never had a session, so the desk
            // starts in the one a fresh desk starts in.
            None => SessionState::new(),
        };
        Ok(ShowFile {
            show,
            session,
            ..ShowFile::new()
        })
    }

    // -- the file format --------------------------------------------------

    /// Reads a table into a map keyed by the identifier inside each document,
    /// checking that the key the row was filed under agrees with it.
    fn rows_by_id<T, K>(
        &self,
        table: &str,
        key: impl Fn(&T) -> K,
    ) -> Result<BTreeMap<K, T>, StoreError>
    where
        T: DeserializeOwned,
        K: Ord + Into<u32> + Copy + core::fmt::Display,
    {
        let mut statement = self
            .connection
            .prepare(&format!("SELECT id, document FROM {table} ORDER BY id"))?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut map = BTreeMap::new();
        for row in rows {
            let (id, document) = row?;
            let value: T = decode(table, &id.to_string(), &document)?;
            let inner = key(&value);
            let raw: u32 = inner.into();
            if i64::from(raw) != id {
                return Err(StoreError::Damaged(format!(
                    "{table} row {id} holds {inner}, which is a different {table}"
                )));
            }
            map.insert(inner, value);
        }
        Ok(map)
    }

    /// The same for the one table whose key is text: the profile library, which
    /// is keyed by `FixtureType::id`.
    fn rows_by_text<T: DeserializeOwned>(
        &self,
        table: &str,
        key: impl Fn(&T) -> String,
    ) -> Result<BTreeMap<String, T>, StoreError> {
        let mut statement = self
            .connection
            .prepare(&format!("SELECT id, document FROM {table} ORDER BY id"))?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut map = BTreeMap::new();
        for row in rows {
            let (id, document) = row?;
            let value: T = decode(table, &id, &document)?;
            let inner = key(&value);
            if inner != id {
                return Err(StoreError::Damaged(format!(
                    "{table} row {id:?} holds {inner:?}, which is a different {table}"
                )));
            }
            map.insert(inner, value);
        }
        Ok(map)
    }

    /// Stamps a new file with the application id, or refuses one that carries
    /// somebody else's.
    fn identify(&self) -> Result<(), StoreError> {
        let found: i32 = self
            .connection
            .query_row("PRAGMA application_id", [], |row| row.get(0))?;
        if found == Self::APPLICATION_ID {
            return Ok(());
        }
        // Zero and an empty schema is a database that has just been created by
        // the `open` above; zero with tables in it is somebody else's file that
        // simply never stamped one, and writing a show into it would destroy
        // whatever it holds.
        if found == 0 && self.user_version()? == 0 && !self.has_any_table()? {
            self.connection
                .pragma_update(None, "application_id", Self::APPLICATION_ID)?;
            return Ok(());
        }
        Err(StoreError::NotAShowFile {
            application_id: found,
        })
    }

    /// Applies every migration the file has not had yet.
    ///
    /// One transaction per version, with the `user_version` set inside it, so a
    /// process killed between two migrations leaves a file at a version that
    /// really is the version it claims.
    fn migrate(&self) -> Result<(), StoreError> {
        let found = self.user_version()?;
        if found > Self::FORMAT_VERSION {
            return Err(StoreError::FutureVersion {
                found,
                supported: Self::FORMAT_VERSION,
            });
        }
        let applied = usize::try_from(found).unwrap_or(usize::MAX);
        for (index, migration) in MIGRATIONS.iter().enumerate().skip(applied) {
            let version = u32::try_from(index).unwrap_or(u32::MAX) + 1;
            self.connection.execute_batch(&format!(
                "BEGIN; {migration} PRAGMA user_version = {version}; COMMIT;"
            ))?;
        }
        Ok(())
    }

    fn user_version(&self) -> Result<u32, StoreError> {
        let version: i64 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        Ok(u32::try_from(version).unwrap_or(u32::MAX))
    }

    fn has_any_table(&self) -> Result<bool, StoreError> {
        let count: i64 = self.connection.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

/// Removes a database and the two files SQLite keeps beside it.
fn remove_database(path: &Path) -> Result<(), StoreError> {
    for path in [
        path.to_path_buf(),
        sidecar(path, "-wal"),
        sidecar(path, "-shm"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(StoreError::Io(format!("{}: {error}", path.display())));
            }
        }
    }
    Ok(())
}

/// `aula.prism` → `aula.prism-wal`, which is what SQLite calls its sidecars.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// The two fields S45 took off an executor, read back out of the same row.
///
/// Deserialise-only, and every field optional: a row written by this build
/// carries neither, and a row written before S34 carries `masterLevel` without
/// `speed`. See [`ShowStore::carry_levels_onto_their_cue_lists`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyExecutorLevel {
    /// The executor the row is, which `rows_by_id` checks against the key.
    id: ExecutorId,
    /// The master this fader was left at.
    #[serde(default)]
    master_level: Option<u16>,
    /// The rate it was playing at.
    #[serde(default)]
    speed: Option<u16>,
}

/// One document, decoded, with the row it came from named if it will not.
fn decode<T: DeserializeOwned>(table: &str, id: &str, document: &[u8]) -> Result<T, StoreError> {
    rmp_serde::from_slice(document)
        .map_err(|error| StoreError::Damaged(format!("{table} row {id}: {error}")))
}

/// One document, encoded.
///
/// `to_vec_named` rather than `to_vec`, for S1's reason: MessagePack's default
/// encoding of a struct is an array, which cannot carry a field name, and every
/// codec in this project therefore writes maps.
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    rmp_serde::to_vec_named(value).map_err(|error| StoreError::NotRepresentable(error.to_string()))
}

/// Everything a save is about to write, encoded and in memory.
///
/// It exists so that the encoding — the last step that can fail, S11's
/// finding — happens before the transaction is opened rather than inside it.
struct Rows {
    fixture_types: Vec<(String, Vec<u8>)>,
    fixtures: Vec<(i64, Vec<u8>)>,
    groups: Vec<(i64, Vec<u8>)>,
    presets: Vec<(i64, Vec<u8>)>,
    sequences: Vec<(i64, Vec<u8>)>,
    executors: Vec<(i64, Vec<u8>)>,
    session: Vec<(i64, Vec<u8>)>,
    views: Vec<(i64, Vec<u8>)>,
}

impl Rows {
    fn encode(file: &ShowFile) -> Result<Self, StoreError> {
        let show = &file.show;
        Ok(Self {
            fixture_types: numbered_text(show.fixture_types(), |value| value.id.clone())?,
            fixtures: numbered(show.fixtures(), |value: &Fixture| value.id.get())?,
            groups: numbered(show.groups(), |value: &Group| value.id.get())?,
            presets: numbered(show.presets(), |value: &Preset| value.id.get())?,
            sequences: numbered(show.sequences(), |value: &Sequence| value.id.get())?,
            executors: numbered(show.executors(), |value: &Executor| value.id.get())?,
            session: numbered(
                core::iter::once(file.session.session()),
                |value: &Session| value.id.get(),
            )?,
            views: numbered(file.session.views(), |value: &View| value.id.get())?,
        })
    }

    /// Writes every table inside one transaction.
    ///
    /// Delete-then-insert rather than a difference: a save writes the show that
    /// is in memory, and a row left behind by an entity the operator deleted
    /// would come back the next time the file was opened.
    fn write(&self, transaction: &Transaction<'_>) -> Result<(), StoreError> {
        for table in SHOW_TABLES.iter().chain(SESSION_TABLES.iter()) {
            transaction.execute(&format!("DELETE FROM {table}"), [])?;
        }
        for (id, document) in &self.fixture_types {
            transaction.execute(
                "INSERT INTO fixture_type (id, document) VALUES (?1, ?2)",
                rusqlite::params![id, document],
            )?;
        }
        for (table, rows) in [
            ("fixture", &self.fixtures),
            ("fixture_group", &self.groups),
            ("preset", &self.presets),
            ("sequence", &self.sequences),
            ("executor", &self.executors),
            ("session", &self.session),
            ("stored_view", &self.views),
        ] {
            let mut statement = transaction.prepare(&format!(
                "INSERT INTO {table} (id, document) VALUES (?1, ?2)"
            ))?;
            for (id, document) in rows {
                statement.execute(rusqlite::params![id, document])?;
            }
        }
        Ok(())
    }
}

/// Encodes a collection into `(key, document)` rows.
fn numbered<'a, T: Serialize + 'a>(
    values: impl Iterator<Item = &'a T>,
    key: impl Fn(&T) -> u32,
) -> Result<Vec<(i64, Vec<u8>)>, StoreError> {
    values
        .map(|value| Ok((i64::from(key(value)), encode(value)?)))
        .collect()
}

/// The same for the profile library, whose key is text.
fn numbered_text<'a, T: Serialize + 'a>(
    values: impl Iterator<Item = &'a T>,
    key: impl Fn(&T) -> String,
) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
    values
        .map(|value| Ok((key(value), encode(value)?)))
        .collect()
}

// -- autosave --------------------------------------------------------------

/// When to write the recovery copy.
///
/// A policy and nothing else: it owns no clock, no thread and no file, because
/// `prism-core` owns none of those. The daemon calls [`Autosave::poll`] with
/// the time since it started and whether the file is dirty, and writes
/// [`ShowStore::write_recovery`] when the answer is yes. That keeps the rule
/// testable on a simulated clock, which is how the interval is asserted here
/// rather than by waiting thirty seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autosave {
    interval: Duration,
    /// When the file became dirty, or `None` while it is clean.
    since: Option<Duration>,
}

impl Default for Autosave {
    fn default() -> Self {
        Self::new()
    }
}

impl Autosave {
    /// `IMPLEMENTATION_PLAN.md` S15: every thirty seconds.
    pub const INTERVAL: Duration = Duration::from_secs(30);

    /// The default policy.
    #[must_use]
    pub const fn new() -> Self {
        Self::every(Self::INTERVAL)
    }

    /// A policy at another interval — the settings dialogue, and the tests.
    #[must_use]
    pub const fn every(interval: Duration) -> Self {
        Self {
            interval,
            since: None,
        }
    }

    /// How often a recovery copy is written while the file stays dirty.
    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    /// Whether a recovery copy is due, given the time since the daemon started
    /// and whether there is anything to recover.
    ///
    /// **The clock starts when the file becomes dirty**, not when the daemon
    /// did, so the first autosave lands one interval after the operator's first
    /// unsaved edit rather than immediately after a long clean spell. Saving
    /// the show resets it: the next autosave is an interval after the *next*
    /// edit.
    ///
    /// Answering yes marks the interval as spent, so a caller that ignores the
    /// answer loses that autosave — which is deliberate. A write that failed is
    /// retried at the next interval rather than at every tick, because a disk
    /// that has just refused is not going to be persuaded 22 milliseconds
    /// later.
    pub fn poll(&mut self, now: Duration, dirty: bool) -> bool {
        if !dirty {
            self.since = None;
            return false;
        }
        let since = *self.since.get_or_insert(now);
        if now.saturating_sub(since) < self.interval {
            return false;
        }
        self.since = Some(now);
        true
    }
}

// -- JSON export and import ------------------------------------------------

/// What [`export_json`] writes and [`import_json`] reads.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    /// Says what this document is, so a JSON file picked up from a stick is
    /// recognisable without knowing where it came from.
    format: String,
    /// The schema version, the same number [`ShowStore::FORMAT_VERSION`] holds.
    version: u32,
    show: Show,
    /// Absent in a version-1 export, which had no session half.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session: Option<SessionState>,
}

/// The `format` member of an export, so a JSON document can say what it is.
const EXPORT_FORMAT: &str = "prismdmx.show";

/// The show and its session as JSON text.
///
/// **This is the interchange format, not the authoritative one.** A `.prism`
/// file is exact; JSON is not, and S1 measured why: `serde_json` writes the
/// shortest text that round-trips through a correctly rounded parser and does
/// not have one, so a float can come back a unit in the last place away from
/// the value that was exported. For a fixture's position in the 3D view that is
/// invisible; for "the loaded show is byte-identical to the saved one" it is
/// false, and the exit criterion is asserted against the file rather than
/// against this.
///
/// What it is for: reading a show in a text editor, diffing two versions of one
/// in version control, and moving a show between machines whose PrismDMX builds
/// disagree about the schema.
///
/// # Errors
///
/// [`StoreError::NotRepresentable`] if the show cannot be encoded.
pub fn export_json(file: &ShowFile) -> Result<String, StoreError> {
    let export = Export {
        format: EXPORT_FORMAT.to_owned(),
        version: ShowStore::FORMAT_VERSION,
        show: file.show.clone(),
        session: Some(file.session.clone()),
    };
    serde_json::to_string_pretty(&export)
        .map_err(|error| StoreError::NotRepresentable(error.to_string()))
}

/// Reads back what [`export_json`] wrote.
///
/// The result is a new show file: nothing to undo, nothing in the programmer,
/// and — like every show — no desk identity, because that belongs to the
/// machine that opens it (S11).
///
/// # Errors
///
/// [`StoreError::Damaged`] if the text is not an export, or holds a value the
/// domain refuses (a non-finite float, an unknown enumerator);
/// [`StoreError::FutureVersion`] if it was written by a newer PrismDMX.
pub fn import_json(text: &str) -> Result<ShowFile, StoreError> {
    // The version is read before the document, so a newer export is refused
    // with the reason rather than with whatever its first unknown field is.
    let envelope: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| StoreError::Damaged(format!("this is not JSON: {error}")))?;
    match envelope.get("format").and_then(serde_json::Value::as_str) {
        Some(EXPORT_FORMAT) => {}
        _ => {
            return Err(StoreError::Damaged(
                "this is not a PrismDMX show export".to_owned(),
            ));
        }
    }
    let version = envelope
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let version = u32::try_from(version).unwrap_or(u32::MAX);
    if version > ShowStore::FORMAT_VERSION {
        return Err(StoreError::FutureVersion {
            found: version,
            supported: ShowStore::FORMAT_VERSION,
        });
    }

    let export: Export = serde_json::from_str(text)
        .map_err(|error| StoreError::Damaged(format!("this export cannot be read: {error}")))?;
    Ok(ShowFile {
        show: export.show,
        // A version-1 export has no session half, and the desk it is opened on
        // starts in the session a fresh desk starts in — `SessionState`'s
        // `Default` is `new`, which is one view and nothing open.
        session: export.session.unwrap_or_default(),
        ..ShowFile::new()
    })
}

#[cfg(test)]
mod tests {
    use super::{Autosave, MIGRATIONS, ShowStore, StoreError, export_json, import_json};
    use crate::ShowFile;
    use crate::testkit::{fixture, par_type, sequence};
    use std::time::Duration;

    /// A show with something in every table, saved into a temporary file.
    fn saved(directory: &std::path::Path) -> (ShowStore, ShowFile) {
        let mut file = ShowFile::new();
        file.show.embed_fixture_type(par_type()).unwrap();
        file.show
            .patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        file.show.store_sequence(sequence(1, Vec::new())).unwrap();
        let mut store = ShowStore::open(directory.join("aula.prism")).unwrap();
        store.save(&mut file).unwrap();
        (store, file)
    }

    /// One executor row as a PrismDMX **before S45** wrote it: the assignment,
    /// and the four fields that have since moved onto the cue list.
    ///
    /// Written out by hand rather than taken from a struct that no longer has
    /// them, which is the point — a fixture built by the code under test cannot
    /// say anything about a file that code has never written.
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PreS45Executor {
        id: u32,
        sequence_id: Option<u32>,
        fader_function: &'static str,
        button_functions: Vec<&'static str>,
        encoder_function: &'static str,
        master_level: u16,
        speed: u16,
        is_active: bool,
        current_cue_index: Option<u32>,
    }

    /// **S45's migration**: the level and the rate a pre-S45 executor carried
    /// end up on the cue list standing on it.
    ///
    /// Two things a `#[serde(default)]` on `Sequence` cannot say, and this is
    /// the test for both. The *rate* as well as the level, because a file
    /// written between S34 and S45 carries one. And **the lowest-numbered
    /// executor wins** when two of them hold one list at different levels: the
    /// file records a state this model says cannot exist, there is no right
    /// answer, and the first fader is the one an operator would point at.
    #[test]
    fn a_pre_s45_executor_puts_its_level_and_its_rate_on_the_cue_list() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("older.prism");
        {
            let mut store = ShowStore::open(&path).unwrap();
            let mut file = crate::testkit::migration_fixture();
            // A second executor on the same cue list, which is the state B18
            // was about and which a file may perfectly well hold.
            file.show
                .store_executor(crate::testkit::executor(18, Some(5)))
                .unwrap();
            store.save(&mut file).unwrap();
        }

        // Rewrite the two executor rows the way a build before S45 wrote them.
        // Executor 17 is the lower number and carries the levels that must win.
        {
            let connection = rusqlite::Connection::open(&path).unwrap();
            for row in [
                PreS45Executor {
                    id: 17,
                    sequence_id: Some(5),
                    fader_function: "Master",
                    button_functions: vec!["Go+", "Go-"],
                    encoder_function: "Speed",
                    master_level: 12_345,
                    speed: 2_048,
                    is_active: true,
                    current_cue_index: Some(1),
                },
                PreS45Executor {
                    id: 18,
                    sequence_id: Some(5),
                    fader_function: "Master",
                    button_functions: Vec::new(),
                    encoder_function: "Empty",
                    master_level: 60_000,
                    speed: 512,
                    is_active: false,
                    current_cue_index: None,
                },
            ] {
                connection
                    .execute(
                        "UPDATE executor SET document = ?2 WHERE id = ?1",
                        rusqlite::params![i64::from(row.id), super::encode(&row).unwrap()],
                    )
                    .unwrap();
            }
        }

        let read = ShowStore::open(&path).unwrap().read().unwrap();
        let sequence = read
            .show
            .sequence(prism_domain::SequenceId::new(5))
            .expect("cue list 5");
        assert_eq!(sequence.master_level, 12_345, "the lower executor's level");
        assert_eq!(sequence.speed, 2_048, "the lower executor's rate");
        // What is *not* migrated: a show reopens with nothing running.
        assert!(!sequence.is_active);
        assert_eq!(sequence.current_cue_index, None);

        // The executor rows survive as the assignments they are.
        let executor = read
            .show
            .executor(prism_domain::ExecutorId::new(17))
            .expect("executor 17");
        assert_eq!(executor.sequence_id, Some(prism_domain::SequenceId::new(5)));
        assert_eq!(
            executor.button_functions,
            vec![
                prism_domain::ExecutorButtonFunction::GoForward,
                prism_domain::ExecutorButtonFunction::GoBack
            ]
        );
        assert_eq!(
            executor.encoder_function,
            prism_domain::ExecutorEncoderFunction::Speed
        );
        // Reading a file is not an edit.
        assert!(!read.is_dirty());
    }

    /// The frozen version-1 file, opened where opening it does no harm.
    ///
    /// Opening a file is what migrates it, so the one in the repository is
    /// copied first — a test that left it at version 2 would be a test that
    /// passes once.
    fn migrated_fixture(directory: &std::path::Path) -> ShowStore {
        let original = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("version-1.prism");
        let path = directory.join("migrated.prism");
        std::fs::copy(original, &path).unwrap();
        ShowStore::open(path).unwrap()
    }

    /// The frozen fixture is the show it was written from — every table, byte
    /// for byte, after being migrated across a schema version.
    ///
    /// `tests/persistence.rs` asserts the same migration field by field, which
    /// is the readable half. This is the exhaustive one: it holds the file to
    /// [`crate::testkit::migration_fixture`], so a fixture that was quietly
    /// regenerated, truncated or half-migrated fails here rather than in a
    /// spot check that happened not to look at the table that went missing.
    #[test]
    fn the_frozen_fixture_is_the_show_it_was_written_from() {
        let directory = tempfile::tempdir().unwrap();
        let store = migrated_fixture(directory.path());
        let read = store.read().unwrap();
        assert_eq!(
            rmp_serde::to_vec_named(&read.show).unwrap(),
            rmp_serde::to_vec_named(&crate::testkit::migration_fixture().show).unwrap()
        );
    }

    #[test]
    fn a_new_file_is_stamped_and_at_the_current_version() {
        let directory = tempfile::tempdir().unwrap();
        let store = ShowStore::open(directory.path().join("aula.prism")).unwrap();
        assert_eq!(store.version(), ShowStore::FORMAT_VERSION);
        assert_eq!(ShowStore::FORMAT_VERSION, MIGRATIONS.len() as u32);
        let id: i32 = store
            .connection
            .query_row("PRAGMA application_id", [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, ShowStore::APPLICATION_ID);
        let mode: String = store
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        store.integrity_check().unwrap();
    }

    #[test]
    fn a_database_that_belongs_to_something_else_is_not_written_into() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("addresses.prism");
        {
            let other = rusqlite::Connection::open(&path).unwrap();
            other
                .execute_batch("CREATE TABLE contact (name TEXT); PRAGMA application_id = 4660;")
                .unwrap();
        }
        assert_eq!(
            ShowStore::open(&path).unwrap_err(),
            StoreError::NotAShowFile {
                application_id: 4660
            }
        );

        // The same without a stamp at all, which is what a database written by
        // a tool that never set one looks like. It has tables in it, so it is
        // not the empty file `open` has just created.
        let path = directory.path().join("unstamped.prism");
        {
            let other = rusqlite::Connection::open(&path).unwrap();
            other
                .execute_batch("CREATE TABLE contact (name TEXT);")
                .unwrap();
        }
        assert_eq!(
            ShowStore::open(&path).unwrap_err(),
            StoreError::NotAShowFile { application_id: 0 }
        );
    }

    #[test]
    fn a_file_from_a_newer_version_is_refused_rather_than_guessed_at() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("future.prism");
        {
            let store = ShowStore::open(&path).unwrap();
            store
                .connection
                .pragma_update(None, "user_version", 99)
                .unwrap();
        }
        assert_eq!(
            ShowStore::open(&path).unwrap_err(),
            StoreError::FutureVersion {
                found: 99,
                supported: ShowStore::FORMAT_VERSION,
            }
        );
    }

    #[test]
    fn a_row_that_does_not_decode_names_itself() {
        let directory = tempfile::tempdir().unwrap();
        let (store, _) = saved(directory.path());
        store
            .connection
            .execute(
                "UPDATE fixture SET document = ?1 WHERE id = 1",
                [b"nonsense".as_slice()],
            )
            .unwrap();
        let error = store.read().unwrap_err();
        assert!(
            matches!(&error, StoreError::Damaged(reason) if reason.contains("fixture row 1")),
            "{error}"
        );
    }

    #[test]
    fn a_row_filed_under_the_wrong_key_is_damage_rather_than_a_rename() {
        // A show whose fixture 1 sits in row 2 would come back with fixture 1
        // under key 2 — a patch in which the number an operator types and the
        // number the show holds have come apart.
        let directory = tempfile::tempdir().unwrap();
        let (store, _) = saved(directory.path());
        store
            .connection
            .execute("UPDATE fixture SET id = 2 WHERE id = 1", [])
            .unwrap();
        let error = store.read().unwrap_err();
        assert!(
            matches!(&error, StoreError::Damaged(reason) if reason.contains("different fixture")),
            "{error}"
        );

        // And the same for the one table keyed by text.
        let directory = tempfile::tempdir().unwrap();
        let (store, _) = saved(directory.path());
        store
            .connection
            .execute("UPDATE fixture_type SET id = 'other'", [])
            .unwrap();
        let error = store.read().unwrap_err();
        assert!(
            matches!(&error, StoreError::Damaged(reason) if reason.contains("different fixture_type")),
            "{error}"
        );
    }

    #[test]
    fn a_write_that_fails_leaves_the_lamp_lit_and_the_file_as_it_was() {
        let directory = tempfile::tempdir().unwrap();
        let (mut store, mut file) = saved(directory.path());
        let before = std::fs::read(store.path()).unwrap();

        // Another writer, which is what a second daemon or a backup tool is.
        // The busy timeout is 250 ms, so this is what a lock nobody releases
        // looks like to a save.
        let blocker = rusqlite::Connection::open(store.path()).unwrap();
        blocker.execute_batch("BEGIN EXCLUSIVE").unwrap();

        file.show
            .patch_fixture(fixture(2, "generic.rgbw.par", 1, 21))
            .unwrap();
        assert!(file.is_dirty());
        let error = store.save(&mut file).unwrap_err();
        assert!(matches!(error, StoreError::Database(_)), "{error}");
        assert!(
            file.is_dirty(),
            "the lamp went out over a save that did not happen"
        );
        assert_eq!(std::fs::read(store.path()).unwrap(), before);

        // And once the other writer lets go, the same save goes through.
        blocker.execute_batch("ROLLBACK").unwrap();
        drop(blocker);
        assert_eq!(
            store.save(&mut file).unwrap(),
            vec![prism_domain::Delta::DirtyFlag {
                unsaved_changes: false
            }]
        );
        assert!(!file.is_dirty());
    }

    #[test]
    fn a_path_that_is_not_there_is_a_refusal_rather_than_a_new_directory() {
        // The ordinary version of this is a show on a network drive that is
        // not mounted, or a stick that has been pulled. Creating the folder
        // would be a desk deciding where the operator's show lives.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-mounted").join("aula.prism");
        let error = ShowStore::open(&path).unwrap_err();
        assert!(matches!(error, StoreError::Database(_)), "{error}");
        assert!(!path.exists());
    }

    #[test]
    fn a_show_file_has_to_be_a_file() {
        // SQLite will not put a write-ahead log on an in-memory database, and
        // it says so by staying in the journal mode it was already in. The
        // same answer comes back from a file system that cannot do the shared
        // memory WAL needs — a network share, which is exactly where a school
        // would think to keep its shows — and a show whose saves are not
        // crash-safe must not look like one that is.
        let error = ShowStore::open(":memory:").unwrap_err();
        assert!(
            matches!(&error, StoreError::Database(reason) if reason.contains("write-ahead log")),
            "{error}"
        );
    }

    #[test]
    fn a_file_whose_structure_is_broken_says_so_rather_than_reading_it() {
        use std::io::{Seek as _, SeekFrom, Write as _};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("aula.prism");
        let (store, _) = saved(directory.path());
        store.integrity_check().unwrap();
        // Closing checkpoints the write-ahead log into the file, so what is
        // corrupted below is the show rather than a log nobody will read.
        drop(store);

        // A bad sector, written where the schema is not: page 1 stays readable,
        // so the file opens and every check above it passes.
        let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.seek(SeekFrom::Start(4096)).unwrap();
        file.write_all(&[0xAB; 512]).unwrap();
        file.sync_all().unwrap();
        drop(file);

        let store = ShowStore::open(&path).unwrap();
        let error = store.integrity_check().unwrap_err();
        assert!(matches!(error, StoreError::Damaged(_)), "{error}");

        // The corruption is a broken *file*, and the schema-level version of
        // it is refused one step earlier, at the door.
        let second = tempfile::tempdir().unwrap();
        let path = second.path().join("schema.prism");
        let (store, _) = saved(second.path());
        drop(store);
        std::fs::copy(second.path().join("aula.prism"), &path).unwrap();
        let broken = rusqlite::Connection::open(&path).unwrap();
        broken
            .execute_batch(
                "PRAGMA writable_schema = ON;
                 UPDATE sqlite_schema SET sql = 'CREATE TABLE fixture(' WHERE name = 'fixture';",
            )
            .unwrap();
        drop(broken);
        let error = ShowStore::open(&path).unwrap_err();
        assert!(matches!(error, StoreError::Database(_)), "{error}");
    }

    #[test]
    fn a_session_that_points_at_something_that_is_not_there_is_damage() {
        // Every edit in `session.rs` maintains these two invariants, so a file
        // is the one place they can arrive broken — a show carried over on a
        // stick from a build that had a defect, or a row somebody edited by
        // hand. A console whose canvas shows a view the file does not have has
        // no way to tell the operator what happened.
        let directory = tempfile::tempdir().unwrap();
        let (store, file) = saved(directory.path());

        let mut lost = file.session.session().clone();
        lost.active_view_id = prism_domain::ViewId::new(99);
        store
            .connection
            .execute(
                "UPDATE session SET document = ?1 WHERE id = 1",
                [rmp_serde::to_vec_named(&lost).unwrap()],
            )
            .unwrap();
        let error = store.read().unwrap_err();
        assert!(
            matches!(&error, StoreError::Damaged(reason) if reason.contains("view 99")),
            "{error}"
        );

        let mut focused = file.session.session().clone();
        focused.focused_window = Some(prism_domain::WindowInstanceId::new(7));
        store
            .connection
            .execute(
                "UPDATE session SET document = ?1 WHERE id = 1",
                [rmp_serde::to_vec_named(&focused).unwrap()],
            )
            .unwrap();
        let error = store.read().unwrap_err();
        assert!(
            matches!(&error, StoreError::Damaged(reason) if reason.contains("window 7")),
            "{error}"
        );
    }

    #[test]
    fn a_recovery_copy_that_cannot_be_removed_is_reported_rather_than_ignored() {
        // A save discards the recovery copy, and a discard that quietly failed
        // would leave a file that outlives the edits it describes — the next
        // start would offer to recover a show that is already saved. It is
        // still not a failed save, and the difference is the Save LED: the
        // show *is* on the platter.
        let directory = tempfile::tempdir().unwrap();
        let (mut store, mut file) = saved(directory.path());
        std::fs::create_dir(store.recovery_path()).unwrap();

        let error = store.discard_recovery().unwrap_err();
        assert!(matches!(error, StoreError::Io(_)), "{error}");

        file.show
            .patch_fixture(fixture(2, "generic.rgbw.par", 1, 21))
            .unwrap();
        let deltas = store.save(&mut file).unwrap();
        assert!(
            !file.is_dirty(),
            "the show was written, so the lamp goes out"
        );
        assert!(deltas.contains(&prism_domain::Delta::DirtyFlag {
            unsaved_changes: false
        }));
        assert!(
            deltas.iter().any(|delta| matches!(
                delta,
                prism_domain::Delta::Notice {
                    level: prism_domain::NoticeLevel::Warn,
                    ..
                }
            )),
            "the operator is not told about a recovery copy that outlived its show"
        );
        // And the fixture really is in the file.
        assert!(
            store
                .read()
                .unwrap()
                .show
                .fixture(prism_domain::FixtureId::new(2))
                .is_some()
        );
    }

    #[test]
    fn every_error_reads_as_itself() {
        for error in [
            StoreError::Database("locked".to_owned()),
            StoreError::Io("no such directory".to_owned()),
            StoreError::NotAShowFile {
                application_id: 4660,
            },
            StoreError::FutureVersion {
                found: 9,
                supported: 2,
            },
            StoreError::Damaged("fixture row 1".to_owned()),
            StoreError::NotRepresentable("not a finite number".to_owned()),
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
        }
        assert_eq!(
            StoreError::NotAShowFile {
                application_id: 4660
            }
            .to_string(),
            "this is not a PrismDMX show file (application id 0x00001234)"
        );
    }

    #[test]
    fn an_autosave_that_is_ignored_is_not_offered_again_until_the_next_interval() {
        let mut autosave = Autosave::every(Duration::from_secs(10));
        assert_eq!(autosave.interval(), Duration::from_secs(10));
        assert_eq!(Autosave::new(), Autosave::default());
        assert!(!autosave.poll(Duration::ZERO, true));
        assert!(autosave.poll(Duration::from_secs(10), true));
        assert!(!autosave.poll(Duration::from_secs(11), true));
        assert!(autosave.poll(Duration::from_secs(20), true));
    }

    #[test]
    fn a_recovery_copy_is_named_after_the_show_it_belongs_to() {
        let directory = tempfile::tempdir().unwrap();
        let store = ShowStore::open(directory.path().join("aula.prism")).unwrap();
        assert_eq!(
            store.recovery_path(),
            directory.path().join("aula.recovery.prism")
        );
        assert!(!store.has_recovery());
        // Discarding one that is not there is not an error: it is the ordinary
        // case at the end of every successful save.
        store.discard_recovery().unwrap();
    }

    #[test]
    fn an_export_says_what_it_is() {
        let mut file = ShowFile::new();
        file.show.embed_fixture_type(par_type()).unwrap();
        let text = export_json(&file).unwrap();
        assert!(text.contains("\"format\": \"prismdmx.show\""));
        let back = import_json(&text).unwrap();
        assert_eq!(
            rmp_serde::to_vec_named(&back).unwrap(),
            rmp_serde::to_vec_named(&file).unwrap()
        );
        // A document with the right shape and the wrong name is not this one.
        let wrong = text.replace("prismdmx.show", "something.else");
        assert!(matches!(import_json(&wrong), Err(StoreError::Damaged(_))));
    }

    /// Rewrites `tests/fixtures/version-1.prism`, the file the migration test
    /// reads.
    ///
    /// `#[ignore]`d on purpose. The fixture is **frozen**: a migration checked
    /// against a file the current code has just written is a migration checked
    /// against itself, and the whole value of the test is that the file
    /// predates the schema it is being migrated to. Run it only when version 1
    /// itself is meant to change, which is to say never:
    ///
    /// ```text
    /// cargo test -p prism-core --lib -- --ignored rewrites_the_version_one_fixture
    /// ```
    #[test]
    #[ignore = "rewrites a checked-in fixture; see the documentation on this test"]
    fn rewrites_the_version_one_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("version-1.prism");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_version_one(&path, &crate::testkit::migration_fixture());
    }

    /// Writes a version-1 show file: the first migration's schema, the six show
    /// tables, and nothing else — which is what a PrismDMX before S12 would
    /// have produced.
    ///
    /// The frozen fixture was written by this function and the test below
    /// exercises it, so the file in the repository has a provenance that is
    /// itself tested rather than a comment saying where it came from.
    fn write_version_one(path: &std::path::Path, file: &ShowFile) {
        super::remove_database(path).unwrap();
        let connection = rusqlite::Connection::open(path).unwrap();
        connection
            .pragma_update(None, "application_id", ShowStore::APPLICATION_ID)
            .unwrap();
        connection
            .execute_batch(&format!(
                "BEGIN; {} PRAGMA user_version = 1; COMMIT;",
                MIGRATIONS[0]
            ))
            .unwrap();

        let rows = super::Rows::encode(file).unwrap();
        for (id, document) in &rows.fixture_types {
            connection
                .execute(
                    "INSERT INTO fixture_type (id, document) VALUES (?1, ?2)",
                    rusqlite::params![id, document],
                )
                .unwrap();
        }
        for (table, values) in [
            ("fixture", &rows.fixtures),
            ("fixture_group", &rows.groups),
            ("preset", &rows.presets),
            ("sequence", &rows.sequences),
            ("executor", &rows.executors),
        ] {
            for (id, document) in values {
                connection
                    .execute(
                        &format!("INSERT INTO {table} (id, document) VALUES (?1, ?2)"),
                        rusqlite::params![id, document],
                    )
                    .unwrap();
            }
        }
        drop(connection);
    }

    /// A version-1 file written here and now, migrated.
    ///
    /// The frozen fixture is the test that matters — see
    /// [`the_frozen_fixture_is_the_show_it_was_written_from`] — and this is the
    /// one that says what the migration *does*: the six show tables survive it
    /// untouched, the two new ones arrive empty, and a show that never had a
    /// session opens in the session a fresh desk starts in.
    #[test]
    fn opening_a_version_one_file_gives_it_a_session_and_leaves_the_show_alone() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("old.prism");
        let original = crate::testkit::migration_fixture();
        write_version_one(&path, &original);

        let store = ShowStore::open(&path).unwrap();
        assert_eq!(store.version(), ShowStore::FORMAT_VERSION);
        let read = store.read().unwrap();
        assert_eq!(
            rmp_serde::to_vec_named(&read.show).unwrap(),
            rmp_serde::to_vec_named(&original.show).unwrap()
        );
        assert_eq!(read.session, crate::SessionState::new());
        assert!(!read.is_dirty());

        // And a second open is not a second migration: the file is already at
        // the current version, so nothing runs and nothing is lost.
        drop(store);
        let store = ShowStore::open(&path).unwrap();
        assert_eq!(store.version(), ShowStore::FORMAT_VERSION);
        assert_eq!(
            rmp_serde::to_vec_named(&store.read().unwrap().show).unwrap(),
            rmp_serde::to_vec_named(&original.show).unwrap()
        );
    }
}
