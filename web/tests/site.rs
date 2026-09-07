//! The site, built and then asked the things S42's exit criteria ask.
//!
//! Every test here builds the **whole** site out of the real repository into a
//! temporary directory. That is deliberate rather than lazy: the criterion is
//! that the site builds *from this repository*, so a test that rendered a
//! fixture would be testing a renderer and not the claim.

use std::path::{Path, PathBuf};

use prism_web::{
    Invocation, PAGES, Site, VERSION, absolute, parse_arguments, rewrite_links, usage,
};

/// The repository root, from this crate's directory.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate is one level under the repository root")
        .to_path_buf()
}

/// A directory of this run's own, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("prism-web-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a temporary directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Builds the site with no installer, and answers where it went.
fn built(name: &str) -> Scratch {
    let scratch = Scratch::new(name);
    let site = Site {
        root: root(),
        installer: None,
    };
    let pages = site.build(&scratch.0).expect("the site builds");
    assert_eq!(pages, PAGES.len());
    scratch
}

/// One page's HTML.
fn page(scratch: &Scratch, path: &str) -> String {
    let file = if path.is_empty() {
        scratch.0.join("index.html")
    } else {
        scratch.0.join(path).join("index.html")
    };
    std::fs::read_to_string(&file)
        .unwrap_or_else(|error| panic!("{} was not written: {error}", file.display()))
}

/* -------------------------------------------------------------------------- */
/* The command line                                                           */
/* -------------------------------------------------------------------------- */

/// The arguments, as strings, the way a shell would hand them over.
fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// With nothing on the command line the site goes where the README says.
#[test]
fn with_no_arguments_the_site_goes_into_web_dist() {
    let parsed = parse_arguments(args(&[]), Path::new("/repo")).expect("nothing is valid");
    assert_eq!(
        parsed,
        Invocation::Render {
            root: PathBuf::from("/repo"),
            out: PathBuf::from("/repo").join("web/dist"),
            installer: None,
        }
    );
}

/// Each flag takes the argument after it, and `--root` moves the default output
/// with it.
#[test]
fn every_flag_takes_the_argument_after_it() {
    let parsed = parse_arguments(
        args(&[
            "--installer",
            "setup.exe",
            "--out",
            "site",
            "--root",
            "/elsewhere",
        ]),
        Path::new("/repo"),
    )
    .expect("all three are valid");
    assert_eq!(
        parsed,
        Invocation::Render {
            root: PathBuf::from("/elsewhere"),
            out: PathBuf::from("site"),
            installer: Some(PathBuf::from("setup.exe")),
        }
    );
    // And `--root` alone moves the output, because the default is under it.
    let moved =
        parse_arguments(args(&["--root", "/elsewhere"]), Path::new("/repo")).expect("valid");
    assert_eq!(
        moved,
        Invocation::Render {
            root: PathBuf::from("/elsewhere"),
            out: PathBuf::from("/elsewhere").join("web/dist"),
            installer: None,
        }
    );
}

/// A flag with nothing after it is a sentence, not a silently missing value.
///
/// This is the failure worth having a test for: `--installer` swallowing the
/// wrong word, or none, is a release page carrying somebody else's checksum, and
/// nothing downstream would say so.
#[test]
fn a_flag_with_no_value_is_refused_by_name() {
    for flag in ["--out", "--root", "--installer"] {
        let complaint = parse_arguments(args(&[flag]), Path::new("/repo"))
            .expect_err("a flag with nothing after it is not valid");
        assert!(complaint.contains(flag), "{complaint}");
    }
}

/// A word it does not know stops it, rather than being ignored.
#[test]
fn an_unknown_word_stops_it() {
    let complaint =
        parse_arguments(args(&["--deploy"]), Path::new("/repo")).expect_err("not a flag");
    assert!(complaint.contains("--deploy"), "{complaint}");
}

/// The binary itself: it prints the usage, and it refuses a word it does not
/// know.
///
/// The four lines in `main.rs` are the ones a library cannot hold — reading the
/// process arguments, and printing — so this runs the **binary**, the way
/// `prismd`'s crash test runs its own. Without it those four lines are the only
/// thing in this crate nothing has ever executed, and *the flag parser is in the
/// library* would be an argument rather than a fact.
#[test]
fn the_binary_prints_its_usage_and_refuses_what_it_does_not_know() {
    let binary = env!("CARGO_BIN_EXE_prism-web");
    let help = std::process::Command::new(binary)
        .arg("--help")
        .output()
        .expect("the binary runs");
    assert!(help.status.success());
    let printed = String::from_utf8_lossy(&help.stdout);
    assert!(printed.contains("--installer"), "{printed}");

    let refused = std::process::Command::new(binary)
        .arg("--deploy")
        .output()
        .expect("the binary runs");
    assert!(!refused.status.success(), "an unknown flag should fail");
    let complaint = String::from_utf8_lossy(&refused.stderr);
    assert!(complaint.contains("--deploy"), "{complaint}");

    // And the ordinary path, into a directory of this test's own.
    let scratch = Scratch::new("binary");
    let out = scratch.0.join("site");
    let built = std::process::Command::new(binary)
        .args(["--root", &root().to_string_lossy()])
        .args(["--out", &out.to_string_lossy()])
        .output()
        .expect("the binary runs");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(out.join("index.html").is_file());
}

/// Help is asked for in both spellings and says how many pages there are.
#[test]
fn help_is_asked_for_in_both_spellings() {
    for word in ["-h", "--help"] {
        assert_eq!(
            parse_arguments(args(&[word]), Path::new("/repo")).expect("valid"),
            Invocation::Help
        );
    }
    let text = usage();
    assert!(text.contains(&PAGES.len().to_string()), "{text}");
    assert!(text.contains("--installer"), "{text}");
}

/* -------------------------------------------------------------------------- */

/// Every page in the navigation exists as a file.
///
/// A link in the navigation that leads to a 404 is the failure this site would
/// have most often and would notice least, because the navigation is written
/// once and the pages are added one at a time.
#[test]
fn every_page_in_the_navigation_is_written() {
    let scratch = built("navigation");
    for entry in &PAGES {
        let html = page(&scratch, entry.path);
        assert!(
            html.contains("</html>"),
            "{} is not a whole document",
            entry.path
        );
    }
}

/// Every page says which version it describes.
///
/// S42's exit criterion, and the reason it can be a test at all is that the
/// number comes from `[workspace.package]` rather than from a template.
#[test]
fn every_page_says_which_version_it_documents() {
    let scratch = built("version");
    for entry in &PAGES {
        let html = page(&scratch, entry.path);
        assert!(
            html.contains(VERSION),
            "{} does not say it documents {VERSION}",
            entry.path
        );
    }
}

/// No page contains a script, of any kind.
///
/// *Readable without JavaScript* is not a claim to be made in prose about a
/// generator that could grow one tomorrow. There is no script tag, no
/// `javascript:` link and no inline handler on any page, and this is what says
/// so.
#[test]
fn no_page_has_any_javascript_on_it() {
    let scratch = built("no-script");
    for entry in &PAGES {
        let html = page(&scratch, entry.path).to_lowercase();
        for forbidden in ["<script", "javascript:", "onclick=", "onload="] {
            assert!(
                !html.contains(forbidden),
                "{} contains {forbidden}, and the site is supposed to be readable with no \
                 JavaScript at all",
                entry.path
            );
        }
    }
}

/// Every page carries the language of its own source.
#[test]
fn a_page_carries_the_language_of_the_document_it_came_from() {
    let scratch = built("lang");
    for entry in &PAGES {
        let html = page(&scratch, entry.path);
        let wanted = format!("<html lang=\"{}\">", entry.lang);
        assert!(html.contains(&wanted), "{} should be {wanted}", entry.path);
    }
}

/// The manuals are the source, not a copy of it.
///
/// Asserted the only way it can be: a sentence that exists in the Markdown file
/// and nowhere in this crate has to appear on the page.
#[test]
fn a_manual_is_rendered_from_the_file_in_docs() {
    let scratch = built("source");
    let markdown = std::fs::read_to_string(root().join("docs/manual/operator.md"))
        .expect("the operator's manual is there");
    let html = page(&scratch, "handbuch/operator");
    for sentence in [
        "Clear</code> lässt los, bevor es vergisst",
        "Ein zweiter Start hängt sich an das Pult",
    ] {
        assert!(
            html.contains(sentence),
            "the operator's page does not carry “{sentence}”"
        );
    }
    // And the other direction: a heading in the file is a heading on the page.
    assert!(markdown.contains("## 11. Das X-Touch"));
    assert!(html.contains("11. Das X-Touch"));
}

/// A relative link that is right in the repository is right on the site.
#[test]
fn a_link_between_two_manuals_becomes_a_link_between_two_pages() {
    let rewritten = rewrite_links(
        r#"<a href="installer.md">x</a> <a href="../../ARCHITECTURE_SPEC.md">y</a>"#,
        "docs/manual/operator.md",
    );
    assert!(rewritten.contains(&format!("href=\"{}\"", absolute("handbuch/installateur"))));
    assert!(rewritten.contains(&format!("href=\"{}\"", absolute("referenz/architektur"))));
}

/// A link to a document the site does not publish goes to GitHub, not to a 404.
#[test]
fn a_link_the_site_does_not_publish_goes_to_the_repository() {
    let rewritten = rewrite_links(
        r#"<a href="../../CLAUDE.md">x</a>"#,
        "docs/manual/developer.md",
    );
    assert!(
        rewritten.contains("https://github.com/flakesystems/PrismDMX/blob/master/CLAUDE.md"),
        "{rewritten}"
    );
}

/// An anchor, an absolute URL and a mail address are left exactly as they were.
#[test]
fn a_link_that_is_not_a_file_is_not_touched() {
    let source = r##"<a href="#5-ein-rig">a</a><a href="https://rustup.rs">b</a><a href="">c</a>"##;
    assert_eq!(rewrite_links(source, "docs/manual/operator.md"), source);
}

/// Every anchor a document points at itself is an anchor the page has.
///
/// Markdown has no anchors: a table of contents is a list of links to `id`s the
/// renderer is expected to have invented, and a renderer that invents none
/// produces a page where every one of them is dead — silently, with nothing to
/// see. Each manual opens with a table of contents, so this is the check that
/// says those thirteen links still land.
///
/// It checks **every** page's own internal links, not only the tables of
/// contents, and it is the one test here that would have caught the first
/// version of this generator.
#[test]
fn every_anchor_a_page_points_at_is_an_anchor_it_has() {
    let scratch = built("anchors");
    for entry in &PAGES {
        let html = page(&scratch, entry.path);
        let ids: Vec<String> = html
            .match_indices("id=\"")
            .filter_map(|(at, _)| {
                let rest = &html[at + 4..];
                rest.find('"').map(|end| rest[..end].to_owned())
            })
            .collect();
        let mut checked = 0_usize;
        for (at, _) in html.match_indices("href=\"#") {
            let rest = &html[at + 7..];
            let Some(end) = rest.find('"') else { continue };
            let target = percent_decoded(&rest[..end]);
            assert!(
                ids.contains(&target),
                "{} links to #{target} and has no such id",
                entry.path
            );
            checked += 1;
        }
        if entry.source.is_some() {
            assert!(
                checked > 0 || !html.contains("href=\"#"),
                "{} has no internal links at all, which is suspicious",
                entry.path
            );
        }
    }
}

/// Percent-decoding, because a German anchor arrives as `ausw%C3%A4hlen`.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A link from one page to a heading on another lands on a heading that exists.
///
/// The same silent failure as the test above, one page along, and it is the one
/// that matters most: the **front page's four steps** point into the two
/// manuals by anchor, and those four links are the whole of what S42 exists to
/// make work. A renumbered chapter would break them with no error anywhere.
#[test]
fn a_link_into_another_page_lands_on_a_heading_that_page_has() {
    let scratch = built("cross-anchors");
    let ids: std::collections::BTreeMap<String, Vec<String>> = PAGES
        .iter()
        .map(|entry| {
            let html = page(&scratch, entry.path);
            let found = html
                .match_indices("id=\"")
                .filter_map(|(at, _)| {
                    let rest = &html[at + 4..];
                    rest.find('"').map(|end| rest[..end].to_owned())
                })
                .collect();
            (absolute(entry.path), found)
        })
        .collect();
    let mut checked = 0_usize;
    for entry in &PAGES {
        let html = page(&scratch, entry.path);
        for (at, _) in html.match_indices("href=\"/") {
            let rest = &html[at + 6..];
            let Some(end) = rest.find('"') else { continue };
            let target = &rest[..end];
            let Some((path, fragment)) = target.split_once('#') else {
                continue;
            };
            let anchors = ids
                .get(path)
                .unwrap_or_else(|| panic!("{} links to {path}, which is not a page", entry.path));
            let fragment = percent_decoded(fragment);
            assert!(
                anchors.contains(&fragment),
                "{} links to {path}#{fragment} and that page has no such heading",
                entry.path
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 3,
        "the front page's steps link into the manuals by anchor; only {checked} such links were \
         found, so this test is no longer checking what it was written for"
    );
}

/// The table of contents of the operator's manual, by name.
///
/// The test above says every anchor resolves; this says the thirteen chapters
/// are still there, so a manual quietly losing a chapter is not a manual that
/// passes because it has fewer links.
#[test]
fn the_operators_manual_still_has_its_thirteen_chapters() {
    let scratch = built("chapters");
    let html = page(&scratch, "handbuch/operator");
    // The numbered ones. `## Inhalt` is an `h2` as well and is the table of
    // contents rather than a chapter, so counting every `h2` would count it.
    let chapters = html
        .match_indices("<h2 id=\"")
        .filter(|(at, _)| {
            html[at + 8..]
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit())
        })
        .count();
    assert_eq!(
        chapters, 13,
        "the operator's manual should have thirteen numbered chapters"
    );
}

/// The checksum on the download page is the file's, computed here.
///
/// This is what *without anybody typing it* means, stated as a test: the page
/// carries the SHA-256 of the bytes it was handed, so a release whose page and
/// whose installer disagree cannot be produced.
#[test]
fn the_download_page_carries_the_checksum_of_the_installer_it_was_given() {
    let scratch = Scratch::new("checksum");
    let installer = scratch.0.join("PrismDMX_test_x64-setup.exe");
    std::fs::write(&installer, b"not really an installer").expect("the file is written");
    // sha256("not really an installer"), taken from a different implementation
    // — `python3 -c "import hashlib; print(hashlib.sha256(b'not really an \
    // installer').hexdigest())"` — because a checksum test that asked this
    // crate what the checksum was would pass for any hash function at all.
    let expected = "110499c3d4d34a94a1ea70ae7e7353d32708e043bc0ccee13ec9fbdb7a9d20b1";
    let out = scratch.0.join("site");
    let site = Site {
        root: root(),
        installer: Some(installer.clone()),
    };
    let hashed = site
        .installer()
        .expect("the installer is readable")
        .expect("there is one");
    assert_eq!(hashed.name, "PrismDMX_test_x64-setup.exe");
    assert_eq!(hashed.sha256.len(), 64);
    assert_eq!(hashed.sha256, expected);
    site.build(&out).expect("the site builds");
    let html = std::fs::read_to_string(out.join("download/index.html")).expect("the page");
    assert!(
        html.contains(&hashed.sha256),
        "the checksum is not on the page"
    );
    assert!(html.contains("PrismDMX_test_x64-setup.exe"));
}

/// Without an installer the page links to the release rather than inventing a
/// number.
#[test]
fn without_an_installer_the_download_page_points_at_the_release() {
    let scratch = built("no-installer");
    let html = page(&scratch, "download");
    assert!(
        html.contains("releases"),
        "there is no link to the releases"
    );
    assert!(
        !html.contains("SHA-256</th>"),
        "a checksum table was printed for a file nothing hashed"
    );
}

/// The front page names the four steps that are this session's whole point.
///
/// *A stranger gets from the front page to a running desk.* No test can check
/// that a person managed it; what a test can check is that the page still tells
/// them how, and that the four steps have not been edited away.
#[test]
fn the_front_page_still_says_how_to_get_to_a_running_desk() {
    let scratch = built("front");
    let html = page(&scratch, "");
    for step in [
        "Herunterladen und installieren",
        "Ein Fixture patchen",
        "1 at full",
    ] {
        assert!(
            html.contains(step),
            "the front page no longer says “{step}”"
        );
    }
    assert!(html.contains("href=\"/download/\""));
}

/// The domain is written by the generator, so it is one constant in one place.
#[test]
fn the_apex_domain_is_written_beside_the_pages() {
    let scratch = built("cname");
    let cname = std::fs::read_to_string(scratch.0.join("CNAME")).expect("CNAME is written");
    assert_eq!(cname.trim(), "prismdmx.de");
}

/// A missing source is a stopped build, not a hole in the navigation.
#[test]
fn a_missing_document_stops_the_build_rather_than_leaving_a_gap() {
    let scratch = Scratch::new("missing");
    let empty = scratch.0.join("nothing-here");
    std::fs::create_dir_all(&empty).expect("a directory");
    let site = Site {
        root: empty,
        installer: None,
    };
    let error = site
        .build(&scratch.0.join("site"))
        .expect_err("a repository with no documents in it cannot produce a site");
    assert!(
        error.0.contains("missing one is a missing page"),
        "{}",
        error.0
    );
}
