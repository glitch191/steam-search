//! Development-only demo library, used for the README screenshot.
//! Enabled with the STEAM_SEARCH_DEMO environment variable in debug builds only.
//! The app IDs are placeholders and do not match these games on Steam.

use crate::steam::Game;

const NAMES: [&str; 20] = [
    "Baldur's Gate 3",
    "Celeste",
    "Cyberpunk 2077",
    "Dark Souls III",
    "Disco Elysium",
    "Dead Cells",
    "Elden Ring",
    "Factorio",
    "Hades",
    "Hollow Knight",
    "Hollow Knight: Silksong",
    "Into the Breach",
    "Outer Wilds",
    "Portal 2",
    "Red Dead Redemption 2",
    "Shovel Knight: Treasure Trove",
    "Slay the Spire",
    "Stardew Valley",
    "Terraria",
    "The Witcher 3: Wild Hunt",
];

pub fn games() -> Vec<Game> {
    NAMES.iter().enumerate().map(|(i, n)| Game { appid: 900_000 + i as u32, name: n.to_string() }).collect()
}
