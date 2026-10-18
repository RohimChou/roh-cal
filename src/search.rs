//! Fuzzy search, like fzy / fzf: the typed characters must appear in the text in the same order, with
//! anything in between (`sf` finds `Work > SFAP`); better matches (characters next to each other, at the
//! start of a word) come first. Matching is done by the nucleo-matcher crate (Helix's fuzzy matcher)

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// A text that matches: which one, and where the typed characters are in it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its index in the list that was searched
    pub index: usize,
    /// The positions of the matched characters (counted in characters from 0, in order), to draw them in
    /// another color
    pub positions: Vec<usize>,
}

/// The texts that `query` matches, best match first (equally good ones keep the list's order); an empty
/// `query` matches every text, in the list's order. Upper / lower case do not matter, and `e` also finds `é`
///
/// E.g. `sf` in [`Work`, `Life > Self-hosting`, `Work > SFAP`] → `Work > SFAP` (s and f next to each other
/// at the start of a word), then `Life > Self-hosting`; `Work` does not match
pub fn search(query: &str, texts: &[String]) -> Vec<Found> {
    if query.is_empty() {
        return (0..texts.len())
            .map(|index| Found {
                index,
                positions: Vec::new(),
            })
            .collect();
    }
    let atom = Atom::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
        false,
    );
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut found: Vec<(u16, Found)> = texts
        .iter()
        .enumerate()
        .filter_map(|(index, text)| {
            // One position per character (`Utf32Str::new` would join a letter and its accent mark into
            // one), so the positions count characters
            let chars: Vec<char> = text.chars().collect();
            let haystack = if text.is_ascii() {
                Utf32Str::Ascii(text.as_bytes())
            } else {
                Utf32Str::Unicode(&chars)
            };
            let mut positions = Vec::new();
            let score = atom.indices(haystack, &mut matcher, &mut positions)?;
            positions.sort_unstable();
            positions.dedup();
            let positions = positions.into_iter().map(|at| at as usize).collect();
            Some((score, Found { index, positions }))
        })
        .collect();
    // A stable sort: equally good matches keep the list's order
    found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    found.into_iter().map(|(_, found)| found).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts found (in order) and the positions matched in each
    fn found<'a>(query: &str, texts: &'a [String]) -> Vec<(&'a str, Vec<usize>)> {
        search(query, texts)
            .into_iter()
            .map(|found| (texts[found.index].as_str(), found.positions))
            .collect()
    }

    fn texts(texts: &[&str]) -> Vec<String> {
        texts.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_typed_characters_appear_in_order_and_the_best_match_comes_first() {
        let calendars = texts(&["Work", "Life > Self-hosting", "Work > SFAP"]);
        // s and f next to each other at the start of SFAP beat S…f in Self; Work has no s
        assert_eq!(
            found("sf", &calendars),
            [
                ("Work > SFAP", vec![7, 8]),
                ("Life > Self-hosting", vec![7, 10]),
            ]
        );
        // The characters must come in the typed order: `wf` finds Work > SFAP, `fw` nothing
        assert_eq!(found("wf", &calendars), [("Work > SFAP", vec![0, 8])]);
        assert_eq!(found("fw", &calendars), []);
        // Upper / lower case do not matter
        assert_eq!(
            found("SFAP", &calendars),
            [("Work > SFAP", vec![7, 8, 9, 10])]
        );
    }

    #[test]
    fn an_empty_query_lists_everything_and_ties_keep_the_order() {
        let calendars = texts(&["Work", "Life", "Work > SFAP"]);
        assert_eq!(
            found("", &calendars),
            [("Work", vec![]), ("Life", vec![]), ("Work > SFAP", vec![])]
        );
        // `wo` matches both Works equally well (at the start): the list's order
        assert_eq!(
            found("wo", &calendars),
            [("Work", vec![0, 1]), ("Work > SFAP", vec![0, 1])]
        );
    }

    #[test]
    fn positions_count_characters() {
        // Chinese characters (3 bytes each) and an accent: positions count characters, not bytes
        let calendars = texts(&["工作 > 專案A", "Café"]);
        assert_eq!(found("專A", &calendars), [("工作 > 專案A", vec![5, 7])]);
        assert_eq!(found("cafe", &calendars), [("Café", vec![0, 1, 2, 3])]);
    }
}
