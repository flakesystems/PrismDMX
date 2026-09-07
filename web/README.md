# `prism-web` — prismdmx.de, generated out of this repository

The documentation site. It has **no content of its own**: every page is one of
the Markdown files this repository already keeps beside the code, read at build
time, rendered, and wrapped in a shell that says which version it describes.

That is S42's first exit criterion made structural rather than promised. The
site cannot drift from a release, because there is no copy of anything to drift.
A manual edited without the site rebuilt is not a site that is wrong, it is a
site that is not built yet.

```bash
cargo run -p prism-web                       # into web/dist
cargo run -p prism-web -- --out /tmp/site
cargo run -p prism-web -- --installer target/release/bundle/nsis/PrismDMX_0.9.2_x64-setup.exe
```

## The four decisions

- **A Rust workspace member, not a static-site framework.** It gets `cargo fmt`,
  `cargo clippy`, the ARM64 cross-check and the crate-README rule for free, adds
  no toolchain a contributor has to install, and takes its version from
  `[workspace.package]` — so *every page says which version it documents* is a
  fact about the build rather than a note somebody remembers to update.
- **No JavaScript.** Not a preference: a documentation page that needs a script
  to be read is one that cannot be read on the machine in the lighting rack.
  There is none on any page, and the style sheet is inlined so a page is one
  request.
- **The checksum is computed, never transcribed.** `--installer` hashes the file
  it is given; `release.yml` hands it the `.exe` it has just built. Without one,
  the download page links to the release rather than printing a number nobody
  derived.
- **The language of a page is the language of its source**, and each page carries
  its own `lang`. [`docs/manual/README.md`](../docs/manual/README.md) has that
  decision in full.

Links are rewritten: a relative link that is right in the repository becomes the
page it published as, or a link to the file on GitHub when the site does not
publish it. Neither document has to know it is being published.

## What it may not contain

**No content, and no platform code.** Content here would be the second copy of a
manual and the one that goes out of date — the point of this crate is that there
is nothing between `docs/manual/` and the page. `#[cfg(target_os = …)]` is
confined to four crates (`ARCHITECTURE_SPEC.md` §10.1) and this is not one of
them; rendering Markdown is the same on every target, and the ARM64 cross-check
compiles it.

## Testing it

```bash
cargo test -p prism-web
```

`tests/site.rs` builds the whole site into a temporary directory and then asks
it the things the exit criteria ask: that every page exists, that every one of
them names the version, that **no page contains a `<script>`**, that a manual's
relative links became the site's, and that a checksum on the download page is
the real SHA-256 of the file it was given.

## Deployment

`.github/workflows/ci.yml`'s seventh job builds this on every commit — a site
that only builds on one machine is the same problem as an installer that only
builds on one machine — and deploys it to GitHub Pages on `master`. The apex
domain is written into `CNAME` by the generator, so it is one constant in one
place; TLS is Pages' own certificate for that domain.

## Sessions

Built in **S42**, alongside S41's manuals, which are its source.
