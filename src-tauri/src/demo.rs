//! Development-only demo library with invented game names, used for screenshots.
//! Enabled with the STEAM_SEARCH_DEMO environment variable in debug builds only.

use crate::steam::Game;

const NAMES: [&str; 24] = [
    "Ashen Lighthouse",
    "Baldric's Gate 3",
    "Brass Orchard",
    "Cinder Rail Tycoon",
    "Copper Fields",
    "Deep Hollow Miners",
    "Ember Knights Arena",
    "Frostline Expedition",
    "Glasswing Courier",
    "Harbor Lights Simulator",
    "Hexa Kart Rally",
    "Iron Meadow",
    "Kestrel Squadron",
    "Lantern Valley",
    "Moonlit Knight Tales",
    "Night Harbor",
    "Pale Knight Requiem",
    "Quarry Kings",
    "Red Canyon Outlaws",
    "Starfall Tactics",
    "The Hollow Knight Chronicle",
    "Tidewater Farm",
    "Velvet Circuit",
    "Woodland Knight Saga",
];

pub fn games() -> Vec<Game> {
    NAMES.iter().enumerate().map(|(i, n)| Game { appid: 900_000 + i as u32, name: n.to_string() }).collect()
}
