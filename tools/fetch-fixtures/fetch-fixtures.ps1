<#
.SYNOPSIS
Installs the GDTF fixture library into `profiles/fixtures/`.

.DESCRIPTION
This is the **install step**, not a build step: the library is somebody else's
data with its own release cadence, and a copy committed to this repository would
be the second one — the one that is out of date. See
`profiles/fixtures/SOURCE.md`.

Windows is the primary platform (D10), so this is the script an installer runs
there. `fetch-fixtures.sh` is the same thing for everything else; neither needs
anything that is not already on the machine.

The Open Fixture Library is **not** installed by this script since S61. It is
still read — a venue's own hand-written profiles are in that format — and
`tools/fetch-fixtures/fetch-ofl.sh` installs its corpus for anyone who wants it.

.PARAMETER Destination
Where to put it. Defaults to `profiles\fixtures\` beside this repository.

.PARAMETER Source
A directory of `.gdtf` files, or one archive of them, already on this machine.
Also `PRISMDMX_GDTF_SOURCE`.

A venue whose desk is not on a network installs from a stick, which is the
ordinary case in the buildings this is for.

.PARAMETER User
.PARAMETER Password
An account on <https://gdtf-share.com>, which is free and is the only way that
service hands the library out: there is no anonymous bulk download. Also
`PRISMDMX_GDTF_USER` and `PRISMDMX_GDTF_PASSWORD`.
#>
[CmdletBinding()]
param(
    [string] $Destination,
    [string] $Source = $env:PRISMDMX_GDTF_SOURCE,
    [string] $User = $env:PRISMDMX_GDTF_USER,
    [string] $Password = $env:PRISMDMX_GDTF_PASSWORD,
    [string] $Share = $(if ($env:PRISMDMX_GDTF_SHARE) { $env:PRISMDMX_GDTF_SHARE } else { 'https://gdtf-share.com/apis/public' })
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Destination) { $Destination = Join-Path $root 'profiles\fixtures' }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("prismdmx-gdtf-" + [System.Guid]::NewGuid().ToString('N'))
$staging = Join-Path $work 'fixtures'
New-Item -ItemType Directory -Path $staging -Force | Out-Null
try {
    if ($Source) {
        Write-Host "installing GDTF fixtures from $Source"
        if (Test-Path -Path $Source -PathType Container) {
            Get-ChildItem -Path $Source -Recurse -Filter '*.gdtf' |
                ForEach-Object { Copy-Item -Path $_.FullName -Destination $staging -Force }
        } else {
            $unpacked = Join-Path $work 'unpacked'
            New-Item -ItemType Directory -Path $unpacked -Force | Out-Null
            # `tar` ships with Windows 10 and later and reads both a .tar.gz
            # and a .zip.
            & tar --extract --file $Source --directory $unpacked
            if ($LASTEXITCODE -ne 0) { throw "tar failed with $LASTEXITCODE" }
            Get-ChildItem -Path $unpacked -Recurse -Filter '*.gdtf' |
                ForEach-Object { Copy-Item -Path $_.FullName -Destination $staging -Force }
        }
    } elseif ($User -and $Password) {
        Write-Host "fetching the GDTF library from $Share"
        # `Invoke-WebRequest` without the progress bar: on a slow link it spends
        # more time drawing than downloading.
        $progress = $ProgressPreference
        $ProgressPreference = 'SilentlyContinue'
        try {
            $session = $null
            Invoke-WebRequest -Uri "$Share/login.php" -Method Post `
                -Body @{ user = $User; password = $Password } `
                -SessionVariable session | Out-Null
            $list = Invoke-RestMethod -Uri "$Share/getList.php" -WebSession $session
            $rids = @($list.list | ForEach-Object { $_.rid } | Sort-Object -Unique)
            if ($rids.Count -eq 0) { throw 'the share returned no fixtures - check the account' }
            Write-Host "downloading $($rids.Count) fixtures"
            foreach ($rid in $rids) {
                $file = Join-Path $staging "$rid.gdtf"
                try {
                    Invoke-WebRequest -Uri "$Share/downloadFile.php?rid=$rid" `
                        -WebSession $session -OutFile $file
                } catch {
                    Write-Warning "  skipped $rid"
                    Remove-Item -Path $file -ErrorAction SilentlyContinue
                }
            }
        } finally {
            $ProgressPreference = $progress
        }
    } else {
        throw @'
No source for the fixture library.

  Set PRISMDMX_GDTF_SOURCE (or -Source) to a directory of .gdtf files, or to
  one archive of them, already on this machine; or

  set PRISMDMX_GDTF_USER and PRISMDMX_GDTF_PASSWORD (or -User and -Password)
  to an account on https://gdtf-share.com, which is free to create. That
  service has no anonymous bulk download, which is why this script cannot
  simply fetch it.

The desk starts without a library: it offers four built-in profiles and says
so. See profiles/fixtures/SOURCE.md.
'@
    }

    $found = @(Get-ChildItem -Path $staging -Filter '*.gdtf')
    if ($found.Count -eq 0) {
        throw 'no .gdtf files were found - the destination is left as it is'
    }

    # Only now is the destination touched. Everything above writes into a
    # temporary directory, so a download that failed half way leaves the library
    # that is already installed exactly where it was.
    if (-not (Test-Path $Destination)) { New-Item -ItemType Directory -Path $Destination | Out-Null }
    # Everything except the file that says where this came from, which is
    # committed and is not the library's.
    Get-ChildItem -Path $Destination -Force |
        Where-Object { $_.Name -ne 'SOURCE.md' } |
        Remove-Item -Recurse -Force

    Copy-Item -Path (Join-Path $staging '*') -Destination $Destination -Recurse -Force
    Write-Host "installed $($found.Count) GDTF fixtures into $Destination"
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
