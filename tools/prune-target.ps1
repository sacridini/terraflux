# Removes the stale builds of the workspace's crates from target/.
#
# The version of every crate is in the hash cargo gives its artifacts, so each release builds them all again under
# new names and nothing removes the old ones: target/ grew to hundreds of GB in a few days (57 copies of
# terraflux_graph, 174 build folders of terraflux-ls-sys). For every crate of the workspace and every kind of unit
# (lib, test, build script, its run), this keeps the newest build and those used up to -KeepHours before it, and
# removes the others' fingerprints, deps (rlib, exe, pdb), build folders and incremental caches. Other crates (from
# crates.io) change only with Cargo.lock and are left alone. A profile whose build is running (cargo's lock is
# held) is skipped.
#
#   powershell -NoProfile -File tools/prune-target.ps1            # prune and report
#   powershell -NoProfile -File tools/prune-target.ps1 -DryRun    # only report
#   powershell -NoProfile -File tools/prune-target.ps1 -Quiet     # report only when something was freed
param(
    [double]$KeepHours = 1,
    [switch]$DryRun,
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$target = Join-Path $root 'target'
if (-not (Test-Path $target)) { exit 0 }

# The workspace's packages (terraflux-ops) and their crate names (terraflux_ops).
$packages = @{}
foreach ($manifest in Get-ChildItem (Join-Path $root 'crates') -Filter Cargo.toml -Recurse -Depth 1) {
    $m = Select-String -Path $manifest.FullName -Pattern '^name\s*=\s*"([^"]+)"' | Select-Object -First 1
    if ($m) { $packages[$m.Matches[0].Groups[1].Value] = $true }
}
$crates = @{ 'build_script_build' = $true }
foreach ($p in $packages.Keys) { $crates[$p.Replace('-', '_')] = $true }

function Split-Hash([string]$name) {
    $i = $name.LastIndexOf('-')
    if ($i -lt 1) { return $null }
    return @($name.Substring(0, $i), $name.Substring($i + 1))
}

function Get-Size([string]$path) {
    if (Test-Path -LiteralPath $path -PathType Leaf) { return (Get-Item -LiteralPath $path).Length }
    $sum = 0L
    foreach ($f in [System.IO.Directory]::EnumerateFiles($path, '*', 'AllDirectories')) {
        $sum += ([System.IO.FileInfo]$f).Length
    }
    return $sum
}

$freed = 0L
$removed = 0
$failed = 0
function Remove-Stale([string]$path) {
    $size = Get-Size $path
    if (-not $DryRun) {
        try { Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction Stop }
        catch { $script:failed++; return }
    }
    $script:freed += $size
    $script:removed++
}

$window = [TimeSpan]::FromHours($KeepHours)

# Every profile folder: target/debug, target/release, target/<profile>, target/<triple>/<profile>.
$profiles = Get-ChildItem $target -Directory -Recurse -Depth 1 -Force |
    Where-Object { Test-Path (Join-Path $_.FullName '.fingerprint') }

foreach ($profile in $profiles) {
    $dir = $profile.FullName

    # Cargo holds this lock while it builds; holding it here makes a build that starts meanwhile wait.
    $lock = $null
    try {
        $lock = [System.IO.File]::Open((Join-Path $dir '.cargo-lock'), 'OpenOrCreate', 'ReadWrite', 'ReadWrite')
        $lock.Lock(0, [long]::MaxValue)
    } catch {
        if ($lock) { $lock.Dispose() }
        if (-not $Quiet) { Write-Output "skipped $($profile.Name): a build is running" }
        continue
    }

    try {
        # The workspace's units, grouped by package and kind; a unit's last use is its fingerprint's newest file.
        $groups = @{}
        $known = @{}
        foreach ($fp in Get-ChildItem (Join-Path $dir '.fingerprint') -Directory -Force) {
            $parts = Split-Hash $fp.Name
            if (-not $parts) { continue }
            $known[$parts[1]] = $true
            if (-not $packages.ContainsKey($parts[0])) { continue }
            $files = @(Get-ChildItem $fp.FullName -File -Force)
            $kind = ($files | Where-Object { $_.Extension -eq '.json' } | ForEach-Object Name | Sort-Object) -join ','
            $used = $fp.LastWriteTime
            foreach ($f in $files) { if ($f.LastWriteTime -gt $used) { $used = $f.LastWriteTime } }
            $key = "$($parts[0])|$kind"
            if (-not $groups.ContainsKey($key)) { $groups[$key] = @() }
            $groups[$key] += [pscustomobject]@{ Name = $fp.Name; Hash = $parts[1]; Used = $used; Path = $fp.FullName }
        }

        $stale = @{}
        foreach ($units in $groups.Values) {
            $newest = ($units | Measure-Object Used -Maximum).Maximum
            foreach ($u in $units) {
                if ($u.Used -lt $newest - $window) { $stale[$u.Hash] = $u.Name; Remove-Stale $u.Path }
            }
        }

        # Their deps, found by hash: an integration test's program is named after the test (app-<hash>.exe), not
        # the crate. A file whose hash has no fingerprint any more is left from a build cargo no longer knows.
        foreach ($sub in 'deps', 'examples') {
            $d = Join-Path $dir $sub
            if (-not (Test-Path $d)) { continue }
            foreach ($f in Get-ChildItem $d -File -Force) {
                $parts = Split-Hash ([System.IO.Path]::GetFileNameWithoutExtension($f.Name))
                if (-not $parts -or $parts[1] -notmatch '^[0-9a-f]{16}$') { continue }
                if ($stale.ContainsKey($parts[1]) -or -not $known.ContainsKey($parts[1])) { Remove-Stale $f.FullName }
            }
        }

        # Build scripts and their out folders (landschaft's CMake build is one of these).
        $b = Join-Path $dir 'build'
        if (Test-Path $b) {
            foreach ($bd in Get-ChildItem $b -Directory -Force) {
                $parts = Split-Hash $bd.Name
                if (-not $parts -or $parts[1] -notmatch '^[0-9a-f]{16}$') { continue }
                if ($stale.ContainsKey($parts[1]) -or -not $known.ContainsKey($parts[1])) { Remove-Stale $bd.FullName }
            }
        }

        # Incremental caches have hashes of their own: per crate, the newest and those used up to -KeepHours before.
        $inc = Join-Path $dir 'incremental'
        if (Test-Path $inc) {
            $byCrate = Get-ChildItem $inc -Directory -Force | Where-Object {
                $p = Split-Hash $_.Name; $p -and $crates.ContainsKey($p[0])
            } | Group-Object { (Split-Hash $_.Name)[0] }
            foreach ($g in $byCrate) {
                $newest = ($g.Group | Measure-Object LastWriteTime -Maximum).Maximum
                foreach ($d in $g.Group) { if ($d.LastWriteTime -lt $newest - $window) { Remove-Stale $d.FullName } }
            }
        }
    } finally {
        $lock.Dispose()
    }
}

if ($removed -gt 0 -or -not $Quiet) {
    $verb = if ($DryRun) { 'would free' } else { 'freed' }
    $note = if ($failed -gt 0) { " ($failed in use, left)" } else { '' }
    Write-Output ("prune-target: {0} {1:N1} GB in {2} stale builds{3}" -f $verb, ($freed / 1GB), $removed, $note)
}
