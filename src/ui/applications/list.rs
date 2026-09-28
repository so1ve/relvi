use std::rc::Rc;

use adw::TimedAnimation;
use adw::prelude::*;
use gtk::pango::EllipsizeMode;
use gtk::{
    Align, Box as GtkBox, Image, Label, ListItem, ListScrollFlags, ListView, Orientation,
    PolicyType, ScrolledWindow, SignalListItemFactory, SingleSelection, gio, glib,
};

use super::super::scroll::smooth_scroll;
use crate::applications::Entry;

const VISIBLE_ROWS: usize = 12;
const MAX_CONTENT_HEIGHT: i32 = 480;

pub struct ApplicationList {
    root: GtkBox,
    list: ListView,
    frame: ScrolledWindow,
    scroll: TimedAnimation,
    model: gio::ListStore,
    selection: SingleSelection,
    empty: Label,
}

impl ApplicationList {
    pub fn new() -> Self {
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = SingleSelection::new(Some(model.clone()));
        selection.set_autoselect(false);

        let factory = SignalListItemFactory::new();
        let list = ListView::new(Some(selection.clone()), Some(factory.clone()));
        list.set_single_click_activate(true);
        list.set_focusable(false);
        list.set_focus_on_click(false);
        list.add_css_class("results-list");

        let frame = ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(PolicyType::Never)
            .focusable(false)
            .build();
        frame.add_css_class("results-frame");
        let scroll = smooth_scroll(&frame, Orientation::Vertical);

        let empty = Label::new(Some("No result"));
        empty.add_css_class("empty-state");
        empty.set_visible(false);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&frame);
        root.append(&empty);

        factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<ListItem>().unwrap();
            let row = ApplicationRow::new();
            item.set_child(Some(&row.widget));

            item.connect_item_notify(move |item| {
                let Some(object) = item.item() else {
                    return;
                };

                let object = object.downcast::<glib::BoxedAnyObject>().unwrap();
                let entry = object.borrow::<Rc<Entry>>();

                row.update(&entry);
            });
        });

        Self {
            root,
            list,
            frame,
            scroll,
            model,
            selection,
            empty,
        }
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn set_entries(&self, entries: Vec<Rc<Entry>>, selected_id: Option<&str>) {
        self.scroll.pause();

        let selected = selected_id
            .and_then(|id| entries.iter().position(|entry| entry.id() == Some(id)))
            .or_else(|| (!entries.is_empty()).then_some(0));
        let has_entries = !entries.is_empty();
        self.update_height(&entries);

        let previous_len = self.model.n_items() as usize;
        let unchanged = |index, entry: &Rc<Entry>| {
            let object = self.model.item(index as u32).unwrap();
            let previous = object.downcast_ref::<glib::BoxedAnyObject>().unwrap();

            Rc::ptr_eq(&previous.borrow::<Rc<Entry>>(), entry)
        };
        let prefix = entries
            .iter()
            .take(previous_len)
            .enumerate()
            .take_while(|(index, entry)| unchanged(*index, entry))
            .count();
        let suffix = entries[prefix..]
            .iter()
            .rev()
            .zip((prefix..previous_len).rev())
            .take_while(|(entry, index)| unchanged(*index, entry))
            .count();
        let removed = previous_len - prefix - suffix;
        let inserted_count = entries.len() - prefix - suffix;
        let inserted = entries
            .into_iter()
            .skip(prefix)
            .take(inserted_count)
            .map(glib::BoxedAnyObject::new)
            .collect::<Vec<_>>();

        if removed != 0 || !inserted.is_empty() {
            self.model.splice(prefix as u32, removed as u32, &inserted);
        }

        self.empty.set_visible(!has_entries);
        self.frame.set_visible(has_entries);
        self.selection
            .set_selected(selected.map_or(gtk::INVALID_LIST_POSITION, |index| index as u32));

        if let Some(selected) = selected.filter(|_| selected_id.is_some()) {
            self.list
                .scroll_to(selected as u32, ListScrollFlags::NONE, None);
        } else {
            self.frame.vadjustment().set_value(0.0);
        }
    }

    pub fn selected(&self) -> Option<Rc<Entry>> {
        self.selection.selected_item().map(|object| {
            let entry = object.downcast_ref::<glib::BoxedAnyObject>().unwrap();

            Rc::clone(&entry.borrow::<Rc<Entry>>())
        })
    }

    pub fn move_selection(&self, offset: i32) {
        self.scroll.pause();

        let count = self.model.n_items() as i32;
        if count == 0 {
            return;
        }

        let current = self.selection.selected();
        let next = if current == gtk::INVALID_LIST_POSITION {
            if offset > 0 { 0 } else { count - 1 }
        } else {
            (current as i32 + offset).rem_euclid(count)
        };
        self.selection.set_selected(next as u32);
        self.list
            .scroll_to(next as u32, ListScrollFlags::NONE, None);
    }

    pub fn connect_activate(&self, activate: impl Fn(Rc<Entry>) + 'static) {
        self.list.connect_activate(move |list, position| {
            let object = list.model().unwrap().item(position).unwrap();
            let entry = object.downcast_ref::<glib::BoxedAnyObject>().unwrap();

            activate(Rc::clone(&entry.borrow::<Rc<Entry>>()));
        });
    }

    fn update_height(&self, entries: &[Rc<Entry>]) {
        let Some(first) = entries.first() else {
            self.frame.set_min_content_height(0);

            return;
        };

        // Titles and descriptions are single-line, so every row has the same
        // height.
        let row = ApplicationRow::new();
        row.update(first);
        let row_height = row.widget.measure(Orientation::Vertical, -1).1;
        let mut height = 0;

        for _ in entries.iter().take(VISIBLE_ROWS) {
            if height > 0 && height + row_height > MAX_CONTENT_HEIGHT {
                break;
            }

            height = (height + row_height).min(MAX_CONTENT_HEIGHT);
        }

        // Automatic scrollbars impose a minimum height even when hidden.
        self.frame
            .set_vscrollbar_policy(if entries.len() as i32 * row_height > height {
                PolicyType::Automatic
            } else {
                PolicyType::Never
            });
        self.frame.set_min_content_height(height);
    }
}

struct ApplicationRow {
    widget: GtkBox,
    icon: Image,
    title: Label,
    subtitle: Label,
}

impl ApplicationRow {
    fn new() -> Self {
        let icon = Image::new();
        icon.set_pixel_size(24);
        icon.add_css_class("app-icon");

        let title = Label::new(None);
        title.add_css_class("result-title");
        title.set_xalign(0.0);
        title.set_ellipsize(EllipsizeMode::End);

        let subtitle = Label::new(None);
        subtitle.add_css_class("result-subtitle");
        subtitle.set_xalign(0.0);
        subtitle.set_hexpand(true);
        subtitle.set_ellipsize(EllipsizeMode::End);

        let widget = GtkBox::new(Orientation::Horizontal, 10);
        widget.add_css_class("application-result");
        widget.set_valign(Align::Center);
        widget.append(&icon);
        widget.append(&title);
        widget.append(&subtitle);

        Self {
            widget,
            icon,
            title,
            subtitle,
        }
    }

    fn update(&self, entry: &Entry) {
        let icon = entry.icon();
        self.icon.set_paintable(icon);
        self.icon.set_visible(icon.is_some());
        self.title.set_text(entry.title());

        if let Some(text) = entry.subtitle() {
            self.subtitle.set_text(text);
            self.subtitle.set_visible(true);
        } else {
            self.subtitle.set_visible(false);
        }
    }
}
