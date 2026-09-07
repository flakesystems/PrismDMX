//! The site, built and then asked the things S42's exit criteria ask.
//!
//! Every test here builds the **whole** site out of the real repository into a
//! temporary directory. That is deliberate rather than lazy: the criterion is
//! that the site builds *from this repository*, so a test that rendered a
//! fixture would be testing a renderer and not the claim.

use std::path::{Path, PathBuf};

use prism_web::{
    Borrowed, DOMAIN, Invocation, LANGUAGES, PAGES, Site, Source, VERSION, absolute,
    parse_arguments, rewrite_links, usage,
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
    // Every page in every language: the count is a multiplication, and asserting
    // it here means a language added to `LANGUAGES` without the generator
    // learning about it fails in every test rather than in none.
    assert_eq!(pages, PAGES.len() * LANGUAGES.len());
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
/* Two languages                                                              */
/* -------------------------------------------------------------------------- */

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

/// Resolves an `href` found on page `here` the way a browser would.
///
/// A page is served as `<path>/index.html`, so the document's base directory is
/// `<path>/` itself and a relative link is resolved against that. Returns the
/// site path of the target page and its fragment, or `None` for a link that
/// leaves the site.
fn resolve(here: &str, href: &str) -> Option<(String, String)> {
    if href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
        || href.starts_with('#')
    {
        return None;
    }
    let (path, fragment) = match href.split_once('#') {
        Some((path, fragment)) => (path, percent_decoded(fragment)),
        None => (href, String::new()),
    };
    let mut parts: Vec<&str> = if here.is_empty() {
        Vec::new()
    } else {
        here.split('/').collect()
    };
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    Some((parts.join("/"), fragment))
}

/// Every href on a page, as it was written.
fn hrefs(html: &str) -> Vec<String> {
    html.match_indices("href=\"")
        .filter_map(|(at, _)| {
            let rest = &html[at + 6..];
            rest.find('"').map(|end| rest[..end].to_owned())
        })
        .collect()
}

/// Every `id` on a page.
fn ids(html: &str) -> Vec<String> {
    html.match_indices("id=\"")
        .filter_map(|(at, _)| {
            let rest = &html[at + 4..];
            rest.find('"').map(|end| rest[..end].to_owned())
        })
        .collect()
}

/// Every page of the site, in every language, as (located path, html).
fn all_pages(scratch: &Scratch) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for language in &LANGUAGES {
        for entry in &PAGES {
            let located = entry.located(language.code);
            let html = page(scratch, &located);
            out.push((located, html));
        }
    }
    out
}

/// Every page exists in every language.
///
/// The site publishes a fixed set of pages in a fixed set of languages, so the
/// number of files is a multiplication rather than a list — and a page that
/// exists in one language and not the other is the failure a bilingual site has
/// most often, because a page is added in the language whoever added it speaks.
#[test]
fn every_page_is_written_in_every_language() {
    let scratch = built("languages");
    for (located, html) in all_pages(&scratch) {
        assert!(
            html.contains("</html>"),
            "{located} is not a whole document"
        );
    }
    // And the root, which belongs to no language and therefore has to choose.
    let root = std::fs::read_to_string(scratch.0.join("index.html")).expect("a root page");
    assert!(
        root.contains("http-equiv=\"refresh\""),
        "the site root does not send the reader anywhere"
    );
    assert!(
        root.contains(&format!("url={}/", LANGUAGES[0].code)),
        "the site root does not go to the default language"
    );
    // A `meta refresh` is something a browser may refuse, so the root also has
    // to be usable as a page: both languages, as real links.
    for language in &LANGUAGES {
        assert!(
            hrefs(&root)
                .iter()
                .any(|href| href == &format!("{}/", language.code)),
            "the site root has no link to {}",
            language.code
        );
    }
}

/// Every page declares the language it is written in.
#[test]
fn every_page_declares_its_own_language() {
    let scratch = built("lang-attribute");
    for language in &LANGUAGES {
        for entry in &PAGES {
            let located = entry.located(language.code);
            let html = page(&scratch, &located);
            assert!(
                html.contains(&format!("<html lang=\"{}\">", language.code)),
                "{located} does not say it is in {}",
                language.code
            );
        }
    }
}

/// The switcher on every page points at **that** page in the other language.
///
/// This is the whole point of a language switcher and the thing most of them get
/// wrong: landing a reader on the front page because they wanted to read the
/// page they were already on, in their own language. The `id` is what makes it
/// checkable — the two paths share nothing else, deliberately, because
/// `/de/handbuch/operator/` should not have to be an English noun.
#[test]
fn the_switcher_lands_on_the_same_page_in_the_other_language() {
    let scratch = built("switcher");
    for language in &LANGUAGES {
        for entry in &PAGES {
            let located = entry.located(language.code);
            let html = page(&scratch, &located);
            for other in &LANGUAGES {
                let want = entry.located(other.code);
                let found = hrefs(&html)
                    .into_iter()
                    .any(|href| resolve(&located, &href).is_some_and(|(target, _)| target == want));
                assert!(
                    found,
                    "{located} has no link to itself in {} (expected {want})",
                    other.code
                );
            }
        }
    }
}

/// Every page names its other language to a search engine.
///
/// Without this a reader who arrives from a search arrives in whichever language
/// happened to be indexed, and the switcher is the only way back — which they
/// will not look for, because they will assume that is all there is.
#[test]
fn every_page_names_its_translations_to_a_search_engine() {
    let scratch = built("hreflang");
    for language in &LANGUAGES {
        for entry in &PAGES {
            let located = entry.located(language.code);
            let html = page(&scratch, &located);
            for other in &LANGUAGES {
                assert!(
                    html.contains(&format!("hreflang=\"{}\"", other.code)),
                    "{located} does not name {} as an alternative",
                    other.code
                );
            }
            assert!(
                html.contains("hreflang=\"x-default\""),
                "{located} names no default language"
            );
        }
    }
}

/// A page whose text is not in its own language says so, before the text.
///
/// The alternative is a reader who starts a German manual, hits English three
/// paragraphs in and concludes the site is broken. Saying it first costs one
/// sentence and is the difference between a gap and a defect.
#[test]
fn a_page_that_borrowed_its_text_says_so_first() {
    let scratch = built("borrowed");
    for language in &LANGUAGES {
        for entry in &PAGES {
            let located = entry.located(language.code);
            let html = page(&scratch, &located);
            let Source::Borrowed { lang, .. } = entry.variant(language.code).source else {
                assert!(
                    !html.contains("class=\"borrowed\""),
                    "{located} is in its own language and should not apologise for it"
                );
                continue;
            };
            let notice = html
                .find("class=\"borrowed\"")
                .unwrap_or_else(|| panic!("{located} borrows its text and does not say so"));
            let body = html.find("<main>").expect("a body");
            assert!(
                notice > body && notice < body + 400,
                "{located} says it borrowed its text, but not at the top where it is read"
            );
            assert!(
                html.contains(&format!("<div lang=\"{lang}\">")),
                "{located} does not mark the borrowed text as being in {lang}"
            );
        }
    }
}

/// How many pages are still waiting for a translation, said out loud.
///
/// Not an assertion that there are none — there are, and pretending otherwise
/// would be the lie this whole mechanism exists to avoid. It is a **ratchet**:
/// the number is written here, so a session that adds an untranslated page has
/// to come and raise it on purpose, and one that translates a page gets to lower
/// it. A silent gap is the only outcome this rules out.
#[test]
fn the_number_of_untranslated_pages_is_written_down() {
    let waiting: Vec<String> = LANGUAGES
        .iter()
        .flat_map(|language| {
            PAGES
                .iter()
                .filter(move |entry| {
                    matches!(
                        entry.variant(language.code).source,
                        Source::Borrowed {
                            why: Borrowed::NotYet,
                            ..
                        }
                    )
                })
                .map(move |entry| entry.located(language.code))
        })
        .collect();
    assert!(
        waiting.is_empty(),
        "every page of this site exists in both languages now, and these do not: {waiting:?} — if \
         a page was added in one language only, either translate it or record here why it is \
         waiting"
    );
}

/* -------------------------------------------------------------------------- */
/* The pages themselves                                                       */
/* -------------------------------------------------------------------------- */

/// Every page says which version it documents.
#[test]
fn every_page_says_which_version_it_documents() {
    let scratch = built("version");
    for (located, html) in all_pages(&scratch) {
        assert!(
            html.contains(VERSION),
            "{located} does not say which version it describes"
        );
    }
}

/// No page has any JavaScript on it.
///
/// A documentation page that needs a script to be read is a page that fails for
/// somebody, and none of this needs one: there is no search box, no analytics
/// and no cookie banner, and the language switcher is two links.
#[test]
fn no_page_has_any_javascript_on_it() {
    let scratch = built("no-script");
    for (located, html) in all_pages(&scratch) {
        let lowered = html.to_lowercase();
        for forbidden in ["<script", "javascript:", " onclick=", " onload="] {
            assert!(
                !lowered.contains(forbidden),
                "{located} contains {forbidden}"
            );
        }
    }
}

/// A manual is rendered from the file in `docs/`, not from a copy.
#[test]
fn a_manual_is_rendered_from_the_file_in_docs() {
    let scratch = built("manual");
    let markdown = std::fs::read_to_string(root().join("docs/manual/operator.de.md"))
        .expect("the operator's manual is in the repository");
    let html = page(&scratch, "de/handbuch/operator");
    for sentence in [
        "Jeder Kanal hat einen Knopf",
        "Die Kommandozeile ist die Bedienung",
    ] {
        assert!(
            html.contains(sentence),
            "the operator's page does not carry \u{201c}{sentence}\u{201d}"
        );
    }
    assert!(markdown.contains("## 11. Das X-Touch"));
    assert!(html.contains("11. Das X-Touch"));
}

/// A relative link that is right in the repository is right on the site.
#[test]
fn a_link_between_two_manuals_becomes_a_link_between_two_pages() {
    let rewritten = rewrite_links(
        r#"<a href="installer.de.md">x</a>"#,
        "docs/manual/operator.de.md",
        "de",
    );
    assert!(
        rewritten.contains(&format!(
            "href=\"{}\"",
            absolute("de/handbuch/installateur")
        )),
        "{rewritten}"
    );
}

/// The same link, rewritten for the other language, goes to the other language.
///
/// A German manual linking its reader into English is the quiet way a bilingual
/// site loses people, and it is one wrong lookup away at all times: the map from
/// file to page has to be built per language, not once.
#[test]
fn a_link_rewritten_for_english_stays_in_english() {
    let rewritten = rewrite_links(
        r#"<a href="developer.en.md">x</a>"#,
        "docs/manual/operator.en.md",
        "en",
    );
    assert!(
        rewritten.contains(&format!("href=\"{}\"", absolute("en/manual/developer"))),
        "{rewritten}"
    );
}

/// A link to a document the site does not publish goes to GitHub, not to a 404.
///
/// The four specifications are exactly this case since the owner took them off
/// the site: they are working documents that change with the source, so a manual
/// that links to one has to reach the file in the repository.
#[test]
fn a_link_the_site_does_not_publish_goes_to_the_repository() {
    let rewritten = rewrite_links(
        r#"<a href="../../ARCHITECTURE_SPEC.md">y</a>"#,
        "docs/manual/operator.de.md",
        "de",
    );
    assert!(
        rewritten
            .contains("https://github.com/flakesystems/PrismDMX/blob/master/ARCHITECTURE_SPEC.md"),
        "{rewritten}"
    );
}

/// A link that is not a file is left exactly as it was.
#[test]
fn a_link_that_is_not_a_file_is_not_touched() {
    let source = r##"<a href="https://example.org/x">a</a> <a href="#anchor">b</a>"##;
    assert_eq!(
        rewrite_links(source, "docs/manual/operator.de.md", "de"),
        source
    );
}

/// Every link on every page lands on a page that exists, and on a heading it has.
///
/// **This is the test the first deployment did not have, and the fault it would
/// have caught was total.** GitHub Pages served the site as a *project* page
/// under a path prefix, and every URL the generator wrote was absolute from `/`
/// — so every link on every page pointed a level above the project. Nothing
/// errored: the stylesheet is inline, so the site rendered perfectly and went
/// nowhere.
///
/// So this resolves each link the way a browser does — against the directory the
/// page is served from — rather than checking the strings the generator wrote.
#[test]
fn every_link_on_every_page_lands_on_a_page_that_exists() {
    let scratch = built("resolution");
    let mut anchors: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (located, html) in all_pages(&scratch) {
        anchors.insert(located, ids(&html));
    }
    let mut links = 0_usize;
    let mut anchored_links = 0_usize;
    for (located, html) in all_pages(&scratch) {
        for href in hrefs(&html) {
            let Some((target, fragment)) = resolve(&located, &href) else {
                continue;
            };
            links += 1;
            let has = anchors.get(&target).unwrap_or_else(|| {
                panic!("{located} links to \u{201c}{href}\u{201d}, which resolves to \u{201c}{target}\u{201d} — no such page")
            });
            if !fragment.is_empty() {
                anchored_links += 1;
                assert!(
                    has.contains(&fragment),
                    "{located} links to {target}#{fragment} and that page has no such heading"
                );
            }
        }
    }
    assert!(
        links >= 200,
        "only {links} internal links were resolved; sixteen pages with a navigation of eight and a \\
         switcher of two should give far more, so this test is no longer seeing the whole site"
    );
    assert!(
        anchored_links >= 2,
        "only {anchored_links} links point at a heading; the index links into the manuals by \\
         anchor, so this test is no longer checking what it was written for"
    );
}

/// No page asks for a path from the site's root, because it may not have one.
#[test]
fn no_page_links_to_a_path_from_the_site_root() {
    let scratch = built("portable");
    let mut documents = all_pages(&scratch);
    documents.push((
        "index.html".to_owned(),
        std::fs::read_to_string(scratch.0.join("index.html")).expect("a root page"),
    ));
    for (located, html) in documents {
        if let Some(at) = html.find("href=\"/") {
            let rest = &html[at + 6..];
            let end = rest.find('"').unwrap_or(rest.len());
            panic!(
                "{located} links to \u{201c}{}\u{201d} — a path from the site root, which is only \
                 correct when the site happens to be mounted at one",
                &rest[..end]
            );
        }
    }
}

/// Both languages of a document have the same chapters.
///
/// **This is the test the owner's decision needs.** The repository used to say,
/// in writing, that nothing would be translated — because two languages per
/// document are two documents that diverge on the first fix, and the reader who
/// gets the stale one has no way of knowing. That decision has been overruled,
/// which is the owner's call; what does not go away is the failure it was
/// avoiding. So the two versions are held against each other by shape: same
/// number of chapters, same headings in the same order, same anchors. It cannot
/// tell you a paragraph is out of date, but it catches the thing that actually
/// happens — a chapter added to one language and not the other.
#[test]
fn both_languages_of_a_document_have_the_same_chapters() {
    let scratch = built("divergence");
    let mut checked = 0_usize;
    for entry in &PAGES {
        // Only where both languages are the document's own: a page that is still
        // waiting for its translation is *known* to differ, and that is what the
        // ratchet test above is for.
        let both_own = LANGUAGES
            .iter()
            .all(|language| matches!(entry.variant(language.code).source, Source::Own(_)));
        if !both_own {
            continue;
        }
        let counts: Vec<(String, usize)> = LANGUAGES
            .iter()
            .map(|language| {
                let located = entry.located(language.code);
                let html = page(&scratch, &located);
                let headings = html.matches("<h2 ").count() + html.matches("<h2>").count();
                (located, headings)
            })
            .collect();
        let (first_path, first) = &counts[0];
        for (other_path, other) in &counts[1..] {
            assert_eq!(
                first, other,
                "{first_path} has {first} chapters and {other_path} has {other} — one language of \\
                 this document has gained or lost a chapter without the other"
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 2,
        "only {checked} documents exist in both languages; this test is not yet earning its place \\
         and should be re-read when the translations land"
    );
}

/* -------------------------------------------------------------------------- */
/* Individual pages                                                           */
/* -------------------------------------------------------------------------- */

/// The operator's manual still has its thirteen chapters.
#[test]
fn the_operators_manual_still_has_its_thirteen_chapters() {
    let scratch = built("chapters");
    let html = page(&scratch, "de/handbuch/operator");
    for chapter in 1..=13 {
        assert!(
            html.contains(&format!(">{chapter}. ")),
            "the operator's manual has lost chapter {chapter}"
        );
    }
}

/// The known-faults page lists exactly the faults that are still open.
///
/// The public page is written by hand, because a register meant for somebody
/// running a show should read like prose and not like a database dump. What it
/// must not do is fall behind the real register — so the ids it names are held
/// against `docs/ISSUES.md`, which is the working document and the truth. A
/// fault that opens and is not published here is precisely the failure this page
/// exists to prevent.
#[test]
fn the_known_faults_page_lists_the_faults_that_are_open() {
    let register = std::fs::read_to_string(root().join("docs/ISSUES.md"))
        .expect("the register is in the repository");
    // A real entry carries a number; the two `Bxx` in the file are templates,
    // and the file itself says so — they were counted as entries once already.
    let mut open: Vec<String> = Vec::new();
    let mut rest = register.as_str();
    while let Some(at) = rest.find("\n### B") {
        let from = at + 5;
        let tail = &rest[from..];
        let end = tail.find("\n### ").unwrap_or(tail.len());
        let block = &tail[..end];
        let id: String = block
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if id.len() > 1
            && id[1..].chars().all(|c| c.is_ascii_digit())
            && block.contains("\u{2610} offen")
        {
            open.push(id);
        }
        rest = &tail[end.max(1)..];
    }
    assert!(
        !open.is_empty(),
        "the register lists no open fault at all, which is more likely a parsing fault here than a \\
         perfect program"
    );
    let scratch = built("faults");
    for language in &LANGUAGES {
        let entry = PAGES
            .iter()
            .find(|entry| entry.id == "known-faults")
            .expect("the site has a known-faults page");
        let located = entry.located(language.code);
        let html = page(&scratch, &located);
        for id in &open {
            assert!(
                html.contains(id.as_str()),
                "{id} is open in docs/ISSUES.md and {located} does not mention it"
            );
        }
    }
}

/// The download page carries the checksum of the installer it was given.
#[test]
fn the_download_page_carries_the_checksum_of_the_installer_it_was_given() {
    let scratch = Scratch::new("checksum");
    let installer = scratch.0.join("PrismDMX_0.0.0_x64-setup.exe");
    std::fs::write(&installer, b"not really an installer").expect("a file");
    // sha256 of those bytes, computed independently of the code under test.
    let site = Site {
        root: root(),
        installer: Some(installer.clone()),
    };
    let out = scratch.0.join("site");
    site.build(&out).expect("the site builds");
    let html = std::fs::read_to_string(out.join("en/download/index.html")).expect("a page");
    assert!(
        html.contains("PrismDMX_0.0.0_x64-setup.exe"),
        "the download page does not name the installer it was given"
    );
    let digest = {
        use sha2::{Digest, Sha256};
        let mut hex = String::new();
        for byte in Sha256::digest(b"not really an installer") {
            hex.push_str(&format!("{byte:02x}"));
        }
        hex
    };
    assert!(
        html.contains(&digest),
        "the download page does not carry the checksum of the file it was handed"
    );
    // And the German page carries the same number, because it is the same file.
    let german = std::fs::read_to_string(out.join("de/download/index.html")).expect("a page");
    assert!(german.contains(&digest), "the German download page differs");
}

/// With no installer the download page points at the release instead.
#[test]
fn without_an_installer_the_download_page_points_at_the_release() {
    let scratch = built("no-installer");
    for located in ["en/download", "de/download"] {
        let html = page(&scratch, located);
        assert!(
            html.contains("releases"),
            "{located} neither has a checksum nor points at the release page"
        );
    }
}

/// The front page is an index of the documentation and says where to start.
#[test]
fn the_front_page_is_an_index_of_the_documentation() {
    let scratch = built("front");
    for (located, expected) in [
        ("en", ["Operator's manual", "Installer's manual"]),
        (
            "de",
            ["Handbuch für den Operator", "Handbuch für den Installateur"],
        ),
    ] {
        let html = page(&scratch, located);
        for sentence in expected {
            assert!(
                html.contains(sentence),
                "{located} no longer points at \u{201c}{sentence}\u{201d}"
            );
        }
        // And the download page, which is step one of the four.
        let goes_to_download = hrefs(&html).into_iter().any(|href| {
            resolve(located, &href).is_some_and(|(target, _)| target.ends_with("download"))
        });
        assert!(
            goes_to_download,
            "{located} does not link to the download page"
        );
    }
}

/// The domain is written by the generator, so it is one constant in one place.
#[test]
fn the_domain_is_written_beside_the_pages() {
    let scratch = built("cname");
    let cname = std::fs::read_to_string(scratch.0.join("CNAME")).expect("CNAME is written");
    assert_eq!(cname.trim(), DOMAIN);
    assert_eq!(cname.trim(), "docs.prismdmx.de");
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
