mod grid;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use emojis::{Emoji, Group};
use gtk::prelude::*;
use gtk::{
    Box as GtkBox, Button, EventControllerKey, Image, Label, Orientation, PropagationPhase,
    SearchEntry, Widget, gdk, glib,
};
use tracing::error;

use self::grid::EmojiGrid;
use super::Page;
use crate::emoji::{Category, Picker, SKIN_TONES};
use crate::ui::components::{CategoryBar, button, search_field};
use crate::ui::keybindings::keybindings;
use crate::ui::text_input;

const CATEGORIES: [(Category, &str); 11] = [
    (Category::All, "All"),
    (Category::Recent, "Recent"),
    (Category::Group(Group::SmileysAndEmotion), "Faces"),
    (Category::Group(Group::PeopleAndBody), "People"),
    (Category::Group(Group::AnimalsAndNature), "Nature"),
    (Category::Group(Group::FoodAndDrink), "Food"),
    (Category::Group(Group::TravelAndPlaces), "Travel"),
    (Category::Group(Group::Activities), "Activities"),
    (Category::Group(Group::Objects), "Objects"),
    (Category::Group(Group::Symbols), "Symbols"),
    (Category::Group(Group::Flags), "Flags"),
];

pub struct EmojiPage {
    root: GtkBox,
    search: SearchEntry,
    categories: CategoryBar<Category>,
    grid: EmojiGrid,
    picker: Rc<Picker>,
    preserved: Cell<Option<&'static Emoji>>,
    name: Label,
    shortcode: Label,
    tone: Button,
    copy: Button,
    insertions: RefCell<VecDeque<&'static Emoji>>,
    error: Image,
}

impl EmojiPage {
    pub fn new() -> Rc<Self> {
        let search = search_field("Search emoji…");
        let categories = CategoryBar::new(CATEGORIES);
        let grid = EmojiGrid::new();

        let name = Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(32)
            .css_classes(["pane-title"])
            .build();
        let shortcode = Label::new(None);
        shortcode.set_xalign(0.0);
        shortcode.add_css_class("result-subtitle");

        let detail = GtkBox::new(Orientation::Vertical, 2);
        detail.set_hexpand(true);
        detail.set_valign(gtk::Align::Center);
        detail.append(&name);
        detail.append(&shortcode);

        let tone = button::text("✋")
            .tooltip_text("Change skin tone (Ctrl+T)")
            .focusable(false)
            .sensitive(false)
            .build();
        tone.add_css_class("emoji-tone");
        let copy = button::text("Copy")
            .tooltip_text("Copy (Ctrl+C)")
            .focusable(false)
            .sensitive(false)
            .build();
        copy.add_css_class("primary-action");

        let error = Image::from_icon_name("dialog-warning-symbolic");
        error.add_css_class("error-indicator");
        error.set_visible(false);

        let footer = GtkBox::new(Orientation::Horizontal, 10);
        footer.add_css_class("emoji-footer");
        footer.append(&detail);
        footer.append(&error);
        footer.append(&tone);
        footer.append(&copy);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&search);
        root.append(categories.widget());
        root.append(grid.widget());
        root.append(&footer);

        let page = Rc::new(Self {
            root,
            search,
            categories,
            grid,
            picker: Picker::new(),
            preserved: Cell::new(None),
            name,
            shortcode,
            tone,
            copy,
            insertions: RefCell::new(VecDeque::new()),
            error,
        });

        page.search.connect_changed(glib::clone!(
            #[weak]
            page,
            move |_| page.refresh(None)
        ));
        page.categories.connect_changed(glib::clone!(
            #[weak]
            page,
            move || page.refresh(None)
        ));
        page.grid.connect_changed(glib::clone!(
            #[weak]
            page,
            move || page.update_selection()
        ));
        page.grid.connect_activate(glib::clone!(
            #[weak]
            page,
            move |emoji| page.insert(emoji)
        ));
        page.copy.connect_clicked(glib::clone!(
            #[weak]
            page,
            move |_| page.copy_selected()
        ));
        page.tone.connect_clicked(glib::clone!(
            #[weak]
            page,
            move |_| page.cycle_tone(1)
        ));

        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            page,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |controller, key, _, modifiers| page.key_pressed(controller, key, modifiers)
        ));
        page.root.add_controller(keys);

        page.picker.connect_results(glib::clone!(
            #[weak]
            page,
            move |matches| {
                // Keep focus on the page before removing the focused grid item
                if matches.is_empty() && page.grid.widget().focus_child().is_some() {
                    page.search.grab_focus();
                }

                page.tone.set_sensitive(true);
                page.update_tone();
                page.grid.show(
                    matches,
                    page.preserved.take(),
                    if page.categories.selected() == Category::Recent
                        && page.search.text().is_empty()
                    {
                        "Copied emoji will appear here"
                    } else {
                        "No emoji found"
                    },
                );
            }
        ));

        page
    }

    fn refresh(&self, selected: Option<&'static Emoji>) {
        self.preserved.set(selected);
        self.copy.set_sensitive(false);
        self.picker
            .search(&self.search.text(), self.categories.selected());
    }

    fn update_selection(&self) {
        let selected = self.grid.selected();
        self.copy
            .set_sensitive(!self.picker.is_pending() && selected.is_some());
        self.name.set_text(selected.map_or("", Emoji::name));
        if let Some(shortcode) = selected.and_then(Emoji::shortcode) {
            self.shortcode.set_text(&format!(":{shortcode}:"));
            self.shortcode.set_visible(true);
        } else {
            self.shortcode.set_visible(false);
        }
    }

    fn copy(&self, emoji: &'static Emoji) {
        self.error.set_visible(false);
        self.root.clipboard().set_text(emoji.as_str());
        self.picker.record(emoji);
    }

    fn copy_selected(&self) {
        if self.picker.is_pending() {
            return;
        }
        if let Some(emoji) = self.grid.selected() {
            self.copy(emoji);
        }
    }

    fn insert(self: &Rc<Self>, emoji: &'static Emoji) {
        if self.picker.is_pending() {
            return;
        }

        {
            let mut insertions = self.insertions.borrow_mut();
            // Keep the active insertion at the front until focus returns.
            insertions.push_back(emoji);

            if insertions.len() > 1 {
                return;
            }
        }

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move {
                loop {
                    let emoji = *page.insertions.borrow().front().unwrap();
                    page.copy(emoji);

                    if let Err(error) = text_input::paste(&page.root).await {
                        page.insertions.borrow_mut().clear();
                        error!(%error, "Could not insert emoji");
                        page.error.set_tooltip_text(Some(&format!(
                            "Emoji copied, but could not be inserted: {error}"
                        )));
                        page.error.set_visible(true);
                        break;
                    }

                    let mut insertions = page.insertions.borrow_mut();
                    insertions.pop_front();

                    if insertions.is_empty() {
                        break;
                    }
                }
            }
        ));
    }

    fn insert_selected(self: &Rc<Self>) {
        if let Some(emoji) = self.grid.selected() {
            self.insert(emoji);
        }
    }

    fn cycle_tone(&self, offset: i32) {
        self.picker.cycle_tone(offset);
        self.update_tone();
        self.refresh(self.grid.selected());
    }

    fn update_tone(&self) {
        let tone = self.picker.tone();
        let hand = emojis::get("✋")
            .unwrap()
            .with_skin_tone(SKIN_TONES[tone])
            .unwrap();
        self.tone.set_label(hand.as_str());
        self.tone
            .set_tooltip_text(Some(&format!("{} (Ctrl+T)", hand.name())));
    }

    fn key_pressed(
        self: &Rc<Self>,
        controller: &EventControllerKey,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        let input = self.search.delegate().unwrap();
        keybindings! {
            key, modifiers;
            Escape => self.root.activate_action("win.hide", None).unwrap(),
            Return | KP_Enter => self.insert_selected(),
            Ctrl + C => self.copy_selected(),
            Down | Ctrl + J | Ctrl + N => self.grid.move_rows(1),
            Up | Ctrl + K | Ctrl + P => self.grid.move_rows(-1),
            Left if !input.has_focus() => self.grid.move_items(-1),
            Right if !input.has_focus() => self.grid.move_items(1),
            Ctrl + H => self.grid.move_items(-1),
            Ctrl + L => self.grid.move_items(1),
            Shift + Tab | Ctrl + Shift + Tab => self.categories.cycle(-1),
            Tab | Ctrl + Tab => self.categories.cycle(1),
            Ctrl + D => self.grid.scroll_pages(0.5),
            Ctrl + U => self.grid.scroll_pages(-0.5),
            Page_Down => self.grid.scroll_pages(1.0),
            Page_Up => self.grid.scroll_pages(-1.0),
            Ctrl + T if self.tone.is_sensitive() => self.cycle_tone(1),
            Ctrl + Shift + T if self.tone.is_sensitive() => self.cycle_tone(-1),
            Ctrl + F => {
                self.search.grab_focus();
                self.search.select_region(0, -1);
            },
            _ => {
                if input.has_focus() {
                    return glib::Propagation::Proceed;
                }
                // Preserve GTK editing and input-method handling when typing from the grid.
                if !controller.forward(&input) {
                    return glib::Propagation::Proceed;
                }
                self.search.grab_focus();
            },
        }

        glib::Propagation::Stop
    }
}

impl Page for EmojiPage {
    fn widget(&self) -> &Widget {
        self.root.upcast_ref()
    }

    fn width(&self) -> i32 {
        540
    }

    fn present(&self) {
        self.refresh(None);
        self.search.grab_focus();
        self.search.select_region(0, -1);
    }
}
