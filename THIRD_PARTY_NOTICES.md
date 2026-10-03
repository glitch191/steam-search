# Third-party notices

steam-search is released under the MIT License (see `LICENSE`). The compiled executable includes
code from the third-party packages below. Their license texts are distributed with each package
and can be read on crates.io or npmjs.com.

## Direct dependencies

| Package | License | Source |
|---|---|---|
| tauri, tauri-build | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |
| tauri-plugin-global-shortcut | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-single-instance | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-opener | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| nucleo-matcher | MPL-2.0 | https://github.com/helix-editor/nucleo |
| strsim | MIT | https://github.com/rapidfuzz/strsim-rs |
| winreg | MIT | https://github.com/gentoo90/winreg-rs |
| serde, serde_json | MIT OR Apache-2.0 | https://github.com/serde-rs |
| @tauri-apps/api | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |

Build tools (not shipped in the executable): @tauri-apps/cli (MIT OR Apache-2.0),
Vite (MIT), TypeScript (Apache-2.0).

## MPL-2.0 components

`nucleo-matcher`, and `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext` and `selectors`
(pulled in by Tauri), are licensed under the Mozilla Public License 2.0. They are used unmodified. Their
source code is available from crates.io (https://crates.io/crates/nucleo-matcher and the
corresponding crate pages) and from the repositories linked above. The MPL-2.0 text is at
https://mozilla.org/MPL/2.0/.

## Transitive dependencies

All other transitive Rust dependencies are under permissive licenses: MIT, Apache-2.0, BSD-3-Clause,
ISC, Zlib, Unicode-3.0, Unlicense, 0BSD or CC0-1.0 (most offer a choice that includes MIT). The full
list with versions is in `src-tauri/Cargo.lock` and can be printed with `cargo metadata`.
