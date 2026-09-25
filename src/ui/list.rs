use std::cell::RefCell;
use std::rc::Rc;

use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Image, Label, ListItem, ListScrollFlags, ListView, Orientation,
    PolicyType, ScrolledWindow, SignalListItemFactory, SingleSelection, gio, glib,
};

use crate::catalog::Entry;

const VISIBLE_ROWS: usize = 9;
const MAX_CONTENT_HEIGHT: i32 = 400;

pub struct ApplicationList {
    root: GtkBox,
    list: ListView,
    frame: ScrolledWindow,
    model: gio::ListStore,
    selection: SingleSelection,
    empty: Label,
    entries: RefCell<Vec<Rc<Entry>>>,
}

impl ApplicationList {
    pub fn new() -> Rc<Self> {
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

        Rc::new(Self {
            root,
            list,
            frame,
            model,
            selection,
            empty,
            entries: RefCell::new(Vec::new()),
        })
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn set_entries(&self, entries: Vec<Rc<Entry>>, selected_id: Option<&str>) {
        let selected = selected_id
            .and_then(|id| entries.iter().position(|entry| entry.id() == Some(id)))
            .or_else(|| (!entries.is_empty()).then_some(0));
        let has_entries = !entries.is_empty();

        let (prefix, removed, inserted) = {
            let mut current = self.entries.borrow_mut();
            let old = std::mem::replace(&mut *current, entries);
            let prefix = old
                .iter()
                .zip(current.iter())
                .take_while(|(left, right)| Rc::ptr_eq(left, right))
                .count();
            let suffix = old[prefix..]
                .iter()
                .rev()
                .zip(current[prefix..].iter().rev())
                .take_while(|(left, right)| Rc::ptr_eq(left, right))
                .count();
            let inserted = current[prefix..current.len() - suffix]
                .iter()
                .cloned()
                .map(glib::BoxedAnyObject::new)
                .collect::<Vec<_>>();

            (prefix, old.len() - prefix - suffix, inserted)
        };

        if removed != 0 || !inserted.is_empty() {
            self.model.splice(prefix as u32, removed as u32, &inserted);
        }

        self.empty.set_visible(!has_entries);
        self.frame.set_visible(has_entries);
        self.update_height();
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
        self.entries
            .borrow()
            .get(self.selection.selected() as usize)
            .cloned()
    }

    pub fn move_selection(&self, offset: i32) {
        let count = self.entries.borrow().len() as i32;
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

    pub fn connect_activate(self: &Rc<Self>, activate: impl Fn(Rc<Entry>) + 'static) {
        self.list.connect_activate(glib::clone!(
            #[weak(rename_to = results)]
            self,
            move |_, position| {
                if let Some(entry) = results.entries.borrow().get(position as usize).cloned() {
                    activate(entry);
                }
            }
        ));
    }

    fn update_height(&self) {
        let entries = self.entries.borrow();
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
        icon.set_pixel_size(28);
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
