use std::collections::HashSet;
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk::gdk::prelude::{DisplayExt, MonitorExt};
use gtk::gio::prelude::{AppInfoExt, Cast, IsA, ListModelExtManual};
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
        let display = gdk::Display::default().unwrap();
        let theme = IconTheme::for_display(&display);
        let scale = display
            .monitors()
            .iter::<gdk::Monitor>()
            .filter_map(Result::ok)
            .map(|monitor| monitor.scale_factor())
            .max()
            .unwrap_or(1);

        let mut entries: Vec<_> = gio::AppInfo::all()
            .into_iter()
            .filter(AppInfoExt::should_show)
            .map(|app| Rc::new(Entry::from_app_info(app, &theme, scale)))
            .collect();
        entries.sort_unstable_by(|left, right| left.title.cmp(&right.title));

        let mut next_field = 0;
        let search_entries = entries.iter().enumerate().map(|(index, entry)| {
            entry.search_entry(u64::try_from(index).unwrap(), &mut next_field)
        });
        let searcher = Searcher::new(search_entries, Config::default());

        Self { entries, searcher }
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
    id: Option<glib::GString>,
    subtitle: Option<glib::GString>,
    icon: Option<gtk::IconPaintable>,
}

impl Entry {
    fn from_app_info(app: gio::AppInfo, theme: &IconTheme, scale: i32) -> Self {
        let title = app.display_name();
        let id = app.id();
        let subtitle = app.description();
        let icon = resolve_icon(&app, theme, scale);

        Self {
            app,
            title,
            id,
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

        if let Some(id) = self.id.as_deref() {
            add(IDENTIFIER, id);
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

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
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

    pub const fn icon(&self) -> Option<&gtk::IconPaintable> {
        self.icon.as_ref()
    }
}

const FALLBACK_ICON: &str = "application-x-executable-symbolic";

fn resolve_icon(app: &gio::AppInfo, theme: &IconTheme, scale: i32) -> Option<gtk::IconPaintable> {
    let icon = if let Some(icon) = app.icon()
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
        28,
        scale,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::empty(),
    ))
}
