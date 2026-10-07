use std::collections::HashSet;
use std::rc::Rc;

use gtk::gio;
use polysearch::{
    ALIAS, Config, Entry as SearchEntry, Field, IDENTIFIER, KEYWORD, PRIMARY_NAME, Searcher,
};

use super::{Entry, History, Target, applications, system};

fn search_entry(entry: &Entry, id: u64, next_field: &mut u32) -> SearchEntry {
    let mut fields = Vec::new();
    let mut seen = HashSet::new();
    let mut add = |role, text: &str| {
        let text = text.trim();

        if text.is_empty() || !seen.insert(text.to_lowercase()) {
            return;
        }

        let id = *next_field;
        *next_field += 1;
        fields.push(Field {
            id,
            role,
            text: text.to_owned(),
        });
    };

    add(PRIMARY_NAME, &entry.title);

    match &entry.target {
        Target::Application(app) => applications::search_fields(app, &mut add),
        Target::SystemAction(action) => {
            for alias in action.aliases {
                add(ALIAS, alias);
            }
        }
    }

    if let Some(description) = entry.subtitle.as_deref() {
        add(KEYWORD, description);
    }

    if let Some(id) = entry.id.as_deref() {
        add(IDENTIFIER, id);
    }

    SearchEntry { id, fields }
}

pub struct Catalog {
    entries: Vec<Rc<Entry>>,
    searcher: Searcher,
}

impl Catalog {
    pub fn load() -> Self {
        let mut entries = applications::scan();
        entries.extend(system::ACTIONS.iter().map(|action| {
            Rc::new(Entry {
                target: Target::SystemAction(action),
                id: Some(action.id.into()),
                title: action.title.into(),
                subtitle: None,
                icon: Some(gio::ThemedIcon::new(action.icon).into()),
                categories: vec!["System"],
            })
        }));
        entries.sort_unstable_by(|left, right| left.title.cmp(&right.title));

        let mut next_field = 0;
        let search_entries = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| search_entry(entry, index as u64, &mut next_field));
        let searcher = Searcher::new(search_entries, Config::default());

        Self { entries, searcher }
    }

    pub fn entries(&self) -> &[Rc<Entry>] {
        &self.entries
    }

    pub fn search(&self, query: &str, category: Option<&str>, history: &History) -> Vec<usize> {
        let query = query.trim();
        let history_score = history.scorer(query);
        let mut matches: Vec<usize> = if query.is_empty() {
            let mut matches: Vec<_> = self
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| (index, history_score(entry.id.as_deref())))
                .collect();
            matches.sort_by(|(_, left), (_, right)| right.total_cmp(left));

            matches.into_iter().map(|(index, _)| index).collect()
        } else {
            self.searcher
                .search(query, self.entries.len(), |id| {
                    (history_score(self.entries[id as usize].id.as_deref()) * 255.0) as u8
                })
                .into_iter()
                .map(|result| result.entry as usize)
                .collect()
        };

        if let Some(category) = category {
            matches.retain(|&index| self.entries[index].categories.contains(&category));
        }

        matches
    }

    pub fn categories(&self) -> Vec<&'static str> {
        applications::MAIN_CATEGORIES
            .iter()
            .copied()
            .filter(|category| {
                self.entries
                    .iter()
                    .any(|entry| entry.categories.contains(category))
            })
            .collect()
    }
}
