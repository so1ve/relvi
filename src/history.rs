use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::store::JsonStore;

const MAX_LEARNED_QUERIES: usize = 256;

#[derive(Clone, Copy, Default)]
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

#[derive(Clone, Default, Deserialize, Serialize)]
struct EntryUsage {
    #[serde(flatten)]
    total: Usage,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    queries: BTreeMap<String, Usage>,
}

impl EntryUsage {
    fn rank(&self, query: &str, now: i64) -> Rank {
        if query.is_empty() {
            return self.total.rank(0, now);
        }

        self.queries
            .iter()
            .filter_map(|(learned, usage)| {
                let strength = query_strength(query, learned);

                (strength != 0).then(|| usage.rank(strength, now))
            })
            .max_by(Rank::compare)
            .unwrap_or_default()
    }
}

fn normalize_query(query: &str) -> String {
    let mut normalized = String::with_capacity(query.len());

    for word in query.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }

        normalized.push_str(word);
    }

    normalized.to_lowercase()
}

pub struct History {
    entries: BTreeMap<String, EntryUsage>,
    store: JsonStore<BTreeMap<String, EntryUsage>>,
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

    pub fn comparator(
        &self,
        query: &str,
    ) -> impl FnMut(Option<&str>, Option<&str>) -> Ordering + '_ {
        let query = normalize_query(query);
        let now = glib::real_time();
        let mut ranks = BTreeMap::new();

        // Score each compared entry once per search. Unmatched
        // entries need no work, and the cache borrows IDs from the
        // loaded history.
        let mut rank = move |id: Option<&str>| {
            let Some((id, usage)) = id.and_then(|id| self.entries.get_key_value(id)) else {
                return Rank::default();
            };

            *ranks
                .entry(id.as_str())
                .or_insert_with(|| usage.rank(&query, now))
        };

        move |left, right| rank(right).compare(&rank(left))
    }

    fn prune_queries(&mut self) {
        let count: usize = self.entries.values().map(|usage| usage.queries.len()).sum();

        for _ in MAX_LEARNED_QUERIES..count {
            let oldest = self
                .entries
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
            self.entries
                .get_mut(&oldest.0)
                .unwrap()
                .queries
                .remove(&oldest.1);
        }
    }
}
