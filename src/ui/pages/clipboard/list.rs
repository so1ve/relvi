use std::sync::Arc;

use gtk::prelude::*;
use gtk::{
    Align, Label, ListItem, ListView, Overlay, PolicyType, ScrolledWindow, SignalListItemFactory,
    SingleSelection, gio, glib,
};

use crate::clipboard::Entry;
use crate::ui::components::ListNavigation;

pub struct HistoryList {
    root: Overlay,
    navigation: ListNavigation,
    model: gio::ListStore,
    selection: SingleSelection,
    empty: Label,
}

impl HistoryList {
    pub fn new() -> Self {
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = SingleSelection::new(Some(model.clone()));
        selection.set_autoselect(false);

        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<ListItem>().unwrap();
            let label = Label::new(None);
            label.add_css_class("history-item");
            label.set_xalign(0.0);
            label.set_wrap(true);
            label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            label.set_lines(2);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(18);
            item.set_child(Some(&label));

            item.connect_item_notify(move |item| {
                let Some(object) = item.item() else {
                    return;
                };
                let object = object.downcast::<glib::BoxedAnyObject>().unwrap();
                let entry = object.borrow::<Arc<Entry>>();
                label.set_text(entry.preview.trim_end());
            });
        });

        let list = ListView::new(Some(selection.clone()), Some(factory));
        list.add_css_class("item-list");
        list.set_focusable(true);
        list.set_single_click_activate(false);

        let frame = ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(PolicyType::Never)
            .vexpand(true)
            .focusable(false)
            .build();
        frame.add_css_class("list-frame");
        let navigation = ListNavigation::attach(&list, &selection, &frame);

        let empty = Label::new(Some("Loading clipboard…"));
        empty.add_css_class("empty-state");
        empty.set_halign(Align::Center);
        empty.set_valign(Align::Center);
        empty.set_can_target(false);

        let root = Overlay::new();
        root.set_child(Some(&frame));
        root.add_overlay(&empty);

        Self {
            root,
            navigation,
            model,
            selection,
            empty,
        }
    }

    pub const fn widget(&self) -> &Overlay {
        &self.root
    }

    pub fn show(
        &self,
        entries: impl Iterator<Item = Arc<Entry>>,
        selected: Option<&str>,
        empty_text: &str,
    ) {
        let items: Vec<_> = entries.map(glib::BoxedAnyObject::new).collect();
        let selected = selected
            .and_then(|text| {
                items
                    .iter()
                    .position(|item| item.borrow::<Arc<Entry>>().text.as_ref() == text)
            })
            .or_else(|| (!items.is_empty()).then_some(0));

        self.empty.set_text(empty_text);
        self.empty.set_visible(items.is_empty());
        self.model.splice(0, self.model.n_items(), &items);
        self.navigation.select(selected.map(|index| index as u32));
    }

    pub fn connect_changed(&self, changed: impl Fn() + 'static) {
        self.selection
            .connect_selected_item_notify(move |_| changed());
    }

    pub fn selected(&self) -> Option<Arc<Entry>> {
        self.selection.selected_item().map(|object| {
            let object = object.downcast::<glib::BoxedAnyObject>().unwrap();

            Arc::clone(&object.borrow::<Arc<Entry>>())
        })
    }

    pub fn move_selection(&self, offset: i32) {
        self.navigation.move_selection(offset);
    }

    pub fn scroll_pages(&self, pages: f64) {
        self.navigation.scroll_pages(pages);
    }
}
