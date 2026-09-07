//! **prismdmx.de — die Startseite des Projekts.**
//!
//! Das Gegenstück zu [`prism_web`](../prism_web/index.html), und die beiden
//! teilen sich eine Domain und sonst nichts:
//!
//! | | `prism-web` | `prism-site` |
//! |---|---|---|
//! | Was es ist | die Handbücher, gerendert | eine Seite, die das Projekt zeigt |
//! | Wo es liegt | `docs.prismdmx.de` | `prismdmx.de` |
//! | Woher der Inhalt kommt | aus `docs/` — **keiner eigener** | aus einer Vorlage, die dieser Crate gehört |
//! | JavaScript | **keins**, per Test | ja, und die Begründung steht in `assets/site.js` |
//!
//! Warum das zwei Crates sind statt einer mit zwei Ausgaben: `prism-web` hat
//! genau eine Eigenschaft, für die es existiert — *es kann nicht von einem
//! Release abweichen, weil es nichts zu kopieren gibt* — und ein Marketingtext
//! darin wäre das erste, was ihm diese Eigenschaft nimmt. Der Test, der jedes
//! `<script>` zurückweist, wäre das zweite. Die Trennung ist billiger als die
//! Ausnahmen.
//!
//! # Was dieser Crate tut
//!
//! Sehr wenig, und das ist der Punkt. Die Seite ist ein Entwurf, kein Dokument:
//! sie wird von Hand geschrieben, weil eine Startseite eine Gestaltung ist. Was
//! **nicht** von Hand geschrieben wird, sind die vier Dinge, die veralten:
//!
//! - die **Version**, die aus `[workspace.package]` kommt und damit dieselbe
//!   ist, die `prismd --version` druckt und die der Installer trägt;
//! - der **Dateiname des Installers**, der die Version enthält;
//! - die **Adresse der Dokumentation**, damit ein Umzug eine Konstante ist;
//! - das **Repository** und seine Releases.
//!
//! Alles vier steht in der Vorlage als `{{name}}` und wird hier eingesetzt. Ein
//! Platzhalter, den niemand einsetzt, ist ein Fehler und keine leere Stelle —
//! [`Site::build`] bricht ab, wenn nach der Ersetzung noch einer im Dokument
//! steht.
//!
//! # Aufruf
//!
//! ```text
//! prism-site [--out <dir>] [--docs <url>]
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/* -------------------------------------------------------------------------- */
/* Konstanten                                                                 */
/* -------------------------------------------------------------------------- */

/// `[workspace.package] version`, geerbt — dieselbe Zahl, die der Installer
/// trägt und die `crates/prism-app/tests/version.rs` zusammenhält.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Das Repository.
pub const REPOSITORY: &str = "https://github.com/flakesystems/PrismDMX";

/// Die Releases, auf die der Download-Knopf zeigt.
///
/// Bewusst die Release-**Seite** und nicht eine Datei: eine fest verdrahtete
/// `…/download/v0.9.2/PrismDMX_0.9.2_x64-setup.exe` wäre ein toter Link an dem
/// Tag, an dem ein Tag anders heißt, und die Release-Seite trägt außerdem die
/// SHA-256, die jemand prüfen soll.
pub const RELEASES: &str = "https://github.com/flakesystems/PrismDMX/releases/latest";

/// Wo die Handbücher liegen — das, was `prism-web` erzeugt.
///
/// Überschreibbar mit `--docs`, damit die Seite auch dann stimmt, wenn beides
/// zum Ausprobieren nebeneinander unter einem `file://`-Pfad liegt.
pub const DOCS: &str = "https://docs.prismdmx.de";

/// Die Domain, unter der diese Seite ausgeliefert wird.
pub const DOMAIN: &str = "prismdmx.de";

/// Die Vorlage und ihre vier Beilagen, einkompiliert.
///
/// `include_str!` statt eines Lesevorgangs zur Laufzeit, aus demselben Grund
/// wie in `prism-web`: der Binary ist dann für sich vollständig und ein
/// Container braucht das Repository nicht mehr, nachdem er gebaut wurde.
const TEMPLATE: &str = include_str!("../assets/index.html");
const STYLE: &str = include_str!("../assets/site.css");
const SCRIPT: &str = include_str!("../assets/site.js");

/// Das Zeichen, und es ist **das echte**.
///
/// `site/assets/logo.png` und `site/assets/favicon.ico` sind Kopien von
/// `ui/public/favicon.ico` — dasselbe Bild, das das Pult im Fenster trägt und
/// das der Installer als Programmsymbol mitgibt. Es ist eine Kopie und keine
/// Nachzeichnung, weil ein nachgebautes Zeichen ein zweites Zeichen ist: es
/// sieht ähnlich aus, es weicht ab, und niemand merkt, welches von beiden das
/// richtige war.
///
/// Binär, also `include_bytes!` — dieselbe Überlegung wie bei den drei Dateien
/// darüber: was einkompiliert ist, macht den Binary für sich vollständig, und
/// ein Container braucht das Repository nicht mehr, nachdem er gebaut wurde.
const LOGO: &[u8] = include_bytes!("../assets/logo.png");
const FAVICON: &[u8] = include_bytes!("../assets/favicon.ico");

/* -------------------------------------------------------------------------- */
/* Fehler                                                                     */
/* -------------------------------------------------------------------------- */

/// Was schiefgehen kann, als der Satz, den jemand liest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildError(pub String);

impl std::fmt::Display for BuildError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

impl std::error::Error for BuildError {}

/* -------------------------------------------------------------------------- */
/* Die Seite                                                                  */
/* -------------------------------------------------------------------------- */

/// Die Startseite, mit den Adressen, unter denen sie gebaut wird.
pub struct Site {
    /// Wohin die Handbücher zeigen. Voreinstellung: [`DOCS`].
    pub docs: String,
}

impl Default for Site {
    fn default() -> Self {
        Self {
            docs: DOCS.to_owned(),
        }
    }
}

impl Site {
    /// Die Werte, die in die Vorlage eingesetzt werden.
    ///
    /// Eine `BTreeMap` und keine Liste von Paaren, weil ein zweimal belegter
    /// Platzhalter dann keine Frage der Reihenfolge ist.
    pub fn values(&self) -> BTreeMap<&'static str, String> {
        let docs = self.docs.trim_end_matches('/').to_owned();
        BTreeMap::from([
            ("version", VERSION.to_owned()),
            ("repository", REPOSITORY.to_owned()),
            ("releases", RELEASES.to_owned()),
            ("docs", docs),
            ("installer", format!("PrismDMX_{VERSION}_x64-setup.exe")),
        ])
    }

    /// Die fertige Seite, als Zeichenkette.
    ///
    /// # Errors
    ///
    /// Wenn nach der Ersetzung noch ein `{{…}}` im Dokument steht. Das ist ein
    /// Tippfehler in der Vorlage, und er soll den Build anhalten statt als
    /// zwei geschweifte Klammern auf der Seite zu landen.
    pub fn page(&self) -> Result<String, BuildError> {
        let mut html = TEMPLATE.to_owned();
        for (key, value) in self.values() {
            html = html.replace(&format!("{{{{{key}}}}}"), &value);
        }
        if let Some(rest) = html.split_once("{{") {
            let name: String = rest.1.chars().take_while(|c| *c != '}').collect();
            return Err(BuildError(format!(
                "die Vorlage enthält den Platzhalter {{{{{name}}}}}, den niemand einsetzt"
            )));
        }
        Ok(html)
    }

    /// Schreibt die Seite und ihre vier Beilagen nach `out`, und antwortet mit
    /// der Anzahl der geschriebenen Dateien.
    ///
    /// # Errors
    ///
    /// Wenn die Vorlage einen offenen Platzhalter hat oder das Verzeichnis
    /// nicht beschreibbar ist.
    pub fn build(&self, out: &Path) -> Result<usize, BuildError> {
        let page = self.page()?;
        std::fs::create_dir_all(out)
            .map_err(|error| BuildError(format!("{}: {error}", out.display())))?;

        let files: [(&str, &[u8]); 5] = [
            ("index.html", page.as_bytes()),
            ("site.css", STYLE.as_bytes()),
            ("site.js", SCRIPT.as_bytes()),
            ("logo.png", LOGO),
            ("favicon.ico", FAVICON),
        ];
        for (name, body) in files {
            let path = out.join(name);
            std::fs::write(&path, body)
                .map_err(|error| BuildError(format!("{}: {error}", path.display())))?;
        }
        Ok(files.len())
    }
}

/* -------------------------------------------------------------------------- */
/* Die Kommandozeile                                                          */
/* -------------------------------------------------------------------------- */

/// Was ein Aufruf bedeutet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// `--help`.
    Help,
    /// Bauen, nach `out`, mit den Handbüchern unter `docs`.
    Render { out: PathBuf, docs: String },
}

/// Was `--help` druckt.
#[must_use]
pub fn usage() -> String {
    format!(
        "prism-site — baut die Startseite von {DOMAIN}.\n\n\
         Aufruf:\n  \
           prism-site [--out <verzeichnis>] [--docs <url>]\n\n\
         Optionen:\n  \
           --out <verzeichnis>  wohin (Voreinstellung: site/dist)\n  \
           --docs <url>         wo die Handbücher liegen (Voreinstellung: {DOCS})\n  \
           -h, --help           dieser Text\n"
    )
}

/// Liest die Argumente.
///
/// # Errors
///
/// Bei einer unbekannten Option oder einer, der ihr Wert fehlt. Beides ist ein
/// Satz und keine Ausnahme, weil das hier eine Kommandozeile ist.
pub fn parse_arguments<I>(arguments: I, root: &Path) -> Result<Invocation, BuildError>
where
    I: IntoIterator<Item = String>,
{
    let mut out: Option<PathBuf> = None;
    let mut docs: Option<String> = None;
    let mut arguments = arguments.into_iter();

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(Invocation::Help),
            "--out" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| BuildError("--out braucht ein Verzeichnis".to_owned()))?;
                out = Some(PathBuf::from(value));
            }
            "--docs" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| BuildError("--docs braucht eine Adresse".to_owned()))?;
                docs = Some(value);
            }
            other => {
                return Err(BuildError(format!(
                    "unbekannte Option {other} — `prism-site --help` sagt, was es gibt"
                )));
            }
        }
    }

    Ok(Invocation::Render {
        out: out.unwrap_or_else(|| root.join("site").join("dist")),
        docs: docs.unwrap_or_else(|| DOCS.to_owned()),
    })
}
