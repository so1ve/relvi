use std::collections::BTreeMap;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};

use crate::catalog::Entry;

const MAX_LEARNED_QUERIES: usize = 256;
const PROMOTION_WINDOW: usize = 12;

/// Launch statistics, loaded once and saved outside the GTK thread.
pub struct History {
    entries: BTreeMap<String, Usage>,
    writer: Option<(Sender<String>, JoinHandle<()>)>,
}

impl History {
    pub fn load() -> Self {
        let path = glib::user_state_dir().join("relvi/history.json");
        let read = || -> io::Result<_> {
            let entries = match fs::read(&path) {
                Ok(contents) => serde_json::from_slice(&contents)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
                Err(error) => return Err(error),
            };

            Ok(entries)
        };

        let entries = match read() {
            Ok(entries) => entries,
            Err(error) => {
                // Never overwrite unreadable history with an empty snapshot.
                eprintln!(
                    "Could not read {}: {error}; history will remain in memory",
                    path.display()
                );

                return Self {
                    entries: BTreeMap::new(),
                    writer: None,
                };
            }
        };

        let (sender, receiver) = mpsc::channel::<String>();
        let writer = match thread::Builder::new()
            .name("relvi-history".into())
            .spawn(move || {
                for contents in receiver {
                    let directory = path.parent().unwrap();
                    if let Err(error) = DirBuilder::new()
                        .recursive(true)
                        .mode(0o700)
                        .create(directory)
                    {
                        eprintln!("Could not create {}: {error}", directory.display());
                        continue;
                    }

                    if let Err(error) = glib::file_set_contents_full(
                        &path,
                        contents.as_bytes(),
                        glib::FileSetContentsFlags::CONSISTENT,
                        0o600,
                    ) {
                        eprintln!("Could not save {}: {error}", path.display());
                    }
                }
            }) {
            Ok(thread) => Some((sender, thread)),
            Err(error) => {
                eprintln!("Could not start history writer: {error}; history will remain in memory");

                None
            }
        };

        Self { entries, writer }
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

        self.save();
    }

    pub fn clear(&mut self) {
        self.entries.clear();

        if self.writer.is_some() {
            self.save();
        } else {
            let path = glib::user_state_dir().join("relvi/history.json");
            if let Err(error) = fs::remove_file(&path)
                && error.kind() != io::ErrorKind::NotFound
            {
                eprintln!("Could not clear {}: {error}", path.display());
            }
        }
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
        while self
            .entries
            .values()
            .map(|usage| usage.queries.len())
            .sum::<usize>()
            > MAX_LEARNED_QUERIES
        {
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

    fn save(&self) {
        if let Some((sender, _)) = &self.writer {
            let contents = serde_json::to_string(&self.entries).unwrap();

            sender.send(contents).unwrap();
        }
    }
}

impl Drop for History {
    fn drop(&mut self) {
        if let Some((sender, writer)) = self.writer.take() {
            drop(sender);
            writer.join().unwrap();
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
struct Usage {
    count: u64,
    // Microseconds since the Unix epoch, retaining order for quick launches.
    last_used: i64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    queries: BTreeMap<String, QueryUsage>,
}

#[derive(Default, Deserialize, Serialize)]
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
