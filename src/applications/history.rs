use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::store::JsonStore;

const MAX_LEARNED_QUERIES: usize = 256;

pub struct History {
    entries: BTreeMap<String, AppUsage>,
    store: JsonStore<BTreeMap<String, AppUsage>>,
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
        usage.total.record(now);

        let query = normalize_query(query);
        if !query.is_empty() {
            usage.queries.entry(query).or_default().record(now);
            self.prune_queries();
        }

        self.store.save(self.entries.clone());
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.store.save(BTreeMap::new());
    }

    pub fn comparator(&self, query: &str) -> impl Fn(Option<&str>, Option<&str>) -> Ordering + '_ {
        let query = normalize_query(query);
        let now = glib::real_time();

        move |left, right| {
            let left = self.rank(left, &query, now);
            let right = self.rank(right, &query, now);

            right.compare(&left)
        }
    }

    fn rank(&self, id: Option<&str>, query: &str, now: i64) -> Rank {
        let Some(usage) = id.and_then(|id| self.entries.get(id)) else {
            return Rank::default();
        };

        if query.is_empty() {
            return usage.total.rank(0, now);
        }

        usage
            .queries
            .iter()
            .filter_map(|(learned, usage)| {
                let strength = query_strength(query, learned);

                (strength != 0).then(|| usage.rank(strength, now))
            })
            .max_by(Rank::compare)
            .unwrap_or_default()
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
struct AppUsage {
    #[serde(flatten)]
    total: Usage,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    queries: BTreeMap<String, Usage>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Usage {
    count: u64,
    // Microseconds since the Unix epoch, retaining order for quick launches.
    last_used: i64,
}

impl Usage {
    const fn record(&mut self, now: i64) {
        self.count = self.count.saturating_add(1);
        self.last_used = now;
    }

    fn rank(&self, strength: u8, now: i64) -> Rank {
        let age = now.saturating_sub(self.last_used).max(0) as f64;
        // Frequency grows logarithmically and the score halves after a week.
        let score =
            (self.count as f64).ln_1p() * (-age / (7.0 * 24.0 * 60.0 * 60.0 * 1_000_000.0)).exp2();

        Rank {
            strength,
            score,
            last_used: self.last_used,
        }
    }
}

#[derive(Default)]
struct Rank {
    strength: u8,
    score: f64,
    last_used: i64,
}

impl Rank {
    fn compare(&self, other: &Self) -> Ordering {
        self.strength
            .cmp(&other.strength)
            .then_with(|| self.score.total_cmp(&other.score))
            .then_with(|| self.last_used.cmp(&other.last_used))
    }
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
