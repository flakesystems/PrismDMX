#!/usr/bin/env bash
#
# Installs the **Open Fixture Library** corpus into `profiles/fixtures/ofl/`.
#
# Split out of `fetch-fixtures.sh` in S61, when the desk's library became GDTF.
# Two callers want this and neither is the ordinary install:
#
#   * `crates/prism-core/tests/fixture_library.rs` reads the whole corpus and
#     asserts what the conversion costs over it. 634 fixtures is the input no
#     hand-written JSON can stand in for, and CI runs this script to get it.
#   * A venue that wants the channel data for lights nobody has published a
#     GDTF for. It goes in a subdirectory of the installed library, which the
#     desk reads as it reads everything else there.
#
#   tools/fetch-fixtures/fetch-ofl.sh [<destination>]
#
# Nothing but `curl` and `tar`, both of which ship with Windows 10 and later, in
# every Linux this runs on and in macOS.
set -euo pipefail

# The revision this desk is tested against. A commit rather than `master`, so
# two machines that install on different days get the same profiles and a show
# patched on one opens the same way on the other. Raise it deliberately.
REVISION="${PRISMDMX_OFL_REVISION:-9322c2d2108913eb6740c8b93f080c311ca41702}"
ARCHIVE="https://codeload.github.com/OpenLightingProject/open-fixture-library/tar.gz/${REVISION}"
# An archive that is already on this machine, instead of fetching one.
#
# Two callers want it and neither is a convenience. A venue whose desk is not on
# a network installs from a file on a stick, which is the ordinary case in the
# buildings this is for; and `crates/prismd/tests/fixture_install.rs` runs this
# script for real — the wipe included — to prove that a venue's own profiles
# survive a re-download (punch-list B43). A test that could only assert that by
# reimplementing the script would be asserting against a copy of it.
#
# Everything after the download is identical, deliberately: the destructive step
# is the one being exercised.
LOCAL_ARCHIVE="${PRISMDMX_OFL_ARCHIVE:-}"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
destination="${1:-${root}/profiles/fixtures/ofl}"
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

if [[ -n "${LOCAL_ARCHIVE}" ]]; then
    echo "installing the Open Fixture Library from ${LOCAL_ARCHIVE}"
    cp "${LOCAL_ARCHIVE}" "${work}/ofl.tar.gz"
else
    echo "fetching the Open Fixture Library at ${REVISION:0:12}"
    curl --fail --location --silent --show-error "${ARCHIVE}" --output "${work}/ofl.tar.gz"
fi

# Only the fixture data and the licence. The rest of that repository is a
# website, and a lighting desk has no use for it.
prefix="open-fixture-library-${REVISION}"
tar --extract --gzip --file "${work}/ofl.tar.gz" --directory "${work}" \
    "${prefix}/fixtures" "${prefix}/LICENSE"

mkdir -p "${destination}"
# Everything except the file that says where this came from, which is committed
# and is not the library's.
find "${destination}" -mindepth 1 -maxdepth 1 ! -name 'SOURCE.md' -exec rm -rf {} +
cp -R "${work}/${prefix}/fixtures/." "${destination}/"
cp "${work}/${prefix}/LICENSE" "${destination}/LICENSE"

fixtures="$(find "${destination}" -name '*.json' ! -name 'manufacturers.json' | wc -l | tr -d ' ')"
echo "installed ${fixtures} Open Fixture Library fixtures into ${destination}"
