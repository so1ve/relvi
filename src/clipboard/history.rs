use std::sync::Arc;

use gtk::glib;

use super::Entry;
use crate::store::ClipboardStore;

const MAX_ENTRIES: usize = 100;
const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;

pub struct History {
    entries: Vec<Arc<Entry>>,
    store: ClipboardStore,
}

impl History {
    pub fn load() -> Self {
        let path = glib::user_state_dir().join("relvi/clipboard.json");
        let store = ClipboardStore::new(path);
        let mut entries: Vec<Arc<Entry>> = Vec::new();
        let mut bytes = 0;

        match store.load() {
            Ok(saved) => {
                for entry in saved {
                    if entries.iter().any(|saved| saved.id == entry.id) {
                        continue;
                    }

                    bytes += entry.memory_size();
                    if bytes > MAX_HISTORY_BYTES {
                        break;
                    }

                    entries.push(entry);

                    if entries.len() == MAX_ENTRIES {
                        break;
                    }
                }
            }
            Err(error) => eprintln!("Could not load clipboard history: {error}"),
        }

        Self { entries, store }
    }

    pub fn entries(&self) -> &[Arc<Entry>] {
        &self.entries
    }

    pub fn record(&mut self, entry: Arc<Entry>) -> bool {
        if self
            .entries
            .first()
            .is_some_and(|first| first.id == entry.id)
        {
            return false;
        }

        self.entries.retain(|saved| saved.id != entry.id);
        self.entries.truncate(MAX_ENTRIES - 1);
        self.entries.insert(0, entry);

        let mut bytes = 0;
        self.entries.retain(|entry| {
            bytes += entry.memory_size();

            bytes <= MAX_HISTORY_BYTES
        });
        self.save();

        true
    }

    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|entry| entry.id != id);
        self.save();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.save();
    }

    pub fn search(&self, query: &str) -> impl Iterator<Item = &Arc<Entry>> {
        let query = query.to_lowercase();

        self.entries.iter().filter(move |entry| {
            query
                .split_whitespace()
                .all(|term| entry.searchable.contains(term))
        })
    }

    fn save(&mut self) {
        self.store.save(self.entries.clone());
    }
}
