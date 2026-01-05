use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::keepassxc::LoginEntry;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyData {
    pub entries: HashMap<String, FrecencyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrecencyEntry {
    pub count: u32,
    pub last_used: u64,
}

impl FrecencyData {
    pub fn record_use(&mut self, uuid: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let entry = self.entries.entry(uuid.to_string()).or_insert(FrecencyEntry {
            count: 0,
            last_used: now,
        });
        entry.count += 1;
        entry.last_used = now;
    }

    pub fn score(&self, uuid: &str) -> f64 {
        let Some(entry) = self.entries.get(uuid) else {
            return 0.0;
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let age_hours = (now - entry.last_used) as f64 / 3600.0;

        // Frecency: count * recency_decay
        // Recency decays by half every 24 hours
        let recency = 0.5_f64.powf(age_hours / 24.0);
        entry.count as f64 * recency
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: LoginEntry,
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
        entries: &[LoginEntry],
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
            // Search in name and login
            let search_text = format!("{} {}", entry.name, entry.login);
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
