//! The ZIP container a `.gdtf` file is — **S60**.
//!
//! # Why this is here rather than a dependency
//!
//! A GDTF file is a ZIP archive: `description.xml` at the top, the device's 3D
//! models under `models/`, the pictures of its gobos under `wheels/`. Reading
//! one needs two things — the container, and DEFLATE.
//!
//! The **container** is 200 lines of fixed-width records, and this desk opens
//! archives that use two of the format's methods and none of its encryption.
//! Writing it here buys three things a general ZIP library does not give: it is
//! read from a `&[u8]` that is already in memory, so no file handle stays open
//! on a directory the operator may be editing; every limit is this project's
//! own and stated in one place ([`MAX_ENTRIES`], [`MAX_FILE`]); and it can be
//! tested against **bytes** rather than against another library's opinion of
//! them, which is the rule S55 (**B55**) wrote for the one other hand-written
//! decoder in this repository.
//!
//! **DEFLATE** is not, and `flate2` does it. A second inflate implementation
//! would be a second inflate implementation.
//!
//! # Nothing here trusts the file
//!
//! A `.gdtf` is a file an operator copied off a manufacturer's website onto a
//! stick, and the desk reads every one it finds in a directory at start-up.
//! Every length is checked against the bytes that are actually there, every
//! entry is capped, and the whole of it answers [`None`] rather than failing:
//! the caller counts the file as rejected and carries on. A lighting desk that
//! would not start because a download was truncated is a worse answer than one
//! that says so and offers the profiles it could read.
//!
//! # What it does not do
//!
//! Encryption, multi-disk archives, and every compression method but **stored**
//! and **deflate**. An entry compressed any other way is [`Entry::UNSUPPORTED`]
//! and reads as `None`; nothing that writes GDTF uses one.

use std::collections::BTreeMap;
use std::io::Read;

/// The end-of-central-directory record's signature.
const EOCD: u32 = 0x0605_4b50;
/// The ZIP64 end-of-central-directory locator's signature.
const EOCD64_LOCATOR: u32 = 0x0706_4b50;
/// The ZIP64 end-of-central-directory record's signature.
const EOCD64: u32 = 0x0606_4b50;
/// A central-directory file header's signature.
const CENTRAL: u32 = 0x0201_4b50;
/// A local file header's signature.
const LOCAL: u32 = 0x0403_4b50;

/// The fixed part of a central-directory file header.
const CENTRAL_FIXED: usize = 46;
/// The fixed part of a local file header.
const LOCAL_FIXED: usize = 30;
/// The fixed part of an end-of-central-directory record.
const EOCD_FIXED: usize = 22;

/// The value a 32-bit field carries when the real one is in a ZIP64 extra
/// field.
const ZIP64_MARKER_32: u32 = 0xFFFF_FFFF;
/// The same, for the 16-bit entry count.
const ZIP64_MARKER_16: u16 = 0xFFFF;
/// The ZIP64 extended-information extra field's tag.
const ZIP64_EXTRA: u16 = 0x0001;

/// The most entries an archive may declare.
///
/// A GDTF file with a model and a picture per gobo slot is a few hundred; ten
/// thousand is far past anything a fixture ships and stops a hand-built
/// archive from making the desk allocate a directory of millions at start-up.
pub const MAX_ENTRIES: usize = 10_000;

/// The most one entry may inflate to — 64 MiB.
///
/// A 3D model of a moving head is a few megabytes. This is the number that
/// stops a **zip bomb**: a kilobyte of archive that inflates to a gigabyte, in
/// a directory the desk reads without being asked to.
pub const MAX_FILE: usize = 64 * 1024 * 1024;

/// One file in an archive, as the central directory describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The compression method: 0 stored, 8 deflate.
    method: u16,
    /// What the header says it inflates to.
    size: u64,
    /// How many bytes of it are in the archive.
    compressed: u64,
    /// The CRC-32 the header states.
    crc: u32,
    /// Where its **local** header starts.
    offset: u64,
}

impl Entry {
    /// Stored: the bytes are in the archive as they are.
    const STORED: u16 = 0;
    /// Deflate, which is what everything that writes GDTF uses.
    const DEFLATE: u16 = 8;
    /// Any other method. Named so that a caller reading this file can see that
    /// the list of two is deliberate.
    pub const UNSUPPORTED: &'static str = "only stored and deflate are read";

    /// What this entry inflates to, in bytes.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }
}

/// An archive that has been read into memory, indexed by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Archive<'a> {
    /// The whole file, which every entry is a slice of.
    bytes: &'a [u8],
    /// The central directory, by the name each entry carries.
    ///
    /// A `BTreeMap`, so the names come out in one order on every machine and a
    /// test that walks them is stable.
    entries: BTreeMap<String, Entry>,
}

impl<'a> Archive<'a> {
    /// Reads an archive's central directory.
    ///
    /// Answers `None` for anything that is not a ZIP archive this reader
    /// understands: a truncated download, a file that is something else
    /// entirely, an archive spanning several disks, or one declaring more than
    /// [`MAX_ENTRIES`]. Nothing is inflated here — [`Self::file`] does that,
    /// one entry at a time, so opening an archive to read one XML document out
    /// of it does not unpack its models.
    #[must_use]
    pub fn read(bytes: &'a [u8]) -> Option<Self> {
        let eocd = find_eocd(bytes)?;
        let (mut count, mut start) = (
            usize::from(u16_at(bytes, eocd + 10)?),
            u64::from(u32_at(bytes, eocd + 16)?),
        );
        // ZIP64, when either number did not fit in the record above. The
        // locator sits immediately before the end record, and points at a
        // second end record that carries the real ones.
        if count == usize::from(ZIP64_MARKER_16) || start == u64::from(ZIP64_MARKER_32) {
            let locator = eocd.checked_sub(20)?;
            if u32_at(bytes, locator)? != EOCD64_LOCATOR {
                return None;
            }
            let record = usize::try_from(u64_at(bytes, locator + 8)?).ok()?;
            if u32_at(bytes, record)? != EOCD64 {
                return None;
            }
            count = usize::try_from(u64_at(bytes, record + 32)?).ok()?;
            start = u64_at(bytes, record + 48)?;
        }
        if count > MAX_ENTRIES {
            return None;
        }

        let mut at = usize::try_from(start).ok()?;
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            if u32_at(bytes, at)? != CENTRAL {
                return None;
            }
            let name_length = usize::from(u16_at(bytes, at + 28)?);
            let extra_length = usize::from(u16_at(bytes, at + 30)?);
            let comment_length = usize::from(u16_at(bytes, at + 32)?);
            let name_at = at.checked_add(CENTRAL_FIXED)?;
            let name = bytes.get(name_at..name_at.checked_add(name_length)?)?;
            let extra = bytes.get(
                name_at.checked_add(name_length)?
                    ..name_at
                        .checked_add(name_length)?
                        .checked_add(extra_length)?,
            )?;

            let mut entry = Entry {
                method: u16_at(bytes, at + 10)?,
                crc: u32_at(bytes, at + 16)?,
                compressed: u64::from(u32_at(bytes, at + 20)?),
                size: u64::from(u32_at(bytes, at + 24)?),
                offset: u64::from(u32_at(bytes, at + 42)?),
            };
            widen(&mut entry, extra);

            // A directory entry — the format's own convention, a name ending in
            // a slash and nothing in it. Not a file, so not in the index.
            if !name.ends_with(b"/")
                && let Ok(name) = std::str::from_utf8(name)
            {
                entries.insert(normalise(name), entry);
            }
            at = name_at
                .checked_add(name_length)?
                .checked_add(extra_length)?
                .checked_add(comment_length)?;
        }
        Some(Self { bytes, entries })
    }

    /// One file's bytes, inflated.
    ///
    /// `None` when there is no such entry, when it is compressed a way this
    /// reader does not know ([`Entry::UNSUPPORTED`]), when it inflates to more
    /// than [`MAX_FILE`], or when what came out does not match the CRC-32 the
    /// archive states for it. The last is the one that matters in a venue: a
    /// file copied off a failing stick reads as a fixture that will not load
    /// rather than as a fixture with a channel missing.
    #[must_use]
    pub fn file(&self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entries.get(&normalise(name))?;
        if entry.size > MAX_FILE as u64 {
            return None;
        }
        // The local header states its own name and extra lengths, and they are
        // **not** the central directory's: the extra field is routinely
        // different in the two places. So the data's start is computed here.
        let header = usize::try_from(entry.offset).ok()?;
        if u32_at(self.bytes, header)? != LOCAL {
            return None;
        }
        let name_length = usize::from(u16_at(self.bytes, header + 26)?);
        let extra_length = usize::from(u16_at(self.bytes, header + 28)?);
        let from = header
            .checked_add(LOCAL_FIXED)?
            .checked_add(name_length)?
            .checked_add(extra_length)?;
        let compressed = usize::try_from(entry.compressed).ok()?;
        let raw = self.bytes.get(from..from.checked_add(compressed)?)?;

        let out = match entry.method {
            Entry::STORED => raw.to_vec(),
            Entry::DEFLATE => inflate(raw, usize::try_from(entry.size).ok()?)?,
            _ => return None,
        };
        if out.len() as u64 != entry.size {
            return None;
        }
        let mut crc = flate2::Crc::new();
        crc.update(&out);
        if crc.sum() != entry.crc {
            return None;
        }
        Some(out)
    }

    /// Every file in the archive, by name, in one order on every machine.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// The first name whose **last path segment** matches `file`, ignoring
    /// case.
    ///
    /// GDTF states `description.xml` at the top of the archive, and files
    /// written by real tools have been seen with a leading directory and with
    /// `Description.xml`. What the format means is unambiguous either way, so
    /// this is where that is forgiven rather than at each call site.
    #[must_use]
    pub fn find(&self, file: &str) -> Option<&str> {
        let wanted = file.to_ascii_lowercase();
        self.entries.keys().map(String::as_str).find(|name| {
            name.rsplit('/')
                .next()
                .is_some_and(|last| last.eq_ignore_ascii_case(&wanted))
        })
    }

    /// How many files it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds none, which a ZIP archive is allowed to be.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A name as this index holds it: backslashes are separators too.
///
/// An archive written on Windows by a tool that built its names by hand carries
/// `models\\gltf\\base.glb`, and the format says a separator is a forward
/// slash. Both mean one path.
fn normalise(name: &str) -> String {
    name.replace('\\', "/")
}

/// Takes the real size and offset out of a ZIP64 extra field.
///
/// The fields are present **only** where the 32-bit one was the marker, and in
/// a fixed order, so they are read in that order and each one consumes its
/// eight bytes.
fn widen(entry: &mut Entry, mut extra: &[u8]) {
    while extra.len() >= 4 {
        let tag = u16::from_le_bytes([extra[0], extra[1]]);
        let length = usize::from(u16::from_le_bytes([extra[2], extra[3]]));
        let Some(body) = extra.get(4..4 + length) else {
            return;
        };
        if tag == ZIP64_EXTRA {
            let mut at = 0;
            let next = |at: &mut usize| -> Option<u64> {
                let value = u64_at(body, *at)?;
                *at += 8;
                Some(value)
            };
            if entry.size == u64::from(ZIP64_MARKER_32)
                && let Some(size) = next(&mut at)
            {
                entry.size = size;
            }
            if entry.compressed == u64::from(ZIP64_MARKER_32)
                && let Some(compressed) = next(&mut at)
            {
                entry.compressed = compressed;
            }
            if entry.offset == u64::from(ZIP64_MARKER_32)
                && let Some(offset) = next(&mut at)
            {
                entry.offset = offset;
            }
            return;
        }
        extra = &extra[4 + length..];
    }
}

/// Where the end-of-central-directory record starts.
///
/// Scanned **backwards**, because the record ends with a comment of up to 64 kB
/// and there is no other way to find it. The scan stops at the furthest back
/// the record can possibly be, so a file that is not an archive at all costs
/// one pass over its last 64 kB rather than over the whole of it.
fn find_eocd(bytes: &[u8]) -> Option<usize> {
    let furthest = bytes.len().saturating_sub(EOCD_FIXED + 0xFFFF);
    let mut at = bytes.len().checked_sub(EOCD_FIXED)?;
    loop {
        if u32_at(bytes, at) == Some(EOCD) {
            // The comment's length has to agree with where the file ends, or
            // this is four bytes of something else that happen to match.
            let comment = usize::from(u16_at(bytes, at + 20)?);
            if at + EOCD_FIXED + comment == bytes.len() {
                return Some(at);
            }
        }
        if at == furthest {
            return None;
        }
        at -= 1;
    }
}

/// Inflates a raw DEFLATE stream, refusing anything longer than it claims.
///
/// `expected` is what the archive says it will be. The reader is given one byte
/// more than that and the result is rejected if it produced it, so an entry
/// whose header understates its size cannot allocate past [`MAX_FILE`].
fn inflate(raw: &[u8], expected: usize) -> Option<Vec<u8>> {
    if expected > MAX_FILE {
        return None;
    }
    let mut out = Vec::with_capacity(expected);
    flate2::read::DeflateDecoder::new(raw)
        .take(expected as u64 + 1)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// A little-endian `u16` at `at`, or `None` when the file ends first.
fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

/// A little-endian `u32` at `at`.
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// A little-endian `u64` at `at`.
fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    let slice = bytes.get(at..at.checked_add(8)?)?;
    let mut eight = [0_u8; 8];
    eight.copy_from_slice(slice);
    Some(u64::from_le_bytes(eight))
}

/// Building an archive, for the tests of this module and of the GDTF reader
/// above it.
///
/// In this crate rather than in a `tests/` directory because both callers are
/// unit tests, and public to the crate rather than to the world because a ZIP
/// **writer** is not something this desk ships: nothing in production writes
/// one.
#[cfg(test)]
pub(crate) mod testkit {
    /// Builds an archive **byte by byte**, which is the whole point of this
    /// file's tests: nothing here is produced by a ZIP library, so nothing here
    /// can agree with the reader by sharing its misunderstanding (B55).
    ///
    /// Everything is stored uncompressed unless `deflate` says otherwise, and
    /// each entry's CRC is computed rather than written down, because a CRC
    /// written down is a CRC that stops being checked the first time a test
    /// case changes.
    pub(crate) struct Builder {
        files: Vec<(String, Vec<u8>, bool)>,
    }

    impl Builder {
        pub(crate) fn new() -> Self {
            Self { files: Vec::new() }
        }

        pub(crate) fn stored(mut self, name: &str, body: &[u8]) -> Self {
            self.files.push((name.to_owned(), body.to_vec(), false));
            self
        }

        pub(crate) fn deflated(mut self, name: &str, body: &[u8]) -> Self {
            self.files.push((name.to_owned(), body.to_vec(), true));
            self
        }

        pub(crate) fn build(self) -> Vec<u8> {
            let mut out = Vec::new();
            let mut central = Vec::new();
            for (name, body, deflate) in &self.files {
                let mut crc = flate2::Crc::new();
                crc.update(body);
                let crc = crc.sum();
                let payload = if *deflate {
                    use std::io::Write;
                    let mut encoder = flate2::write::DeflateEncoder::new(
                        Vec::new(),
                        flate2::Compression::default(),
                    );
                    encoder.write_all(body).expect("a vector takes bytes");
                    encoder.finish().expect("it finishes")
                } else {
                    body.clone()
                };
                let method: u16 = if *deflate { 8 } else { 0 };
                let offset = u32::try_from(out.len()).expect("a small archive");

                out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
                out.extend_from_slice(&20_u16.to_le_bytes()); // version needed
                out.extend_from_slice(&0_u16.to_le_bytes()); // flags
                out.extend_from_slice(&method.to_le_bytes());
                out.extend_from_slice(&0_u16.to_le_bytes()); // time
                out.extend_from_slice(&0_u16.to_le_bytes()); // date
                out.extend_from_slice(&crc.to_le_bytes());
                out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                out.extend_from_slice(&(body.len() as u32).to_le_bytes());
                out.extend_from_slice(&(name.len() as u16).to_le_bytes());
                // A local extra field that the central directory does not have,
                // because that is the case a reader gets wrong.
                out.extend_from_slice(&4_u16.to_le_bytes());
                out.extend_from_slice(name.as_bytes());
                out.extend_from_slice(&[0xCA, 0xFE, 0x00, 0x00]);
                out.extend_from_slice(&payload);

                central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
                central.extend_from_slice(&20_u16.to_le_bytes()); // made by
                central.extend_from_slice(&20_u16.to_le_bytes()); // needed
                central.extend_from_slice(&0_u16.to_le_bytes()); // flags
                central.extend_from_slice(&method.to_le_bytes());
                central.extend_from_slice(&0_u16.to_le_bytes()); // time
                central.extend_from_slice(&0_u16.to_le_bytes()); // date
                central.extend_from_slice(&crc.to_le_bytes());
                central.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                central.extend_from_slice(&(body.len() as u32).to_le_bytes());
                central.extend_from_slice(&(name.len() as u16).to_le_bytes());
                central.extend_from_slice(&0_u16.to_le_bytes()); // extra
                central.extend_from_slice(&0_u16.to_le_bytes()); // comment
                central.extend_from_slice(&0_u16.to_le_bytes()); // disk
                central.extend_from_slice(&0_u16.to_le_bytes()); // internal
                central.extend_from_slice(&0_u32.to_le_bytes()); // external
                central.extend_from_slice(&offset.to_le_bytes());
                central.extend_from_slice(name.as_bytes());
            }
            let start = u32::try_from(out.len()).expect("a small archive");
            let size = u32::try_from(central.len()).expect("a small directory");
            let count = u16::try_from(self.files.len()).expect("a few files");
            out.extend_from_slice(&central);
            out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes()); // this disk
            out.extend_from_slice(&0_u16.to_le_bytes()); // directory's disk
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&start.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes()); // comment
            out
        }
    }

    /// An archive holding one stored file, which is what a `.gdtf` under test
    /// is: a `description.xml` and nothing else.
    pub(crate) fn one_file(name: &str, body: &[u8]) -> Vec<u8> {
        Builder::new().stored(name, body).build()
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::Builder;
    use super::{Archive, MAX_FILE};

    #[test]
    fn a_stored_entry_reads_back_exactly() {
        let bytes = Builder::new().stored("description.xml", b"<GDTF/>").build();
        let archive = Archive::read(&bytes).expect("it is an archive");
        assert_eq!(archive.len(), 1);
        assert_eq!(
            archive.file("description.xml").as_deref(),
            Some(&b"<GDTF/>"[..])
        );
    }

    #[test]
    fn a_deflated_entry_reads_back_exactly() {
        // Long enough that deflate actually compresses it, so the stored path
        // cannot be passing this test by accident.
        let body = "<GDTF><FixtureType Name=\"Test\"/></GDTF>".repeat(200);
        let bytes = Builder::new()
            .deflated("description.xml", body.as_bytes())
            .build();
        let archive = Archive::read(&bytes).expect("it is an archive");
        assert_eq!(
            archive.file("description.xml").as_deref(),
            Some(body.as_bytes())
        );
    }

    #[test]
    fn the_local_extra_field_is_read_from_the_local_header() {
        // The builder writes a four-byte extra field locally and none in the
        // central directory. A reader that computed the data's start from the
        // central directory's lengths reads four bytes of rubbish and a CRC
        // failure — which is why both files are checked and not just the first.
        let bytes = Builder::new()
            .stored("a.txt", b"first")
            .stored("b.txt", b"second")
            .build();
        let archive = Archive::read(&bytes).expect("it is an archive");
        assert_eq!(archive.file("a.txt").as_deref(), Some(&b"first"[..]));
        assert_eq!(archive.file("b.txt").as_deref(), Some(&b"second"[..]));
    }

    /// Where the one central-directory header of a one-file archive starts:
    /// back past the end record, the name and the fixed part.
    ///
    /// Written out rather than searched for, because a test that found the
    /// header by looking for its signature could find the local one instead
    /// and then be corrupting a field this reader does not read.
    fn central_of(bytes: &[u8], name: &str) -> usize {
        bytes.len() - 22 - 46 - name.len()
    }

    #[test]
    fn a_wrong_crc_reads_as_nothing() {
        let mut bytes = Builder::new().stored("a.txt", b"first").build();
        // The CRC in the central directory, which is what `file` checks
        // against. The local header keeps the right one, so this is exactly the
        // half-corrupt file a failing stick produces.
        let at = central_of(&bytes, "a.txt") + 16;
        assert_ne!(bytes[at], 0x00, "the CRC of this body is not nought");
        bytes[at] ^= 0xFF;
        let archive = Archive::read(&bytes).expect("the directory still parses");
        assert_eq!(archive.file("a.txt"), None);
    }

    #[test]
    fn a_truncated_archive_is_not_an_archive() {
        let bytes = Builder::new().stored("a.txt", b"first").build();
        assert_eq!(Archive::read(&bytes[..bytes.len() - 4]), None);
        assert_eq!(Archive::read(&[]), None);
        assert_eq!(Archive::read(b"this is not a zip file at all"), None);
    }

    #[test]
    fn a_directory_entry_is_not_a_file() {
        let bytes = Builder::new()
            .stored("models/", b"")
            .stored("models/base.glb", b"glb")
            .build();
        let archive = Archive::read(&bytes).expect("it is an archive");
        assert_eq!(archive.names().collect::<Vec<_>>(), ["models/base.glb"]);
    }

    #[test]
    fn a_backslash_is_a_separator() {
        let bytes = Builder::new().stored("models\\base.glb", b"glb").build();
        let archive = Archive::read(&bytes).expect("it is an archive");
        assert_eq!(archive.names().collect::<Vec<_>>(), ["models/base.glb"]);
        assert_eq!(
            archive.file("models/base.glb").as_deref(),
            Some(&b"glb"[..])
        );
    }

    #[test]
    fn a_description_is_found_whatever_it_is_called() {
        let bytes = Builder::new()
            .stored("Fixture/Description.XML", b"<GDTF/>")
            .build();
        let archive = Archive::read(&bytes).expect("it is an archive");
        assert_eq!(
            archive.find("description.xml"),
            Some("Fixture/Description.XML")
        );
        assert_eq!(archive.find("models.xml"), None);
    }

    #[test]
    fn an_unsupported_method_reads_as_nothing() {
        let mut bytes = Builder::new().stored("a.txt", b"first").build();
        // Method 14 is LZMA, which is a legal ZIP and not one this reads.
        let at = central_of(&bytes, "a.txt") + 10;
        assert_eq!(bytes[at], 0, "it was stored before this");
        bytes[at] = 14;
        let archive = Archive::read(&bytes).expect("the directory still parses");
        assert_eq!(archive.file("a.txt"), None);
    }

    #[test]
    fn an_entry_that_claims_more_than_the_cap_reads_as_nothing() {
        let mut bytes = Builder::new().stored("a.txt", b"first").build();
        let at = central_of(&bytes, "a.txt") + 24;
        let claimed = u32::try_from(MAX_FILE + 1).expect("the cap fits in 32 bits");
        bytes[at..at + 4].copy_from_slice(&claimed.to_le_bytes());
        let archive = Archive::read(&bytes).expect("the directory still parses");
        assert_eq!(archive.file("a.txt"), None);
    }

    #[test]
    fn a_name_that_is_not_there_reads_as_nothing() {
        let bytes = Builder::new().stored("a.txt", b"first").build();
        let archive = Archive::read(&bytes).expect("it is an archive");
        assert_eq!(archive.file("b.txt"), None);
        assert!(!archive.is_empty());
    }
}
