# steam-search

Lightweight Windows tray launcher for installed Steam games. Press `Ctrl+Shift+Insert`, type a few
letters, press Enter.

![Search window showing results for the query "knight", the first one selected](docs/screenshots/search-window.png)

*The search window with the query "knight" (development demo mode).*

## Features

- Opens just above the system tray icon, on the monitor that holds the taskbar (below it when the
  taskbar is at the top of the screen). The window grows upward as results appear.
- Starts with Windows by default (can be turned off in **Settings**).
- Scans every Steam library on every drive at startup, then again in the background each time the
  window opens. Results appear from the cache immediately.
- Lists only fully installed games; Steam tools (Proton, Steam Linux Runtime, Steamworks Common
  Redistributables) are ignored.
- Tolerant search: initials (`hk`, `bg3`), words in any order, case and accents ignored, simple typos.
- Keyboard only: Up and Down move the selection, Enter launches, Escape closes. Clicking a row also
  launches the game. The window closes when it loses focus.
- Single instance: starting the program again opens the search window of the running instance.

## Requirements

- Windows 10 or 11 (64-bit), with Steam installed.
- Microsoft Edge WebView2 Runtime (included in Windows 11, usually present on Windows 10).

## Download

Get `steam-search-<version>-windows.exe` and its `.sha256` file from the **Releases** page. To check
the download, compare the file content with:

```powershell
(Get-FileHash .\steam-search-<version>-windows.exe -Algorithm SHA256).Hash.ToLower()
```

The executable is not code-signed, so Windows SmartScreen may warn on first launch ("More info",
then "Run anyway"). There is no installer: put the file in a folder of your choice and run it.

## Settings

Right-click the tray icon and choose **Settings...**. Changes apply and are saved immediately; close
the window when done.

- **General**: start with Windows, the keyboard shortcut, the maximum number of results and the
  Steam folder (only needed if automatic detection picks the wrong folder or none; the page shows
  which folder is in use).
- **Font**: font and size of the game list, chosen from the installed fonts and a list of sizes as in
  a word processor, with a preview using your own game names. **Reset to default** returns to
  Segoe UI, 16.

To change the shortcut, click **Change...** next to it and press the new combination (at least one
modifier, or a function key alone). If another application already uses it, the page says so and
waits for another combination. Escape, or clicking elsewhere, cancels and keeps the current one.
While steam-search runs, the shortcut is captured globally and no longer reaches other applications.

### Configuration file

The settings are stored in `%APPDATA%\steam-search\config.json`, created on first launch. The file is
read when the program starts, so edit it by hand only with steam-search closed. Invalid values fall
back to their default. Log file: `%APPDATA%\steam-search\steam-search.log`.

```json
{
  "hotkey": "Ctrl+Shift+Insert",
  "maxResults": 50,
  "steamPath": "",
  "startWithWindows": true,
  "fontFamily": "",
  "fontSize": 16
}
```

| Key | Default | Meaning |
|---|---|---|
| `hotkey` | `Ctrl+Shift+Insert` | Global shortcut that opens or closes the window. Modifiers: `Ctrl`, `Shift`, `Alt`, `Win`. `Ins` is accepted for `Insert`. |
| `maxResults` | `50` | Maximum number of results (1-500). Eight rows are visible, the rest scrolls. |
| `steamPath` | empty | Steam folder, only needed if automatic detection fails (JSON escaping: `"D:\\Steam"`). |
| `startWithWindows` | `true` | Starts the program at sign-in (current user's `Run` registry entry). It appears in Settings > Apps > Startup and in Task Manager, where it can also be turned off; that choice is kept. |
| `fontFamily` | empty | Font of the game list, as one installed font name (`"Consolas"`). Empty, or a font that is not installed, uses Segoe UI. |
| `fontSize` | `16` | Font size of the game list in pixels at 100 % display scaling (8-40). Rows grow with it; Windows display scaling still applies on top. |

## Building and installing from source

Prerequisites: Node.js 24 (`.nvmrc`), Rust (`rust-toolchain.toml`, installed by rustup) and the
Microsoft C++ Build Tools.

```powershell
npm ci
powershell -ExecutionPolicy Bypass -File scripts\install.ps1
```

`scripts\install.ps1` stops any running copy, builds the release executable
(`npx tauri build --no-bundle`), copies it to `%LOCALAPPDATA%\Programs\steam-search\` and starts it.

To rebuild and reinstall automatically after every source change, run the watcher (it keeps
running in the background; its output goes to `%APPDATA%\steam-search\watch-install.log`):

```powershell
Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','scripts\watch-install.ps1'
```

Tests: `cargo test` in `src-tauri`. Development mode: `npx tauri dev`; with `STEAM_SEARCH_DEMO=1` it
shows a fixed demo list instead of your library (debug builds only).

The GitHub workflow `.github/workflows/release.yml` builds the executable on `windows-latest` and
attaches it, with its SHA-256 file, to each published release. It can also be run by hand from the
Actions tab ("Release build"), in which case the files become a workflow artifact.

## Project structure

```
index.html, src/main.ts   search window (TypeScript, no framework, Vite)
settings.html, src/settings.ts   settings window
src-tauri/src/main.rs     tray, window placement, hotkey, settings, commands
src-tauri/src/fonts.rs    installed font families (DirectWrite)
src-tauri/src/steam.rs    Steam detection and installed games
src-tauri/src/vdf.rs      parser for .vdf and .acf files
src-tauri/src/search.rs   fuzzy search and typo fallback
src-tauri/src/config.rs   config.json and log file
scripts/                  install and watch scripts
```

## Resource usage

Measured on Windows 11, release build, about 50 installed games: executable 3.7 MB; hotkey to
focused search field 16-22 ms; idle memory 5.5 MB for the main process and 147 MB including the
WebView2 processes; idle CPU over 60 s: 0 s for the main process, about 0.05 s for WebView2. No timer
or loop runs while the window is hidden.

## License

MIT, see [LICENSE](LICENSE). Third-party licenses: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
