use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use emojis::{Emoji, SkinTone};
use gtk::gio;
use polysearch::{ALIAS, Config, Entry, Field, KEYWORD, PRIMARY_NAME, Searcher};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::{Category, SKIN_TONES};
use crate::store::JsonStore;

#[derive(Clone, Default, Deserialize, Serialize)]
struct Preferences {
    recent: Vec<String>,
    tone: usize,
}

struct Index {
    entries: Vec<&'static Emoji>,
    searcher: Searcher,
}

impl Index {
    fn new() -> Self {
        let entries: Vec<_> = emojis::iter().collect();
        let mut annotations: HashMap<String, [String; 2]> =
            serde_json::from_str(include_str!("../../resources/emoji/annotations.json")).unwrap();
        let mut field_id = 0;
        let searcher = Searcher::new(
            entries.iter().enumerate().map(|(id, emoji)| {
                let mut fields = Vec::new();
                let mut add = |role, text: String| {
                    fields.push(Field {
                        id: field_id,
                        role,
                        text,
                    });
                    field_id += 1;
                };

                add(PRIMARY_NAME, emoji.name().to_owned());
                for shortcode in emoji.shortcodes() {
                    add(ALIAS, shortcode.replace('_', " "));
                }
                if let Some([name, keywords]) =
                    annotations.remove(&emoji.as_str().replace('\u{fe0f}', ""))
                {
                    add(PRIMARY_NAME, name);
                    for keyword in keywords.lines() {
                        add(KEYWORD, keyword.to_owned());
                    }
                }

                Entry {
                    id: id as u64,
                    fields,
                }
            }),
            Config::default(),
        );

        Self { entries, searcher }
    }

    fn search(
        &self,
        input: &str,
        category: Category,
        preferences: &Preferences,
    ) -> Vec<&'static Emoji> {
        let query = input.trim().trim_matches(':');
        let recent: Vec<_> = preferences
            .recent
            .iter()
            .map(|value| emojis::get(value).unwrap())
            .collect();
        let rank = |emoji: &Emoji| {
            let base = emoji.with_skin_tone(SkinTone::Default).unwrap_or(emoji);

            recent
                .iter()
                .position(|used| used.with_skin_tone(SkinTone::Default).unwrap_or(used) == base)
        };
        let matches_category = |emoji: &&Emoji| match category {
            Category::All => true,
            Category::Recent => recent.contains(emoji),
            Category::Group(group) => emoji.group() == group,
        };

        if let Some(emoji) = emojis::get(query) {
            return std::iter::once(emoji).filter(matches_category).collect();
        }

        let mut matches = if query.is_empty() {
            if category == Category::Recent {
                return recent;
            }

            let mut entries = self.entries.clone();
            entries.sort_by_key(|emoji| rank(emoji).unwrap_or(usize::MAX));

            entries
        } else {
            self.searcher
                .search(&query.replace('_', " "), self.entries.len(), |id| {
                    rank(self.entries[id as usize]).map_or(0, |rank| (255 / (rank + 1)) as u8)
                })
                .into_iter()
                .map(|result| self.entries[result.entry as usize])
                .collect()
        };

        if let Some(emoji) = emojis::get_by_shortcode(query) {
            matches.retain(|candidate| *candidate != emoji);
            matches.insert(0, emoji);
        }

        if category == Category::Recent {
            matches = matches
                .into_iter()
                .flat_map(|emoji| {
                    recent.iter().copied().filter(move |used| {
                        used.with_skin_tone(SkinTone::Default).unwrap_or(used) == emoji
                    })
                })
                .collect();
        } else {
            matches.retain(matches_category);
            for emoji in &mut matches {
                *emoji = emoji
                    .with_skin_tone(SKIN_TONES[preferences.tone])
                    .unwrap_or(emoji);
            }
        }

        matches
    }
}

pub struct Catalog {
    index: Arc<Index>,
    preferences: Preferences,
    store: JsonStore<Preferences>,
}

impl Catalog {
    pub fn load() -> Self {
        let started = Instant::now();
        let index = Arc::new(Index::new());
        let store = JsonStore::new(glib::user_state_dir().join("relvi/emoji.json"));
        let mut preferences: Preferences = match store.load() {
            Ok(saved) => saved.unwrap_or_default(),
            Err(error) => {
                warn!(%error, "Could not load emoji preferences");

                Preferences::default()
            }
        };
        preferences.tone = preferences.tone.min(SKIN_TONES.len() - 1);
        preferences
            .recent
            .retain(|value| emojis::get(value).is_some());
        preferences.recent.truncate(48);

        debug!(
            elapsed_ms = started.elapsed().as_millis(),
            count = index.entries.len(),
            "Emoji catalog ready"
        );

        Self {
            index,
            preferences,
            store,
        }
    }

    pub fn search(
        &self,
        input: String,
        category: Category,
    ) -> gio::JoinHandle<Vec<&'static Emoji>> {
        let index = Arc::clone(&self.index);
        let preferences = self.preferences.clone();

        gio::spawn_blocking(move || index.search(&input, category, &preferences))
    }

    pub const fn tone(&self) -> usize {
        self.preferences.tone
    }

    pub fn cycle_tone(&mut self, offset: i32) {
        self.preferences.tone =
            (self.preferences.tone as i32 + offset).rem_euclid(SKIN_TONES.len() as i32) as usize;
        self.store.save(self.preferences.clone());
    }

    pub fn record(&mut self, emoji: &Emoji) {
        self.preferences
            .recent
            .retain(|value| value != emoji.as_str());
        self.preferences.recent.insert(0, emoji.as_str().to_owned());
        self.preferences.recent.truncate(48);
        self.store.save(self.preferences.clone());
    }
}
