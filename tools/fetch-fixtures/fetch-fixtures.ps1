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
#>
[CmdletBinding()]
param(
    [string] $Destination,
    [string] $Revision = $(if ($env:PRISMDMX_OFL_REVISION) { $env:PRISMDMX_OFL_REVISION } else { '9322c2d2108913eb6740c8b93f080c311ca41702' })
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Destination) { $Destination = Join-Path $root 'profiles\fixtures' }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("prismdmx-ofl-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    $short = $Revision.Substring(0, [Math]::Min(12, $Revision.Length))
    Write-Host "fetching the Open Fixture Library at $short"
    $archive = Join-Path $work 'ofl.tar.gz'
    # `Invoke-WebRequest` without the progress bar: on a slow link it spends
    # more time drawing than downloading.
    $progress = $ProgressPreference
    $ProgressPreference = 'SilentlyContinue'
    try {
        Invoke-WebRequest -Uri "https://codeload.github.com/OpenLightingProject/open-fixture-library/tar.gz/$Revision" -OutFile $archive
    } finally {
        $ProgressPreference = $progress
    }

    # Only the fixture data and the licence. The rest of that repository is a
    # website, and a lighting desk has no use for it. `tar` ships with Windows
    # 10 and later.
    $prefix = "open-fixture-library-$Revision"
    & tar --extract --gzip --file $archive --directory $work "$prefix/fixtures" "$prefix/LICENSE"
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
