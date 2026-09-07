# `prism-app` — the shell, and the program a person installs

The desktop window, and the thing an installer installs. Deliberately thin: it
hosts the interface, it finds or starts `prismd`, and it holds **no show state
and no session state** — so closing it leaves the show running. That is decision
**D9**.

Its binary is called `PrismDMX`, because that is the name a Start menu, a task
manager and a `HKCU\…\Run` entry show a person. The crate keeps its own name for
`cargo`.

## Four questions, and every one of them is arithmetic

- `attach` — *is a desk already running here, and can this window reach it?*
- `autostart` — *does the start-up entry agree with the switch the settings
  window writes?*
- `dialogs` — *what does the operating system's file dialogue open with, for
  each path an operator can type?*
- `spawn` — *what arguments does a daemon this shell starts get, and where is
  its executable?*

Each is a function a test calls with **no Tauri anywhere**, which is
`CLAUDE.md`'s rule stated for a shell: *no test may need a window.* What is
genuinely left to the platform — that a tray item is clickable, that a native
dialogue returns what was picked, that a registry value survives a log-out — is
a row in `ARCHITECTURE_SPEC.md` §14 with a recipe somebody can follow, not a
claim nobody checked.

## What it may not contain

This is **one of the four crates `ARCHITECTURE_SPEC.md` §10.1 allows
`#[cfg(target_os = …)]`** — and the only one whose platform code is the whole
purpose of it. What it must not do is hold state: a value this shell knows and
the daemon does not is a value a second client cannot see, and a second client
is the ordinary case.

**The ARM64 cross-check does not compile this crate**, and that exclusion is
§10.1 rather than an exception to it: the job exists to catch platform code
leaking into a crate that is supposed to be neutral, and this is the crate it is
supposed to leak *into*. What builds it is the Windows CI job, on every commit,
all the way to the installer — because an installer that only ever builds on one
person's machine is a file rather than a release.

## Testing it

```bash
cargo test -p prism-app
```

On Linux this needs a webview toolkit present (`libgtk-3-dev`,
`libwebkit2gtk-4.1-dev`); on Windows it needs nothing but the MSVC toolchain.
`tests/version.rs` is the one worth knowing about: the workspace manifest is the
only place a version lives, `tauri.conf.json` deliberately carries none, and
that test is what holds the three together.

## Building the installer

```bash
cargo build --release -p prismd
cd crates/prism-app && ../../ui/node_modules/.bin/tauri build --config tauri.bundle.conf.json
```

The engine goes first because the bundle carries it as a resource. The second
configuration is what names the payload — the daemon, the fixture library and
the surface profile — and it is separate so an ordinary `cargo build --workspace`
does not demand a release build of the daemon. The installer lands in
`target/release/bundle/nsis/`.

## Where the depth is

| | |
|---|---|
| Lifecycle, tiers of autostart, the tray | `ARCHITECTURE_SPEC.md` §10.3 |
| What a person does with it | [`docs/manual/operator.md`](../../docs/manual/operator.md) |
| What only a desktop can verify | `ARCHITECTURE_SPEC.md` §14, the 🪟 row |
| Everything else | `cargo doc -p prism-app --open` |

## Sessions

Built in **S29**, which is where PrismDMX became a program rather than two
processes. Extended by **S51** (full screen, and noticing a daemon that was
killed underneath it). `PROGRESS.md` §2.46 and §2.47.
