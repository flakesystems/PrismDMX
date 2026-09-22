//! What the library read last time, so it need not read it again (2026-09-22).
//!
//! A published GDTF library is twelve thousand files, and parsing their
//! descriptions is most of a minute on a desk's processor. Almost none of them
//! change between two starts. So after a read the library writes down, per
//! file, what it got out of it — the menu entries and the two facts per mode a
//! patch window shows ([`super::Held`]) — beside the file's length and time of
//! last change; the next start takes a file whose length and time are the same
//! from here instead of parsing it. A file that changed, a new file, and every
//! file after a change to the reader ([`VERSION`]) are parsed again.
//!
//! **Nothing here is trusted either.** An index that is missing, unreadable,
//! from another version or simply wrong about a file costs a re-read and
//! nothing else: the worst case is the start the desk had before there was an
//! index. The file is written beside the show, in the data directory, and it
//! is a cache — deleting it is always safe.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use prism_domain::LibraryEntry;
use serde::{Deserialize, Serialize};

use super::gdtf;

/// Bumped whenever what a file reads **as** changes — a new field on a menu
/// entry, a reader that finds more beams — so an index written by an older
/// reader is thrown away rather than believed.
pub const VERSION: u32 = 1;

/// One profile, as the index remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct IndexedProfile {
    pub(super) entry: LibraryEntry,
    pub(super) has_intensity: bool,
    pub(super) beams: u16,
    pub(super) guid: Option<String>,
}

/// One file, as the index remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct IndexedFile {
    path: String,
    own: bool,
    length: u64,
    /// Nanoseconds since the epoch of its last change, as the file system says.
    modified: u64,
    profiles: Vec<IndexedProfile>,
    counts: gdtf::Conversion,
}

/// The file on disk.
#[derive(Debug, Default, Serialize, Deserialize)]
struct IndexDocument {
    version: u32,
    files: Vec<IndexedFile>,
}

/// A file's length and time of last change: what says it is the file the index
/// read. `None` when the file system will not say, which is a file to read.
///
/// Taken from metadata the caller already has — a directory listing's, which
/// on Windows costs nothing where asking the file costs opening it.
pub(super) fn stamp_of(metadata: &std::fs::Metadata) -> Option<(u64, u64)> {
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some((metadata.len(), u64::try_from(modified.as_nanos()).ok()?))
}

/// The index the library reads with and writes after — see the module
/// documentation.
#[derive(Debug, Default)]
pub struct LibraryIndex {
    /// Where it lives, or nowhere for a library that keeps no index.
    path: Option<PathBuf>,
    /// What the last run wrote, by file and by whether it was the venue's own.
    known: HashMap<(String, bool), IndexedFile>,
    /// What this run read, in the order it was read, to be written.
    seen: Vec<IndexedFile>,
    /// How many files this run took from the index rather than parsing.
    hits: usize,
}

impl LibraryIndex {
    /// The index at `path`, or an empty one when there is none there that
    /// this reader can use.
    #[must_use]
    pub fn open(path: &Path) -> Self {
        let known = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<IndexDocument>(&bytes).ok())
            .filter(|document| document.version == VERSION)
            .map(|document| {
                document
                    .files
                    .into_iter()
                    .map(|file| ((file.path.clone(), file.own), file))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            path: Some(path.to_path_buf()),
            known,
            seen: Vec::new(),
            hits: 0,
        }
    }

    /// What the index remembers of a file that has not changed since.
    pub(super) fn lookup(
        &mut self,
        path: &Path,
        own: bool,
        stamp: (u64, u64),
    ) -> Option<(Vec<IndexedProfile>, gdtf::Conversion)> {
        let key = (path.to_string_lossy().into_owned(), own);
        let file = self.known.get(&key)?;
        if (file.length, file.modified) != stamp {
            return None;
        }
        self.hits += 1;
        Some((file.profiles.clone(), file.counts))
    }

    /// Remembers what one file read as, for the next start.
    pub(super) fn record(
        &mut self,
        path: &Path,
        own: bool,
        stamp: (u64, u64),
        profiles: Vec<IndexedProfile>,
        counts: gdtf::Conversion,
    ) {
        self.seen.push(IndexedFile {
            path: path.to_string_lossy().into_owned(),
            own,
            length: stamp.0,
            modified: stamp.1,
            profiles,
            counts,
        });
    }

    /// How many files this run took from the index.
    #[must_use]
    pub const fn hits(&self) -> usize {
        self.hits
    }

    /// Writes what this run read — only that, so a file that has gone is
    /// forgotten. Written beside and then moved over the old one, so a desk
    /// that loses power mid-write keeps the last whole index. A failure is not
    /// an error: the next start reads the files again.
    pub fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let document = IndexDocument {
            version: VERSION,
            files: self.seen.clone(),
        };
        let Ok(bytes) = serde_json::to_vec(&document) else {
            return;
        };
        let partial = path.with_extension("partial");
        if std::fs::write(&partial, bytes).is_ok() && std::fs::rename(&partial, path).is_err() {
            let _ = std::fs::remove_file(&partial);
        }
    }
}
