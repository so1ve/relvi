use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    ApplicationWindow, Box as GtkBox, EventControllerKey, Label, Orientation, PropagationPhase,
    SearchEntry, Widget, gdk, gio, glib,
};

use super::applications::Applications;
use super::list::ApplicationList;
use crate::catalog::Entry;

pub struct Palette {
    root: GtkBox,
    window: ApplicationWindow,
    search: SearchEntry,
    results: Rc<ApplicationList>,
    applications: RefCell<Applications>,
    error: Label,
    monitor: gio::AppInfoMonitor,
}

impl Palette {
    pub fn new(window: &ApplicationWindow) -> Rc<Self> {
        let search = SearchEntry::builder().placeholder_text("Search…").build();
        search.set_search_delay(0);
        search.add_css_class("palette-search");
        search.set_hexpand(true);

        let error = Label::new(None);
        error.set_xalign(0.0);
        error.set_wrap(true);
        error.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        error.add_css_class("launch-error");
        error.set_visible(false);

        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.add_css_class("palette-header");
        header.append(&search);

        let results = ApplicationList::new();
        let root = GtkBox::new(Orientation::Vertical, 0);
        root.add_css_class("palette");
        root.append(&header);
        root.append(results.widget());
        root.append(&error);

        let palette = Rc::new(Self {
            root,
            window: window.clone(),
            search,
            results,
            applications: RefCell::new(Applications::new()),
            error,
            monitor: gio::AppInfoMonitor::get(),
        });

        palette.search.connect_changed(glib::clone!(
            #[weak]
            palette,
            move |_| palette.update(None)
        ));
        palette.search.connect_activate(glib::clone!(
            #[weak]
            palette,
            move |_| {
                if let Some(entry) = palette.results.selected() {
                    palette.launch(&entry);
                }
            }
        ));
        palette.results.connect_activate(glib::clone!(
            #[weak]
            palette,
            move |entry| palette.launch(&entry)
        ));

        // AppInfo::all() rearms the monitor; icon changes also rebuild the
        // snapshot.
        let refresh = glib::clone!(
            #[weak]
            palette,
            move || {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    palette,
                    move || {
                        let selected = palette
                            .results
                            .selected()
                            .and_then(|entry| entry.id().map(str::to_owned));

                        palette.applications.borrow_mut().refresh();
                        palette.update(selected.as_deref());
                    }
                ));
            }
        );
        let icon_refresh = refresh.clone();
        palette.monitor.connect_changed(move |_| refresh());
        gtk::IconTheme::for_display(&WidgetExt::display(window))
            .connect_changed(move |_| icon_refresh());

        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            palette,
            #[upgrade_or]
            Propagation::Proceed,
            move |_, key, _, modifiers| palette.key_pressed(key, modifiers)
        ));
        window.add_controller(keys);
        palette.update(None);

        palette
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn clear_history(&self) {
        self.applications.borrow_mut().clear_history();
        self.update(None);
    }

    pub fn focus(&self) {
        self.update(None);
        self.search.grab_focus();
    }

    fn update(&self, selected_id: Option<&str>) {
        self.error.set_visible(false);
        let entries = self.applications.borrow().search(&self.search.text());

        self.results.set_entries(entries, selected_id);
    }

    fn launch(&self, entry: &Entry) {
        self.error.set_visible(false);
        let context = WidgetExt::display(&self.window).app_launch_context();
        let query = self.search.text();
        let result = self
            .applications
            .borrow_mut()
            .launch(entry, &query, &context);

        match result {
            Ok(()) => self.window.set_visible(false),
            Err(reason) => {
                self.error.set_text(&reason);
                self.error.set_visible(true);
            }
        }
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> Propagation {
        let Some(focus) = GtkWindowExt::focus(&self.window) else {
            return Propagation::Proceed;
        };
        if focus != self.search.clone().upcast::<Widget>() && !focus.is_ancestor(&self.search) {
            return Propagation::Proceed;
        }
        if key == gdk::Key::Escape {
            self.window.set_visible(false);
            return Propagation::Stop;
        }

        let modifiers = modifiers
            & (gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::SHIFT_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK);
        let control = modifiers == gdk::ModifierType::CONTROL_MASK;
        let offset = match (control, key) {
            (false, gdk::Key::Down) | (true, gdk::Key::j) | (true, gdk::Key::n) => 1,
            (false, gdk::Key::Up) | (true, gdk::Key::k) | (true, gdk::Key::p) => -1,
            _ => return Propagation::Proceed,
        };
        self.results.move_selection(offset);

        Propagation::Stop
    }
}
