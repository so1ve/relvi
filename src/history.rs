use std::collections::BTreeMap;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};

use crate::catalog::Entry;

/// Launch statistics, loaded once and saved outside the GTK thread.
pub struct History {
    entries: BTreeMap<String, Usage>,
    writer: Option<(Sender<String>, JoinHandle<()>)>,
}

impl History {
    pub fn load(path: &Path) -> Self {
        let read = || -> io::Result<_> {
            let entries: BTreeMap<String, Usage> = match fs::read(path) {
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
        let history_path = path.to_owned();

        let writer = match thread::Builder::new()
            .name("relvi-history".into())
            .spawn(move || {
                for contents in receiver {
                    let directory = history_path.parent().unwrap();
                    if let Err(error) = DirBuilder::new()
                        .recursive(true)
                        .mode(0o700)
                        .create(directory)
                    {
                        eprintln!("Could not create {}: {error}", directory.display());
                        continue;
                    }

                    if let Err(error) = glib::file_set_contents_full(
                        &history_path,
                        contents.as_bytes(),
                        glib::FileSetContentsFlags::CONSISTENT,
                        0o600,
                    ) {
                        eprintln!("Could not save {}: {error}", history_path.display());
                    }
                }
            }) {
            Ok(thread) => Some((sender, thread)),
            Err(error) => {
                eprintln!(
                    "Could not start writer for {}: {error}; history will remain in memory",
                    path.display()
                );

                None
            }
        };

        Self { entries, writer }
    }

    pub fn record(&mut self, id: &str) {
        let usage = self.entries.entry(id.to_owned()).or_insert(Usage {
            count: 0,
            last_used: 0,
        });
        usage.count = usage.count.saturating_add(1);
        usage.last_used = glib::real_time();

        if let Some((sender, _)) = &self.writer {
            let contents = serde_json::to_string(&self.entries).unwrap();

            sender.send(contents).unwrap();
        }
    }

    pub fn sort(&self, entries: &mut [Rc<Entry>]) {
        let now = glib::real_time();
        let rank = |entry: &Entry| {
            entry
                .id()
                .and_then(|id| self.entries.get(id.as_str()))
                .map(|usage| (usage.score(now), usage.last_used))
                .unwrap_or((0.0, 0))
        };

        // Stable sorting retains the catalog's name order for equal ranks.
        entries.sort_by(|left, right| {
            let left = rank(left);
            let right = rank(right);

            right
                .0
                .total_cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
        });
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

#[derive(Deserialize, Serialize)]
struct Usage {
    count: u64,
    // Microseconds since the Unix epoch, retaining order for quick launches.
    last_used: i64,
}

impl Usage {
    fn score(&self, now: i64) -> f64 {
        let age = now.saturating_sub(self.last_used).max(0) as f64;

        // Logarithmic frequency prevents lifetime counts from dominating;
        // an application's score halves after a week without use.
        (self.count as f64).ln_1p() * (-age / (7.0 * 24.0 * 60.0 * 60.0 * 1_000_000.0)).exp2()
    }
}
