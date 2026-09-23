use std::collections::HashSet;
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk::gio::prelude::{AppInfoExt, Cast, IsA};
use gtk::{IconTheme, gdk, gio, glib};
use polysearch::{
    ALIAS, Config, Entry as SearchEntry, Field, IDENTIFIER, KEYWORD, LOCALIZED_NAME, PRIMARY_NAME,
    SearchResult, Searcher,
};

/// Immutable application metadata and its in-memory search index.
pub struct Catalog {
    entries: Vec<Rc<Entry>>,
    searcher: Searcher,
}

impl Catalog {
    pub fn load() -> Self {
        let mut entries: Vec<_> = gio::AppInfo::all()
            .into_iter()
            .filter(AppInfoExt::should_show)
            .map(|app| Rc::new(Entry::from_app_info(app)))
            .collect();
        entries.sort_unstable_by(|left, right| left.title.cmp(&right.title));

        let mut next_field = 0;
        let search_entries = entries.iter().enumerate().map(|(index, entry)| {
            entry.search_entry(u64::try_from(index).unwrap(), &mut next_field)
        });
        let searcher = Searcher::new(search_entries, Config::default());

        Self { entries, searcher }
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn search(&self, query: &str) -> Vec<Rc<Entry>> {
        let query = query.trim();
        if query.is_empty() {
            return self.entries.clone();
        }

        self.searcher
            .search(query, self.entries.len())
            .into_iter()
            .map(|SearchResult { entry, .. }| {
                Rc::clone(&self.entries[usize::try_from(entry).unwrap()])
            })
            .collect()
    }
}

pub struct Entry {
    app: gio::AppInfo,
    title: glib::GString,
    subtitle: Option<glib::GString>,
    icon: Option<gio::Icon>,
}

impl Entry {
    fn from_app_info(app: gio::AppInfo) -> Self {
        let title = app.display_name();
        let subtitle = app.description();
        let icon = resolve_icon(&app);

        Self {
            app,
            title,
            subtitle,
            icon,
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
        add(LOCALIZED_NAME, &self.app.name());

        if let Some(desktop) = self.app.downcast_ref::<DesktopAppInfo>() {
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

        if let Some(description) = self.subtitle.as_deref() {
            add(KEYWORD, description);
        }

        if let Some(id) = self.app.id() {
            add(IDENTIFIER, &id);
        }

        if let Some(executable) = self
            .app
            .executable()
            .file_name()
            .and_then(|name| name.to_str())
        {
            add(IDENTIFIER, executable);
        }

        SearchEntry { id, fields }
    }

    pub fn id(&self) -> Option<glib::GString> {
        self.app.id()
    }

    pub fn launch(&self, context: &impl IsA<gio::AppLaunchContext>) -> Result<(), glib::Error> {
        self.app.launch(&[], Some(context))
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    pub const fn icon(&self) -> Option<&gio::Icon> {
        self.icon.as_ref()
    }
}

const FALLBACK_ICON: &str = "application-x-executable-symbolic";

fn resolve_icon(app: &gio::AppInfo) -> Option<gio::Icon> {
    let display = gdk::Display::default()?;
    let theme = IconTheme::for_display(&display);

    if let Some(icon) = app.icon()
        && theme.has_gicon(&icon)
    {
        return Some(icon);
    }

    theme
        .has_icon(FALLBACK_ICON)
        .then(|| gio::ThemedIcon::new(FALLBACK_ICON).upcast())
}
