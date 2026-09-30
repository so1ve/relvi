use std::cell::Cell;

use gtk::prelude::*;
use gtk::{
    Adjustment, ListScrollFlags, ListView, Orientation, ScrolledWindow, SingleSelection, glib,
};

use super::SmoothScroll;

#[derive(Clone, Copy)]
struct ScrollSelection {
    scroll_offset: f64,
    selected: u32,
}

impl ScrollSelection {
    fn update(&mut self, adjustment: &Adjustment, selection: &SingleSelection) -> u32 {
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
                }

                return selected;
            }
        }

        self.selected = selected.clamp(first, last);

        self.selected
    }
}

pub struct ListNavigation {
    list: ListView,
    selection: SingleSelection,
    scroll: SmoothScroll,
}

impl ListNavigation {
    pub fn attach(list: &ListView, selection: &SingleSelection, frame: &ScrolledWindow) -> Self {
        let scroll = SmoothScroll::attach(frame, Orientation::Vertical);
        let adjustment = frame.vadjustment();
        let previous = Cell::new(ScrollSelection {
            scroll_offset: adjustment.value(),
            selected: selection.selected(),
        });
        adjustment.connect_value_changed(glib::clone!(
            #[weak]
            selection,
            move |adjustment| {
                let mut state = previous.get();
                let selected = state.update(adjustment, &selection);
                previous.set(state);
                selection.set_selected(selected);
            }
        ));

        selection.connect_selected_item_notify(glib::clone!(
            #[weak]
            list,
            #[weak]
            adjustment,
            move |selection| {
                if selection.selected() != gtk::INVALID_LIST_POSITION {
                    list.scroll_to(selection.selected(), ListScrollFlags::FOCUS, None);

                    // Focusing also changes GTK's scroll anchor. Reset it from
                    // the viewport so changing focus does not move the list.
                    adjustment.emit_by_name::<()>("value-changed", &[]);
                }
            }
        ));

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
            self.list.scroll_to(position, ListScrollFlags::NONE, None);
        }
    }

    pub fn move_selection(&self, offset: i32) {
        let count = self.selection.n_items() as i32;
        if count == 0 {
            return;
        }

        let current = self.selection.selected();
        let next = if current == gtk::INVALID_LIST_POSITION {
            if offset < 0 { count - 1 } else { 0 }
        } else {
            (current as i32 + offset).rem_euclid(count)
        };
        self.select(Some(next as u32));

        if self.list.is_focusable() {
            self.list.grab_focus();
        }
    }

    pub fn scroll_pages(&self, pages: f64) {
        if self.list.is_focusable() {
            self.list.grab_focus();
        }

        self.scroll.scroll_pages(pages);
    }
}
