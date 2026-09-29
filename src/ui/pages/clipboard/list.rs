use std::sync::Arc;

use gtk::prelude::*;
use gtk::{
    Align, Label, ListItem, ListScrollFlags, ListView, Orientation, Overlay, PolicyType,
    ScrolledWindow, SignalListItemFactory, SingleSelection, gio, glib,
};

use crate::clipboard::Entry;
use crate::ui::components::SmoothScroll;

pub struct HistoryList {
    root: Overlay,
    list: ListView,
    scroll: SmoothScroll,
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
        list.set_single_click_activate(false);

        let frame = ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(PolicyType::Never)
            .vexpand(true)
            .focusable(false)
            .build();
        frame.add_css_class("list-frame");
        let scroll = SmoothScroll::new(&frame, Orientation::Vertical);
        scroll.follow_selection(&selection);

        let empty = Label::new(Some("Loading clipboard…"));
        empty.add_css_class("empty-state");
        empty.set_halign(Align::Center);
        empty.set_valign(Align::Center);
        empty.set_can_target(false);

        let root = Overlay::new();
        root.set_child(Some(&frame));
        root.add_overlay(&empty);

        selection.connect_selected_item_notify(glib::clone!(
            #[weak]
            list,
            move |selection| {
                if selection.selected() != gtk::INVALID_LIST_POSITION
                    && list.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN)
                {
                    list.scroll_to(selection.selected(), ListScrollFlags::FOCUS, None);
                }
            }
        ));

        Self {
            root,
            list,
            scroll,
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
        self.scroll.stop();
        self.model.splice(0, self.model.n_items(), &items);
        self.selection
            .set_selected(selected.map_or(gtk::INVALID_LIST_POSITION, |index| index as u32));
        if let Some(selected) = selected {
            self.list
                .scroll_to(selected as u32, ListScrollFlags::NONE, None);
        }
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
        self.scroll.stop();
        let count = self.model.n_items() as i32;
        if count == 0 {
            return;
        }

        let next = (self.selection.selected() as i32 + offset).rem_euclid(count) as u32;
        self.list.grab_focus();
        self.selection.set_selected(next);
        self.list.scroll_to(next, ListScrollFlags::FOCUS, None);
    }

    pub fn scroll_pages(&self, pages: f64) {
        self.move_selection(0);
        self.scroll.scroll_pages(pages);
    }
}
