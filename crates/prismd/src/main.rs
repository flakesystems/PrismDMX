//! The PrismDMX engine daemon.
//!
//! This process must stay alive. It owns the engine tick, the show file, the
//! session state, every DMX output and the MIDI control surface. Clients — the
//! desktop shell and the Web Remote — may come and go without affecting it;
//! that separation is decision D2 in `ARCHITECTURE_SPEC.md`.
//!
//! Everything is in the library beside this file, because a binary has no test
//! target and the exit criteria for S17 are all statements about a daemon that
//! is running. This is the entry point and nothing else: read the arguments,
//! start, wait to be told to stop, stop.
//!
//! Wired up in session **S17**.

// The two places a program is allowed to write to a terminal: what the user
// asked for with `--help` and `--version`, and the reason it could not start.
// Everything else goes through `prismd::log`, which is what `CLAUDE.md` means
// by a structured logger with levels.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line program answers --help on stdout and a refusal on stderr"
)]

use std::process::ExitCode;

use prismd::cli::{Invocation, Options};
use prismd::daemon::Daemon;
use prismd::log;

fn main() -> ExitCode {
    let options = match prismd::cli::parse(std::env::args().skip(1)) {
        Ok(Invocation::Run(options)) => *options,
        Ok(Invocation::Help) => {
            print!("{}", prismd::cli::usage());
            return ExitCode::SUCCESS;
        }
        Ok(Invocation::Version) => {
            println!("prismd {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        // S36: how a person finds the name to write into `--surface`. It exits
        // **successfully** on a machine with nothing plugged in, printing an
        // empty list, because *there is no MIDI device here* is an answer and
        // not a failure to look. The text is built in the library, where it has
        // a test; this line is the printing, which is all a binary does.
        Ok(Invocation::MidiPorts) => {
            print!("{}", prismd::cli::midi_port_report(&prism_midi::ports()));
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("prismd: {error}");
            return ExitCode::FAILURE;
        }
    };

    // The runtime is `core-main` in `ARCHITECTURE_SPEC.md` §3's table, at
    // ordinary priority. The engine tick is not on it — it is an OS thread of
    // its own, raised by `prismd::engine`, and a priority *class* would apply
    // to every thread here (S6 measured what that costs).
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("prismd: the async runtime could not be started: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(run(options))
}

async fn run(options: Options) -> ExitCode {
    let mut daemon = match Daemon::start(&options).await {
        Ok(daemon) => daemon,
        Err(error) => {
            // Before the logger has a destination worth using, and the one
            // message somebody starting a daemon by hand has to see.
            eprintln!("prismd: {error}");
            return ExitCode::FAILURE;
        }
    };

    daemon
        .run(options.run_for, async {
            // §10.3: *the daemon exits only on explicit instruction*. Ctrl-C is
            // one, a service stop is one, and closing a window is not.
            if let Err(error) = tokio::signal::ctrl_c().await {
                log::error(
                    "daemon",
                    &format!("the interrupt signal could not be listened for: {error}"),
                );
                // Falling through would stop the daemon, which is precisely
                // what must not happen because a signal handler failed.
                std::future::pending::<()>().await;
            }
        })
        .await;
    daemon.shutdown().await;
    ExitCode::SUCCESS
}
