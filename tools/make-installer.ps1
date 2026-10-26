# SPDX-License-Identifier: GPL-3.0-or-later
# Terraflux's Windows installer and portable zip, in target\installer:
#
#   terraflux-<version>-x64.msi            installs for the user, no administrator asked
#                                          (%LOCALAPPDATA%\Programs\Terraflux, Start menu, PATH)
#   terraflux-<version>-windows-x64.zip    the same files, to unpack anywhere
#
# Both hold terraflux.exe (the release build), the Visual C++ and OpenMP runtimes next to it (a clean Windows has
# neither; Microsoft lets them be deployed with the application) and GDAL in the folder `gdal`
# (tools\bundle-gdal.ps1). The MSI is built by WiX 5, a .NET tool of the repository (.config\dotnet-tools.json,
# fetched by `dotnet tool restore`).
#
#   tools\make-installer.ps1                       # GDAL from C:\Program Files\GDAL (GISInternals)
#   tools\make-installer.ps1 -Gdal C:\OSGeo4W\bin -SkipBuild
param(
    [string]$Gdal = "C:\Program Files\GDAL",
    [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
    if (-not $SkipBuild) {
        cargo build --release -p terraflux-cli
        if ($LASTEXITCODE) { throw "cargo build failed" }
    }
    $out = "target\installer"
    $stage = "$out\stage"
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item target\release\terraflux.exe, LICENSE, README.md, CHANGELOG.md $stage

    # GDAL, its data and plugins
    & "$PSScriptRoot\bundle-gdal.ps1" -From $Gdal -To "$stage\gdal"

    # the Visual C++ runtime and OpenMP of the newest Visual Studio, next to terraflux.exe
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { throw "no Visual Studio (vswhere.exe) to take the Visual C++ runtime from" }
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $redist = Get-ChildItem "$vs\VC\Redist\MSVC" -Directory |
        Where-Object { $_.Name -match '^\d+(\.\d+)+$' } |
        Sort-Object { [version]$_.Name } |
        Select-Object -Last 1
    if (-not $redist) { throw "no Visual C++ redistributable files in $vs\VC\Redist\MSVC" }
    foreach ($part in "CRT", "OpenMP") {
        $dir = Get-ChildItem "$($redist.FullName)\x64" -Directory -Filter "Microsoft.VC*.$part" | Select-Object -First 1
        if (-not $dir) { throw "no $part runtime in $($redist.FullName)\x64" }
        Copy-Item "$($dir.FullName)\*.dll" $stage
    }
    "Visual C++ runtime $($redist.Name)"

    # the portable zip
    $zip = "$out\terraflux-$version-windows-x64.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path "$stage\*" -DestinationPath $zip

    # the MSI
    dotnet tool restore
    if ($LASTEXITCODE) { throw "dotnet tool restore failed" }
    # its pages (WixUI) and the action that opens Terraflux at the end (WixShellExec)
    foreach ($ext in "WixToolset.UI.wixext/5.0.2", "WixToolset.Util.wixext/5.0.2") {
        dotnet wix extension add -g $ext
        if ($LASTEXITCODE) { throw "the WiX extension $ext could not be added" }
    }
    # the pages' pictures: Terraflux's icon on its grey, at the left of the first and last pages, at the right of
    # the banner of the others
    Add-Type -AssemblyName System.Drawing
    $logo = [System.Drawing.Image]::FromFile((Resolve-Path assets\logo\png\terraflux-icon-256.png).Path)
    function Write-Picture([int]$w, [int]$h, [int]$panel, [int]$size, [int]$x, [int]$y, [string]$path) {
        $bmp = New-Object System.Drawing.Bitmap $w, $h
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.Clear([System.Drawing.Color]::White)
        if ($panel -gt 0) {
            $grey = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(43, 43, 43))
            $g.FillRectangle($grey, 0, 0, $panel, $h)
        }
        $g.DrawImage($logo, $x, $y, $size, $size)
        $g.Dispose()
        $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Bmp)
        $bmp.Dispose()
    }
    $dialog = Join-Path (Resolve-Path $out).Path "dialog.bmp"
    $banner = Join-Path (Resolve-Path $out).Path "banner.bmp"
    Write-Picture 493 312 164 112 26 70 $dialog
    Write-Picture 493 58 0 44 441 7 $banner
    $logo.Dispose()
    $msi = "$out\terraflux-$version-x64.msi"
    $stageFull = (Resolve-Path $stage).Path
    $icon = (Resolve-Path assets\logo\terraflux.ico).Path
    dotnet wix build installer\terraflux.wxs -arch x64 -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
        -d "Version=$version" -d "Stage=$stageFull" -d "Icon=$icon" -d "Dialog=$dialog" -d "Banner=$banner" -o $msi
    if ($LASTEXITCODE) { throw "wix build failed" }
    foreach ($f in $msi, $zip) { "{0} ({1:N0} MB)" -f $f, ((Get-Item $f).Length / 1MB) }
}
finally {
    Pop-Location
}
