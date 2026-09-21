//! What an update of the library does — **S62**.
//!
//! Every test here drives [`super::update`] through a [`Fake`], which is the
//! whole reason the trait exists: the orchestration is the part that can be
//! wrong, and it can be checked without a network, without an account, and
//! without the service being up. What is left uncovered is [`super::Https`],
//! which is three URLs and a cookie jar.

use std::collections::BTreeMap;

use super::{Listed, Progress, Share, ShareError, parse_list, update};

/// A `.gdtf` archive, so a downloaded fixture is a real one.
fn archive(manufacturer: &str, name: &str) -> Vec<u8> {
    crate::testkit::gdtf_archive(manufacturer, name, 1)
}

/// GDTF Share, from memory.
struct Fake {
    /// What `login` should answer.
    credentials: Result<(), ShareError>,
    /// What `list` should answer.
    listing: Result<Vec<Listed>, ShareError>,
    /// What each revision downloads as; anything missing fails.
    files: BTreeMap<u64, Vec<u8>>,
    /// Whether `login` was called before anything else.
    signed_in: bool,
    /// Which revisions were asked for, in order.
    asked: Vec<u64>,
}

impl Fake {
    fn new(listing: Vec<Listed>) -> Self {
        Self {
            credentials: Ok(()),
            listing: Ok(listing),
            files: BTreeMap::new(),
            signed_in: false,
            asked: Vec::new(),
        }
    }

    fn with(mut self, rid: u64, bytes: Vec<u8>) -> Self {
        self.files.insert(rid, bytes);
        self
    }
}

impl Share for Fake {
    fn login(&mut self, user: &str, password: &str) -> Result<(), ShareError> {
        assert!(!user.is_empty(), "an empty user name reached the service");
        assert!(
            !password.is_empty(),
            "an empty password reached the service"
        );
        self.credentials.clone()?;
        self.signed_in = true;
        Ok(())
    }

    fn list(&mut self) -> Result<Vec<Listed>, ShareError> {
        assert!(self.signed_in, "the list was asked for before signing in");
        self.listing.clone()
    }

    fn download(&mut self, rid: u64) -> Result<Vec<u8>, ShareError> {
        assert!(self.signed_in, "a file was asked for before signing in");
        self.asked.push(rid);
        self.files
            .get(&rid)
            .cloned()
            .ok_or_else(|| ShareError::Unreachable("no such revision".to_owned()))
    }
}

fn listed(rid: u64, manufacturer: &str, fixture: &str) -> Listed {
    Listed {
        rid,
        manufacturer: manufacturer.to_owned(),
        fixture: fixture.to_owned(),
    }
}

/// **The whole of an update**: sign in, list, download, write, report.
#[test]
fn the_published_library_arrives_in_the_operators_folder() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut share = Fake::new(vec![
        listed(11, "Robe", "Robin T1"),
        listed(22, "Chauvet", "Rogue R2"),
    ])
    .with(11, archive("Robe", "Robin T1"))
    .with(22, archive("Chauvet", "Rogue R2"));

    let mut seen = Vec::new();
    let report = update(&mut share, "somebody", "a secret", dir.path(), |progress| {
        seen.push(progress);
        true
    })
    .expect("the update runs");

    assert_eq!(report.listed, 2);
    assert_eq!(report.written, 2);
    assert_eq!(report.skipped, 0);

    // Named the way the service publishes them, so an operator recognises
    // what is in their folder.
    assert!(dir.path().join("Robe@Robin T1@11.gdtf").is_file());
    assert!(dir.path().join("Chauvet@Rogue R2@22.gdtf").is_file());

    // And the caller was told after each one, so a progress row can move.
    assert_eq!(
        seen,
        [
            Progress { done: 1, total: 2 },
            Progress { done: 2, total: 2 },
        ]
    );
}

/// A refused account stops before anything is written.
#[test]
fn a_refused_account_writes_nothing_and_says_which_fault_it_was() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut share = Fake::new(vec![listed(1, "Robe", "T1")]);
    share.credentials = Err(ShareError::Credentials);

    let error = update(&mut share, "somebody", "wrong", dir.path(), |_| true)
        .expect_err("a refused account is an error");
    assert_eq!(error, ShareError::Credentials);
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("it exists").count(),
        0,
        "nothing was written"
    );
}

/// **One bad revision does not cost the other two thousand.**
#[test]
fn a_fixture_that_will_not_download_is_skipped_and_the_rest_arrive() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut share = Fake::new(vec![
        listed(1, "Robe", "Good"),
        listed(2, "Robe", "Missing"),
        listed(3, "Robe", "Also good"),
    ])
    .with(1, archive("Robe", "Good"))
    // 2 is not in the map: it fails to download.
    .with(3, archive("Robe", "Also good"));

    let report =
        update(&mut share, "somebody", "a secret", dir.path(), |_| true).expect("the update runs");
    assert_eq!(report.listed, 3);
    assert_eq!(report.written, 2);
    assert_eq!(report.skipped, 1);
    assert!(!dir.path().join("Robe@Missing@2.gdtf").exists());
}

/// **Something that downloads and is not a fixture never reaches the folder.**
///
/// The same order the single-file import follows: read first, write second.
#[test]
fn something_that_is_not_a_fixture_is_skipped_before_it_is_written() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut share = Fake::new(vec![listed(7, "Somebody", "Not a fixture")])
        .with(7, b"a web page saying the service is down".to_vec());

    let report =
        update(&mut share, "somebody", "a secret", dir.path(), |_| true).expect("the update runs");
    assert_eq!(report.written, 0);
    assert_eq!(report.skipped, 1);
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("it exists").count(),
        0,
        "nothing that is not a fixture was written"
    );
}

/// Answering `false` stops the update, which is what a cancel is.
#[test]
fn the_caller_can_stop_it_part_way() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut share = Fake::new(vec![
        listed(1, "Robe", "One"),
        listed(2, "Robe", "Two"),
        listed(3, "Robe", "Three"),
    ])
    .with(1, archive("Robe", "One"))
    .with(2, archive("Robe", "Two"))
    .with(3, archive("Robe", "Three"));

    let report = update(&mut share, "somebody", "a secret", dir.path(), |progress| {
        progress.done < 2
    })
    .expect("the update runs");
    assert_eq!(report.written, 2, "it stopped after the second");
    assert_eq!(share.asked, [1, 2], "the third was never asked for");
}

/// The file name is one a file system will take.
#[test]
fn a_fixture_whose_name_a_file_system_objects_to_still_gets_a_file() {
    let awkward = listed(5, "Maker/Co", "24/7 : Wash");
    assert_eq!(awkward.file_name(), "Maker-Co@24-7 - Wash@5.gdtf");
}

/// What `getList.php` answers, read without the service.
#[test]
fn the_list_is_read_out_of_what_the_service_answers() {
    let text = r#"{"list":[
        {"rid":101,"manufacturer":"Robe","fixture":"Robin T1 Profile","revision":"3"},
        {"rid":102,"manufacturer":"Chauvet","fixture":"Rogue R2"}
    ]}"#;
    assert_eq!(
        parse_list(text),
        [
            listed(101, "Robe", "Robin T1 Profile"),
            listed(102, "Chauvet", "Rogue R2"),
        ]
    );

    // A bare array is taken too, and a row with no `rid` is not a fixture.
    assert_eq!(
        parse_list(r#"[{"rid":7,"manufacturer":"M","fixture":"F"},{"manufacturer":"M"}]"#),
        [listed(7, "M", "F")]
    );
    // Anything that is not the answer at all is no fixtures, not a panic.
    assert!(parse_list("<html>service unavailable</html>").is_empty());
    assert!(parse_list("").is_empty());
}
