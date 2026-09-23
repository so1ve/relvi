use std::rc::Rc;

use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    ApplicationWindow, Box as GtkBox, EventControllerKey, Label, Orientation, PropagationPhase,
    SearchEntry, gdk, gio, glib,
};

use super::applications::Applications;
use crate::catalog::{Catalog, Entry};

/// Search input, application activation, and catalog change notifications.
pub struct Palette {
    root: GtkBox,
    search: SearchEntry,
    applications: Rc<Applications>,
    error: Label,
    monitor: gio::AppInfoMonitor,
}

impl Palette {
    pub fn new(window: &ApplicationWindow) -> Self {
        let monitor = gio::AppInfoMonitor::get();
        let applications = Applications::new(Catalog::load());

        let search = SearchEntry::builder().placeholder_text("Search…").build();
        search.set_search_delay(0);
        search.add_css_class("palette-search");

        let error = Label::new(None);
        error.set_xalign(0.0);
        error.set_wrap(true);
        error.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        error.add_css_class("launch-error");
        error.set_visible(false);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.add_css_class("palette");
        root.append(&search);
        root.append(applications.widget());
        root.append(&error);

        let palette = Self {
            root,
            search,
            applications,
            error,
            monitor,
        };

        let applications = &palette.applications;
        let search = &palette.search;
        let error = &palette.error;

        palette.search.connect_changed(glib::clone!(
            #[weak]
            applications,
            #[weak]
            error,
            move |entry| {
                error.set_visible(false);
                applications.set_query(entry.text().as_str());
            }
        ));

        // Let GtkText handle IME confirmation before SearchEntry activates.
        palette.search.connect_activate(glib::clone!(
            #[weak]
            applications,
            #[weak]
            window,
            #[weak]
            error,
            move |search| {
                if let Some(entry) = applications.selected_entry() {
                    launch(
                        &entry,
                        &applications,
                        search.text().as_str(),
                        &window,
                        &error,
                    );
                }
            }
        ));

        palette.applications.connect_activate(glib::clone!(
            #[weak]
            applications,
            #[weak]
            search,
            #[weak]
            window,
            #[weak]
            error,
            move |entry| {
                launch(
                    &entry,
                    &applications,
                    search.text().as_str(),
                    &window,
                    &error,
                );
            }
        ));

        // Gio coalesces changes until AppInfo::all() rearms the monitor.
        // Rebuild during idle, including while hidden, so activation stays
        // cheap.
        palette.monitor.connect_changed(glib::clone!(
            #[weak]
            applications,
            #[weak]
            search,
            #[weak]
            error,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    applications,
                    #[weak]
                    search,
                    #[weak]
                    error,
                    move || {
                        applications.replace_catalog(Catalog::load(), search.text().as_str());
                        error.set_visible(false);
                    }
                ));
            }
        ));

        let key_controller = EventControllerKey::new();
        key_controller.set_propagation_phase(PropagationPhase::Capture);

        key_controller.connect_key_pressed(glib::clone!(
            #[weak]
            window,
            #[weak]
            applications,
            #[upgrade_or]
            Propagation::Proceed,
            move |_, key, _, modifiers| {
                let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
                let offset = match (control, key) {
                    (false, gdk::Key::Down) | (true, gdk::Key::j) | (true, gdk::Key::n) => 1,
                    (false, gdk::Key::Up) | (true, gdk::Key::k) | (true, gdk::Key::p) => -1,
                    (false, gdk::Key::Escape) => {
                        window.set_visible(false);

                        return Propagation::Stop;
                    }
                    _ => return Propagation::Proceed,
                };

                applications.move_selection(offset);

                Propagation::Stop
            }
        ));
        window.add_controller(key_controller);

        palette
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn clear_history(&self) {
        self.applications.clear_history(self.search.text().as_str());
    }

    pub fn focus(&self) {
        self.error.set_visible(false);
        self.search.grab_focus();
    }
}

fn launch(
    entry: &Entry,
    applications: &Applications,
    query: &str,
    window: &ApplicationWindow,
    error: &Label,
) {
    let context = WidgetExt::display(window).app_launch_context();

    match entry.launch(&context) {
        Ok(()) => {
            error.set_visible(false);
            window.set_visible(false);

            applications.record_launch(entry, query);
        }
        Err(reason) => {
            error.set_text(&format!("Could not launch {}: {reason}", entry.title()));
            error.set_visible(true);
        }
    }
}
