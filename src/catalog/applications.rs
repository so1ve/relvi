use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk::gio;
use gtk::gio::prelude::*;
use polysearch::{ALIAS, IDENTIFIER, KEYWORD, LOCALIZED_NAME, Role};

use super::{Entry, Target};

/// Main categories from the Desktop Menu Specification, in specification order.
pub const MAIN_CATEGORIES: [&str; 11] = [
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

pub fn scan() -> Vec<Rc<Entry>> {
    gio::AppInfo::all()
        .into_iter()
        .filter(AppInfoExt::should_show)
        .map(|app| {
            let categories = app
                .downcast_ref::<DesktopAppInfo>()
                .and_then(DesktopAppInfo::categories);
            let categories = MAIN_CATEGORIES
                .iter()
                .copied()
                .filter(|category| {
                    categories
                        .as_deref()
                        .is_some_and(|value| value.split(';').any(|token| token == *category))
                })
                .collect();

            Rc::new(Entry {
                id: app.id(),
                title: app.display_name(),
                subtitle: app.description(),
                icon: app.icon(),
                categories,
                target: Target::Application(app),
            })
        })
        .collect()
}

pub fn search_fields(app: &gio::AppInfo, mut add: impl FnMut(Role, &str)) {
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

    if let Some(executable) = app.executable().file_name().and_then(|name| name.to_str()) {
        add(IDENTIFIER, executable);
    }
}
