//! **prismdmx.de, generated out of this repository** — S42.
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
pub const DOMAIN: &str = "prismdmx.de";

/// The style sheet, inlined into every page.
const STYLE: &str = include_str!("../assets/site.css");

/* -------------------------------------------------------------------------- */
/* What a page is                                                             */
/* -------------------------------------------------------------------------- */

/// One page of the site.
pub struct Page {
    /// Where it is written, relative to the output directory, without
    /// `index.html`. The empty string is the front page.
    pub path: &'static str,
    /// What the browser tab and the navigation call it.
    pub title: &'static str,
    /// `de` or `en` — the language of the **source**, not of the site.
    pub lang: &'static str,
    /// The Markdown file it is rendered from, relative to the repository root,
    /// or `None` for a page this crate composes.
    pub source: Option<&'static str>,
}

/// The navigation, which is the same on every page and is the site's own map.
///
/// Ordered the way somebody arrives: what it is, then how to get it, then what
/// to do with it, then what changed, then the references. A reader who does not
/// know what they want should be able to get to a running desk by going down
/// this list.
pub const PAGES: [Page; 12] = [
    Page {
        path: "",
        title: "PrismDMX",
        lang: "de",
        source: None,
    },
    Page {
        path: "download",
        title: "Herunterladen",
        lang: "de",
        source: None,
    },
    Page {
        path: "handbuch/operator",
        title: "Handbuch für den Operator",
        lang: "de",
        source: Some("docs/manual/operator.md"),
    },
    Page {
        path: "handbuch/installateur",
        title: "Handbuch für den Installateur",
        lang: "de",
        source: Some("docs/manual/installer.md"),
    },
    Page {
        path: "handbuch/entwickler",
        title: "Developer's manual",
        lang: "en",
        source: Some("docs/manual/developer.md"),
    },
    Page {
        path: "aenderungen",
        title: "Änderungen",
        lang: "de",
        source: Some("CHANGELOG.md"),
    },
    Page {
        path: "referenz/kommandozeile",
        title: "Referenz: die Kommandozeile",
        lang: "en",
        source: Some("docs/COMMAND_LINE.md"),
    },
    Page {
        path: "referenz/dmx-merge",
        title: "Referenz: der DMX-Merge",
        lang: "en",
        source: Some("docs/DMX_MERGE.md"),
    },
    Page {
        path: "referenz/ipc-protokoll",
        title: "Referenz: das IPC-Protokoll",
        lang: "en",
        source: Some("docs/IPC_PROTOCOL.md"),
    },
    Page {
        path: "referenz/mcu-mapping",
        title: "Referenz: das X-Touch",
        lang: "en",
        source: Some("docs/MCU_MAPPING.md"),
    },
    Page {
        path: "referenz/architektur",
        title: "Referenz: die Architektur",
        lang: "en",
        source: Some("ARCHITECTURE_SPEC.md"),
    },
    Page {
        path: "fehler",
        title: "Bekannte Fehler",
        lang: "de",
        source: Some("docs/ISSUES.md"),
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
        "prism-web — renders prismdmx.de out of this repository.\n\n\
         Usage: prism-web [OPTIONS]\n\n\
         Options:\n\
         \x20 --out <DIR>        where to write the site (default: web/dist)\n\
         \x20 --root <DIR>       the repository to read the documents from\n\
         \x20 --installer <PATH> hash this file for the download page's checksum\n\
         \x20 -h, --help         print this and stop\n\n\
         {} pages, every one of them a document this repository already keeps\n\
         beside the code.",
        PAGES.len()
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
        for page in &PAGES {
            let body = match page.source {
                Some(source) => self.rendered(source)?,
                None => self.composed(page, installer.as_ref()),
            };
            // Relativised last, over the finished document, so that the
            // navigation, the composed pages and the link rewriter can all
            // speak in the site's own absolute paths and exactly one function
            // knows where the site is actually mounted.
            let html = relativised(&wrap(page, &body), page.path);
            let directory = if page.path.is_empty() {
                out.to_path_buf()
            } else {
                out.join(page.path)
            };
            std::fs::create_dir_all(&directory)
                .map_err(|error| BuildError(format!("{}: {error}", directory.display())))?;
            let file = directory.join("index.html");
            std::fs::write(&file, html)
                .map_err(|error| BuildError(format!("{}: {error}", file.display())))?;
        }
        // GitHub Pages serves the apex domain from this file, and it is written
        // rather than committed so that the domain is one constant in one place.
        std::fs::write(out.join("CNAME"), format!("{DOMAIN}\n"))
            .map_err(|error| BuildError(format!("CNAME: {error}")))?;
        // Nothing here is a search engine's business until the domain is live,
        // and a site that says so is better than one that is quietly indexed
        // half-built. Removed by hand when the domain is answering.
        std::fs::write(out.join(".nojekyll"), "")
            .map_err(|error| BuildError(format!(".nojekyll: {error}")))?;
        Ok(PAGES.len())
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
    fn rendered(&self, source: &str) -> Result<String, BuildError> {
        let path = self.root.join(source);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            BuildError(format!(
                "{} could not be read ({error}) — the site is generated from this repository's \
                 own documents, so a missing one is a missing page",
                path.display()
            ))
        })?;
        Ok(rewrite_links(&anchored(&to_html(&text)), source))
    }

    /// A page this crate composes rather than renders.
    fn composed(&self, page: &Page, installer: Option<&Installer>) -> String {
        let markdown = match page.path {
            "download" => download_page(installer),
            _ => front_page(),
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
pub fn rewrite_links(html: &str, from: &str) -> String {
    let published: BTreeMap<&str, String> = PAGES
        .iter()
        .filter_map(|page| page.source.map(|source| (source, absolute(page.path))))
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
/// `prismdmx.de`, in a subdirectory of any web server, and from a `file://` URL
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
pub fn wrap(page: &Page, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + STYLE.len() + 4096);
    let title = if page.path.is_empty() {
        "PrismDMX — DMX-Lichtpult für Häuser und Schulen".to_owned()
    } else {
        format!("{} — PrismDMX", page.title)
    };
    let _ = write!(
        out,
        "<!DOCTYPE html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n<meta \
         name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n",
        page.lang,
        escape(&title)
    );
    let _ = write!(out, "<style>\n{STYLE}</style>\n</head>\n<body>\n");
    out.push_str("<header class=\"top\">\n<a class=\"wordmark\" href=\"/\">PrismDMX</a>\n");
    let _ = writeln!(
        out,
        "<p class=\"version\">Diese Seite beschreibt Version <strong>{VERSION}</strong></p>"
    );
    out.push_str("</header>\n<nav aria-label=\"Seiten\"><ul>\n");
    for entry in &PAGES {
        let here = if entry.path == page.path {
            " class=\"here\" aria-current=\"page\""
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "<li><a href=\"{}\"{here}>{}</a></li>",
            absolute(entry.path),
            escape(entry.title)
        );
    }
    out.push_str("</ul></nav>\n<main>\n");
    out.push_str(body);
    out.push_str("</main>\n<footer>\n");
    let _ = write!(
        out,
        "<p>PrismDMX {VERSION} · <a href=\"{REPOSITORY}\">Quelltext auf GitHub</a> · <a \
         href=\"{REPOSITORY}/blob/master/LICENSE\">MIT</a></p>\n<p>Diese Seite wird aus dem \
         Repository erzeugt: was hier steht, steht dort. Kein JavaScript.</p>\n"
    );
    out.push_str("</footer>\n</body>\n</html>\n");
    out
}

/* -------------------------------------------------------------------------- */
/* The two composed pages                                                     */
/* -------------------------------------------------------------------------- */

/// The front page, whose one job is the sentence S42 exists to make true.
///
/// *A stranger gets from here to a running desk.* So the four steps are above
/// everything else, they are numbered, and each one names the page that has the
/// rest of it. Everything below them is context for somebody who has already
/// decided.
fn front_page() -> String {
    let markdown = format!(
        r#"# Ein DMX-Lichtpult für Häuser und Schulen

PrismDMX ist ein Lichtpult, das als Programm läuft: **Engine und Oberfläche sind
ein Installationsprogramm**, und die Engine läuft weiter, wenn Sie das Fenster
schließen. Für Windows, ohne Administratorrechte, kostenlos und quelloffen.

> **Offene Beta.** Version {VERSION} ist vollständig genug, um ein Rig zu bauen,
> eine Show zu programmieren und sie auf echter Hardware zu fahren. Sie hat noch
> keine Vorstellung in einem fremden Haus gefahren. Genau dafür ist die Beta da —
> und ein Weg zurück gehört dazu.

## Von hier zu laufendem Licht

1. **[Herunterladen und installieren](/download/)** — Windows 10 oder 11, ein
   Installationsprogramm, keine Administratorrechte, nichts danebenzuinstallieren.
2. **Sagen, womit das Haus verkabelt ist** — *Settings → Outputs*: Art-Net, sACN
   oder ein Open-DMX-USB-Adapter. Das steht im
   [Handbuch für den Installateur](/handbuch/installateur/#4-die-ausgänge).
3. **Ein Fixture patchen** — Fenster *Patch*, die Bibliothek nach Namen
   durchsuchen, Nummer und Adresse vergeben.
   [Kapitel 5 des Operator-Handbuchs](/handbuch/operator/#5-ein-rig-patchen).
4. **Auf voll ziehen** — `1 at full` in die Kommandozeile tippen und `Enter`.
   Wenn Licht kommt, läuft das Pult.

Wenn Sie dabei irgendwo hängen bleiben, ist das ein Fehler in dieser Seite und
kein Fehler bei Ihnen: [schreiben Sie ihn auf]({REPOSITORY}/issues).

## Was es kann

- **Ein Rig** aus 634 Fixtures und 2 871 Profilen der Open Fixture Library, über
  bis zu 64 Universen. Jeder DMX-Kanal eines gepatchten Fixtures hat einen Knopf.
- **Ausgänge für ein echtes Haus**: Art-Net mit Node-Erkennung, sACN (E1.31) und
  Open DMX USB / FTDI — mehrere gleichzeitig, jeder mit den Universen, für die er
  verkabelt ist, umkonfigurierbar im laufenden Betrieb.
- **Programmieren und Fahren**: Gruppen, Presets, Cue-Listen mit Tracking,
  Fade- und Delay-Zeiten, Speichermodi, Undo, Executors mit belegbaren Fadern,
  Encodern und Tasten.
- **Die Kommandozeile ist die Bedienung**: jede Taste schreibt eine Zeile, und
  jede Zeile lässt sich auf eine Taste legen.
- **Ein Behringer X-Touch** über USB, jede Taste umbelegbar — und es bedient das
  Pult, **ohne dass ein Fenster offen ist**.

## Was es noch nicht kann

Hier genannt, statt von Ihnen entdeckt zu werden: kein 3D-Visualizer, keine
Web-Fernbedienung, kein Timecode, kein OSC, kein PSN, keine Effekt-Engine.
Autostart nur unter Windows, und ein Installationsprogramm nur für Windows. Die
[vollständige Liste](/handbuch/operator/#13-was-dieses-pult-noch-nicht-kann)
steht im Handbuch, die [bekannten Fehler](/fehler/) auf ihrer eigenen Seite.

## Die Handbücher

- **[Für den Operator](/handbuch/operator/)** — wer damit eine Show baut und
  fährt.
- **[Für den Installateur](/handbuch/installateur/)** — wer das Haus verkabelt
  und das Pult einrichtet.
- **[Für Entwickler](/handbuch/entwickler/)** — wer den Quelltext ändert.
  Englisch, wie der Quelltext.
"#
    );
    to_html(&markdown)
}

/// The download page.
///
/// The checksum is printed **only** when this build was handed the installer to
/// hash. A page that carried a transcribed number would be one more place for a
/// number to be wrong, and the number is the one thing on this page that has to
/// be right.
fn download_page(installer: Option<&Installer>) -> String {
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
    let markdown = format!(
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
[Änderungen](/aenderungen/).

## Aus dem Quelltext bauen

Für Linux, macOS oder einen Raspberry Pi gibt es kein Installationsprogramm — die
Engine ist portabel und wird bei jedem Commit für ARM64 gegengeprüft, aber
veröffentlicht wird nur Windows. Der Weg steht im
[Handbuch für Entwickler](/handbuch/entwickler/#1-getting-a-build).
"#
    );
    to_html(&markdown)
}
