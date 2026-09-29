use std::sync::Arc;
use std::thread;
use std::time::Duration;

use wl_clipboard_watch::{Config, Event, Transfer, Watcher};

use crate::store::JsonStore;

const MAX_ENTRIES: usize = 100;
const MAX_TEXT_BYTES: usize = 64 * 1024;

pub struct Entry {
    pub text: Arc<str>,
    pub preview: String,
    searchable: String,
}

impl Entry {
    fn new(text: Arc<str>) -> Option<Self> {
        if text.len() > MAX_TEXT_BYTES || text.trim().is_empty() || text.contains('\0') {
            return None;
        }

        let preview = text
            .split_whitespace()
            .flat_map(|word| word.chars().chain(std::iter::once(' ')))
            .take(160)
            .collect::<String>();
        let searchable = text.to_lowercase();

        Some(Self {
            text,
            preview,
            searchable,
        })
    }
}

pub struct History {
    entries: Vec<Arc<Entry>>,
    store: JsonStore<Vec<Arc<str>>>,
}

impl History {
    pub fn load() -> Self {
        let store =
            JsonStore::<Vec<Arc<str>>>::new(glib::user_state_dir().join("relvi/clipboard.json"));
        let saved = match store.load() {
            Ok(entries) => entries.unwrap_or_default(),
            Err(error) => {
                eprintln!("Could not load clipboard history: {error}");
                Vec::new()
            }
        };
        let mut entries: Vec<Arc<Entry>> = Vec::new();

        for text in saved.into_iter().take(MAX_ENTRIES) {
            if !entries.iter().any(|entry| entry.text == text)
                && let Some(entry) = Entry::new(text)
            {
                entries.push(Arc::new(entry));
            }
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
            .is_some_and(|first| first.text == entry.text)
        {
            return false;
        }

        self.entries.retain(|previous| previous.text != entry.text);
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_ENTRIES);
        self.save();

        true
    }

    pub fn remove(&mut self, text: &str) {
        self.entries.retain(|entry| entry.text.as_ref() != text);
        self.save();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.save();
    }

    pub fn search(&self, query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        let terms: Vec<_> = query.split_whitespace().collect();

        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| terms.iter().all(|term| entry.searchable.contains(term)))
            .map(|(index, _)| index)
            .collect()
    }

    fn save(&mut self) {
        self.store.save(
            self.entries
                .iter()
                .map(|entry| Arc::clone(&entry.text))
                .collect(),
        );
    }
}

pub fn watch() -> async_channel::Receiver<Result<Arc<Entry>, String>> {
    let (updates, events) = async_channel::bounded(8);

    thread::spawn(move || {
        let config = Config::new(MAX_TEXT_BYTES, Duration::from_secs(2)).unwrap();
        let mut watcher = match Watcher::connect_with(config) {
            Ok(watcher) => watcher,
            Err(error) => {
                let _ = updates.send_blocking(Err(error.to_string()));

                return;
            }
        };

        while !updates.is_closed() {
            let selection = match watcher.next_event() {
                Ok(Event::Selection(selection)) => selection,
                Ok(Event::Cleared) => continue,
                Err(error) => {
                    let _ = updates.send_blocking(Err(error.to_string()));
                    break;
                }
            };

            if selection.offers("x-kde-passwordManagerHint") {
                continue;
            }

            let mime_type = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"]
                .into_iter()
                .find_map(|preferred| {
                    selection
                        .mime_types()
                        .iter()
                        .find(|mime| mime.eq_ignore_ascii_case(preferred))
                });
            let Some(mime_type) = mime_type else {
                continue;
            };

            let bytes = match watcher.receive(&selection, mime_type) {
                Ok(Transfer::Complete(bytes)) => bytes,
                Ok(Transfer::Stale) => continue,
                Err(error) => {
                    eprintln!("Could not read clipboard text: {error}");
                    continue;
                }
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            let Some(entry) = Entry::new(text.into()) else {
                continue;
            };

            if updates.send_blocking(Ok(Arc::new(entry))).is_err() {
                break;
            }
        }
    });

    events
}
