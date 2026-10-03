//! Steam detection and installed game listing. All Steam files are read only.

use crate::vdf;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub appid: u32,
    pub name: String,
}

/// Steam manifest flag meaning "fully installed".
const STATE_FULLY_INSTALLED: u64 = 4;
const REDIST_APPID: u32 = 228980;
const EXCLUDED_PREFIXES: [&str; 3] = ["Proton", "Steam Linux Runtime", "Steamworks Common Redistributables"];

/// Returns the Steam installation folder: the configured override first, then the registry.
pub fn find_steam_path(override_path: &str) -> Option<PathBuf> {
    if !override_path.trim().is_empty() {
        let path = PathBuf::from(override_path.trim());
        return path.join("steamapps").is_dir().then_some(path);
    }
    registry_steam_path().filter(|p| p.join("steamapps").is_dir())
}

fn registry_steam_path() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let read = |root, key: &str, value: &str| -> Option<PathBuf> {
        let s: String = RegKey::predef(root).open_subkey(key).ok()?.get_value(value).ok()?;
        (!s.is_empty()).then(|| PathBuf::from(s.replace('/', "\\")))
    };
    read(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath")
        .or_else(|| read(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"))
}

/// Lists every library folder: the Steam folder itself plus those in `libraryfolders.vdf`.
pub fn library_paths(steam: &Path) -> Vec<PathBuf> {
    let mut paths = vec![steam.to_path_buf()];
    let file = steam.join("steamapps").join("libraryfolders.vdf");
    if let Some(root) = fs::read_to_string(&file).ok().and_then(|t| vdf::parse(&t).ok()) {
        if let Some(folders) = root.get("libraryfolders") {
            for (key, value) in folders.entries() {
                if key.parse::<u32>().is_err() {
                    continue;
                }
                // Current format: "0" { "path" "..." }. Older format: "1" "D:\\SteamLibrary".
                let path = match value {
                    vdf::Value::Obj(_) => value.str("path"),
                    vdf::Value::Str(s) => Some(s.as_str()),
                };
                if let Some(p) = path {
                    paths.push(PathBuf::from(p));
                }
            }
        }
    }
    let mut seen = Vec::new();
    paths.retain(|p| {
        let key = p.to_string_lossy().trim_end_matches('\\').to_lowercase();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    paths
}

/// Reads all fully installed games of one library.
pub fn scan_library(library: &Path) -> Vec<Game> {
    let steamapps = library.join("steamapps");
    let Ok(dir) = fs::read_dir(&steamapps) else {
        return Vec::new();
    };
    dir.flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.starts_with("appmanifest_") && name.ends_with(".acf")
        })
        .filter_map(|e| parse_manifest(&fs::read_to_string(e.path()).ok()?, &steamapps))
        .collect()
}

fn parse_manifest(text: &str, steamapps: &Path) -> Option<Game> {
    let root = vdf::parse(text).ok()?;
    let state = root.get("AppState")?;
    let appid: u32 = state.str("appid")?.trim().parse().ok()?;
    let name = state.str("name")?.trim().to_string();
    let installdir = state.str("installdir")?;
    let flags: u64 = state.str("StateFlags")?.trim().parse().ok()?;
    if flags & STATE_FULLY_INSTALLED == 0 || installdir.is_empty() || name.is_empty() {
        return None;
    }
    if !steamapps.join("common").join(installdir).is_dir() {
        return None;
    }
    if appid == REDIST_APPID || EXCLUDED_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return None;
    }
    Some(Game { appid, name })
}

/// Scans all libraries and returns installed games sorted by name, without duplicates.
pub fn installed_games(steam: &Path) -> Vec<Game> {
    let mut games: Vec<Game> = library_paths(steam).iter().flat_map(|l| scan_library(l)).collect();
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.appid.cmp(&b.appid)));
    games.dedup_by_key(|g| g.appid);
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

    /// Builds a fake Steam layout in a temp folder with two libraries.
    fn fake_steam(test: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("steam-search-test-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let steam = base.join("Steam");
        let second = base.join("SecondLibrary");
        for lib in [&steam, &second] {
            fs::create_dir_all(lib.join("steamapps").join("common")).unwrap();
        }
        let vdf = fs::read_to_string(Path::new(FIXTURES).join("libraryfolders.vdf"))
            .unwrap()
            .replace("{STEAM}", &steam.to_string_lossy().replace('\\', "\\\\"))
            .replace("{SECOND}", &second.to_string_lossy().replace('\\', "\\\\"));
        fs::write(steam.join("steamapps").join("libraryfolders.vdf"), vdf).unwrap();

        let copy = |lib: &Path, file: &str, installdir: Option<&str>| {
            fs::copy(Path::new(FIXTURES).join(file), lib.join("steamapps").join(file)).unwrap();
            if let Some(dir) = installdir {
                fs::create_dir_all(lib.join("steamapps").join("common").join(dir)).unwrap();
            }
        };
        copy(&steam, "appmanifest_1001.acf", Some("Lantern Valley"));
        copy(&steam, "appmanifest_1002.acf", Some("Updating Game")); // StateFlags without bit 4
        copy(&steam, "appmanifest_1003.acf", None); // folder missing
        copy(&steam, "appmanifest_228980.acf", Some("Steamworks Shared"));
        copy(&second, "appmanifest_2001.acf", Some("Copper Fields"));
        copy(&second, "appmanifest_2002.acf", Some("Proton 9.0"));
        (base, steam)
    }

    #[test]
    fn finds_all_libraries() {
        let (base, steam) = fake_steam("libs");
        let libs = library_paths(&steam);
        assert_eq!(libs.len(), 2, "{libs:?}");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn lists_only_fully_installed_games() {
        let (base, steam) = fake_steam("games");
        let names: Vec<String> = installed_games(&steam).into_iter().map(|g| g.name).collect();
        assert_eq!(names, vec!["Copper Fields", "Lantern Valley"]);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn override_path_must_contain_steamapps() {
        let (base, steam) = fake_steam("override");
        assert_eq!(find_steam_path(&steam.to_string_lossy()), Some(steam.clone()));
        assert_eq!(find_steam_path(&base.join("Nowhere").to_string_lossy()), None);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn reads_old_library_format() {
        let base = std::env::temp_dir().join(format!("steam-search-test-old-{}", std::process::id()));
        fs::create_dir_all(base.join("steamapps")).unwrap();
        fs::write(
            base.join("steamapps").join("libraryfolders.vdf"),
            "\"LibraryFolders\"\n{\n\t\"TimeNextStatsReport\"\t\"1\"\n\t\"1\"\t\"X:\\\\Games\\\\Library\"\n}\n",
        )
        .unwrap();
        let libs = library_paths(&base);
        assert_eq!(libs[1], PathBuf::from(r"X:\Games\Library"));
        fs::remove_dir_all(base).unwrap();
    }
}
