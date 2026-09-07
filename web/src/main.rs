//! `prism-web` — renders docs.prismdmx.de out of this repository.
//!
//! ```text
//! prism-web [--out <dir>] [--root <dir>] [--installer <path>]
//! ```
//!
//! `--out` defaults to `web/dist`, `--root` to the repository this binary was
//! compiled in, and `--installer` is what `release.yml` hands the `.exe` it has
//! just built so the download page's checksum is computed rather than
//! transcribed.
//!
//! **Everything this binary does is in [`prism_web`], because a binary target
//! has no tests** — `prismd`'s own rule, and it applies to three flags as much
//! as to a daemon: `--installer` taking the wrong argument would be a release
//! page carrying somebody else's checksum, and nothing would say so. What is
//! left here is the four lines that cannot be tested anyway: reading the process
//! arguments, and printing.

use std::path::PathBuf;
use std::process::ExitCode;

use prism_web::{Invocation, Site, parse_arguments, usage};

/// This crate's own directory's parent, which is the repository root.
fn default_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate is one level under the repository root")
        .to_path_buf()
}

fn main() -> ExitCode {
    let invocation = match parse_arguments(std::env::args().skip(1), &default_root()) {
        Ok(invocation) => invocation,
        Err(complaint) => {
            report(&complaint);
            return ExitCode::FAILURE;
        }
    };
    let (root, out, installer) = match invocation {
        Invocation::Help => {
            say(&usage());
            return ExitCode::SUCCESS;
        }
        Invocation::Render {
            root,
            out,
            installer,
        } => (root, out, installer),
    };
    match (Site { root, installer }).build(&out) {
        Ok(pages) => {
            say(&format!("{pages} pages written to {}", out.display()));
            ExitCode::SUCCESS
        }
        Err(error) => {
            report(&error.0);
            ExitCode::FAILURE
        }
    }
}

// This binary writes to a terminal on purpose: it is a build tool, and the
// workspace lint that forbids `println!` is about the daemon's structured
// logger. `CLAUDE.md`'s rule is that production code logs; a generator reports.
#[allow(clippy::print_stdout)]
fn say(message: &str) {
    println!("{message}");
}

#[allow(clippy::print_stderr)]
fn report(message: &str) {
    eprintln!("prism-web: {message}");
}
