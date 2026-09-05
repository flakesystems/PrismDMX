# The fixture library goes here, and it is **not** in this repository

Everything else in this directory is the fixture data of the
[Open Fixture Library](https://open-fixture-library.org), **downloaded at
install time**. Only this file is committed; `.gitignore` keeps the rest out.

| | |
|---|---|
| Upstream | <https://github.com/OpenLightingProject/open-fixture-library> |
| Revision | `9322c2d2108913eb6740c8b93f080c311ca41702` |
| Schema | `12.5.1` — [fixture-format.md](https://github.com/OpenLightingProject/open-fixture-library/blob/master/docs/fixture-format.md) |
| Licence | MIT, © 2017 Florian & Felix Edelmann and contributors. `LICENSE` lands here with the data |
| Size | 634 fixtures across 132 manufacturers, 2 798 modes, about 8.5 MB |

## Installing it

```bash
tools/fetch-fixtures/fetch-fixtures.sh
```

```powershell
tools\fetch-fixtures\fetch-fixtures.ps1
```

Both take an optional destination and use nothing but `curl`/`Invoke-WebRequest`
and `tar`, all of which ship with Windows 10 and later, with every Linux this
runs on and with macOS. A downloader with a dependency would be a dependency
somebody has to install before they can install anything.

The revision is **pinned**, not `master`. Two machines that install a week apart
get the same profiles, so a show patched on one opens the same way on the other;
raising it is a deliberate edit to both scripts and to this table.

## Your own profiles do **not** go here

This directory is emptied on every run of the installer, deliberately: a
half-replaced copy of somebody else's data is worse than none. Anything of yours
put here survives until the next install and no longer — which is punch-list
entry **B43**, and this paragraph is the half of it that is documentation.

A venue's own profiles go in **`fixtures/` inside the daemon's data directory**
(`%APPDATA%\PrismDMX\fixtures` on Windows, `prismd::paths::fixtures_dir`
everywhere). Nothing an installer does touches it. Two shapes work there:

- a loose `.json` at the top, filed under `custom/<file stem>` — a light nobody
  has a profile for;
- a **manufacturer directory** laid out exactly as this one is, filed under
  `<directory>/<file stem>` — which is how a profile in *here* is corrected: the
  desk reads its own directory first and keeps the first profile it finds for a
  fixture, so yours replaces the vendored one whole.

Either way the patch window marks the row as the venue's own. See `README.md`.

## Why it is not committed

It has an upstream and a release cadence. A copy in this repository would be a
second copy of somebody else's data — and the one that is out of date, because
nothing here would ever tell us it had moved. The pinned revision is what buys
the reproducibility that vendoring is usually for, without the repository
carrying 8.5 MB of data it does not own.

## What happens when it is not there

The desk starts, and says so. `prism_core::FixtureLibrary::generic()` is four
built-in profiles — a dimmer, two PARs and a moving head — so a rig can still be
patched, and the daemon logs how many profiles it found and where it looked.
A lighting desk that would not start because a directory was missing would be a
worse answer than a lighting desk with four profiles in it.

## What reads it

`prism_core::library::ofl` — one `prism_domain::FixtureType` per OFL **mode**.
The conversion is deliberately lossy and that module's documentation says
exactly how; the short version is that this domain model has fifteen
`AttributeType`s and OFL fixtures carry capabilities there is no room for.

## The operator's own fixtures

They do **not** go here — this directory is replaced wholesale by the next
install. A file dropped into `fixtures/` inside the daemon's **data directory**
is read in exactly this format and **overrides** a key of the same name from
this tree, so a venue can correct a profile without losing the correction.
