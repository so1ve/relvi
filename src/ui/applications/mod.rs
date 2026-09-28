mod categories;
mod list;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    Box as GtkBox, Button, EventControllerKey, IconTheme, Label, Orientation, PropagationPhase,
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
    confirmation: RefCell<Option<GtkBox>>,
    monitor: gio::AppInfoMonitor,
    theme: IconTheme,
    hide: Box<dyn Fn()>,
}

impl ApplicationsView {
    pub fn new(display: &gdk::Display, hide: impl Fn() + 'static) -> Rc<Self> {
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
            confirmation: RefCell::new(None),
            monitor: gio::AppInfoMonitor::get(),
            theme,
            hide: Box::new(hide),
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
        self.cancel_confirmation();
        self.error.set_visible(false);
        let entries = self
            .applications
            .borrow()
            .search(&self.search.text(), self.categories.selected());

        self.results.set_entries(entries, selected_id);
    }

    fn activate(self: &Rc<Self>, entry: Rc<Entry>) {
        self.error.set_visible(false);

        let Some(question) = entry.confirmation() else {
            self.launch(entry);

            return;
        };

        let label = Label::new(Some(question));
        label.set_xalign(0.0);
        label.set_hexpand(true);

        let cancel = Button::with_label("Cancel");
        let accept = Button::with_label(entry.title());
        accept.add_css_class("destructive-action");

        let confirmation = GtkBox::new(Orientation::Horizontal, 8);
        confirmation.add_css_class("confirmation");
        confirmation.append(&label);
        confirmation.append(&cancel);
        confirmation.append(&accept);

        cancel.connect_clicked(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| {
                view.cancel_confirmation();
            }
        ));
        accept.connect_clicked(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| {
                view.cancel_confirmation();
                view.launch(Rc::clone(&entry));
            }
        ));

        self.results.widget().set_visible(false);
        self.search.set_sensitive(false);
        self.categories.widget().set_sensitive(false);
        self.root.append(&confirmation);
        self.confirmation.replace(Some(confirmation));
        cancel.grab_focus();
    }

    pub fn cancel_confirmation(&self) -> bool {
        let Some(confirmation) = self.confirmation.take() else {
            return false;
        };

        self.root.remove(&confirmation);
        self.results.widget().set_visible(true);
        self.search.set_sensitive(true);
        self.categories.widget().set_sensitive(true);
        self.search.grab_focus();

        true
    }

    fn launch(self: &Rc<Self>, entry: Rc<Entry>) {
        let context = self.root.display().app_launch_context();
        let query = self.search.text();

        // Release the layer surface's keyboard grab before polkit can ask for
        // authentication.
        self.root.set_sensitive(false);
        (self.hide)();

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = view)]
            self,
            async move {
                let result = entry.launch(&context).await;
                view.root.set_sensitive(true);

                match result {
                    Ok(()) => view.applications.borrow_mut().record_launch(&entry, &query),
                    Err(reason) => {
                        view.error
                            .set_text(&format!("Could not run {}: {reason}", entry.title()));
                        view.error.set_visible(true);
                        view.root
                            .root()
                            .unwrap()
                            .downcast::<gtk::Window>()
                            .unwrap()
                            .present();
                        view.search.grab_focus();
                    }
                }
            }
        ));
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

fn icon_scale(display: &gdk::Display) -> i32 {
    display
        .monitors()
        .iter::<gdk::Monitor>()
        .map(|monitor| monitor.unwrap().scale_factor())
        .max()
        .unwrap_or(1)
}
