/**
 * `npm run setup` — everything the interface needs before `npm run dev`.
 *
 * 1. checks that Node and Rust are installed,
 * 2. installs the npm packages,
 * 3. generates `src/bindings/` from Rust,
 * 4. builds the daemon once, so `npm run dev:all` starts quickly afterwards.
 *
 * Safe to run again at any time; nothing it does is destructive.
 *
 * (Bootstrapping note: on a fresh clone `npm run setup` works before
 * `npm install`, because this script needs nothing but Node itself.)
 */

import { REPO_DIR, UI_DIR, fail, has, run, step } from "./lib.mjs";

const [major] = process.versions.node.split(".").map(Number);
if (major === undefined || major < 20) {
  fail(`Node ${process.versions.node} is too old; install Node 22 or newer from https://nodejs.org.`);
}
if (!has("cargo")) {
  fail(
    "cargo was not found. The daemon and the type bindings are built from Rust:\n" +
      "  install it from https://rustup.rs (default options), then open a new terminal\n" +
      "  and run `npm run setup` again. You will not have to write any Rust.",
  );
}

step("Installing npm packages");
run("npm", ["install"], UI_DIR);

step("Generating TypeScript bindings (the first run compiles for a few minutes)");
run("cargo", ["test", "--quiet", "-p", "prism-domain"], REPO_DIR);

step("Building the daemon (prismd)");
run("cargo", ["build", "--quiet", "-p", "prismd"], REPO_DIR);

step("Ready. Start everything with:  npm run dev:all");
