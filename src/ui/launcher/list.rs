use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{
    Adjustment, Align, Box as GtkBox, IconTheme, Image, Label, ListItem, ListScrollFlags, ListView,
    Orientation, PolicyType, ScrolledWindow, SignalListItemFactory, SingleSelection, gio, glib,
};

use super::super::scroll::SmoothScroll;
use crate::catalog::Entry;

const VISIBLE_ROWS: usize = 12;
const MAX_CONTENT_HEIGHT: i32 = 480;

const FALLBACK_ICON: &str = "application-x-executable-symbolic";

struct ResultItem {
    entry: Rc<Entry>,
    icon: Option<gtk::IconPaintable>,
}

struct ResultRow {
    widget: GtkBox,
    icon: Image,
    title: Label,
    subtitle: Label,
}

impl ResultRow {
    fn new() -> Self {
        let icon = Image::new();
        icon.set_pixel_size(24);
        icon.add_css_class("result-icon");

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
        widget.add_css_class("result-row");
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

    fn update(&self, item: &ResultItem) {
        let entry = &item.entry;
        let icon = item.icon.as_ref();
        self.icon.set_paintable(icon);
        self.icon.set_visible(icon.is_some());
        self.title.set_text(&entry.title);

        if let Some(text) = entry.subtitle.as_deref() {
            self.subtitle.set_text(text);
            self.subtitle.set_visible(true);
        } else {
            self.subtitle.set_visible(false);
        }
    }
}

fn resolve_icon(
    icon: Option<&gio::Icon>,
    theme: &IconTheme,
    scale: i32,
) -> Option<gtk::IconPaintable> {
    let icon = if let Some(icon) = icon
        && theme.has_gicon(icon)
    {
        icon.clone()
    } else if theme.has_icon(FALLBACK_ICON) {
        gio::ThemedIcon::new(FALLBACK_ICON).upcast()
    } else {
        return None;
    };

    Some(theme.lookup_by_gicon(
        &icon,
        24,
        scale,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::empty(),
    ))
}

#[derive(Clone, Copy)]
struct SelectionAnchor {
    scroll_offset: f64,
    selected: u32,
    row_offset: f64,
}

impl SelectionAnchor {
    fn update(&mut self, adjustment: &Adjustment, selection: &SingleSelection) -> u32 {
        let previous_offset = self.scroll_offset;
        self.scroll_offset = adjustment.value().floor();

        let selected = selection.selected();
        let page_size = adjustment.page_size();
        let content_height = adjustment.upper() - adjustment.lower();

        if selected == gtk::INVALID_LIST_POSITION || content_height <= page_size {
            self.selected = selected;
            self.row_offset = 0.0;

            return selected;
        }

        let count = selection.n_items();
        let row_height = content_height / f64::from(count);
        let top = self.scroll_offset - adjustment.lower();
        let first = ((top / row_height).ceil() as u32).min(count - 1);
        let last = (((top + page_size) / row_height).floor() as u32)
            .saturating_sub(1)
            .max(first)
            .min(count - 1);

        if selected != self.selected {
            let position = f64::from(selected) * row_height;

            // Keep a selection made by keyboard navigation while GTK reveals
            // it.
            if position < previous_offset || position + row_height > previous_offset + page_size {
                if (first..=last).contains(&selected) {
                    self.selected = selected;
                    self.row_offset = position - top;
                }

                return selected;
            }

            self.row_offset = position - previous_offset;
        }

        self.selected = (((top + self.row_offset) / row_height).round() as u32).clamp(first, last);

        self.selected
    }
}

pub struct ResultList {
    root: GtkBox,
    list: ListView,
    frame: ScrolledWindow,
    scroll: SmoothScroll,
    model: gio::ListStore,
    items: RefCell<Vec<glib::BoxedAnyObject>>,
    selection: SingleSelection,
    empty: Label,
    measure_row: ResultRow,
}

impl ResultList {
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
        let scroll = SmoothScroll::new(&frame, Orientation::Vertical);
        let adjustment = frame.vadjustment();
        let anchor = Cell::new(SelectionAnchor {
            scroll_offset: adjustment.value(),
            selected: selection.selected(),
            row_offset: 0.0,
        });
        adjustment.connect_value_changed(glib::clone!(
            #[weak]
            selection,
            move |adjustment| {
                let mut state = anchor.get();
                let selected = state.update(adjustment, &selection);
                anchor.set(state);
                selection.set_selected(selected);
            }
        ));

        let empty = Label::new(Some("No result"));
        empty.add_css_class("empty-state");
        empty.set_visible(false);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&frame);
        root.append(&empty);

        factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<ListItem>().unwrap();
            let row = ResultRow::new();
            item.set_child(Some(&row.widget));

            item.connect_item_notify(move |item| {
                let Some(object) = item.item() else {
                    return;
                };

                let object = object.downcast::<glib::BoxedAnyObject>().unwrap();
                let item = object.borrow::<ResultItem>();

                row.update(&item);
            });
        });

        Self {
            root,
            list,
            frame,
            scroll,
            model,
            items: RefCell::new(Vec::new()),
            selection,
            empty,
            measure_row: ResultRow::new(),
        }
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn load(&self, entries: &[Rc<Entry>], theme: &IconTheme, scale: i32) {
        let items = entries
            .iter()
            .map(|entry| {
                glib::BoxedAnyObject::new(ResultItem {
                    entry: Rc::clone(entry),
                    icon: resolve_icon(entry.icon.as_ref(), theme, scale),
                })
            })
            .collect();
        self.items.replace(items);
    }

    pub fn show_matches(&self, matches: Vec<usize>, selected_id: Option<&str>) {
        self.scroll.stop();

        let items = self.items.borrow();
        let selected = selected_id
            .and_then(|id| {
                matches.iter().position(|&index| {
                    items[index].borrow::<ResultItem>().entry.id.as_deref() == Some(id)
                })
            })
            .or_else(|| (!matches.is_empty()).then_some(0));
        let entries: Vec<_> = matches
            .into_iter()
            .map(|index| items[index].clone())
            .collect();
        let has_entries = !entries.is_empty();
        self.update_height(&entries);

        let previous_len = self.model.n_items() as usize;
        let unchanged =
            |index, entry: &glib::BoxedAnyObject| self.model.item(index as u32).unwrap() == *entry;
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
        let inserted = &entries[prefix..entries.len() - suffix];

        if removed != 0 || !inserted.is_empty() {
            self.model.splice(prefix as u32, removed as u32, inserted);
        }

        self.empty.set_visible(!has_entries);
        self.frame.set_visible(has_entries);
        self.selection
            .set_selected(selected.map_or(gtk::INVALID_LIST_POSITION, |index| index as u32));

        if let Some(selected) = selected {
            self.list
                .scroll_to(selected as u32, ListScrollFlags::NONE, None);
        }
    }

    pub fn selected(&self) -> Option<Rc<Entry>> {
        self.selection.selected_item().map(|object| {
            let entry = object.downcast_ref::<glib::BoxedAnyObject>().unwrap();

            Rc::clone(&entry.borrow::<ResultItem>().entry)
        })
    }

    pub fn scroll_pages(&self, pages: f64) {
        self.scroll.scroll_pages(pages);
    }

    pub fn move_selection(&self, offset: i32) {
        self.scroll.stop();

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

            activate(Rc::clone(&entry.borrow::<ResultItem>().entry));
        });
    }

    fn update_height(&self, entries: &[glib::BoxedAnyObject]) {
        let Some(first) = entries.first() else {
            self.frame.set_min_content_height(0);

            return;
        };

        // Titles and descriptions are single-line, so every row has the same
        // height.
        self.measure_row.update(&first.borrow::<ResultItem>());
        let row_height = self.measure_row.widget.measure(Orientation::Vertical, -1).1;
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
