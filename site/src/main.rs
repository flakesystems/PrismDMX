//! `prism-site` — baut die Startseite von prismdmx.de.
//!
//! ```text
//! prism-site [--out <dir>] [--docs <url>]
//! ```
//!
//! **Alles, was dieser Binary tut, steht in [`prism_site`], weil ein
//! Binary-Target keine Tests hat** — `prismd`s eigene Regel, und `prism-web`
//! hat sie schon einmal auf einen Generator angewandt. Was hier bleibt, sind
//! die vier Zeilen, die ohnehin nicht prüfbar sind: die Argumente lesen und
//! etwas drucken.

use std::path::PathBuf;
use std::process::ExitCode;

use prism_site::{Invocation, Site, parse_arguments, usage};

/// Das Elternverzeichnis dieses Crates, also die Wurzel des Repositories.
fn default_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("der Crate liegt eine Ebene unter der Repository-Wurzel")
        .to_path_buf()
}

fn main() -> ExitCode {
    let invocation = match parse_arguments(std::env::args().skip(1), &default_root()) {
        Ok(invocation) => invocation,
        Err(complaint) => {
            report(&complaint.0);
            return ExitCode::FAILURE;
        }
    };
    let (out, docs) = match invocation {
        Invocation::Help => {
            say(&usage());
            return ExitCode::SUCCESS;
        }
        Invocation::Render { out, docs } => (out, docs),
    };
    match (Site { docs }).build(&out) {
        Ok(files) => {
            say(&format!(
                "{files} Dateien geschrieben nach {}",
                out.display()
            ));
            ExitCode::SUCCESS
        }
        Err(error) => {
            report(&error.0);
            ExitCode::FAILURE
        }
    }
}

/// Auf die Standardausgabe. Das Lint verbietet `println!` in Produktionscode;
/// eine Kommandozeile, die nichts sagt, ist aber keine Kommandozeile.
#[allow(clippy::print_stdout)]
fn say(text: &str) {
    println!("{}", text.trim_end());
}

/// Auf die Fehlerausgabe, aus demselben Grund.
#[allow(clippy::print_stderr)]
fn report(text: &str) {
    eprintln!("prism-site: {text}");
}
