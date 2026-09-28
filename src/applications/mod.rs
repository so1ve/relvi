mod command;
mod history;

use std::collections::HashSet;
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk::gio::prelude::*;
use gtk::{IconTheme, gio, glib};
use polysearch::{
    ALIAS, Config, Entry as SearchEntry, Field, IDENTIFIER, KEYWORD, LOCALIZED_NAME, PRIMARY_NAME,
    SearchResult, Searcher,
};

use self::command::{COMMANDS, Command};
use self::history::History;

pub struct Applications {
    entries: Vec<Rc<Entry>>,
    searcher: Searcher,
    history: History,
}

impl Applications {
    pub fn load(theme: &IconTheme, scale: i32) -> Self {
        let (entries, searcher) = scan(theme, scale);

        Self {
            entries,
            searcher,
            history: History::load(),
        }
    }

    pub fn search(&self, query: &str, category: Option<&str>) -> Vec<Rc<Entry>> {
        let query = query.trim();
        let compare_history = self.history.comparator(query);
        let mut entries = if query.is_empty() {
            let mut entries = self.entries.clone();
            entries.sort_by(|left, right| compare_history(left.id(), right.id()));

            entries
        } else {
            self.searcher
                .search(query, self.entries.len(), |left, right| {
                    compare_history(
                        self.entries[left as usize].id(),
                        self.entries[right as usize].id(),
                    )
                })
                .into_iter()
                .map(|SearchResult { entry, .. }| Rc::clone(&self.entries[entry as usize]))
                .collect()
        };

        if let Some(category) = category {
            entries.retain(|entry| entry.categories.contains(&category));
        }

        entries
    }

    pub fn categories(&self) -> Vec<&'static str> {
        MAIN_CATEGORIES
            .iter()
            .copied()
            .filter(|category| {
                self.entries
                    .iter()
                    .any(|entry| entry.categories.contains(category))
            })
            .collect()
    }

    pub fn refresh(&mut self, theme: &IconTheme, scale: i32) {
        (self.entries, self.searcher) = scan(theme, scale);
    }

    pub fn refresh_icons(&mut self, theme: &IconTheme, scale: i32) {
        for entry in &mut self.entries {
            let entry = Rc::make_mut(entry);
            entry.icon = resolve_icon(entry.action.icon(), theme, scale);
        }
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn record_launch(&mut self, entry: &Entry, query: &str) {
        if let Some(id) = entry.id() {
            self.history.record(id, query);
        }
    }
}

pub fn clear_history() {
    History::load().clear();
}

fn scan(theme: &IconTheme, scale: i32) -> (Vec<Rc<Entry>>, Searcher) {
    let mut entries: Vec<_> = gio::AppInfo::all()
        .into_iter()
        .filter(AppInfoExt::should_show)
        .map(|app| Rc::new(Entry::new(app, theme, scale)))
        .collect();
    entries.extend(
        COMMANDS
            .iter()
            .map(|command| Rc::new(Entry::command(command, theme, scale))),
    );
    entries.sort_unstable_by(|left, right| left.title.cmp(&right.title));

    let mut next_field = 0;
    let search_entries = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| entry.search_entry(index as u64, &mut next_field));
    let searcher = Searcher::new(search_entries, Config::default());

    (entries, searcher)
}

#[derive(Clone)]
pub struct Entry {
    action: Action,
    title: glib::GString,
    id: Option<glib::GString>,
    subtitle: Option<glib::GString>,
    icon: Option<gtk::IconPaintable>,
    categories: Vec<&'static str>,
}

impl Entry {
    fn new(app: gio::AppInfo, theme: &IconTheme, scale: i32) -> Self {
        let title = app.display_name();
        let id = app.id();
        let subtitle = app.description();
        let icon = resolve_icon(app.icon(), theme, scale);
        let categories = main_categories(&app);

        Self {
            action: Action::Application(app),
            title,
            id,
            subtitle,
            icon,
            categories,
        }
    }

    fn command(command: &'static Command, theme: &IconTheme, scale: i32) -> Self {
        let action = Action::Command(command);
        let icon = resolve_icon(action.icon(), theme, scale);

        Self {
            action,
            title: command.title.into(),
            id: Some(command.id.into()),
            subtitle: None,
            icon,
            categories: vec!["System"],
        }
    }

    pub const fn confirmation(&self) -> Option<&'static str> {
        match &self.action {
            Action::Application(_) => None,
            Action::Command(command) => command.confirmation,
        }
    }

    pub async fn launch(
        &self,
        context: &impl IsA<gio::AppLaunchContext>,
    ) -> Result<(), glib::Error> {
        match &self.action {
            Action::Application(app) => app.launch(&[], Some(context)),
            Action::Command(command) => command.run().await,
        }
    }

    fn search_entry(&self, id: u64, next_field: &mut u64) -> SearchEntry {
        let mut fields = Vec::new();
        let mut seen = HashSet::new();

        let mut add = |role, text: &str| {
            let text = text.trim();

            if text.is_empty() || !seen.insert(text.to_lowercase()) {
                return;
            }

            let id = u32::try_from(*next_field).unwrap();
            *next_field += 1;
            fields.push(Field {
                id,
                role,
                text: text.to_owned(),
            });
        };

        add(PRIMARY_NAME, &self.title);
        match &self.action {
            Action::Application(app) => {
                add(LOCALIZED_NAME, &app.name());

                if let Some(desktop) = app.downcast_ref::<DesktopAppInfo>() {
                    if let Some(name) = desktop.string("Name") {
                        add(ALIAS, &name);
                    }

                    if let Some(name) = desktop.generic_name() {
                        add(KEYWORD, &name);
                    }

                    for keyword in desktop.keywords() {
                        add(KEYWORD, &keyword);
                    }
                }

                if let Some(executable) =
                    app.executable().file_name().and_then(|name| name.to_str())
                {
                    add(IDENTIFIER, executable);
                }
            }
            Action::Command(command) => {
                for alias in command.aliases {
                    add(ALIAS, alias);
                }
            }
        }

        if let Some(description) = self.subtitle.as_deref() {
            add(KEYWORD, description);
        }

        if let Some(id) = self.id.as_deref() {
            add(IDENTIFIER, id);
        }

        SearchEntry { id, fields }
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    pub const fn icon(&self) -> Option<&gtk::IconPaintable> {
        self.icon.as_ref()
    }
}

#[derive(Clone)]
enum Action {
    Application(gio::AppInfo),
    Command(&'static Command),
}

impl Action {
    fn icon(&self) -> Option<gio::Icon> {
        match self {
            Self::Application(app) => app.icon(),
            Self::Command(command) => Some(gio::ThemedIcon::new(command.icon).upcast()),
        }
    }
}

/// Main categories from the Desktop Menu Specification that Relvi filters by,
/// in specification order.
const MAIN_CATEGORIES: [&str; 11] = [
    "AudioVideo",
    "Development",
    "Education",
    "Game",
    "Graphics",
    "Network",
    "Office",
    "Science",
    "Settings",
    "System",
    "Utility",
];

fn main_categories(app: &gio::AppInfo) -> Vec<&'static str> {
    let Some(desktop) = app.downcast_ref::<DesktopAppInfo>() else {
        return Vec::new();
    };
    let Some(categories) = desktop.categories() else {
        return Vec::new();
    };

    MAIN_CATEGORIES
        .iter()
        .copied()
        .filter(|category| categories.split(';').any(|token| token == *category))
        .collect()
}

const FALLBACK_ICON: &str = "application-x-executable-symbolic";

fn resolve_icon(
    icon: Option<gio::Icon>,
    theme: &IconTheme,
    scale: i32,
) -> Option<gtk::IconPaintable> {
    let icon = if let Some(icon) = icon
        && theme.has_gicon(&icon)
    {
        icon
    } else if theme.has_icon(FALLBACK_ICON) {
        gio::ThemedIcon::new(FALLBACK_ICON).upcast()
    } else {
        return None;
    };

    Some(theme.lookup_by_gicon(
        &icon,
        24,
        scale,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::empty(),
    ))
}
