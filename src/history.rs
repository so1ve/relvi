use std::collections::BTreeMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::catalog::Entry;
use crate::store::JsonStore;

const MAX_LEARNED_QUERIES: usize = 256;
const PROMOTION_WINDOW: usize = 12;

pub struct History {
    entries: BTreeMap<String, Usage>,
    store: JsonStore<BTreeMap<String, Usage>>,
}

impl History {
    pub fn load() -> Self {
        let path = glib::user_state_dir().join("relvi/history.json");
        let store = JsonStore::new(path.clone());
        let entries = match store.load() {
            Ok(Some(entries)) => entries,
            Ok(None) => BTreeMap::new(),
            Err(error) => {
                eprintln!("Could not read {}: {error}", path.display());
                BTreeMap::new()
            }
        };

        Self { entries, store }
    }

    pub fn record(&mut self, id: &str, query: &str) {
        let now = glib::real_time();
        let usage = self.entries.entry(id.to_owned()).or_default();
        usage.count = usage.count.saturating_add(1);
        usage.last_used = now;

        let query = normalize_query(query);
        if !query.is_empty() {
            let learned = usage.queries.entry(query).or_default();
            learned.count = learned.count.saturating_add(1);
            learned.last_used = now;
            self.prune_queries();
        }

        self.store.save(self.entries.clone());
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.store.save(BTreeMap::new());
    }

    pub fn sort(&self, entries: &mut [Rc<Entry>], query: &str) {
        let query = normalize_query(query);
        let now = glib::real_time();

        if query.is_empty() {
            // Stable sorting retains the catalog's name order for equal ranks.
            entries.sort_by(|left, right| {
                let left = self.app_rank(left, now);
                let right = self.app_rank(right, now);

                right
                    .0
                    .total_cmp(&left.0)
                    .then_with(|| right.1.cmp(&left.1))
            });

            return;
        }

        // Learning only reorders good matches; polysearch alone chooses which
        // applications match and the rest keep their original ranking.
        let limit = entries.len().min(PROMOTION_WINDOW);
        entries[..limit].sort_by(|left, right| {
            let left = self.query_rank(left, &query, now);
            let right = self.query_rank(right, &query, now);

            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.total_cmp(&left.1))
                .then_with(|| right.2.cmp(&left.2))
        });
    }

    fn app_rank(&self, entry: &Entry, now: i64) -> (f64, i64) {
        entry
            .id()
            .and_then(|id| self.entries.get(id))
            .map(|usage| (score(usage.count, usage.last_used, now), usage.last_used))
            .unwrap_or((0.0, 0))
    }

    fn query_rank(&self, entry: &Entry, query: &str, now: i64) -> (u8, f64, i64) {
        let Some(usage) = entry.id().and_then(|id| self.entries.get(id)) else {
            return (0, 0.0, 0);
        };

        usage
            .queries
            .iter()
            .filter_map(|(learned, use_count)| {
                let strength = query_strength(query, learned);
                (strength != 0).then(|| {
                    (
                        strength,
                        score(use_count.count, use_count.last_used, now),
                        use_count.last_used,
                    )
                })
            })
            .max_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            })
            .unwrap_or((0, 0.0, 0))
    }

    fn prune_queries(&mut self) {
        let entries = &mut self.entries;

        while entries
            .values()
            .map(|usage| usage.queries.len())
            .sum::<usize>()
            > MAX_LEARNED_QUERIES
        {
            let oldest = entries
                .iter()
                .flat_map(|(id, usage)| {
                    usage
                        .queries
                        .iter()
                        .map(move |(query, learned)| (id, query, learned.last_used))
                })
                .min_by_key(|(_, _, last_used)| *last_used)
                .map(|(id, query, _)| (id.clone(), query.clone()))
                .unwrap();
            entries
                .get_mut(&oldest.0)
                .unwrap()
                .queries
                .remove(&oldest.1);
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Usage {
    count: u64,
    // Microseconds since the Unix epoch, retaining order for quick launches.
    last_used: i64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    queries: BTreeMap<String, QueryUsage>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct QueryUsage {
    count: u64,
    last_used: i64,
}

fn score(count: u64, last_used: i64, now: i64) -> f64 {
    let age = now.saturating_sub(last_used).max(0) as f64;

    // Frequency grows logarithmically and the score halves after a week.
    (count as f64).ln_1p() * (-age / (7.0 * 24.0 * 60.0 * 60.0 * 1_000_000.0)).exp2()
}

fn normalize_query(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn query_strength(query: &str, learned: &str) -> u8 {
    if query == learned {
        return 2;
    }

    let query_len = query.chars().count();
    let learned_len = learned.chars().count();
    if query_len >= 2
        && learned_len >= 2
        && query_len.abs_diff(learned_len) <= 2
        && (query.starts_with(learned) || learned.starts_with(query))
    {
        return 1;
    }

    0
}
