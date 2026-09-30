use emojis::Emoji;
use gtk::prelude::*;
use gtk::{
    GridView, Label, ListItem, Overlay, PolicyType, ScrolledWindow, SignalListItemFactory,
    SingleSelection, gio, glib,
};

use crate::ui::components::ListNavigation;

const COLUMNS: u32 = 10;

pub struct EmojiGrid {
    root: Overlay,
    view: GridView,
    model: gio::ListStore,
    selection: SingleSelection,
    navigation: ListNavigation<GridView>,
    empty: Label,
}

impl EmojiGrid {
    pub fn new() -> Self {
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = SingleSelection::new(Some(model.clone()));
        selection.set_autoselect(false);

        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<ListItem>().unwrap();
            let label = Label::new(None);
            label.add_css_class("emoji-cell");
            item.set_child(Some(&label));

            item.connect_item_notify(move |item| {
                let Some(object) = item.item() else {
                    return;
                };
                let object = object.downcast::<glib::BoxedAnyObject>().unwrap();
                let emoji = *object.borrow::<&'static Emoji>();
                label.set_text(emoji.as_str());
                label.set_tooltip_text(Some(emoji.name()));
                label.update_property(&[gtk::accessible::Property::Label(emoji.name())]);
            });
        });

        let view = GridView::new(Some(selection.clone()), Some(factory));
        view.set_min_columns(COLUMNS);
        view.set_max_columns(COLUMNS);
        view.set_single_click_activate(true);
        view.add_css_class("item-grid");

        let frame = ScrolledWindow::builder()
            .child(&view)
            .hscrollbar_policy(PolicyType::Never)
            .height_request(288)
            .vexpand(true)
            .focusable(false)
            .css_classes(["list-frame"])
            .build();
        let navigation = ListNavigation::attach(&view, &selection, &frame);

        let empty = Label::new(Some("Loading emoji…"));
        empty.add_css_class("empty-state");
        empty.set_can_target(false);
        let root = Overlay::new();
        root.set_child(Some(&frame));
        root.add_overlay(&empty);

        Self {
            root,
            view,
            model,
            selection,
            navigation,
            empty,
        }
    }

    pub const fn widget(&self) -> &Overlay {
        &self.root
    }

    pub fn show(&self, emojis: Vec<&'static Emoji>, selected: Option<&Emoji>, empty: &str) {
        let selected = selected
            .and_then(|selected| {
                let base = selected
                    .with_skin_tone(emojis::SkinTone::Default)
                    .unwrap_or(selected);

                emojis.iter().position(|emoji| {
                    emoji
                        .with_skin_tone(emojis::SkinTone::Default)
                        .unwrap_or(emoji)
                        == base
                })
            })
            .or_else(|| (!emojis.is_empty()).then_some(0));
        self.empty.set_text(empty);
        self.empty.set_visible(emojis.is_empty());
        let items: Vec<_> = emojis.into_iter().map(glib::BoxedAnyObject::new).collect();
        self.model.splice(0, self.model.n_items(), &items);
        self.navigation.select(selected.map(|index| index as u32));
    }

    pub fn selected(&self) -> Option<&'static Emoji> {
        self.selection.selected_item().map(|object| {
            let object = object.downcast::<glib::BoxedAnyObject>().unwrap();

            *object.borrow::<&'static Emoji>()
        })
    }

    pub fn connect_changed(&self, changed: impl Fn() + 'static) {
        self.selection
            .connect_selected_item_notify(move |_| changed());
    }

    pub fn connect_activate(&self, activate: impl Fn(&'static Emoji) + 'static) {
        self.view.connect_activate(move |view, position| {
            let item = view
                .model()
                .unwrap()
                .item(position)
                .unwrap()
                .downcast::<glib::BoxedAnyObject>()
                .unwrap();
            activate(*item.borrow::<&'static Emoji>());
        });
    }

    pub fn move_items(&self, offset: i32) {
        self.navigation.move_items(offset);
    }

    pub fn move_rows(&self, offset: i32) {
        self.navigation.move_rows(offset);
    }

    pub fn scroll_pages(&self, pages: f64) {
        self.navigation.scroll_pages(pages);
    }
}
