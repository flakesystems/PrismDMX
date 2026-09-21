#!/usr/bin/env bash
#
# Installs the **GDTF fixture library** into `profiles/fixtures/`.
#
# This is the **install step**, not a build step: the library is somebody else's
# data with its own release cadence, and a copy committed to this repository
# would be the second one — the one that is out of date. See
# `profiles/fixtures/SOURCE.md`.
#
#   tools/fetch-fixtures/fetch-fixtures.sh [<destination>]
#
# Where the fixtures come from, in the order they are tried:
#
#   PRISMDMX_GDTF_SOURCE   a directory of `.gdtf` files, or one archive of
#                          them, already on this machine. A venue whose desk is
#                          not on a network installs from a stick, which is the
#                          ordinary case in the buildings this is for.
#   PRISMDMX_GDTF_USER     an account on <https://gdtf-share.com>, which is
#   PRISMDMX_GDTF_PASSWORD free and is the only way that service hands the
#                          library out. There is no anonymous bulk download;
#                          that is the service's decision, not this script's.
#
# Nothing but `curl` and `tar`, both of which ship with Windows 10 and later,
# with every Linux this runs on and with macOS. A downloader with a dependency
# would be a dependency somebody has to install before they can install
# anything.
#
# The Open Fixture Library is **not** installed by this script since S61. It is
# still read — a venue's own hand-written profiles are in that format — and
# `tools/fetch-fixtures/fetch-ofl.sh` installs its corpus beside the GDTF for
# anyone who wants it.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
destination="${1:-${root}/profiles/fixtures}"

# A directory or an archive that is already on this machine, instead of
# fetching anything. Also what `crates/prismd/tests/fixture_install.rs` passes,
# so that the test runs this script for real — the wipe included — to prove
# that a venue's own profiles survive a re-install (punch-list B43). A test that
# could only assert that by reimplementing the script would be asserting against
# a copy of it.
SOURCE="${PRISMDMX_GDTF_SOURCE:-}"

# https://gdtf-share.com's own API. `getList` names every published fixture and
# `downloadFile` hands one over, both behind a session cookie that `login`
# sets.
SHARE="${PRISMDMX_GDTF_SHARE:-https://gdtf-share.com/apis/public}"
USER_NAME="${PRISMDMX_GDTF_USER:-}"
PASSWORD="${PRISMDMX_GDTF_PASSWORD:-}"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
staging="${work}/fixtures"
mkdir -p "${staging}"

collect_from_source() {
    if [[ -d "${SOURCE}" ]]; then
        echo "installing GDTF fixtures from ${SOURCE}"
        # `-print0`/`-0` rather than a glob: a published `.gdtf` is named
        # `Maker@Fixture@3.gdtf` and a venue's directory has spaces in it.
        find "${SOURCE}" -type f -iname '*.gdtf' -print0 |
            xargs -0 -I {} cp {} "${staging}/"
    else
        echo "installing GDTF fixtures from ${SOURCE}"
        # An archive of them. `tar` reads both a `.tar.gz` and a `.zip` on
        # every platform this runs on.
        mkdir -p "${work}/unpacked"
        tar --extract --file "${SOURCE}" --directory "${work}/unpacked"
        find "${work}/unpacked" -type f -iname '*.gdtf' -print0 |
            xargs -0 -I {} cp {} "${staging}/"
    fi
}

collect_from_share() {
    echo "fetching the GDTF library from ${SHARE}"
    local jar="${work}/cookies"
    curl --fail --silent --show-error --cookie-jar "${jar}" \
        --data-urlencode "user=${USER_NAME}" \
        --data-urlencode "password=${PASSWORD}" \
        "${SHARE}/login.php" > "${work}/login.json"

    curl --fail --silent --show-error --cookie "${jar}" \
        "${SHARE}/getList.php" > "${work}/list.json"

    # The revision ids, without a JSON parser: the list is one flat array of
    # objects and `rid` is a number. A dependency on `jq` would be a dependency
    # somebody has to install before they can install anything.
    grep -o '"rid"[[:space:]]*:[[:space:]]*[0-9]*' "${work}/list.json" |
        grep -o '[0-9]*$' | sort -n -u > "${work}/rids"

    local count
    count="$(wc -l < "${work}/rids" | tr -d ' ')"
    if [[ "${count}" -eq 0 ]]; then
        echo "the share returned no fixtures - check the account" >&2
        exit 1
    fi
    echo "downloading ${count} fixtures"
    local rid
    while read -r rid; do
        curl --fail --silent --show-error --cookie "${jar}" \
            "${SHARE}/downloadFile.php?rid=${rid}" \
            --output "${staging}/${rid}.gdtf" || {
            echo "  skipped ${rid}" >&2
            rm -f "${staging}/${rid}.gdtf"
        }
    done < "${work}/rids"
}

if [[ -n "${SOURCE}" ]]; then
    collect_from_source
elif [[ -n "${USER_NAME}" && -n "${PASSWORD}" ]]; then
    collect_from_share
else
    cat >&2 <<'WHY'
No source for the fixture library.

  Set PRISMDMX_GDTF_SOURCE to a directory of .gdtf files, or to one archive of
  them, already on this machine; or

  set PRISMDMX_GDTF_USER and PRISMDMX_GDTF_PASSWORD to an account on
  https://gdtf-share.com, which is free to create. That service has no
  anonymous bulk download, which is why this script cannot simply fetch it.

The desk starts without a library: it offers four built-in profiles and says
so. See profiles/fixtures/SOURCE.md.
WHY
    exit 2
fi

installed="$(find "${staging}" -type f -iname '*.gdtf' | wc -l | tr -d ' ')"
if [[ "${installed}" -eq 0 ]]; then
    echo "no .gdtf files were found - the destination is left as it is" >&2
    exit 1
fi

# Only now is the destination touched. Everything above writes into a temporary
# directory, so a download that failed half way leaves the library that is
# already installed exactly where it was — which is the whole reason the wipe is
# the second-to-last thing this script does rather than the first.
mkdir -p "${destination}"
# Everything except the file that says where this came from, which is committed
# and is not the library's.
find "${destination}" -mindepth 1 -maxdepth 1 ! -name 'SOURCE.md' -exec rm -rf {} +
cp -R "${staging}/." "${destination}/"

echo "installed ${installed} GDTF fixtures into ${destination}"
