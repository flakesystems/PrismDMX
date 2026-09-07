//! Was an einer Startseite prüfbar ist — und das ist wenig.
//!
//! Die Gestaltung ist **ausdrücklich nicht** getestet, und das ist eine
//! Entscheidung des Eigentümers: diese Seite ist kein Teil der kritischen
//! Software, sie steuert kein Licht, und ein Test über einen Farbverlauf wäre
//! ein Test über einen Geschmack. `CLAUDE.md`s Testpolitik gilt für `engine/`,
//! `programmer/` und `protocols/` — nicht hierfür.
//!
//! Was geprüft wird, ist die eine Sorte Fehler, die eine Startseite trotzdem
//! machen kann und die niemand sieht, bevor sie ausgeliefert ist: **eine Seite,
//! auf der etwas Falsches steht.** Eine Version, die nicht die des Releases
//! ist; ein Platzhalter, den niemand eingesetzt hat; eine Datei, die im
//! Verzeichnis fehlt, weil sie beim Kopieren vergessen wurde.

use std::path::{Path, PathBuf};

use prism_site::{DOCS, Invocation, REPOSITORY, Site, VERSION, parse_arguments};

/// Ein Verzeichnis, das sich selbst aufräumt — dasselbe Muster wie in
/// `web/tests/site.rs`, damit ein Test nichts hinterlässt.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("prism-site-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("ein temporäres Verzeichnis");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Kein `{{…}}` überlebt den Build.
///
/// Das ist der Fehler, der beim Bearbeiten der Vorlage passiert und den kein
/// Blick auf die Seite fängt, weil zwei geschweifte Klammern in einem dunklen
/// Fließtext aussehen wie Gestaltung.
#[test]
fn no_placeholder_survives_the_build() {
    let page = Site::default().page().expect("die Seite baut");
    assert!(!page.contains("{{"), "ein Platzhalter ist stehen geblieben");
}

/// Die Seite nennt die Version, die dieser Build ist.
///
/// `prism-web` hält dieselbe Regel über zwölf Seiten. Hier ist sie an einer
/// Stelle, aber es ist dieselbe Aussage: eine Startseite, die eine ältere
/// Version bewirbt, als das Release trägt, ist eine falsche Seite.
#[test]
fn the_page_names_the_version_this_build_is() {
    let page = Site::default().page().expect("die Seite baut");
    assert!(page.contains(VERSION), "die Seite nennt {VERSION} nicht");
    assert!(
        page.contains(&format!("PrismDMX_{VERSION}_x64-setup.exe")),
        "der Dateiname des Installers trägt die Version nicht"
    );
}

/// Die drei Adressen, die der Eigentümer ausdrücklich verlangt hat, stehen drin.
///
/// Das Repository, die Dokumentation und die Releases. Ein Test dafür ist
/// billig und fängt genau den Fall, in dem jemand die Vorlage umbaut und einen
/// Link dabei verliert.
#[test]
fn the_page_links_the_repository_and_the_documentation() {
    let page = Site::default().page().expect("die Seite baut");
    for address in [REPOSITORY, DOCS] {
        assert!(page.contains(address), "die Seite verlinkt {address} nicht");
    }
}

/// `--docs` erreicht die Seite, damit ein Umzug eine Konstante ist.
#[test]
fn the_documentation_address_is_one_place() {
    let site = Site {
        docs: "https://handbuch.example/".to_owned(),
    };
    let page = site.page().expect("die Seite baut");
    assert!(page.contains("https://handbuch.example/handbuch/operator/"));
    assert!(
        !page.contains(DOCS),
        "die voreingestellte Adresse steht noch drin, obwohl eine andere gegeben wurde"
    );
}

/// Fünf Dateien, und alle fünf landen im Verzeichnis.
///
/// Die Seite ohne ihr Stylesheet ist die eine Auslieferung, die aussieht wie
/// ein kaputter Server und keiner ist.
#[test]
fn the_build_writes_every_file_the_page_asks_for() {
    let scratch = Scratch::new("build");
    let written = Site::default()
        .build(scratch.path())
        .expect("der Build läuft");
    assert_eq!(written, 5);
    for name in [
        "index.html",
        "site.css",
        "site.js",
        "logo.png",
        "favicon.ico",
    ] {
        assert!(
            scratch.path().join(name).is_file(),
            "{name} fehlt im Ausgabeverzeichnis"
        );
    }
}

/// Das Zeichen ist **das echte** und keine Nachzeichnung.
///
/// `site/assets/logo.png` und `site/assets/favicon.ico` sind Kopien von
/// `ui/public/favicon.ico`, das das Pult im Fenster trägt. Ein nachgebautes
/// Zeichen wäre ein zweites Zeichen: es sieht ähnlich aus, es weicht ab, und
/// niemand merkt, welches von beiden das richtige war. Der Test vergleicht die
/// Bytes, damit das eine Tatsache über den Build bleibt und keine Absicht.
#[test]
fn the_mark_on_the_page_is_the_one_the_desk_wears() {
    let scratch = Scratch::new("mark");
    Site::default()
        .build(scratch.path())
        .expect("der Build läuft");

    let desk = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("der Crate liegt unter der Wurzel")
        .join("ui")
        .join("public")
        .join("favicon.ico");
    let desk = std::fs::read(&desk).expect("das Zeichen des Pults");
    let served = std::fs::read(scratch.path().join("favicon.ico")).expect("das ausgelieferte");
    assert_eq!(
        desk, served,
        "das Zeichen auf der Seite ist nicht mehr das, das das Pult trägt"
    );
    // Und das PNG daneben ist dasselbe Bild, nur in einem Format, das ein
    // `<img>` überall zeichnet — also nicht byteweise vergleichbar, aber
    // vorhanden und nicht leer.
    let logo = std::fs::metadata(scratch.path().join("logo.png")).expect("das Logo");
    assert!(logo.len() > 1024, "das Logo ist zu klein, um eins zu sein");
}

/// Ohne Skript bleibt die Seite lesbar — geprüft an dem, was `site.js` *nicht*
/// erzeugen darf.
///
/// Der Absatz in `site.js` sagt es als Regel; hier steht die eine Hälfte davon,
/// die eine Maschine beantworten kann: jeder Abschnitt der Seite und jeder
/// Link, den der Eigentümer verlangt hat, steht im ausgelieferten HTML und wird
/// nicht erst zur Laufzeit eingesetzt.
#[test]
fn the_page_is_readable_without_the_script() {
    let page = Site::default().page().expect("die Seite baut");
    for anchor in [
        "id=\"kern\"",
        "id=\"funktionen\"",
        "id=\"demos\"",
        "id=\"stand\"",
        "id=\"zukunft\"",
        "id=\"download\"",
    ] {
        assert!(page.contains(anchor), "der Abschnitt {anchor} fehlt");
    }
    // Die Zukunft ist der Abschnitt, der am ehesten in ein Skript rutscht,
    // weil er eine Zeitleiste ist. Er ist Text.
    assert!(
        page.contains("eigenes Pult"),
        "der Ausblick auf eigene Hardware fehlt"
    );
}

/// Die Kommandozeile, an ihren zwei Enden.
#[test]
fn the_command_line_reads_what_it_documents() {
    let root = Path::new("/repo");
    assert_eq!(
        parse_arguments(["--help".to_owned()], root),
        Ok(Invocation::Help)
    );
    assert_eq!(
        parse_arguments(["--out".to_owned(), "/tmp/x".to_owned()], root),
        Ok(Invocation::Render {
            out: PathBuf::from("/tmp/x"),
            docs: DOCS.to_owned(),
        })
    );
    assert!(parse_arguments(["--out".to_owned()], root).is_err());
    assert!(parse_arguments(["--was".to_owned()], root).is_err());
}
