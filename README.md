# steam-search

A lightweight Windows launcher for the Steam games installed on your PC. It lives in the system
tray. Press `Ctrl+Shift+Insert` (or click the tray icon), type a few letters, pick a game with the
arrow keys and press Enter: the game starts through Steam.

![Search window showing five results for the query "knight", the first one selected](docs/screenshots/search-window.png)

*The search window with the query "knight". All game names in this screenshot are fictional
(development demo mode).*

## Features

- Finds every Steam library on every drive (registry plus `libraryfolders.vdf`), with no setup.
- Lists only fully installed games. Owned but not installed games and Steam tools (Proton,
  Steam Linux Runtime, Steamworks Common Redistributables) are ignored.
- Tolerant search: initials (`hk`, `bg3`), words in any order, case and accents ignored, and a
  fallback for simple typos (swapped or extra letters).
- Keyboard only: Up and Down move the selection, Enter launches, Escape closes. A click on a row
  also launches the game.
- The library is rescanned in the background each time the window opens; results appear from the
  cache immediately.
- Single instance: starting the program again opens the search window of the running instance.

## Requirements

- Windows 10 or 11 (64-bit).
- Steam installed.
- Microsoft Edge WebView2 Runtime. It is part of Windows 11. On Windows 10 it is usually already
  installed; if not, get the Evergreen runtime from Microsoft.

## Download

1. Open the **Releases** page of this repository.
2. Download `steam-search-<version>-windows.exe` and the matching `.sha256` file.
3. Optional, verify the checksum in PowerShell and compare it with the content of the `.sha256` file:

   ```powershell
   (Get-FileHash .\steam-search-<version>-windows.exe -Algorithm SHA256).Hash.ToLower()
   ```

The executable is not code-signed, so Windows SmartScreen may show a warning on first launch.
Choose "More info" then "Run anyway".

## Installation and launch

There is no installer. Put the executable in a folder of your choice (for example
`%LOCALAPPDATA%\Programs\steam-search\`) and run it. A magnifier icon appears in the tray.
To start it with Windows, tick **Start with Windows** in the tray menu.

Tray menu: Open search, Rescan library, Start with Windows, Open config file, Reload settings,
the version number, Quit.

## Configuration

Settings are stored in `%APPDATA%\steam-search\config.json`, created with default values on first
launch. Open it with **Open config file**, edit it, then choose **Reload settings** (no restart needed).

```json
{
  "hotkey": "Ctrl+Shift+Insert",
  "maxResults": 50,
  "steamPath": ""
}
```

| Key | Default | Meaning |
|---|---|---|
| `hotkey` | `Ctrl+Shift+Insert` | Global shortcut that opens or closes the window. Modifiers: `Ctrl`, `Shift`, `Alt`, `Win`. Keys: letters, digits, `F1`-`F24`, `Insert` (or `Ins`), `Space`, etc. Example: `Alt+F2`. |
| `maxResults` | `50` | Maximum number of results (1-500). Eight rows are visible, the rest scrolls. |
| `steamPath` | empty | Steam installation folder, only needed if automatic detection fails. Example: `D:\\Steam`. |

While steam-search runs, the hotkey is captured globally: it no longer reaches other applications.
If another application already uses it, the tray tooltip says so and the log records it; pick
another combination in `config.json`. An invalid value is replaced by its default (and logged)
instead of stopping the program.

The log file is `%APPDATA%\steam-search\steam-search.log`.

## Building from source

Prerequisites: Node.js 24 (see `.nvmrc`), Rust (the version in `rust-toolchain.toml` is installed
automatically by rustup), and the Tauri prerequisites for Windows (Microsoft C++ Build Tools).

```powershell
npm ci
npx tauri build --no-bundle
```

The executable is written to `src-tauri\target\release\steam-search.exe`. Run the tests with
`cargo test` in `src-tauri`. For development, `npx tauri dev` starts the app with live reload;
setting the environment variable `STEAM_SEARCH_DEMO=1` before it loads a list of invented games
(debug builds only, absent from release builds).

### Release workflow

`.github/workflows/release.yml` runs on `windows-latest` when a GitHub release is published: it
runs the tests, builds the executable, and attaches `steam-search-<tag>-windows.exe` and its
SHA-256 file to the release. It can also be started by hand (Actions tab, "Release build",
"Run workflow"); the files are then available as a workflow artifact.

## Project structure

```
index.html, src/main.ts, src/style.css   search window (TypeScript, no framework, Vite)
src-tauri/src/main.rs                    tray, window, hotkey, commands
src-tauri/src/steam.rs                   Steam detection and installed games
src-tauri/src/vdf.rs                     parser for .vdf and .acf files
src-tauri/src/search.rs                  fuzzy search and typo fallback
src-tauri/src/config.rs                  config.json and log file
src-tauri/src/autostart.rs               Start with Windows (HKCU Run key)
src-tauri/src/demo.rs                    fictional games for development screenshots
src-tauri/tests/fixtures/                fictional Steam files used by the tests
```

Test data and screenshots use invented game names only.

## Resource usage

Measured on Windows 11 with a release build and a library of about 50 installed games:

| Measure | Result |
|---|---|
| Executable size | 3.7 MB |
| Hotkey to search field focused (warm) | 16-22 ms (logged by the app on each opening) |
| Idle memory, main process | 5.5 MB private |
| Idle memory, including WebView2 processes | 147 MB private (347 MB working set) |
| Idle CPU over 60 s | 0 s for the main process, about 0.05 s in total for the WebView2 processes |

When the window is hidden, there is no timer or loop: the program waits for the hotkey or a tray
click. Library scans run on a separate thread.

## License

MIT, see [LICENSE](LICENSE). Third-party licenses are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
