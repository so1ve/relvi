mod activation;
mod list;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    Box as GtkBox, EventControllerKey, IconTheme, Label, Orientation, PropagationPhase,
    SearchEntry, Widget, gdk, glib,
};

use self::list::ResultList;
use super::Page;
use crate::launcher::{Change, Launcher};
use crate::ui::components::{CategoryBar, search_field};
use crate::ui::keybindings::keybindings;

fn icon_scale(display: &gdk::Display) -> i32 {
    display
        .monitors()
        .iter::<gdk::Monitor>()
        .map(|monitor| monitor.unwrap().scale_factor())
        .max()
        .unwrap_or(1)
}

fn category_choices(
    launcher: &Launcher,
) -> impl Iterator<Item = (Option<&'static str>, &'static str)> {
    std::iter::once((None, "All")).chain(launcher.categories().into_iter().map(|category| {
        let label = match category {
            "AudioVideo" => "Media",
            "Game" => "Games",
            "Network" => "Internet",
            "Utility" => "Utilities",
            name => name,
        };

        (Some(category), label)
    }))
}

pub struct LauncherPage {
    root: GtkBox,
    search: SearchEntry,
    categories: CategoryBar<Option<&'static str>>,
    results: ResultList,
    launcher: Rc<Launcher>,
    error: Label,
    confirmation: RefCell<Option<GtkBox>>,
    theme: IconTheme,
}

impl LauncherPage {
    pub fn new(launcher: Rc<Launcher>) -> Rc<Self> {
        let search = search_field("Search…");

        let error = Label::new(None);
        error.set_xalign(0.0);
        error.set_wrap(true);
        error.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        error.add_css_class("launch-error");
        error.set_visible(false);

        let display = search.display();
        let theme = IconTheme::for_display(&display);
        let categories = CategoryBar::new(category_choices(&launcher));

        let results = ResultList::new();
        results.load(&launcher.entries(), &theme, icon_scale(&display));
        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&search);
        root.append(categories.widget());
        root.append(results.widget());
        root.append(&error);

        let page = Rc::new(Self {
            root,
            search,
            categories,
            results,
            launcher,
            error,
            confirmation: RefCell::new(None),
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

        page.launcher.connect_changed(glib::clone!(
            #[weak]
            page,
            move |change| match change {
                Change::Catalog => {
                    let selected = page.results.selected();
                    page.results.load(
                        &page.launcher.entries(),
                        &page.theme,
                        icon_scale(&page.root.display()),
                    );
                    page.categories.set_items(category_choices(&page.launcher));
                    page.update_results(selected.as_ref().and_then(|entry| entry.id.as_deref()));
                }
                Change::History => page.update_results(None),
            }
        ));
        page.theme.connect_changed(glib::clone!(
            #[weak]
            page,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    page,
                    move || page.reload_icons()
                ));
            }
        ));

        page.update_results(None);

        page
    }

    fn reload_icons(&self) {
        let selected = self.results.selected();
        self.results.load(
            &self.launcher.entries(),
            &self.theme,
            icon_scale(&self.root.display()),
        );
        self.update_results(selected.as_ref().and_then(|entry| entry.id.as_deref()));
    }

    fn update_results(&self, selected_id: Option<&str>) {
        self.cancel_confirmation();
        self.error.set_visible(false);
        let matches = self
            .launcher
            .search(&self.search.text(), self.categories.selected());

        self.results.show_matches(matches, selected_id);
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> Propagation {
        keybindings! {
            key, modifiers;
            Escape => {
                if self.cancel_confirmation() {
                    self.search.grab_focus();
                } else {
                    self.root.activate_action("win.hide", None).unwrap();
                }

                return Propagation::Stop;
            },
            _ => {},
        }

        if self.confirmation.borrow().is_some() {
            return Propagation::Proceed;
        }

        keybindings! {
            key, modifiers;
            Ctrl + H | Shift + Tab | Ctrl + Shift + Tab => self.categories.cycle(-1),
            Ctrl + L | Tab | Ctrl + Tab => self.categories.cycle(1),
            Down | Ctrl + J | Ctrl + N => self.results.move_items(1),
            Up | Ctrl + K | Ctrl + P => self.results.move_items(-1),
            Ctrl + D => self.results.scroll_pages(0.5),
            Ctrl + U => self.results.scroll_pages(-0.5),
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
        self.search.set_text("");
        self.categories.reset();
        self.update_results(None);
        self.search.grab_focus();
    }
}
