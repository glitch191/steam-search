# Builds the release executable and installs it in %LOCALAPPDATA%\Programs\steam-search\,
# replacing and restarting the running copy. The app registers itself to start with Windows.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$target = Join-Path $env:LOCALAPPDATA "Programs\steam-search"
$exe = Join-Path $target "steam-search.exe"

# Any running copy (installed or started from the build folder) locks the files and
# would take the single instance slot, so stop them all first.
Get-Process steam-search, steam_search -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

Push-Location $root
try {
    npx tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "build failed" }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Force $target | Out-Null
Copy-Item (Join-Path $root "src-tauri\target\release\steam-search.exe") $exe -Force
# Start it through Explorer so it runs as a normal desktop process. Started directly from a
# packaged app (such as a terminal inside one), its registry writes would be redirected to a
# private copy and the startup entry would not be visible to Windows.
Start-Process explorer.exe -ArgumentList "`"$exe`""
Write-Host "Installed and started $exe"
