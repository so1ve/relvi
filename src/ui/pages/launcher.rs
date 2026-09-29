mod activation;
mod categories;
mod list;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    ApplicationWindow, Box as GtkBox, EventControllerKey, IconTheme, Label, Orientation,
    PropagationPhase, SearchEntry, Widget, gdk, gio, glib,
};

use self::categories::Categories;
use self::list::ResultList;
use super::Page;
use crate::catalog::Catalog;
use crate::history::History;
use crate::ui::components::search_field;

fn icon_scale(display: &gdk::Display) -> i32 {
    display
        .monitors()
        .iter::<gdk::Monitor>()
        .map(|monitor| monitor.unwrap().scale_factor())
        .max()
        .unwrap_or(1)
}

pub struct LauncherPage {
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

impl LauncherPage {
    pub fn new(window: &ApplicationWindow) -> Rc<Self> {
        let search = search_field("Search…");

        let error = Label::new(None);
        error.set_xalign(0.0);
        error.set_wrap(true);
        error.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        error.add_css_class("launch-error");
        error.set_visible(false);

        let display = WidgetExt::display(window);
        let theme = IconTheme::for_display(&display);
        let catalog = Catalog::load();
        let categories = Categories::new(&catalog.categories());

        let results = ResultList::new();
        results.load(catalog.entries(), &theme, icon_scale(&display));
        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&search);
        root.append(categories.widget());
        root.append(results.widget());
        root.append(&error);

        let page = Rc::new(Self {
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

        page.search.connect_changed(glib::clone!(
            #[weak]
            page,
            move |_| page.update_results(None)
        ));
        page.search.connect_activate(glib::clone!(
            #[weak]
            page,
            move |_| {
                if let Some(entry) = page.results.selected() {
                    page.activate(entry);
                }
            }
        ));
        page.root.connect_unmap(glib::clone!(
            #[weak]
            page,
            move |_| {
                page.cancel_confirmation();
            }
        ));

        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            page,
            #[upgrade_or]
            Propagation::Proceed,
            move |_, key, _, modifiers| page.key_pressed(key, modifiers)
        ));
        page.root.add_controller(keys);

        page.results.connect_activate(glib::clone!(
            #[weak]
            page,
            move |entry| page.activate(entry)
        ));
        page.categories.connect_changed(glib::clone!(
            #[weak]
            page,
            move || page.update_results(None)
        ));

        page.monitor.connect_changed(glib::clone!(
            #[weak]
            page,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    page,
                    move || page.refresh_catalog()
                ));
            }
        ));
        page.theme.connect_changed(glib::clone!(
            #[weak]
            page,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    page,
                    move || {
                        let selected = page.results.selected();
                        page.results.load(
                            page.catalog.borrow().entries(),
                            &page.theme,
                            icon_scale(&page.root.display()),
                        );
                        page.update_results(
                            selected.as_ref().and_then(|entry| entry.id.as_deref()),
                        );
                    }
                ));
            }
        ));

        page.update_results(None);

        page
    }

    pub fn clear_history(&self) {
        self.history.borrow_mut().clear();
        self.update_results(None);
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
        let modifiers = modifiers & gtk::accelerator_get_default_mod_mask();
        if key == gdk::Key::Escape && modifiers.is_empty() {
            if self.cancel_confirmation() {
                self.search.grab_focus();
            } else {
                self.window.set_visible(false);
            }

            return Propagation::Stop;
        }

        if self.confirmation.borrow().is_some() {
            return Propagation::Proceed;
        }

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
            gdk::Key::d if control => self.results.scroll_pages(0.5),
            gdk::Key::u if control => self.results.scroll_pages(-0.5),
            _ => return Propagation::Proceed,
        }

        Propagation::Stop
    }
}

impl Page for LauncherPage {
    fn widget(&self) -> &Widget {
        self.root.upcast_ref()
    }

    fn width(&self) -> i32 {
        540
    }

    fn present(&self) {
        self.update_results(None);
        self.search.grab_focus();
    }
}
