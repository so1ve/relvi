use std::cell::Cell;

use gtk::prelude::*;
use gtk::{
    Adjustment, GridView, ListScrollFlags, ListView, Orientation, ScrolledWindow, SingleSelection,
    Widget, glib,
};

use super::SmoothScroll;

pub trait ItemView: IsA<Widget> + Clone + 'static {
    fn columns(&self) -> u32;
    fn reveal(&self, position: u32, flags: ListScrollFlags);
}

impl ItemView for ListView {
    fn columns(&self) -> u32 {
        1
    }

    fn reveal(&self, position: u32, flags: ListScrollFlags) {
        self.scroll_to(position, flags, None);
    }
}

impl ItemView for GridView {
    fn columns(&self) -> u32 {
        self.max_columns()
    }

    fn reveal(&self, position: u32, flags: ListScrollFlags) {
        self.scroll_to(position, flags, None);
    }
}

#[derive(Clone, Copy)]
struct ScrollSelection {
    scroll_offset: f64,
    selected: u32,
}

impl ScrollSelection {
    fn update(
        &mut self,
        adjustment: &Adjustment,
        selection: &SingleSelection,
        columns: u32,
    ) -> u32 {
        let previous_offset = self.scroll_offset;
        self.scroll_offset = adjustment.value().floor();

        let selected = selection.selected();
        let page_size = adjustment.page_size();
        let content_height = adjustment.upper() - adjustment.lower();

        if selected == gtk::INVALID_LIST_POSITION || content_height <= page_size {
            self.selected = selected;

            return selected;
        }

        let count = selection.n_items();
        let rows = count.div_ceil(columns);
        let row_height = content_height / f64::from(rows);
        let top = self.scroll_offset - adjustment.lower();
        let first = ((top / row_height).ceil() as u32).min(rows - 1);
        let last = (((top + page_size) / row_height).floor() as u32)
            .saturating_sub(1)
            .max(first)
            .min(rows - 1);

        if selected != self.selected {
            let position = f64::from(selected / columns) * row_height;

            // Keep a selection made by keyboard navigation while GTK reveals
            // it.
            if position < previous_offset || position + row_height > previous_offset + page_size {
                if (first..=last).contains(&(selected / columns)) {
                    self.selected = selected;
                }

                return selected;
            }
        }

        self.selected =
            ((selected / columns).clamp(first, last) * columns + selected % columns).min(count - 1);

        self.selected
    }
}

pub struct ListNavigation<V> {
    list: V,
    selection: SingleSelection,
    scroll: SmoothScroll,
}

impl<V: ItemView> ListNavigation<V> {
    pub fn attach(list: &V, selection: &SingleSelection, frame: &ScrolledWindow) -> Self {
        let scroll = SmoothScroll::attach(frame, Orientation::Vertical);
        let adjustment = frame.vadjustment();
        let columns = list.columns();
        let previous = Cell::new(ScrollSelection {
            scroll_offset: adjustment.value(),
            selected: selection.selected(),
        });
        adjustment.connect_value_changed(glib::clone!(
            #[weak]
            selection,
            move |adjustment| {
                let mut state = previous.get();
                let selected = state.update(adjustment, &selection, columns);
                previous.set(state);
                selection.set_selected(selected);
            }
        ));

        let weak_list = list.downgrade();
        selection.connect_selected_item_notify(move |selection| {
            let Some(list) = weak_list.upgrade() else {
                return;
            };
            if selection.selected() != gtk::INVALID_LIST_POSITION {
                list.reveal(selection.selected(), ListScrollFlags::FOCUS);
            }
        });

        Self {
            list: list.clone(),
            selection: selection.clone(),
            scroll,
        }
    }

    pub fn select(&self, position: Option<u32>) {
        self.scroll.stop();
        self.selection
            .set_selected(position.unwrap_or(gtk::INVALID_LIST_POSITION));
        if let Some(position) = position {
            self.list.reveal(position, ListScrollFlags::NONE);
        }
    }

    pub fn move_items(&self, offset: i32) {
        self.move_selection(offset, 1);
    }

    pub fn move_rows(&self, offset: i32) {
        self.move_selection(offset, self.list.columns() as i32);
    }

    pub fn scroll_pages(&self, pages: f64) {
        if self.list.is_focusable() {
            self.list.grab_focus();
        }

        self.scroll.scroll_pages(pages);
    }

    fn move_selection(&self, offset: i32, row_width: i32) {
        let count = self.selection.n_items() as i32;
        if count == 0 {
            return;
        }

        let current = self.selection.selected();
        let next = if current == gtk::INVALID_LIST_POSITION {
            if offset < 0 { count - 1 } else { 0 }
        } else {
            let row = (current as i32 / row_width + offset)
                .rem_euclid((count + row_width - 1) / row_width);

            (row * row_width + current as i32 % row_width).min(count - 1)
        };
        self.select(Some(next as u32));

        if self.list.is_focusable() {
            self.list.grab_focus();
        }
    }
}
