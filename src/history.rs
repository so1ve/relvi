use std::collections::BTreeMap;
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use crate::catalog::Entry;

/// Launch statistics, loaded once and saved outside the GTK thread.
pub struct History {
    entries: BTreeMap<String, Usage>,
    writer: Option<(Sender<String>, JoinHandle<()>)>,
}

impl History {
    pub fn load(path: &Path) -> Self {
        let read = || -> Result<BTreeMap<String, Usage>, glib::Error> {
            let file = glib::KeyFile::new();
            if let Err(error) = file.load_from_file(path, glib::KeyFileFlags::NONE) {
                return if error.matches(glib::FileError::Noent) {
                    Ok(BTreeMap::new())
                } else {
                    Err(error)
                };
            }

            let mut entries = BTreeMap::new();
            for group in file.groups().iter() {
                let id = file.string(group, "id")?;
                let count = file.uint64(group, "count")?;
                let last_used = file.int64(group, "last_used")?;
                if id.is_empty() || count == 0 || last_used < 0 || entries.contains_key(id.as_str())
                {
                    return Err(glib::Error::new(
                        glib::KeyFileError::InvalidValue,
                        "Invalid or duplicate launch history record",
                    ));
                }
                entries.insert(id.to_string(), Usage { count, last_used });
            }
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
                    let directory = history_path.parent().expect("history path has a parent");
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
            let file = glib::KeyFile::new();
            for (index, (id, usage)) in self.entries.iter().enumerate() {
                // IDs are string values so KeyFile handles escaping them.
                let group = index.to_string();
                file.set_string(&group, "id", id);
                file.set_uint64(&group, "count", usage.count);
                file.set_int64(&group, "last_used", usage.last_used);
            }
            sender
                .send(file.to_data().to_string())
                .expect("history writer stopped unexpectedly");
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
            writer.join().expect("history writer panicked");
        }
    }
}

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn successful_records_survive_restart_and_keep_ids_intact() {
        let directory =
            std::env::temp_dir().join(format!("relvi-history-{}", glib::uuid_string_random()));
        let path = directory.join("history.ini");
        let id = "org.example.[Editor]\\中文.desktop";
        {
            let mut history = History::load(&path);
            history.record(id);
            history.record("org.example.Browser.desktop");
            history.record(id);
        }
        let file = glib::KeyFile::new();
        file.load_from_file(&path, glib::KeyFileFlags::NONE)
            .unwrap();
        assert_eq!(file.groups().len(), 2);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        {
            let mut history = History::load(&path);
            assert_eq!(history.entries[id].count, 2);
            assert!(history.entries[id].last_used > 0);
            history.record(id);
        }
        let history = History::load(&path);
        assert_eq!(history.entries[id].count, 3);
        drop(history);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_history_is_preserved_and_recording_continues_in_memory() {
        let directory =
            std::env::temp_dir().join(format!("relvi-history-{}", glib::uuid_string_random()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("history.ini");
        for contents in [
            "broken history",
            "[0]\nid=editor.desktop\ncount=1\n",
            "[0]\nid=editor.desktop\ncount=0\nlast_used=1\n",
        ] {
            fs::write(&path, contents).unwrap();
            let mut history = History::load(&path);
            history.record("browser.desktop");
            assert_eq!(history.entries["browser.desktop"].count, 1);
            drop(history);
            assert_eq!(fs::read_to_string(&path).unwrap(), contents);
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_save_keeps_in_memory_counts() {
        let directory =
            std::env::temp_dir().join(format!("relvi-history-{}", glib::uuid_string_random()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("history.ini");
        let mut history = History::load(&path);
        // A directory in place of the file makes the atomic replacement fail.
        fs::create_dir(&path).unwrap();
        history.record("editor.desktop");
        assert_eq!(history.entries["editor.desktop"].count, 1);
        drop(history);
        assert!(path.is_dir());
        fs::remove_dir_all(directory).unwrap();
    }
}
