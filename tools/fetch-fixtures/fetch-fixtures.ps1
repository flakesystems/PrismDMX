<#
.SYNOPSIS
Downloads the Open Fixture Library into `profiles/fixtures/`.

.DESCRIPTION
This is the **install step**, not a build step: the library has an upstream with
its own release cadence, and a copy committed to this repository would be the
second one — the one that is out of date. See `profiles/fixtures/SOURCE.md`.

Windows is the primary platform (D10), so this is the script an installer runs
there. `fetch-fixtures.sh` is the same thing for everything else; neither needs
anything that is not already on the machine.

.PARAMETER Destination
Where to put it. Defaults to `profiles/fixtures/` beside this repository.

.PARAMETER Revision
Which revision of the library to install. Defaults to the one this desk is
tested against — a fixed commit rather than `master`, so two machines that
install on different days get the same profiles and a show patched on one opens
the same way on the other.

.PARAMETER Archive
An archive that is already on this machine, instead of fetching one. Also
`PRISMDMX_OFL_ARCHIVE`.

Two callers want it and neither is a convenience. A venue whose desk is not on a
network installs from a file on a stick, which is the ordinary case in the
buildings this is for; and `crates/prismd/tests/fixture_install.rs` runs this
script for real — the wipe included — to prove that a venue's own profiles
survive a re-download (punch-list B43). A test that could only assert that by
reimplementing the script would be asserting against a copy of it.

Everything after the download is identical, deliberately: the destructive step
is the one being exercised.
#>
[CmdletBinding()]
param(
    [string] $Destination,
    [string] $Revision = $(if ($env:PRISMDMX_OFL_REVISION) { $env:PRISMDMX_OFL_REVISION } else { '9322c2d2108913eb6740c8b93f080c311ca41702' }),
    [string] $Archive = $env:PRISMDMX_OFL_ARCHIVE
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Destination) { $Destination = Join-Path $root 'profiles\fixtures' }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("prismdmx-ofl-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    # `$tarball` and not `$archive`: **PowerShell variable names are
    # case-insensitive**, so a local `$archive` *is* the `$Archive` parameter and
    # assigning to it makes `if ($Archive)` true on every run — which sends a
    # machine that was told nothing down the local-archive branch, to copy a file
    # that is not there. CI caught it on the commit that introduced it; the two
    # names are kept apart deliberately.
    $tarball = Join-Path $work 'ofl.tar.gz'
    if ($Archive) {
        Write-Host "installing the Open Fixture Library from $Archive"
        Copy-Item -Path $Archive -Destination $tarball -Force
    } else {
        $short = $Revision.Substring(0, [Math]::Min(12, $Revision.Length))
        Write-Host "fetching the Open Fixture Library at $short"
        # `Invoke-WebRequest` without the progress bar: on a slow link it spends
        # more time drawing than downloading.
        $progress = $ProgressPreference
        $ProgressPreference = 'SilentlyContinue'
        try {
            Invoke-WebRequest -Uri "https://codeload.github.com/OpenLightingProject/open-fixture-library/tar.gz/$Revision" -OutFile $tarball
        } finally {
            $ProgressPreference = $progress
        }
    }

    # Only the fixture data and the licence. The rest of that repository is a
    # website, and a lighting desk has no use for it. `tar` ships with Windows
    # 10 and later.
    $prefix = "open-fixture-library-$Revision"
    & tar --extract --gzip --file $tarball --directory $work "$prefix/fixtures" "$prefix/LICENSE"
    if ($LASTEXITCODE -ne 0) { throw "tar failed with $LASTEXITCODE" }

    if (-not (Test-Path $Destination)) { New-Item -ItemType Directory -Path $Destination | Out-Null }
    # Everything except the file that says where this came from, which is
    # committed and is not the library's.
    Get-ChildItem -Path $Destination -Force |
        Where-Object { $_.Name -ne 'SOURCE.md' } |
        Remove-Item -Recurse -Force

    Copy-Item -Path (Join-Path $work "$prefix\fixtures\*") -Destination $Destination -Recurse -Force
    Copy-Item -Path (Join-Path $work "$prefix\LICENSE") -Destination (Join-Path $Destination 'LICENSE') -Force

    $count = (Get-ChildItem -Path $Destination -Recurse -Filter '*.json' |
        Where-Object { $_.Name -ne 'manufacturers.json' }).Count
    Write-Host "installed $count fixtures into $Destination"
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
