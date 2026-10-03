# Watches the source files and runs install.ps1 after each change, so the installed
# executable is always up to date. Stop it by ending its PowerShell process.
$root = Split-Path -Parent $PSScriptRoot
$install = Join-Path $PSScriptRoot "install.ps1"
$log = Join-Path $env:APPDATA "steam-search\watch-install.log"
New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null

$watcher = New-Object IO.FileSystemWatcher $root
$watcher.IncludeSubdirectories = $true
$watcher.NotifyFilter = [IO.NotifyFilters]"LastWrite, FileName"
$watched = '\\(src|src-tauri\\src|src-tauri\\icons|src-tauri\\capabilities)\\|\\(index\.html|package\.json|vite\.config\.ts|src-tauri\\Cargo\.toml|src-tauri\\tauri\.conf\.json)$'

while ($true) {
    $change = $watcher.WaitForChanged([IO.WatcherChangeTypes]::All)
    if (("\" + $change.Name) -notmatch $watched) { continue }
    # Let a burst of saves settle before building.
    do { $more = $watcher.WaitForChanged([IO.WatcherChangeTypes]::All, 2000) } until ($more.TimedOut)
    "$(Get-Date -Format s) change in $($change.Name), rebuilding" | Add-Content $log
    & powershell -NoProfile -ExecutionPolicy Bypass -File $install *>> $log
    "$(Get-Date -Format s) done (exit $LASTEXITCODE)" | Add-Content $log
}
