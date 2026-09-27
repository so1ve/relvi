mod categories;
mod list;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    Box as GtkBox, EventControllerKey, IconTheme, Label, Orientation, PropagationPhase,
    SearchEntry, gdk, gio, glib,
};

use self::categories::Categories;
use self::list::ApplicationList;
use crate::applications::{Applications, Entry};

pub struct ApplicationsView {
    root: GtkBox,
    search: SearchEntry,
    categories: Rc<Categories>,
    results: ApplicationList,
    applications: RefCell<Applications>,
    error: Label,
    monitor: gio::AppInfoMonitor,
    theme: IconTheme,
    on_launch: Box<dyn Fn()>,
}

impl ApplicationsView {
    pub fn new(display: &gdk::Display, on_launch: impl Fn() + 'static) -> Rc<Self> {
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

        let theme = IconTheme::for_display(display);
        let applications = Applications::load(&theme, icon_scale(display));
        let categories = Categories::new(&applications.categories());

        let results = ApplicationList::new();
        let root = GtkBox::new(Orientation::Vertical, 0);
        root.add_css_class("palette");
        root.append(&header);
        root.append(categories.widget());
        root.append(results.widget());
        root.append(&error);

        let view = Rc::new(Self {
            root,
            search,
            categories,
            results,
            applications: RefCell::new(applications),
            error,
            monitor: gio::AppInfoMonitor::get(),
            theme,
            on_launch: Box::new(on_launch),
        });

        view.search.connect_changed(glib::clone!(
            #[weak]
            view,
            move |_| view.update(None)
        ));
        view.search.connect_activate(glib::clone!(
            #[weak]
            view,
            move |_| {
                if let Some(entry) = view.results.selected() {
                    view.launch(&entry);
                }
            }
        ));
        view.results.connect_activate(glib::clone!(
            #[weak]
            view,
            move |entry| view.launch(&entry)
        ));
        view.categories.connect_changed(glib::clone!(
            #[weak]
            view,
            move || view.update(None)
        ));

        view.monitor.connect_changed(glib::clone!(
            #[weak]
            view,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    view,
                    move || view.refresh()
                ));
            }
        ));
        view.theme.connect_changed(glib::clone!(
            #[weak]
            view,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    view,
                    move || {
                        let selected = view.results.selected();
                        view.applications
                            .borrow_mut()
                            .refresh_icons(&view.theme, icon_scale(&view.root.display()));
                        view.update(selected.as_ref().and_then(|entry| entry.id()));
                    }
                ));
            }
        ));

        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            view,
            #[upgrade_or]
            Propagation::Proceed,
            move |_, key, _, modifiers| view.key_pressed(key, modifiers)
        ));
        view.root.add_controller(keys);
        view.update(None);

        view
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

    fn refresh(&self) {
        let selected = self.results.selected();

        // Reading AppInfo::all() also rearms the desktop application monitor.
        self.applications
            .borrow_mut()
            .refresh(&self.theme, icon_scale(&self.root.display()));
        self.categories
            .set_categories(&self.applications.borrow().categories());
        self.update(selected.as_ref().and_then(|entry| entry.id()));
    }

    fn update(&self, selected_id: Option<&str>) {
        self.error.set_visible(false);
        let entries = self
            .applications
            .borrow()
            .search(&self.search.text(), self.categories.selected());

        self.results.set_entries(entries, selected_id);
    }

    fn launch(&self, entry: &Entry) {
        self.error.set_visible(false);
        let context = self.root.display().app_launch_context();
        let query = self.search.text();
        let result = self
            .applications
            .borrow_mut()
            .launch(entry, &query, &context);

        match result {
            Ok(()) => (self.on_launch)(),
            Err(reason) => {
                self.error
                    .set_text(&format!("Could not launch {}: {reason}", entry.title()));
                self.error.set_visible(true);
            }
        }
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> Propagation {
        let modifiers = modifiers
            & (gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::SHIFT_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK);

        if matches!(key, gdk::Key::Tab | gdk::Key::ISO_Left_Tab)
            && !modifiers.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK)
        {
            let backwards =
                key == gdk::Key::ISO_Left_Tab || modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            self.categories.cycle(if backwards { -1 } else { 1 });

            return Propagation::Stop;
        }

        let control = modifiers == gdk::ModifierType::CONTROL_MASK;
        let offset = match key {
            gdk::Key::Down if modifiers.is_empty() => 1,
            gdk::Key::Up if modifiers.is_empty() => -1,
            gdk::Key::j | gdk::Key::n if control => 1,
            gdk::Key::k | gdk::Key::p if control => -1,
            _ => return Propagation::Proceed,
        };
        self.results.move_selection(offset);

        Propagation::Stop
    }
}

fn icon_scale(display: &gdk::Display) -> i32 {
    display
        .monitors()
        .iter::<gdk::Monitor>()
        .map(|monitor| monitor.unwrap().scale_factor())
        .max()
        .unwrap_or(1)
}
