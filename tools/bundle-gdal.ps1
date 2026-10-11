# Put a GDAL next to terraflux.exe, in the folder `gdal`, as the installers bring it: its libraries, its data
# (GDAL's and PROJ's) and its plugins. Terraflux loads it when it runs (see crates/gdal).
#
#   tools\bundle-gdal.ps1                                   # from C:\Program Files\GDAL (GISInternals)
#   tools\bundle-gdal.ps1 -From C:\OSGeo4W\bin -To dist\gdal
param(
    [string]$From = "C:\Program Files\GDAL",
    [string]$To = "target\release\gdal"
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path "$From")) { throw "no GDAL in $From" }
New-Item -ItemType Directory -Force $To | Out-Null
Copy-Item "$From\*.dll" $To
# the certificates of the web (basemaps over HTTPS)
if (Test-Path "$From\curl-ca-bundle.crt") { Copy-Item "$From\curl-ca-bundle.crt" $To }
# the data and plugins, where Terraflux looks for them next to the library
foreach ($d in "gdal-data", "gdalplugins", "projlib") {
    if (Test-Path "$From\$d") { Copy-Item -Recurse -Force "$From\$d" "$To" }
}
# OSGeo4W keeps the data under share\
$share = Join-Path (Split-Path $From) "share"
if (Test-Path "$share\gdal") { Copy-Item -Recurse -Force "$share\gdal" "$To\gdal-data" }
if (Test-Path "$share\proj") { Copy-Item -Recurse -Force "$share\proj" "$To\projlib" }
$size = (Get-ChildItem -Recurse $To | Measure-Object -Sum Length).Sum / 1MB
"GDAL copied to $To ({0:N0} MB)" -f $size
