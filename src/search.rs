use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::database::Entry;
use crate::frecency::FrecencyData;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: Entry,
    pub score: u32,
    pub frecency: f64,
}

pub struct Searcher {
    matcher: Matcher,
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    pub fn search(
        &mut self,
        query: &str,
        entries: &[Entry],
        frecency: &FrecencyData,
    ) -> Vec<SearchResult> {
        if query.is_empty() {
            // Return all entries sorted by frecency
            let mut results: Vec<_> = entries
                .iter()
                .map(|e| SearchResult {
                    entry: e.clone(),
                    score: 0,
                    frecency: frecency.score(&e.uuid),
                })
                .collect();
            results.sort_by(|a, b| b.frecency.partial_cmp(&a.frecency).unwrap());
            return results;
        }

        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut results = Vec::new();
        let mut buf = Vec::new();

        for entry in entries {
            // Search in title and username
            let search_text = format!("{} {}", entry.title, entry.username);
            let haystack = Utf32Str::new(&search_text, &mut buf);

            if let Some(score) = pattern.score(haystack, &mut self.matcher) {
                results.push(SearchResult {
                    entry: entry.clone(),
                    score,
                    frecency: frecency.score(&entry.uuid),
                });
            }
            buf.clear();
        }

        // Sort by: fuzzy score (primary) + frecency boost
        results.sort_by(|a, b| {
            let a_combined = a.score as f64 + a.frecency * 10.0;
            let b_combined = b.score as f64 + b.frecency * 10.0;
            b_combined.partial_cmp(&a_combined).unwrap()
        });

        results
    }
}
