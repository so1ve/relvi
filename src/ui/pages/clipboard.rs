mod list;
mod preview;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{
    ApplicationWindow, Box as GtkBox, Button, EventControllerKey, Image, Label, Orientation,
    PropagationPhase, SearchEntry, Widget, gdk, gio, glib,
};

use self::list::HistoryList;
use self::preview::Preview;
use super::Page;
use crate::clipboard::{self, History};
use crate::ui::components::{button, search_field, toolbar};
use crate::ui::keybindings::keybindings;

pub struct ClipboardPage {
    window: ApplicationWindow,
    root: GtkBox,
    search: SearchEntry,
    list: HistoryList,
    preview: Preview,
    history: RefCell<Option<History>>,
    pending_clear: Cell<bool>,
    count: Label,
    copy: Button,
    remove: Button,
    clear: Button,
    error: Image,
}

impl ClipboardPage {
    pub fn new(window: &ApplicationWindow) -> Rc<Self> {
        let search = search_field("Search clipboard…");
        let count = Label::new(Some("History"));

        let error = Image::from_icon_name("dialog-warning-symbolic");
        error.add_css_class("error-indicator");
        error.set_visible(false);

        let clear = button::text("Clear")
            .tooltip_text("Clear history (Ctrl+Shift+Delete)")
            .sensitive(false)
            .build();
        let history_toolbar = toolbar(&count, &[error.upcast_ref(), clear.upcast_ref()]);

        let list = HistoryList::new();

        let history_pane = GtkBox::new(Orientation::Vertical, 0);
        history_pane.add_css_class("history-pane");
        history_pane.set_width_request(208);
        history_pane.set_hexpand(false);
        history_pane.append(&history_toolbar);
        history_pane.append(list.widget());

        let heading = Label::new(Some("Preview"));
        heading.add_css_class("pane-title");

        let remove = button::icon("edit-delete-symbolic")
            .tooltip_text("Remove selected (Ctrl+Delete)")
            .sensitive(false)
            .build();
        let copy = button::icon_text("edit-copy-symbolic", "Copy")
            .tooltip_text("Copy (Ctrl+C)")
            .sensitive(false)
            .build();
        copy.add_css_class("primary-action");

        let preview_toolbar = toolbar(&heading, &[remove.upcast_ref(), copy.upcast_ref()]);

        let preview = Preview::new();

        let detail = GtkBox::new(Orientation::Vertical, 0);
        detail.add_css_class("preview-pane");
        detail.set_hexpand(true);
        detail.append(&preview_toolbar);
        detail.append(preview.widget());

        let body = GtkBox::new(Orientation::Horizontal, 0);
        body.add_css_class("split-pane");
        body.set_height_request(400);
        body.set_overflow(gtk::Overflow::Hidden);
        body.append(&history_pane);
        body.append(&detail);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&search);
        root.append(&body);

        let page = Rc::new(Self {
            window: window.clone(),
            root,
            search,
            list,
            preview,
            history: RefCell::new(None),
            pending_clear: Cell::new(false),
            count,
            copy,
            remove,
            clear,
            error,
        });

        page.search.connect_changed(glib::clone!(
            #[weak]
            page,
            move |_| {
                let selected = page.list.selected();
                page.refresh(selected.as_ref().map(|entry| entry.id.as_str()));
            }
        ));
        page.search.connect_activate(glib::clone!(
            #[weak]
            page,
            move |_| page.copy_selected()
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

        page.list.connect_changed(glib::clone!(
            #[weak]
            page,
            move || page.update_preview()
        ));
        page.copy.connect_clicked(glib::clone!(
            #[weak]
            page,
            move |_| page.copy_selected()
        ));
        page.remove.connect_clicked(glib::clone!(
            #[weak]
            page,
            move |_| page.remove_selected()
        ));
        page.clear.connect_clicked(glib::clone!(
            #[weak]
            page,
            move |_| page.clear()
        ));

        let weak = Rc::downgrade(&page);
        glib::spawn_future_local(async move {
            let history = gio::spawn_blocking(History::load).await.unwrap();
            {
                let Some(page) = weak.upgrade() else {
                    return;
                };
                page.history.replace(Some(history));
                if page.pending_clear.replace(false) {
                    page.clear();
                } else {
                    page.refresh(None);
                }
            }

            let events = clipboard::watch();
            while let Ok(event) = events.recv().await {
                let Some(page) = weak.upgrade() else {
                    break;
                };

                match event {
                    Ok(entry) => {
                        let selected = page.list.selected();
                        let changed = page.history.borrow_mut().as_mut().unwrap().record(entry);
                        if changed {
                            page.refresh(selected.as_ref().map(|entry| entry.id.as_str()));
                        }
                    }
                    Err(error) => {
                        page.error.set_tooltip_text(Some(&format!(
                            "Clipboard monitoring unavailable: {error}"
                        )));
                        page.error.set_visible(true);
                    }
                }
            }
        });

        page
    }

    fn copy_selected(&self) {
        let Some(entry) = self.list.selected() else {
            return;
        };

        if let Err(error) = clipboard::copy(&entry.content, &self.root.clipboard()) {
            self.error
                .set_tooltip_text(Some(&format!("Could not copy entry: {error}")));
            self.error.set_visible(true);

            return;
        }

        self.history.borrow_mut().as_mut().unwrap().record(entry);
        self.refresh(None);
        self.window.set_visible(false);
    }

    pub fn clear(&self) {
        if let Some(history) = self.history.borrow_mut().as_mut() {
            history.clear();
        } else {
            self.pending_clear.set(true);

            return;
        }

        self.refresh(None);
        self.search.grab_focus();
    }

    fn key_pressed(
        &self,
        controller: &EventControllerKey,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation {
        keybindings! {
            key, modifiers;
            Escape => self.window.set_visible(false),
            Return | KP_Enter => self.copy_selected(),
            Ctrl + C if self.list.selected().is_some() => self.copy_selected(),
            Ctrl + Delete => self.remove_selected(),
            Ctrl + Shift + Delete => self.clear(),
            Down | Ctrl + J | Ctrl + N => self.list.move_selection(1),
            Up | Ctrl + K | Ctrl + P => self.list.move_selection(-1),
            Ctrl + D => self.list.scroll_pages(0.5),
            Ctrl + U => self.list.scroll_pages(-0.5),
            _ => {
                if matches!(key, gdk::Key::Tab | gdk::Key::ISO_Left_Tab) {
                    return glib::Propagation::Proceed;
                }

                let input = self.search.delegate().unwrap();
                if input.has_focus() || !controller.forward(&input) {
                    return glib::Propagation::Proceed;
                }

                // Forward the original event so GTK handles editing and input
                // methods.
                self.search.grab_focus();
            },
        }

        glib::Propagation::Stop
    }

    fn remove_selected(&self) {
        if let Some(entry) = self.list.selected() {
            self.history
                .borrow_mut()
                .as_mut()
                .unwrap()
                .remove(&entry.id);
            self.refresh(None);
        }

        self.search.grab_focus();
    }

    fn refresh(&self, selected: Option<&str>) {
        let history = self.history.borrow();
        let Some(history) = history.as_ref() else {
            return;
        };
        let query = self.search.text();
        self.count
            .set_text(&format!("History · {}", history.entries().len()));
        self.clear.set_sensitive(!history.entries().is_empty());
        self.list.show(
            history.search(&query).cloned(),
            selected,
            if query.trim().is_empty() {
                "Clipboard is empty"
            } else {
                "No result"
            },
        );
    }

    fn update_preview(&self) {
        let selected = self.list.selected();
        self.copy.set_sensitive(selected.is_some());
        self.remove.set_sensitive(selected.is_some());
        self.preview
            .show(selected.as_ref().map(|entry| &entry.content));
    }
}

impl Page for ClipboardPage {
    fn widget(&self) -> &Widget {
        self.root.upcast_ref()
    }

    fn width(&self) -> i32 {
        720
    }

    fn present(&self) {
        self.search.grab_focus();
    }
}
