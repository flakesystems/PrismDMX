//! The PrismDMX engine daemon.
//!
//! This process must stay alive. It owns the engine tick, the show file, the
//! session state, every DMX output and the MIDI control surface. Clients - the
//! desktop shell and the Web Remote - may come and go without affecting it;
//! that separation is decision D2 in `ARCHITECTURE_SPEC.md`.
//!
//! Wired up in session **S17**.

fn main() {
    // S17: lock file and single-instance check, load show, start engine tick,
    // start output drivers, start surface controller, serve IPC.
}
