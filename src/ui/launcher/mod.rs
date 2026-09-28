mod activation;
mod categories;
mod list;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    ApplicationWindow, Box as GtkBox, EventControllerKey, IconTheme, Label, Orientation,
    PropagationPhase, SearchEntry, gdk, gio, glib,
};

use self::categories::Categories;
use self::list::ResultList;
use crate::catalog::Catalog;
use crate::history::History;

fn icon_scale(display: &gdk::Display) -> i32 {
    display
        .monitors()
        .iter::<gdk::Monitor>()
        .map(|monitor| monitor.unwrap().scale_factor())
        .max()
        .unwrap_or(1)
}

pub struct LauncherView {
    window: ApplicationWindow,
    root: GtkBox,
    search: SearchEntry,
    categories: Rc<Categories>,
    results: ResultList,
    catalog: RefCell<Catalog>,
    history: RefCell<History>,
    error: Label,
    confirmation: RefCell<Option<GtkBox>>,
    monitor: gio::AppInfoMonitor,
    theme: IconTheme,
}

impl LauncherView {
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

        let display = WidgetExt::display(window);
        let theme = IconTheme::for_display(&display);
        let catalog = Catalog::load();
        let categories = Categories::new(&catalog.categories());

        let results = ResultList::new();
        results.load(catalog.entries(), &theme, icon_scale(&display));
        let root = GtkBox::new(Orientation::Vertical, 0);
        root.add_css_class("palette");
        root.append(&header);
        root.append(categories.widget());
        root.append(results.widget());
        root.append(&error);

        let view = Rc::new(Self {
            window: window.clone(),
            root,
            search,
            categories,
            results,
            catalog: RefCell::new(catalog),
            history: RefCell::new(History::load()),
            error,
            confirmation: RefCell::new(None),
            monitor: gio::AppInfoMonitor::get(),
            theme,
        });

        view.search.connect_changed(glib::clone!(
            #[weak]
            view,
            move |_| view.update_results(None)
        ));
        view.search.connect_activate(glib::clone!(
            #[weak]
            view,
            move |_| {
                if let Some(entry) = view.results.selected() {
                    view.activate(entry);
                }
            }
        ));
        view.results.connect_activate(glib::clone!(
            #[weak]
            view,
            move |entry| view.activate(entry)
        ));
        view.categories.connect_changed(glib::clone!(
            #[weak]
            view,
            move || view.update_results(None)
        ));

        view.monitor.connect_changed(glib::clone!(
            #[weak]
            view,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    view,
                    move || view.refresh_catalog()
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
                        view.results.load(
                            view.catalog.borrow().entries(),
                            &view.theme,
                            icon_scale(&view.root.display()),
                        );
                        view.update_results(
                            selected.as_ref().and_then(|entry| entry.id.as_deref()),
                        );
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
        view.update_results(None);

        view
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn clear_history(&self) {
        self.history.borrow_mut().clear();
        self.update_results(None);
    }

    pub fn focus(&self) {
        self.update_results(None);
        self.search.grab_focus();
    }

    fn refresh_catalog(&self) {
        let selected = self.results.selected();

        // Reading AppInfo::all() also rearms the desktop application monitor.
        let catalog = Catalog::load();
        self.results.load(
            catalog.entries(),
            &self.theme,
            icon_scale(&self.root.display()),
        );
        self.catalog.replace(catalog);
        self.categories
            .set_categories(&self.catalog.borrow().categories());
        self.update_results(selected.as_ref().and_then(|entry| entry.id.as_deref()));
    }

    fn update_results(&self, selected_id: Option<&str>) {
        self.cancel_confirmation();
        self.error.set_visible(false);
        let matches = self.catalog.borrow().search(
            &self.search.text(),
            self.categories.selected(),
            &self.history.borrow(),
        );

        self.results.show_matches(matches, selected_id);
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> Propagation {
        if self.confirmation.borrow().is_some() {
            return Propagation::Proceed;
        }

        let modifiers = modifiers
            & (gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::SHIFT_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK);

        let control = modifiers == gdk::ModifierType::CONTROL_MASK;
        match key {
            gdk::Key::h if control => self.categories.cycle(-1),
            gdk::Key::l if control => self.categories.cycle(1),
            gdk::Key::Tab | gdk::Key::ISO_Left_Tab
                if !modifiers
                    .intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK) =>
            {
                let backwards = key == gdk::Key::ISO_Left_Tab
                    || modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                self.categories.cycle(if backwards { -1 } else { 1 });
            }
            gdk::Key::Down if modifiers.is_empty() => self.results.move_selection(1),
            gdk::Key::Up if modifiers.is_empty() => self.results.move_selection(-1),
            gdk::Key::j | gdk::Key::n if control => self.results.move_selection(1),
            gdk::Key::k | gdk::Key::p if control => self.results.move_selection(-1),
            _ => return Propagation::Proceed,
        }

        Propagation::Stop
    }
}
