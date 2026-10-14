/**
 * `npm run bindings` — regenerates `src/bindings/`, the TypeScript types the
 * daemon speaks, from the Rust source.
 *
 * Run it after pulling a change to `crates/`, or whenever TypeScript says a
 * module in `../bindings` cannot be found. It is `cargo test -p prism-domain`:
 * that crate's tests write the files, which is what keeps them from going stale.
 */

import { REPO_DIR, fail, has, run, step } from "./lib.mjs";

if (!has("cargo")) {
  fail("cargo was not found. Install Rust from https://rustup.rs and open a new terminal.");
}

step("Generating TypeScript bindings from crates/prism-domain (the first run compiles for a few minutes)");
run("cargo", ["test", "--quiet", "-p", "prism-domain"], REPO_DIR);
step("Done — src/bindings/ is up to date.");
