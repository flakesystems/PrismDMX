# The fixture library goes here, and it is **not** in this repository

Everything else in this directory is fixture data **installed at install time**.
Only this file is committed; `.gitignore` keeps the rest out.

Since **S60** the desk's library is **GDTF**. The Open Fixture Library is still
*read* — it is what a venue writes a profile by hand in — but it is no longer
what the installer fetches. Both live in this directory and the desk reads both.

## What GDTF is, and why the library moved to it

[GDTF](https://gdtf.eu) — General Device Type Format, DIN SPEC 15800. A `.gdtf`
file is a ZIP archive holding a `description.xml`, the device's **3D models**,
and the **picture of every gobo on every wheel**.

The Open Fixture Library describes *channels*. It says a fixture has a gobo
wheel with seven slots and what each slot is called. It does not say what the
gobos look like, how big the fixture is, where its beam comes out of the body,
or which way the body points when the yoke is at home.

The 3D viewer (**S30**) needs every one of those, and so does the PSN follow
calculation after it. GDTF states all of them, in the file the manufacturer
publishes, and it is what the rest of this industry has standardised on: a
venue's MVR file — the format a rig is exchanged in — refers to fixtures by
their GDTF identity and by nothing else.

## Installing it

```bash
tools/fetch-fixtures/fetch-fixtures.sh
```

```powershell
tools\fetch-fixtures\fetch-fixtures.ps1
```

Nothing but `curl`/`Invoke-WebRequest` and `tar`, all of which ship with Windows
10 and later, with every Linux this runs on and with macOS. A downloader with a
dependency would be a dependency somebody has to install before they can install
anything.

**Where the fixtures come from**, in the order the script tries:

| | |
|---|---|
| `PRISMDMX_GDTF_SOURCE` | A directory of `.gdtf` files, or one archive of them, already on this machine. A venue whose desk is not on a network installs from a stick, which is the ordinary case in the buildings this is for |
| `PRISMDMX_GDTF_USER`, `PRISMDMX_GDTF_PASSWORD` | An account on <https://gdtf-share.com>, which is free |

**There is no anonymous bulk download of GDTF Share.** That is the service's
decision and not this project's, and it has two consequences written down here
so that nobody spends an afternoon looking for the switch:

- the installer cannot simply fetch the library the way `fetch-ofl` fetches the
  Open Fixture Library, so it asks for an account or for a folder;
- **no CI job installs it**, so the GDTF reader has no corpus test. What holds
  it is `prism_core::library::gdtf`'s own unit tests, which build their archives
  **byte by byte** and need no network, and one end-to-end test that writes a
  `.gdtf` into a desk's own folder and patches it in a browser.

Run the script with nothing set and it says all of this and stops, rather than
leaving a half-installed directory behind.

## Installing the Open Fixture Library corpus as well

```bash
tools/fetch-fixtures/fetch-ofl.sh      # into profiles/fixtures/ofl/
```

```powershell
tools\fetch-fixtures\fetch-ofl.ps1
```

634 fixtures across 132 manufacturers at schema 12.5.1, from revision
`9322c2d2108913eb6740c8b93f080c311ca41702` — **pinned**, not `master`, so two
machines that install a week apart get the same profiles and a show patched on
one opens the same way on the other. MIT, © 2017 Florian & Felix Edelmann and
contributors; `LICENSE` lands with the data.

It goes into a **subdirectory of its own**, because each installer empties what
it writes and neither may take the other's data with it. The desk reads a
directory with a `manufacturers.json` in it as a tree rather than as one
manufacturer, which is what makes that work.

Two callers want it: the corpus tests of the Open Fixture Library reader
(`crates/prism-core/tests/fixture_library.rs`, which CI runs on every commit),
and a venue that wants channel data for lights nobody has published a GDTF for.

## Your own profiles do **not** go here

This directory is emptied on every run of an installer, deliberately: a
half-replaced copy of somebody else's data is worse than none. Anything of yours
put here survives until the next install and no longer — which is punch-list
entry **B43**, and this paragraph is the half of it that is documentation.

A venue's own profiles go in **`fixtures/` inside the daemon's data directory**
(`%APPDATA%\PrismDMX\fixtures` on Windows, `prismd::paths::fixtures_dir`
everywhere). Nothing an installer does touches it, and the daemon makes it on
its first start with a `README.txt` inside (punch-list **B54**). Three shapes
work there:

- a **`.gdtf` file**, anywhere in the folder. Its key is what the file says the
  fixture is, so a manufacturer's own published archive dropped in here
  overrides the installed copy of that fixture **whatever either of them is
  called** — which is a better identity than a file name, and it is the
  format's own;
- a loose `.json` at the top, filed under `custom/<file stem>` — a light nobody
  has published a GDTF for. A channel list in JSON is a far kinder thing to
  write by hand than a ZIP archive of XML, which is the whole reason the older
  format is still read;
- a **manufacturer directory** of `.json` files, laid out exactly as the Open
  Fixture Library lays one out, filed under `<directory>/<file stem>` — which is
  how a profile in the corpus is corrected.

Either way the patch window marks the row as the venue's own, and says which
format it came from. See `README.md`.

## Why none of it is committed

It has an upstream and a release cadence. A copy in this repository would be a
second copy of somebody else's data — and the one that is out of date, because
nothing here would ever tell us it had moved. For GDTF there is a second reason
that is not a preference: those archives are the manufacturers', and this
project has no licence to redistribute them.

## What happens when it is not there

The desk starts, and says so. `prism_core::FixtureLibrary::generic()` is four
built-in profiles — a dimmer, two PARs and a moving head — so a rig can still be
patched, and the daemon logs how many profiles it found, where it looked, and
**how many of them are GDTF**. A library with no GDTF in it gets a line of its
own at start-up, because a desk whose library was installed before S60 works and
carries none of what the 3D viewer draws.

A lighting desk that would not start because a directory was missing would be a
worse answer than a lighting desk with four profiles in it.

## What reads it

- `prism_core::library::gdtf` — one `prism_domain::FixtureType` per GDTF **DMX
  mode**, plus the `FixturePhysical` the viewer draws: the device's size, its
  model, and every beam with where it sits and which way it points. That
  module's documentation says exactly what the conversion costs and what it
  leaves.
- `prism_core::library::zip` — the container, 200 lines of the format's own
  fixed-width records, tested against bytes.
- `prism_core::library::ofl` — the same job for the Open Fixture Library's JSON.
  Its conversion is lossy in its own way and that module says how.

## The operator's own fixtures

They do **not** go here — this directory is replaced wholesale by the next
install. A file dropped into `fixtures/` inside the daemon's **data directory**
is read in either format and **overrides** the same fixture from this tree, so a
venue can correct a profile without losing the correction.
