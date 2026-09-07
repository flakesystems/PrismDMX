//! **docs.prismdmx.de, generated out of this repository** — S42.
//!
//! # Why the site is built here rather than anywhere else
//!
//! S42's first exit criterion is that the site builds *from this repository*, so
//! that what a stranger reads cannot drift from what they download. The way to
//! guarantee that is not discipline: it is for the manuals to be the **source**
//! of the pages rather than the origin of a copy. So there is no content in this
//! crate. Every page is one of the Markdown files this repository already keeps
//! beside the code, read at run time, rendered, and wrapped in a shell that says
//! which version it is describing.
//!
//! The consequence worth knowing: **a manual edited without the site rebuilt is
//! not a site that is wrong, it is a site that is not built yet.** There is no
//! state anywhere in between.
//!
//! # The four decisions
//!
//! - **The generator is Rust and a workspace member**, not a static-site
//!   framework. It gets `cargo fmt`, `cargo clippy`, the ARM64 cross-check and
//!   the README rule for free, adds no toolchain a contributor has to install,
//!   and takes the version from `[workspace.package]` — so *the page says which
//!   version it documents* is a fact about the build rather than a note somebody
//!   remembers to update.
//! - **No JavaScript**, and not as a preference: a documentation page that needs
//!   a script to be read is one that cannot be read on the machine in the
//!   lighting rack. There is none on any page. The style sheet is one file, and
//!   it is inlined so a page is one request.
//! - **The checksum is computed, never transcribed.** [`Site::installer`] takes
//!   the path of an installer and hashes it; `release.yml` hands it the `.exe`
//!   it has just built. Without one, the download page links to the release
//!   rather than printing a number nobody derived.
//! - **The language of a page is the language of its source.** The two manuals
//!   for the people in the building are German, the developer's manual is
//!   English, and each page carries the right `lang` attribute rather than one
//!   for the whole site. `docs/manual/README.md` has that decision in full.
//!
//! # Links
//!
//! A manual links to its neighbours as `installer.md`, and to the specification
//! as `../../ARCHITECTURE_SPEC.md`. [`rewrite_links`] turns the first into the
//! page it became and the second into a link to GitHub, so a relative link that
//! is right in the repository is right on the site as well and neither document
//! has to know it is being published.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use pulldown_cmark::{Options, Parser, html};
use sha2::{Digest, Sha256};

/// The version every page says it documents.
///
/// `[workspace.package] version`, inherited, which is the same number
/// `prismd --version` prints and the same one the installer carries —
/// `crates/prism-app/tests/version.rs` is what holds those together.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the releases are.
pub const RELEASES: &str = "https://github.com/flakesystems/PrismDMX/releases";

/// The repository, for links a page cannot serve itself.
pub const REPOSITORY: &str = "https://github.com/flakesystems/PrismDMX";

/// The domain this site is served from.
///
/// **A subdomain rather than the apex**, because the apex belongs to the
/// promotion site, which is a different job on different hosting: this one is
/// generated out of the repository and follows the code, that one is designed
/// and follows a release. Keeping them apart means neither has to be a
/// compromise, and a subdomain is also the easier DNS — a plain `CNAME` record,
/// where an apex needs `ALIAS`, `ANAME` or Pages' four `A` records.
pub const DOMAIN: &str = "docs.prismdmx.de";

/// The style sheet, inlined into every page.
const STYLE: &str = include_str!("../assets/site.css");

/* -------------------------------------------------------------------------- */
/* What a page is                                                             */
/* -------------------------------------------------------------------------- */

/// A language the site is published in.
pub struct Language {
    /// The ISO code, which is both the URL prefix and the `lang` attribute.
    pub code: &'static str,
    /// What the switcher calls it — in that language, because the person who
    /// wants it cannot necessarily read the other one.
    pub name: &'static str,
    /// The line a page shows when its text is not in this language yet.
    pub borrowed_not_yet: &'static str,
    /// What the switcher's link is labelled for screen readers.
    pub switch_label: &'static str,
}

/// The two languages, **English first because it is the default**.
///
/// The order is the order of the switcher and of `hreflang`, and the first entry
/// is what the site's root redirects to.
pub const LANGUAGES: [Language; 2] = [
    Language {
        code: "en",
        name: "English",
        borrowed_not_yet: "This page is not translated yet — you are reading the German original.",
        switch_label: "Read this page in English",
    },
    Language {
        code: "de",
        name: "Deutsch",
        borrowed_not_yet: "Diese Seite ist noch nicht übersetzt — Sie lesen das englische Original.",
        switch_label: "Diese Seite auf Deutsch lesen",
    },
];

/// Why a page's text is not in the language of the page.
///
/// One case only, deliberately: **everything this site publishes is meant to
/// exist in both languages.** The documents that are English on purpose — the
/// architecture, the IPC protocol, the X-Touch mapping, the DMX merge — are
/// internal to the repository and are not published here at all, so the site
/// never has to explain a language it chose to keep. What is left is a gap that
/// is going to be filled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Borrowed {
    /// No translation exists **yet**.
    ///
    /// The page is still published, still in the navigation and still says which
    /// language it is actually in — because a reader who hits a missing page
    /// learns nothing, and a reader who silently gets the other language learns
    /// something false. A test counts these, so the number is visible and can
    /// only be argued down.
    NotYet,
}

/// Where one language's version of a page gets its text.
#[derive(Clone, Copy)]
pub enum Source {
    /// Rendered from this Markdown file, which is written in this language.
    Own(&'static str),
    /// Composed by this crate, in this language.
    Composed,
    /// Written in another language, for the reason given.
    Borrowed {
        /// The file to render.
        file: &'static str,
        /// The language it is actually in.
        lang: &'static str,
        /// Why.
        why: Borrowed,
    },
}

/// One language's version of a page.
pub struct Variant {
    /// The path **under the language prefix**, without `index.html`. The empty
    /// string is that language's index page.
    pub path: &'static str,
    /// What the tab and the navigation call it.
    pub title: &'static str,
    /// Where the text comes from.
    pub source: Source,
}

/// One page of the site, in every language it is published in.
///
/// A page is one thing with two texts rather than two pages that happen to be
/// about the same subject: that is what lets the switcher say *this page, in the
/// other language* instead of dropping the reader on a front page, and what lets
/// a test hold the two versions against each other.
pub struct Page {
    /// A stable name for the page, independent of language and of URL.
    ///
    /// Nothing outside the crate is built from it and no reader ever sees it; it
    /// exists so that a German path can be renamed without a test losing track
    /// of which English page it is the counterpart of.
    pub id: &'static str,
    /// The English version.
    pub en: Variant,
    /// The German version.
    pub de: Variant,
}

impl Page {
    /// This page in one language.
    ///
    /// # Panics
    ///
    /// On a language code the site does not publish, which is a programming
    /// error rather than input: `LANGUAGES` is the only source of codes.
    #[must_use]
    pub fn variant(&self, lang: &str) -> &Variant {
        match lang {
            "en" => &self.en,
            "de" => &self.de,
            other => panic!("the site does not publish {other}"),
        }
    }

    /// This page's full path in one language, language prefix included.
    #[must_use]
    pub fn located(&self, lang: &str) -> String {
        let path = self.variant(lang).path;
        if path.is_empty() {
            lang.to_owned()
        } else {
            format!("{lang}/{path}")
        }
    }
}

/// The navigation, which is the same on every page and is the site's own map.
///
/// Ordered the way somebody arrives: what this is, then how to get it, then what
/// to do with it, then what changed, then the references and what is broken.
///
/// **The paths differ per language on purpose.** `/de/handbuch/operator/` and
/// `/en/manual/operator/` are the same page, and a German reader should not have
/// to read an English noun to find it. The `id` is what ties them together, so
/// the switcher and the tests never have to guess.
pub const PAGES: [Page; 8] = [
    Page {
        id: "index",
        en: Variant {
            path: "",
            title: "Documentation",
            source: Source::Composed,
        },
        de: Variant {
            path: "",
            title: "Dokumentation",
            source: Source::Composed,
        },
    },
    Page {
        id: "download",
        en: Variant {
            path: "download",
            title: "Download",
            source: Source::Composed,
        },
        de: Variant {
            path: "download",
            title: "Herunterladen",
            source: Source::Composed,
        },
    },
    Page {
        id: "manual-operator",
        en: Variant {
            path: "manual/operator",
            title: "Operator's manual",
            source: Source::Borrowed {
                file: "docs/manual/operator.de.md",
                lang: "de",
                why: Borrowed::NotYet,
            },
        },
        de: Variant {
            path: "handbuch/operator",
            title: "Handbuch für den Operator",
            source: Source::Own("docs/manual/operator.de.md"),
        },
    },
    Page {
        id: "manual-installer",
        en: Variant {
            path: "manual/installer",
            title: "Installer's manual",
            source: Source::Borrowed {
                file: "docs/manual/installer.de.md",
                lang: "de",
                why: Borrowed::NotYet,
            },
        },
        de: Variant {
            path: "handbuch/installateur",
            title: "Handbuch für den Installateur",
            source: Source::Own("docs/manual/installer.de.md"),
        },
    },
    Page {
        id: "manual-developer",
        en: Variant {
            path: "manual/developer",
            title: "Developer's manual",
            source: Source::Own("docs/manual/developer.en.md"),
        },
        de: Variant {
            path: "handbuch/entwickler",
            title: "Handbuch für Entwickler",
            source: Source::Borrowed {
                file: "docs/manual/developer.en.md",
                lang: "en",
                why: Borrowed::NotYet,
            },
        },
    },
    Page {
        id: "command-line",
        en: Variant {
            path: "command-line",
            title: "The command line",
            source: Source::Own("docs/site/command-line.en.md"),
        },
        de: Variant {
            path: "kommandozeile",
            title: "Die Kommandozeile",
            source: Source::Own("docs/site/command-line.de.md"),
        },
    },
    Page {
        id: "changelog",
        en: Variant {
            path: "changelog",
            title: "Changelog",
            source: Source::Own("CHANGELOG.md"),
        },
        de: Variant {
            path: "aenderungen",
            title: "Änderungen",
            source: Source::Borrowed {
                file: "CHANGELOG.md",
                lang: "en",
                why: Borrowed::NotYet,
            },
        },
    },
    Page {
        id: "known-faults",
        en: Variant {
            path: "known-faults",
            title: "Known faults",
            source: Source::Own("docs/site/known-faults.en.md"),
        },
        de: Variant {
            path: "fehler",
            title: "Bekannte Fehler",
            source: Source::Own("docs/site/known-faults.de.md"),
        },
    },
];

/* -------------------------------------------------------------------------- */
/* The command line                                                           */
/* -------------------------------------------------------------------------- */

/// What the binary was asked to do.
///
/// It lives here rather than in `main.rs` for `prismd`'s own reason, written
/// down in that crate's documentation: **a binary target has no tests.** Reading
/// three flags is not much to get wrong, and that is exactly the shape of thing
/// that is wrong for a year — `--installer` taking the *next* argument rather
/// than its own is a release page with somebody else's checksum on it.
#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    /// Print the usage and stop.
    Help,
    /// Render the site.
    Render {
        /// The repository to read the documents from.
        root: PathBuf,
        /// Where to write the site.
        out: PathBuf,
        /// The installer to hash for the download page, if one was named.
        installer: Option<PathBuf>,
    },
}

/// Reads the command line, given the root to fall back on.
///
/// `default_root` is the binary's own compile-time location, which a test
/// supplies for itself — so this function has no idea where it is running and
/// can be asked the same question twice with two different answers.
///
/// # Errors
///
/// A word it does not know, or a flag with no value after it. Both come back as
/// a sentence rather than a code, because the only caller prints it.
pub fn parse_arguments<I>(arguments: I, default_root: &Path) -> Result<Invocation, String>
where
    I: IntoIterator<Item = String>,
{
    let mut root = default_root.to_path_buf();
    let mut out: Option<PathBuf> = None;
    let mut installer: Option<PathBuf> = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{argument} needs a value"))
        };
        match argument.as_str() {
            "--out" => out = Some(value()?),
            "--root" => root = value()?,
            "--installer" => installer = Some(value()?),
            "-h" | "--help" => return Ok(Invocation::Help),
            other => return Err(format!("prism-web does not know {other}")),
        }
    }
    let out = out.unwrap_or_else(|| root.join("web/dist"));
    Ok(Invocation::Render {
        root,
        out,
        installer,
    })
}

/// What the binary prints when it is asked.
#[must_use]
pub fn usage() -> String {
    format!(
        "prism-web — renders {DOMAIN} out of this repository.\n\n\
         Usage: prism-web [OPTIONS]\n\n\
         Options:\n\
         \x20 --out <DIR>        where to write the site (default: web/dist)\n\
         \x20 --root <DIR>       the repository to read the documents from\n\
         \x20 --installer <PATH> hash this file for the download page's checksum\n\
         \x20 -h, --help         print this and stop\n\n\
         {} pages in {} languages — {} files — every one of them a document this\n\
         repository already keeps beside the code.",
        PAGES.len(),
        LANGUAGES.len(),
        PAGES.len() * LANGUAGES.len()
    )
}

/* -------------------------------------------------------------------------- */
/* The site                                                                   */
/* -------------------------------------------------------------------------- */

/// What a build of the site needs to know.
pub struct Site {
    /// The repository root, which is where every source is read from.
    pub root: PathBuf,
    /// The installer this build should print the checksum of, if there is one.
    ///
    /// `release.yml` passes the `.exe` it has just built. Nothing else does, so
    /// an ordinary CI build produces a download page that links to the release
    /// instead of printing a number it did not derive.
    pub installer: Option<PathBuf>,
}

/// What went wrong, in a sentence somebody can act on.
#[derive(Debug)]
pub struct BuildError(pub String);

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BuildError {}

/// The name and checksum of a built installer.
pub struct Installer {
    /// The file name, which is what a person sees in their downloads.
    pub name: String,
    /// Its SHA-256, lower case, **computed from the file**.
    pub sha256: String,
}

impl Site {
    /// Renders every page into `out`, and answers how many were written.
    ///
    /// # Errors
    ///
    /// If a source is missing or the output cannot be written. A missing source
    /// is a hard error rather than a skipped page: a site with a hole in its
    /// navigation is worse than a build that stopped.
    pub fn build(&self, out: &Path) -> Result<usize, BuildError> {
        let installer = self.installer()?;
        let mut written = 0_usize;
        for language in &LANGUAGES {
            for page in &PAGES {
                let variant = page.variant(language.code);
                let located = page.located(language.code);
                let body = match variant.source {
                    Source::Own(file) => self.rendered(file, language.code)?,
                    Source::Borrowed { file, lang, why } => {
                        // The banner opens a `<div lang=…>` around the text it
                        // is warning about, and this closes it: the language of
                        // the document is a property of the text, not of the
                        // sentence above it.
                        let notice = banner(language, lang, why);
                        format!("{notice}{}</div>\n", self.rendered(file, language.code)?)
                    }
                    Source::Composed => self.composed(page, language, installer.as_ref()),
                };
                // Relativised last, over the finished document, so that the
                // navigation, the composed pages and the link rewriter can all
                // speak in the site's own absolute paths and exactly one
                // function knows where the site is actually mounted.
                let html = relativised(&wrap(page, language, &body), &located);
                let directory = out.join(&located);
                std::fs::create_dir_all(&directory)
                    .map_err(|error| BuildError(format!("{}: {error}", directory.display())))?;
                let file = directory.join("index.html");
                std::fs::write(&file, html)
                    .map_err(|error| BuildError(format!("{}: {error}", file.display())))?;
                written += 1;
            }
        }
        // The root, which belongs to no language and therefore has to choose
        // one. GitHub Pages cannot redirect, so this is the only thing it can
        // be: a page that sends the reader on. No JavaScript — a `meta refresh`
        // and a link that works when even that is refused.
        std::fs::write(out.join("index.html"), relativised(&root_redirect(), ""))
            .map_err(|error| BuildError(format!("index.html: {error}")))?;
        // Pages serves the domain from this file, and it is written rather than
        // committed so that the domain is one constant in one place.
        std::fs::write(out.join("CNAME"), format!("{DOMAIN}\n"))
            .map_err(|error| BuildError(format!("CNAME: {error}")))?;
        // Nothing here is a search engine's business until the domain is live,
        // and a site that says so is better than one that is quietly indexed
        // half-built. Removed by hand when the domain is answering.
        std::fs::write(out.join(".nojekyll"), "")
            .map_err(|error| BuildError(format!(".nojekyll: {error}")))?;
        Ok(written)
    }

    /// The installer's name and checksum, computed from the file.
    ///
    /// # Errors
    ///
    /// If the path was given and cannot be read.
    pub fn installer(&self) -> Result<Option<Installer>, BuildError> {
        let Some(path) = self.installer.as_ref() else {
            return Ok(None);
        };
        let bytes = std::fs::read(path)
            .map_err(|error| BuildError(format!("{}: {error}", path.display())))?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("PrismDMX-setup.exe")
            .to_owned();
        let digest = Sha256::digest(&bytes);
        let mut sha256 = String::with_capacity(64);
        for byte in digest {
            let _ = write!(sha256, "{byte:02x}");
        }
        Ok(Some(Installer { name, sha256 }))
    }

    /// One Markdown source, rendered, with its links rewritten.
    fn rendered(&self, source: &str, lang: &str) -> Result<String, BuildError> {
        let path = self.root.join(source);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            BuildError(format!(
                "{} could not be read ({error}) — the site is generated from this repository's \
                 own documents, so a missing one is a missing page",
                path.display()
            ))
        })?;
        Ok(rewrite_links(&anchored(&to_html(&text)), source, lang))
    }

    /// A page this crate composes rather than renders.
    fn composed(&self, page: &Page, language: &Language, installer: Option<&Installer>) -> String {
        let markdown = match page.id {
            "download" => download_page(language, installer),
            _ => front_page(language),
        };
        // Anchored like a rendered page, but **not** link-rewritten: these two
        // write the site's own paths already, and putting them through the
        // rewriter would ask it to resolve `/download/` as a file.
        anchored(&markdown)
    }
}

/* -------------------------------------------------------------------------- */
/* Markdown                                                                   */
/* -------------------------------------------------------------------------- */

/// Markdown to HTML, with the extensions this repository's documents use.
///
/// Tables above all: every one of these documents is half table, and a
/// specification rendered as pipe characters is a specification nobody reads.
#[must_use]
pub fn to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    let mut out = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut out, Parser::new_ext(markdown, options));
    out
}

/// Gives every heading an `id`, so a document's own table of contents works.
///
/// Markdown has no anchors of its own: `[Kapitel 5](#5-ein-rig-patchen)` is a
/// link to an `id` that the *renderer* is expected to have invented, and
/// `pulldown-cmark` deliberately invents none. Without this every table of
/// contents in every one of these documents is a list of dead links — which is
/// the failure a site like this has most often and notices least, because
/// nothing errors and the page looks finished.
///
/// The rule is GitHub's, because that is the rule the documents were written
/// against and the one they are read under in the repository: the heading's
/// text, lower-cased, with everything that is not a letter, a digit, a space or
/// a hyphen removed, and spaces turned into hyphens. Non-ASCII letters are kept
/// — `#6-auswählen-und-programmieren` is a real anchor — and a repeated slug
/// gets a number, the way a repeated heading does there.
#[must_use]
pub fn anchored(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 512);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut rest = html;
    while let Some(at) = rest.find("<h") {
        let after = &rest[at + 2..];
        let level = after.as_bytes().first().copied().unwrap_or(b' ');
        if !(b'1'..=b'6').contains(&level) || after.as_bytes().get(1) != Some(&b'>') {
            // Not a bare `<hN>` — either not a heading at all, or one that
            // already carries attributes, which the author asked for.
            let (before, remainder) = rest.split_at(at + 2);
            out.push_str(before);
            rest = remainder;
            continue;
        }
        let open_end = at + 4;
        let close = format!("</h{}>", level as char);
        let Some(body_end) = rest[open_end..].find(&close) else {
            break;
        };
        let body = &rest[open_end..open_end + body_end];
        let mut slug = slug(&strip_tags(body));
        let count = seen.entry(slug.clone()).or_insert(0);
        if *count > 0 {
            let _ = write!(slug, "-{count}");
        }
        *count += 1;
        out.push_str(&rest[..at]);
        let _ = write!(out, "<h{} id=\"{slug}\">", level as char);
        out.push_str(body);
        rest = &rest[open_end + body_end..];
    }
    out.push_str(rest);
    out
}

/// A heading's text, without the markup inside it.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut inside = false;
    for character in html.chars() {
        match character {
            '<' => inside = true,
            '>' => inside = false,
            other if !inside => out.push(other),
            _ => {}
        }
    }
    out
}

/// GitHub's own anchor rule, so the anchors a document already carries work.
#[must_use]
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.trim().to_lowercase().chars() {
        if character.is_alphanumeric() {
            out.push(character);
        } else if character == ' ' || character == '-' {
            out.push('-');
        }
    }
    out
}

/// Turns the links a document has in the repository into links the site has.
///
/// Three cases, and the third is why this exists at all:
///
/// 1. A link to a document the site publishes becomes that page.
/// 2. A link to a document it does not becomes a link to GitHub, so the reader
///    lands on the real file rather than on a 404.
/// 3. Anything else — an anchor, an absolute URL, a mail address — is left
///    exactly as it was.
///
/// `from` is the source's path in the repository, because a relative link is
/// relative to the document that carries it.
#[must_use]
pub fn rewrite_links(html: &str, from: &str, lang: &str) -> String {
    // Keyed by source file, valued with **this language's** page. A file can
    // serve two languages — the references are English text under a German path
    // as well as an English one — so the map has to be built per language or a
    // German manual would link its reader out of German.
    let published: BTreeMap<&str, String> = PAGES
        .iter()
        .filter_map(|page| {
            let variant = page.variant(lang);
            let file = match variant.source {
                Source::Own(file) | Source::Borrowed { file, .. } => file,
                Source::Composed => return None,
            };
            Some((file, absolute(&page.located(lang))))
        })
        .collect();
    let base = Path::new(from).parent().unwrap_or(Path::new(""));
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find("href=\"") {
        let (before, after) = rest.split_at(at + 6);
        out.push_str(before);
        let Some(end) = after.find('"') else {
            out.push_str(after);
            return out;
        };
        let (target, remainder) = after.split_at(end);
        out.push_str(&rewritten(target, base, &published));
        rest = remainder;
    }
    out.push_str(rest);
    out
}

/// One link target, resolved.
fn rewritten(target: &str, base: &Path, published: &BTreeMap<&str, String>) -> String {
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with('/')
        || target.contains("://")
        || target.starts_with("mailto:")
    {
        return target.to_owned();
    }
    let (path, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, format!("#{anchor}")),
        None => (target, String::new()),
    };
    let resolved = normalise(&base.join(path));
    if let Some(page) = published.get(resolved.as_str()) {
        return format!("{page}{anchor}");
    }
    format!("{REPOSITORY}/blob/master/{resolved}{anchor}")
}

/// A path with `..` and `.` taken out, as a forward-slash string.
fn normalise(path: &Path) -> String {
    let text = path.to_string_lossy();
    let mut parts: Vec<&str> = Vec::new();
    for part in text.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// A page's path as an absolute one, with the trailing slash a directory index
/// wants.
///
/// This is the site's **internal** form and not what is written to disk: every
/// URL is built this way and then made relative by [`relativised`], which is the
/// last thing that happens to a page. Keeping one canonical form in the middle
/// means the navigation, the composed pages and the link rewriter all say
/// `/download/` and exactly one function knows what that turns into.
#[must_use]
pub fn absolute(path: &str) -> String {
    if path.is_empty() {
        "/".to_owned()
    } else {
        format!("/{path}/")
    }
}

/// How many directories deep a page sits.
fn depth(path: &str) -> usize {
    if path.is_empty() {
        0
    } else {
        path.split('/').count()
    }
}

/// Every site-absolute URL in a page, made relative to that page.
///
/// **This is what makes the site work wherever it is put, and it exists because
/// the first deployment did not.** GitHub Pages served the site as a *project*
/// page — `flakesystems.github.io/PrismDMX/` — where the whole site sits under a
/// path prefix. Every `href="/handbuch/operator/"` then points a level above the
/// project, at somebody else's repository, and every link on every page is dead
/// while the page itself still looks completely finished. Nothing errors: the
/// styling is inline, so the site renders perfectly and simply goes nowhere.
///
/// The fix is deliberately **not** a `--base` flag. A base path is a setting
/// that has to match the deployment, and the two get separated the first time
/// the site moves — which for this project is a certainty, because the domain is
/// coming and a self-hosted server is being kept in reserve. Relative URLs have
/// nothing to match: the same output is correct at `/PrismDMX/`, at the apex of
/// `docs.prismdmx.de`, in a subdirectory of any web server, and from a `file://` URL
/// on a machine with no network at all — which is the one that matters for a
/// venue reading a manual off a stick.
///
/// **The one thing this depends on** is that a directory URL keeps its trailing
/// slash: at `/handbuch/operator` without it a browser resolves `../../` one
/// level too high. Every static server redirects the slashless form to the
/// slashed one — Pages does, nginx does — which is why relative links are how
/// portable static sites are built. It is a dependency rather than an assumption,
/// so it is written down.
#[must_use]
pub fn relativised(html: &str, here: &str) -> String {
    let up = "../".repeat(depth(here));
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find("href=\"/") {
        let (before, after) = rest.split_at(at + 6);
        out.push_str(before);
        let Some(end) = after.find('"') else {
            out.push_str(after);
            return out;
        };
        let (target, remainder) = after.split_at(end);
        let relative = format!("{up}{}", &target[1..]);
        // The root, seen from the root: a browser needs something to resolve.
        out.push_str(if relative.is_empty() { "./" } else { &relative });
        rest = remainder;
    }
    out.push_str(rest);
    out
}

/* -------------------------------------------------------------------------- */
/* The shell every page wears                                                 */
/* -------------------------------------------------------------------------- */

/// Escapes the four characters that would otherwise be markup.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Wraps a rendered body in the navigation, the header and the footer.
///
/// **The version is in the header of every page**, which is S42's exit
/// criterion, and it is `VERSION` rather than a string in a template — so it
/// moves when the workspace's does and cannot be forgotten.
#[must_use]
pub fn wrap(page: &Page, language: &Language, body: &str) -> String {
    let variant = page.variant(language.code);
    let located = page.located(language.code);
    let mut out = String::with_capacity(body.len() + STYLE.len() + 4096);
    let title = if variant.path.is_empty() {
        match language.code {
            "de" => "PrismDMX — Dokumentation".to_owned(),
            _ => "PrismDMX — Documentation".to_owned(),
        }
    } else {
        format!("{} — PrismDMX", variant.title)
    };
    let _ = write!(
        out,
        "<!DOCTYPE html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n<meta \
         name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n",
        language.code,
        escape(&title)
    );
    // Every language of this page, named to a search engine — so the reader who
    // arrives from a search arrives in their own language rather than in the
    // one that happened to be indexed. `x-default` is the site's default.
    for other in &LANGUAGES {
        let _ = writeln!(
            out,
            "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">",
            other.code,
            absolute(&page.located(other.code))
        );
    }
    let _ = writeln!(
        out,
        "<link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\">",
        absolute(&page.located(LANGUAGES[0].code))
    );
    let _ = write!(out, "<style>\n{STYLE}</style>\n</head>\n<body>\n");
    let _ = write!(
        out,
        "<header class=\"top\">\n<a class=\"wordmark\" href=\"{}\">PrismDMX</a>\n",
        absolute(language.code)
    );
    let _ = writeln!(
        out,
        "<p class=\"version\">{}</p>",
        match language.code {
            "de" => format!("Diese Seite beschreibt Version <strong>{VERSION}</strong>"),
            _ => format!("This page documents version <strong>{VERSION}</strong>"),
        }
    );
    // The switcher, and it points at **this page** in the other language rather
    // than at that language's front page. Landing a reader on a front page
    // because they wanted to read the same thing in their own language is the
    // commonest way a language switcher is useless.
    out.push_str("<ul class=\"languages\">\n");
    for other in &LANGUAGES {
        let current = other.code == language.code;
        let mark = if current {
            " class=\"here\" aria-current=\"true\""
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "<li><a href=\"{}\" hreflang=\"{}\" lang=\"{}\" title=\"{}\"{mark}>{}</a></li>",
            absolute(&page.located(other.code)),
            other.code,
            other.code,
            escape(other.switch_label),
            escape(other.name)
        );
    }
    out.push_str("</ul>\n");
    let _ = writeln!(
        out,
        "</header>\n<nav aria-label=\"{}\"><ul>",
        match language.code {
            "de" => "Seiten",
            _ => "Pages",
        }
    );
    for entry in &PAGES {
        let here = if entry.id == page.id {
            " class=\"here\" aria-current=\"page\""
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "<li><a href=\"{}\"{here}>{}</a></li>",
            absolute(&entry.located(language.code)),
            escape(entry.variant(language.code).title)
        );
    }
    out.push_str("</ul></nav>\n<main>\n");
    out.push_str(body);
    out.push_str("</main>\n<footer>\n");
    let _ = write!(
        out,
        "<p>PrismDMX {VERSION} · <a href=\"{REPOSITORY}\">{}</a> · <a \
         href=\"{REPOSITORY}/blob/master/LICENSE\">MIT</a></p>\n<p>{}</p>\n",
        match language.code {
            "de" => "Quelltext auf GitHub",
            _ => "Source on GitHub",
        },
        match language.code {
            "de" =>
                "Diese Seite wird aus dem Repository erzeugt: was hier steht, steht dort. Kein \
                 JavaScript.",
            _ =>
                "This site is generated out of the repository: what it says is what is there. No \
                 JavaScript.",
        }
    );
    let _ = writeln!(out, "<!-- {located} -->");
    out.push_str("</footer>\n</body>\n</html>\n");
    out
}

/// The line a page shows when its text is not in the page's own language.
///
/// It is the **first** thing in the body, before the document's own title,
/// because a reader who is going to hit a language they did not ask for should
/// find that out before they start reading rather than three paragraphs in.
#[must_use]
pub fn banner(page_language: &Language, text_language: &str, why: Borrowed) -> String {
    let Borrowed::NotYet = why;
    let sentence = page_language.borrowed_not_yet;
    format!("<p class=\"borrowed\" lang=\"{}\">{}</p>\n", page_language.code, escape(sentence))
        // The text that follows is in another language, and saying so is what
        // lets a screen reader change voice rather than read German with an
        // English one.
        + &format!("<div lang=\"{text_language}\">\n")
}

/// The page at the site's root, which belongs to no language.
///
/// GitHub Pages cannot redirect, so the root has to be a document that sends the
/// reader on. It uses `meta refresh` rather than JavaScript, and it carries a
/// real link as well, because a `meta refresh` is something a browser is allowed
/// to refuse. Both languages are named on it, so a reader who lands here and
/// wants the other one does not have to follow the default first.
#[must_use]
pub fn root_redirect() -> String {
    let default = LANGUAGES[0].code;
    let mut out = String::with_capacity(1024);
    let _ = write!(
        out,
        "<!DOCTYPE html>\n<html lang=\"{default}\">\n<head>\n<meta charset=\"utf-8\">\n<meta \
         http-equiv=\"refresh\" content=\"0; url={default}/\">\n<link rel=\"canonical\" \
         href=\"{}\">\n<title>PrismDMX — Documentation</title>\n",
        absolute(default)
    );
    for other in &LANGUAGES {
        let _ = writeln!(
            out,
            "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">",
            other.code,
            absolute(other.code)
        );
    }
    let _ = write!(out, "<style>\n{STYLE}</style>\n</head>\n<body>\n<main>\n");
    out.push_str("<h1>PrismDMX</h1>\n<ul>\n");
    for other in &LANGUAGES {
        let _ = writeln!(
            out,
            "<li><a href=\"{}\" hreflang=\"{}\" lang=\"{}\">{}</a></li>",
            absolute(other.code),
            other.code,
            other.code,
            escape(other.name)
        );
    }
    out.push_str("</ul>\n</main>\n</body>\n</html>\n");
    out
}

/* -------------------------------------------------------------------------- */
/* The two composed pages                                                     */
/* -------------------------------------------------------------------------- */

/// The front page, which is an **index of the documentation** and nothing else.
///
/// It used to be a landing page — what PrismDMX is, what it can do, what it
/// cannot. That job now belongs to the promotion site on its own hosting, and
/// two pages telling the same story is two stories that diverge. So this one
/// answers only the question a person on a documentation site actually has:
/// *which of these documents is mine, and where do I start.*
///
/// The four steps survive as **one line with a link**, because the path from
/// nothing to a running desk is the thing this whole site exists to make work
/// and it belongs where a reader looking for it will look.
fn front_page(language: &Language) -> String {
    let markdown = match language.code {
        "de" => format!(
            r#"# Dokumentation

Alles, was zu PrismDMX **{VERSION}** geschrieben ist, erzeugt aus dem Repository:
was hier steht, steht dort.

## Wo Sie anfangen

| Sie sind | Lesen Sie |
|---|---|
| Jemand, der eine Show baut und fährt | [Handbuch für den Operator](/de/handbuch/operator/) |
| Jemand, der das Haus verkabelt und das Pult einrichtet | [Handbuch für den Installateur](/de/handbuch/installateur/) |
| Jemand, der den Quelltext ändert | [Handbuch für Entwickler](/de/handbuch/entwickler/) |

**Zum ersten Mal hier?** Der kürzeste Weg von nichts zu laufendem Licht sind vier
Schritte: [herunterladen und installieren](/de/download/), sagen, womit das Haus
verkabelt ist (*Settings → Outputs*, siehe
[Kapitel 4 des Installateur-Handbuchs](/de/handbuch/installateur/#4-die-ausgänge)),
[ein Fixture patchen](/de/handbuch/operator/#5-ein-rig-patchen), und `1 at full`
in die Kommandozeile tippen. Kommt Licht, läuft das Pult.

Wenn Sie unterwegs hängen bleiben, ist das ein Fehler in dieser Dokumentation und
keiner bei Ihnen: [schreiben Sie ihn auf]({REPOSITORY}/issues).

## Was es sonst noch gibt

- [Die Kommandozeile](/de/kommandozeile/) — alle neunundzwanzig Wörter, die das
  Pult versteht, mit Beispielen.
- [Herunterladen](/de/download/) — der Installer, was er tut und wohin er
  schreibt.
- [Änderungen](/de/aenderungen/) — was sich zwischen den Versionen geändert hat.
- [Bekannte Fehler](/de/fehler/) — was offen ist, bevor Sie es entdecken.

Die Spezifikationen des Projekts — Architektur, IPC-Protokoll, X-Touch-Mapping
und DMX-Merge — stehen bewusst nicht hier: das sind Arbeitsdokumente, die sich
mit dem Quelltext ändern, und sie liegen im
[Repository]({REPOSITORY}) neben dem Code, zu dem sie gehören.

## Zwei Sprachen

Diese Dokumentation gibt es auf Englisch und auf Deutsch; oben rechts wird
umgeschaltet, und die Umschaltung führt auf **dieselbe Seite** in der anderen
Sprache. Englisch ist die Standardsprache. Seiten, die noch nicht übersetzt sind,
sagen das oben auf der Seite, statt Sie stillschweigend in der anderen Sprache
lesen zu lassen.
"#
        ),
        _ => format!(
            r#"# Documentation

Everything written about PrismDMX **{VERSION}**, generated out of the repository:
what it says here is what is there.

## Where to start

| You are | Read |
|---|---|
| Someone building and running a show | [Operator's manual](/en/manual/operator/) |
| Someone wiring the building and setting the desk up | [Installer's manual](/en/manual/installer/) |
| Someone changing the source | [Developer's manual](/en/manual/developer/) |

**First time here?** The shortest path from nothing to light on a stage is four
steps: [download and install](/en/download/), tell it how the building is wired
(*Settings → Outputs*, see
[chapter 4 of the installer's manual](/en/manual/installer/#4-die-ausgänge)),
[patch a fixture](/en/manual/operator/#5-ein-rig-patchen), and type `1 at full`
into the command line. If light comes on, the desk is running.

If you get stuck anywhere along it, that is a fault in this documentation and not
in you: [write it down]({REPOSITORY}/issues).

## What else is here

- [The command line](/en/command-line/) — all twenty-nine words the desk
  understands, with examples.
- [Download](/en/download/) — the installer, what it does and where it writes.
- [Changelog](/en/changelog/) — what changed between versions.
- [Known faults](/en/known-faults/) — what is open, before you find it.

The project's specifications — the architecture, the IPC protocol, the X-Touch
mapping and the DMX merge — are deliberately not here: they are working documents
that change with the source, and they live in the
[repository]({REPOSITORY}) next to the code they describe.

## Two languages

This documentation is published in English and German; the switch is at the top
right, and it lands you on **the same page** in the other language. English is
the default. A page that is not translated yet says so at the top rather than
quietly handing you the other language.
"#
        ),
    };
    to_html(&markdown)
}

/// The download page.
///
/// The checksum is printed **only** when this build was handed the installer to
/// hash. A page that carried a transcribed number would be one more place for a
/// number to be wrong, and the number is the one thing on this page that has to
/// be right.
fn download_page(language: &Language, installer: Option<&Installer>) -> String {
    let markdown = match language.code {
        "de" => {
            let checksum = match installer {
                Some(installer) => format!(
                    r#"## Die Prüfsumme

Der Build ist **nicht signiert**, deshalb ist die Prüfsumme das, was Sie statt
einer Signatur prüfen können. Sie wird von demselben Lauf gebildet, der die Datei
erzeugt hat; niemand tippt sie ab.

| Datei | SHA-256 |
|---|---|
| `{}` | `{}` |

```powershell
Get-FileHash .\{} -Algorithm SHA256
```
"#,
                    installer.name, installer.sha256, installer.name
                ),
                None => format!(
                    r#"## Die Prüfsumme

Der Build ist **nicht signiert**, deshalb ist die Prüfsumme das, was Sie statt
einer Signatur prüfen können. Sie steht auf der
[Release-Seite]({RELEASES}) unter der Datei, gebildet von demselben Lauf, der die
Datei erzeugt hat.

```powershell
Get-FileHash .\PrismDMX_{VERSION}_x64-setup.exe -Algorithm SHA256
```
"#
                ),
            };
            format!(
                r#"# Herunterladen

**Version {VERSION}**, eine Vorabversion.

<p class="cta"><a href="{RELEASES}/latest">PrismDMX {VERSION} für Windows
herunterladen</a></p>

Windows 10 oder 11, 64 Bit. Die Datei heißt `PrismDMX_{VERSION}_x64-setup.exe`.

## Was die Installation tut

Sie installiert **pro Benutzer** und braucht **keine Administratorrechte** — ein
Schullaptop, den der Operator nicht verwalten darf, ist der Normalfall.

Es muss nichts danebeninstalliert werden: das ganze Programm ist gegen die
statische C-Laufzeitbibliothek gelinkt, es gibt kein Visual-C++-Redistributable
zu besorgen. Die eine Ausnahme ist die **Edge-WebView2-Laufzeit**, die das
Fenster zeichnet — Windows 11 hat sie, Windows 10 hat sie überall dort, wo Edge
aktualisiert wurde, und das Installationsprogramm holt sie bei Microsoft, wenn
sie fehlt. Das ist der einzige Schritt, der Internet braucht.

| | Wohin |
|---|---|
| Das Programm | `%LOCALAPPDATA%\PrismDMX` |
| Shows, Einstellungen, eigene Profile | `%APPDATA%\PrismDMX` |

**Deinstallieren entfernt das Programm und sonst nichts.** Ihre Shows und
Einstellungen bleiben liegen, und eine spätere Installation findet sie wieder.

## Windows wird warnen

Der Build ist **nicht signiert**, also zeigt Windows beim ersten Start *Der
Computer wurde durch Windows geschützt*. Das ist kein Urteil über die Datei; es
ist, was Windows über jede ausführbare Datei sagt, die es selten gesehen hat, und
ein Signaturzertifikat hat dieses Projekt noch nicht.

Weiter geht es mit **Weitere Informationen** → **Trotzdem ausführen**. Wenn Ihnen
das zu weit geht, ist das eine völlig vernünftige Haltung: prüfen Sie stattdessen
die Prüfsumme, oder warten Sie auf einen signierten Build.

{checksum}

## Ältere Versionen

Jede Version mit ihren Anmerkungen steht auf der
[Release-Seite]({RELEASES}). Was sich zwischen ihnen geändert hat, steht unter
[Änderungen](/de/aenderungen/).

## Aus dem Quelltext bauen

Für Linux, macOS oder einen Raspberry Pi gibt es kein Installationsprogramm — die
Engine ist portabel und wird bei jedem Commit für ARM64 gegengeprüft, aber
veröffentlicht wird nur Windows. Der Weg steht im
[Handbuch für Entwickler](/de/handbuch/entwickler/).
"#
            )
        }
        _ => {
            let checksum = match installer {
                Some(installer) => format!(
                    r#"## The checksum

The build is **not signed**, so the checksum is what you can check instead of a
signature. It is computed by the same run that produced the file; nobody types it
out.

| File | SHA-256 |
|---|---|
| `{}` | `{}` |

```powershell
Get-FileHash .\{} -Algorithm SHA256
```
"#,
                    installer.name, installer.sha256, installer.name
                ),
                None => format!(
                    r#"## The checksum

The build is **not signed**, so the checksum is what you can check instead of a
signature. It is on the [release page]({RELEASES}) under the file, computed by
the same run that produced it.

```powershell
Get-FileHash .\PrismDMX_{VERSION}_x64-setup.exe -Algorithm SHA256
```
"#
                ),
            };
            format!(
                r#"# Download

**Version {VERSION}**, a pre-release.

<p class="cta"><a href="{RELEASES}/latest">Download PrismDMX {VERSION} for
Windows</a></p>

Windows 10 or 11, 64-bit. The file is called `PrismDMX_{VERSION}_x64-setup.exe`.

## What the installation does

It installs **per user** and needs **no administrator rights** — a school laptop
the operator is not allowed to administer is the normal case.

Nothing has to be installed alongside it: the whole program is linked against the
static C runtime, so there is no Visual C++ redistributable to find. The one
exception is the **Edge WebView2 runtime**, which draws the window — Windows 11
has it, Windows 10 has it anywhere Edge has been updated, and the installer
fetches it from Microsoft if it is missing. That is the only step that needs the
internet.

| | Where |
|---|---|
| The program | `%LOCALAPPDATA%\PrismDMX` |
| Shows, settings, your own profiles | `%APPDATA%\PrismDMX` |

**Uninstalling removes the program and nothing else.** Your shows and settings
stay where they are, and a later installation finds them again.

## Windows will warn you

The build is **not signed**, so on first start Windows shows *Windows protected
your PC*. That is not a judgement about the file; it is what Windows says about
any executable it has rarely seen, and this project does not have a signing
certificate yet.

You continue with **More info** → **Run anyway**. If that is further than you
want to go, that is an entirely reasonable position: check the checksum instead,
or wait for a signed build.

{checksum}

## Older versions

Every version with its notes is on the [release page]({RELEASES}). What changed
between them is under [changelog](/en/changelog/).

## Building from source

There is no installer for Linux, macOS or a Raspberry Pi — the engine is portable
and is cross-checked for ARM64 on every commit, but only Windows is published.
The route is in the [developer's manual](/en/manual/developer/#1-getting-a-build).
"#
            )
        }
    };
    to_html(&markdown)
}
