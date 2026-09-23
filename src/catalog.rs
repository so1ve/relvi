use std::collections::HashSet;
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk::gio::prelude::{AppInfoExt, Cast, IsA};
use gtk::{gio, glib};
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
    title: String,
    kind: String,
    icon: Option<gio::Icon>,
}

impl Entry {
    fn from_app_info(app: gio::AppInfo) -> Self {
        let title = app.display_name().to_string();
        let kind = app
            .description()
            .filter(|description| !description.trim().is_empty())
            .map(|description| description.to_string())
            .unwrap_or_else(|| "Application".to_owned());
        let icon = app.icon();

        Self {
            app,
            title,
            kind,
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

        if let Some(description) = self.app.description() {
            add(KEYWORD, &description);
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

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub const fn icon(&self) -> Option<&gio::Icon> {
        self.icon.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn desktop_metadata_is_searchable_without_replacing_the_display_name() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("org.example.CatalogFixture.desktop");
        let key_file = glib::KeyFile::new();
        key_file.load_from_data(
            "[Desktop Entry]\nType=Application\nName=Original Writer\nName[zh_CN]=本地化编辑器\nX-GNOME-FullName=Display Writer\nGenericName=Quill Workbench\nKeywords=Halcyon;Quartz;Display Writer;;\nComment=Silhouette documents\nExec=true --private-catalog-argument\nIcon=text-editor\n",
            glib::KeyFileFlags::KEEP_TRANSLATIONS,
        ).unwrap();
        fs::write(&file, key_file.to_data().as_bytes()).unwrap();

        let app = DesktopAppInfo::from_filename(&file).unwrap();
        let expected_title = app.display_name();
        let entry = Entry::from_app_info(app.upcast());

        assert_eq!(entry.title(), expected_title.as_str());
        assert_eq!(
            entry.id().as_deref(),
            Some("org.example.CatalogFixture.desktop")
        );
        assert!(entry.icon().is_some());

        let indexed = entry.search_entry(0, &mut 0);
        assert_eq!(
            indexed
                .fields
                .iter()
                .filter(|field| field.text == entry.title())
                .count(),
            1
        );

        let searcher = Searcher::new([indexed], Config::default());
        for query in [
            "original",
            "quill",
            "halcyon",
            "silhouette",
            "catalogfixture",
            "true",
        ] {
            assert_eq!(searcher.search(query, 1)[0].entry, 0, "query={query}");
        }
        assert!(searcher.search("private-catalog-argument", 1).is_empty());

        key_file.remove_key("Desktop Entry", "Comment").unwrap();
        let entry = Entry::from_app_info(DesktopAppInfo::from_keyfile(&key_file).unwrap().upcast());

        assert_eq!(entry.kind(), "Application");
        assert!(
            entry
                .search_entry(1, &mut 0)
                .fields
                .iter()
                .all(|field| field.text != "Application")
        );
    }

    #[test]
    fn launching_uses_the_retained_app_info_and_returns_spawn_errors() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("launch-fixture");
        fs::copy(glib::find_program_in_path("true").unwrap(), &executable).unwrap();

        let app = gio::AppInfo::create_from_commandline(
            glib::shell_quote(&executable),
            Some("Catalog launch fixture"),
            gio::AppInfoCreateFlags::NONE,
        )
        .unwrap();
        let entry = Entry::from_app_info(app);
        let context = gio::AppLaunchContext::new();

        entry.launch(&context).unwrap();

        fs::remove_file(executable).unwrap();
        assert!(entry.launch(&context).is_err());
    }
}
