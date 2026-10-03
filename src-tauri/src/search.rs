//! Game name matching: fuzzy subsequence search with an edit distance fallback for typos.

use crate::steam::Game;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{chars, Config, Matcher, Utf32Str};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub appid: u32,
    pub name: String,
    /// Character positions (not bytes) to show in bold.
    pub indices: Vec<u32>,
}

pub struct Searcher {
    matcher: Matcher,
}

impl Searcher {
    pub fn new() -> Self {
        // DEFAULT already lowercases and strips diacritics on the haystack side.
        // match_paths() is not used: game names are not paths.
        Self { matcher: Matcher::new(Config::DEFAULT) }
    }

    pub fn search(&mut self, games: &[Game], query: &str, limit: usize) -> Vec<Hit> {
        let query = fold(query);
        if query.trim().is_empty() {
            return Vec::new();
        }
        let mut hits = self.fuzzy(games, &query);
        if hits.is_empty() {
            hits = typo_fallback(games, &query);
        }
        hits.sort_by(|(sa, a), (sb, b)| {
            sb.cmp(sa).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        hits.into_iter().take(limit).map(|(_, h)| h).collect()
    }

    /// Each whitespace separated word is an independent fuzzy atom, so word order does not matter.
    fn fuzzy(&mut self, games: &[Game], query: &str) -> Vec<(u32, Hit)> {
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        let mut indices = Vec::new();
        games
            .iter()
            .filter_map(|g| {
                indices.clear();
                let haystack = Utf32Str::new(&g.name, &mut buf);
                let score = pattern.indices(haystack, &mut self.matcher, &mut indices)?;
                indices.sort_unstable();
                indices.dedup();
                Some((score, Hit { appid: g.appid, name: g.name.clone(), indices: indices.clone() }))
            })
            .collect()
    }
}

/// Lowercases and removes diacritics, so "Pokémon" and "pokemon" compare equal.
fn fold(s: &str) -> String {
    s.chars().map(|c| chars::normalize(c)).flat_map(char::to_lowercase).collect()
}

/// Splits a name into words with the char index where each word starts.
fn words(name: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut start = 0;
    for (i, c) in name.chars().enumerate() {
        if c.is_alphanumeric() {
            if current.is_empty() {
                start = i;
            }
            current.push(c);
        } else if !current.is_empty() {
            out.push((start, fold(&std::mem::take(&mut current))));
        }
    }
    if !current.is_empty() {
        out.push((start, fold(&current)));
    }
    out
}

fn allowed_distance(len: usize) -> usize {
    match len {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    }
}

/// Every query word must match a name word by prefix or within the allowed OSA distance.
/// The distance is checked against the whole name word and against its prefix of the same
/// length, so a typo in a partly typed word still matches.
fn typo_fallback(games: &[Game], query: &str) -> Vec<(u32, Hit)> {
    let terms: Vec<&str> = query.split_whitespace().collect();
    games
        .iter()
        .filter_map(|g| {
            let name_words = words(&g.name);
            let mut score = 0;
            let mut indices = Vec::new();
            for term in &terms {
                let max = allowed_distance(term.chars().count());
                let best = name_words
                    .iter()
                    .filter_map(|(start, word)| {
                        let prefix: String = word.chars().take(term.chars().count()).collect();
                        let d = if word.starts_with(term) {
                            0
                        } else {
                            strsim::osa_distance(term, word).min(strsim::osa_distance(term, &prefix))
                        };
                        (d <= max).then_some((d, *start, word.chars().count()))
                    })
                    .min_by_key(|(d, _, _)| *d)?;
                let (d, start, len) = best;
                score += (3 - d.min(3)) as u32;
                indices.extend((start..start + len).map(|i| i as u32));
            }
            indices.sort_unstable();
            indices.dedup();
            Some((score, Hit { appid: g.appid, name: g.name.clone(), indices }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fictional game names only.
    fn library() -> Vec<Game> {
        [
            "The Hollow Knight Chronicle",
            "Baldric's Gate 3",
            "Hexa Kart",
            "Night Harbor",
            "Pokémon Garden",
            "Copper Fields",
            "Lantern Valley",
            "Halo Keepers",
            "Starfall Tactics",
        ]
        .iter()
        .enumerate()
        .map(|(i, n)| Game { appid: 100 + i as u32, name: n.to_string() })
        .collect()
    }

    fn top(query: &str, n: usize) -> Vec<String> {
        Searcher::new().search(&library(), query, n).into_iter().map(|h| h.name).collect()
    }

    #[test]
    fn empty_query_returns_nothing() {
        assert!(top("", 8).is_empty());
        assert!(top("   ", 8).is_empty());
    }

    #[test]
    fn initials_match_word_starts() {
        assert!(top("hk", 3).contains(&"The Hollow Knight Chronicle".to_string()), "{:?}", top("hk", 8));
        assert_eq!(top("bg3", 1), vec!["Baldric's Gate 3"]);
    }

    #[test]
    fn typo_falls_back_to_edit_distance() {
        assert_eq!(top("hollow knigth", 1), vec!["The Hollow Knight Chronicle"]);
        assert_eq!(top("lantren", 1), vec!["Lantern Valley"]);
    }

    #[test]
    fn word_order_does_not_matter() {
        assert_eq!(top("knight hollow", 1), vec!["The Hollow Knight Chronicle"]);
    }

    #[test]
    fn case_and_accents_are_ignored() {
        assert_eq!(top("POKEMON", 1), vec!["Pokémon Garden"]);
        assert_eq!(top("pokémon", 1), vec!["Pokémon Garden"]);
        assert_eq!(top("Cöpper", 1), vec!["Copper Fields"]);
    }

    #[test]
    fn no_match_returns_empty() {
        assert!(top("zzzz", 8).is_empty());
    }

    #[test]
    fn limit_is_respected_and_ties_are_alphabetical() {
        let games: Vec<Game> = ["Delta B", "Alpha B", "Charlie B"]
            .iter()
            .enumerate()
            .map(|(i, n)| Game { appid: i as u32, name: n.to_string() })
            .collect();
        let names: Vec<String> = Searcher::new().search(&games, "b", 2).into_iter().map(|h| h.name).collect();
        assert_eq!(names, vec!["Alpha B", "Charlie B"]);
    }

    #[test]
    fn indices_are_char_positions() {
        let hits = Searcher::new().search(&library(), "pok", 1);
        assert_eq!(hits[0].indices, vec![0, 1, 2]);
    }
}
