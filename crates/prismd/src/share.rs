//! GDTF Share, behind a seam — **S62**.
//!
//! # Why there is a trait here at all
//!
//! `CLAUDE.md` says a test may not need a device. A test that needs the
//! **internet** is the same promise broken a different way, and this desk has
//! no way to reach `gdtf-share.com` from a build machine anyway: that service
//! has no anonymous access, so there is nothing a CI job could log in as.
//!
//! So everything that talks to the network is [`Share`], and everything that
//! decides *what to do* is [`update`], which takes one. The real implementation
//! is [`Https`] and is the only part no test covers; the orchestration — log
//! in, list, download, write, count, say what happened — is covered by a fake
//! that answers from memory.
//!
//! # What this is not
//!
//! It is **not** a way to redistribute anything. The operator logs in with
//! their own account and the files land in their own fixture folder; see
//! `docs/FIXTURE_LIBRARY.md` §2 and decision **D12**. Nothing downloaded here
//! is ever packed into an installer.
//!
//! # The API
//!
//! Three endpoints under `https://gdtf-share.com/apis/public/`, documented by
//! the GDTF/MVR developers and offered there for exactly this use — see
//! `docs/FIXTURE_LIBRARY.md` §3, which also records the one trap: the official
//! document says the login is JSON and the working reference implementation
//! posts **form data**, which is what this does.

use std::path::{Path, PathBuf};

/// One published fixture, as `getList.php` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// The revision id `downloadFile.php` takes.
    pub rid: u64,
    /// Who makes it, for the file name and for what an operator is told.
    pub manufacturer: String,
    /// What it is called.
    pub fixture: String,
}

impl Listed {
    /// The file name this fixture is written under.
    ///
    /// `Maker@Fixture@rid.gdtf`, which is the shape GDTF Share publishes and
    /// so the shape an operator will recognise in their folder. Everything a
    /// file system objects to is replaced, because a fixture called `24/7` is
    /// a fixture somebody published.
    #[must_use]
    pub fn file_name(&self) -> String {
        let safe = |text: &str| -> String {
            text.chars()
                .map(|character| match character {
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
                    other if other.is_control() => '-',
                    other => other,
                })
                .collect::<String>()
                .trim()
                .to_owned()
        };
        format!(
            "{}@{}@{}.gdtf",
            safe(&self.manufacturer),
            safe(&self.fixture),
            self.rid
        )
    }
}

/// What went wrong, in words an operator can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareError {
    /// The account was refused.
    Credentials,
    /// The service could not be reached, or answered with something else.
    Unreachable(String),
    /// A file could not be written.
    Write(String),
}

impl std::fmt::Display for ShareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credentials => write!(
                formatter,
                "GDTF Share refused the user name or the password"
            ),
            Self::Unreachable(why) => write!(formatter, "GDTF Share could not be reached: {why}"),
            Self::Write(why) => write!(formatter, "a fixture could not be written: {why}"),
        }
    }
}

impl std::error::Error for ShareError {}

/// Everything that touches the network, so that nothing else does.
pub trait Share {
    /// Signs in. Everything after this is on the session it opened.
    ///
    /// # Errors
    ///
    /// [`ShareError::Credentials`] for an account that was refused,
    /// [`ShareError::Unreachable`] for anything else.
    fn login(&mut self, user: &str, password: &str) -> Result<(), ShareError>;

    /// Every published fixture.
    ///
    /// # Errors
    ///
    /// [`ShareError::Unreachable`].
    fn list(&mut self) -> Result<Vec<Listed>, ShareError>;

    /// One fixture's `.gdtf`.
    ///
    /// # Errors
    ///
    /// [`ShareError::Unreachable`].
    fn download(&mut self, rid: u64) -> Result<Vec<u8>, ShareError>;
}

/// How far an update has got, for the operator to watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Fixtures written so far.
    pub done: u32,
    /// Fixtures the list named.
    pub total: u32,
}

/// What an update did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    /// Fixtures written.
    pub written: u32,
    /// Fixtures the list named.
    pub listed: u32,
    /// Fixtures that would not download, or would not read as a fixture.
    ///
    /// Counted and skipped rather than fatal: one bad revision out of three
    /// thousand must not cost an operator the other two thousand nine hundred
    /// and ninety-nine.
    pub skipped: u32,
}

/// Takes the whole published library into `directory`.
///
/// `into` is called after every fixture, so a caller can say how far it has
/// got. Answering `false` **stops** the update, which is what a cancel is.
///
/// # What it writes, and what it leaves
///
/// One `.gdtf` per fixture, named by [`Listed::file_name`]. A fixture that
/// will not download, or that downloads and is not a fixture, is counted and
/// skipped — the checking is here and not at the end, so nothing that is not a
/// fixture profile ever lands in an operator's folder.
///
/// # Errors
///
/// [`ShareError::Credentials`] and [`ShareError::Unreachable`] from the two
/// steps before the downloading starts, because those are the ones that mean
/// *nothing will work*; after that a failure is a skipped fixture and not an
/// error. [`ShareError::Write`] if the directory cannot be made.
pub fn update(
    share: &mut dyn Share,
    user: &str,
    password: &str,
    directory: &Path,
    mut into: impl FnMut(Progress) -> bool,
) -> Result<Report, ShareError> {
    share.login(user, password)?;
    let listed = share.list()?;
    std::fs::create_dir_all(directory).map_err(|error| ShareError::Write(error.to_string()))?;

    let total = u32::try_from(listed.len()).unwrap_or(u32::MAX);
    let mut report = Report {
        listed: total,
        ..Report::default()
    };
    for fixture in listed {
        match share.download(fixture.rid) {
            // Read before it is written, the same order the single-file import
            // follows: an operator's folder should never hold something this
            // desk would not have accepted.
            Ok(bytes) if is_a_fixture(&bytes) => {
                let path: PathBuf = directory.join(fixture.file_name());
                if std::fs::write(&path, &bytes).is_ok() {
                    report.written += 1;
                } else {
                    report.skipped += 1;
                }
            }
            _ => report.skipped += 1,
        }
        if !into(Progress {
            done: report.written + report.skipped,
            total,
        }) {
            break;
        }
    }
    Ok(report)
}

/// Whether these bytes are a fixture profile this desk can read.
fn is_a_fixture(bytes: &[u8]) -> bool {
    let (built, _) = prism_core::library::gdtf::read_archive(bytes, true);
    !built.is_empty()
}

/// GDTF Share over HTTPS — the one part of this module no test covers.
///
/// Everything it does is the three documented endpoints and a cookie jar.
/// There is nothing to assert about it that would not be asserting about
/// `reqwest`, and there is no way to reach the service from a build machine:
/// see this module's own documentation.
pub struct Https {
    /// The base every call hangs off, so a test build or a mirror can move it.
    base: String,
    /// Blocking on purpose: this runs on a thread of its own, away from the
    /// tick and away from the async runtime that serves clients.
    client: reqwest::blocking::Client,
}

impl Https {
    /// A client against the published service.
    ///
    /// # Errors
    ///
    /// [`ShareError::Unreachable`] if a TLS client cannot be built at all,
    /// which on a desk means the platform's certificate store is unreadable.
    pub fn new(base: &str) -> Result<Self, ShareError> {
        let client = reqwest::blocking::Client::builder()
            // A desk on a slow venue link is the ordinary case; a desk waiting
            // for ever on a service that has stopped answering is not.
            .timeout(std::time::Duration::from_secs(60))
            .cookie_store(true)
            .build()
            .map_err(|error| ShareError::Unreachable(error.to_string()))?;
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            client,
        })
    }
}

impl Share for Https {
    fn login(&mut self, user: &str, password: &str) -> Result<(), ShareError> {
        // **Form-encoded, not JSON.** The official document says
        // `Content-Type: application/json` and the working reference
        // implementation posts form data; this follows what works. See
        // `docs/FIXTURE_LIBRARY.md` §3.
        let response = self
            .client
            .post(format!("{}/login.php", self.base))
            .form(&[("user", user), ("password", password)])
            .send()
            .map_err(|error| ShareError::Unreachable(error.to_string()))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ShareError::Credentials);
        }
        if !response.status().is_success() {
            return Err(ShareError::Unreachable(response.status().to_string()));
        }
        Ok(())
    }

    fn list(&mut self) -> Result<Vec<Listed>, ShareError> {
        let response = self
            .client
            .get(format!("{}/getList.php", self.base))
            .send()
            .map_err(|error| ShareError::Unreachable(error.to_string()))?;
        if !response.status().is_success() {
            return Err(ShareError::Unreachable(response.status().to_string()));
        }
        let text = response
            .text()
            .map_err(|error| ShareError::Unreachable(error.to_string()))?;
        Ok(parse_list(&text))
    }

    fn download(&mut self, rid: u64) -> Result<Vec<u8>, ShareError> {
        let response = self
            .client
            .get(format!("{}/downloadFile.php?rid={rid}", self.base))
            .send()
            .map_err(|error| ShareError::Unreachable(error.to_string()))?;
        if !response.status().is_success() {
            return Err(ShareError::Unreachable(response.status().to_string()));
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| ShareError::Unreachable(error.to_string()))
    }
}

/// Reads what `getList.php` answered.
///
/// Kept out of [`Https`] so it can be tested without a network: the shape of
/// the answer is the part that can be wrong, and the part that will change
/// under us if the service ever reshapes it.
#[must_use]
pub fn parse_list(text: &str) -> Vec<Listed> {
    let Ok(document) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    // The service answers `{"list": [...]}`; a bare array is accepted too,
    // because that costs one line and a reader that refuses a shape nobody
    // promised would refuse it in front of an operator.
    let rows = document
        .get("list")
        .and_then(serde_json::Value::as_array)
        .or_else(|| document.as_array());
    let Some(rows) = rows else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            Some(Listed {
                rid: row.get("rid").and_then(serde_json::Value::as_u64)?,
                manufacturer: row
                    .get("manufacturer")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Unknown")
                    .to_owned(),
                fixture: row
                    .get("fixture")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Fixture")
                    .to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
