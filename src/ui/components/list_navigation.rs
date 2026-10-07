use gtk::prelude::*;
use gtk::{
    GridView, ListScrollFlags, ListView, Orientation, ScrolledWindow, SingleSelection, Widget, glib,
};

use super::SmoothScroll;

pub trait ItemView: IsA<Widget> + Clone + 'static {
    fn columns(&self) -> u32;
    fn reveal(&self, position: u32);
}

impl ItemView for ListView {
    fn columns(&self) -> u32 {
        1
    }

    fn reveal(&self, position: u32) {
        self.scroll_to(position, ListScrollFlags::FOCUS, None);
    }
}

impl ItemView for GridView {
    fn columns(&self) -> u32 {
        self.max_columns()
    }

    fn reveal(&self, position: u32) {
        self.scroll_to(position, ListScrollFlags::FOCUS, None);
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
            self.list.reveal(position);
        }
    }

    pub fn move_items(&self, offset: i32) {
        self.move_selection(offset, 1);
    }

    pub fn move_rows(&self, offset: i32) {
        self.move_selection(offset, self.list.columns() as i32);
    }

    pub fn scroll_pages(&self, pages: f64) {
        let weak_list = self.list.downgrade();
        self.scroll.scroll_pages(
            pages,
            glib::clone!(
                #[weak(rename_to = selection)]
                self.selection,
                move |adjustment| {
                    let Some(list) = weak_list.upgrade() else {
                        return;
                    };

                    let count = selection.n_items();
                    let height = adjustment.upper() - adjustment.lower();
                    if count == 0 || height == 0.0 {
                        return;
                    }

                    let columns = list.columns();
                    let rows = count.div_ceil(columns);
                    let row_height = height / f64::from(rows);
                    let top = adjustment.value() - adjustment.lower();
                    let first = ((top / row_height).ceil() as u32).min(rows - 1);
                    let last = (((top + adjustment.page_size()) / row_height).floor() as u32)
                        .saturating_sub(1)
                        .max(first)
                        .min(rows - 1);
                    let current = selection.selected().min(count - 1);
                    let selected = ((current / columns).clamp(first, last) * columns
                        + current % columns)
                        .min(count - 1);

                    selection.set_selected(selected);
                    list.reveal(selected);

                    if list.is_focusable() {
                        list.grab_focus();
                    }
                }
            ),
        );
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
