use std::cell::RefCell;
use std::rc::Rc;

use gtk::pango::EllipsizeMode;
use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Image, Label, ListBox, ListBoxRow, Orientation, PolicyType,
    ScrolledWindow, SelectionMode, Viewport,
};

use crate::catalog::{Catalog, Entry};
use crate::history::History;

const VISIBLE_ROWS: usize = 9;

/// Reusable result rows and the snapshot currently backing them.
pub struct Applications {
    root: GtkBox,
    list: ListBox,
    frame: ScrolledWindow,
    viewport: Viewport,
    empty: Label,
    state: RefCell<Results>,
}

impl Applications {
    pub fn new(catalog: Catalog) -> Rc<Self> {
        let list = ListBox::new();
        list.set_selection_mode(SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.set_focusable(false);
        list.set_focus_on_click(false);
        list.add_css_class("results-list");

        let viewport = Viewport::builder().child(&list).build();

        let frame = ScrolledWindow::builder()
            .child(&viewport)
            .propagate_natural_height(true)
            .hscrollbar_policy(PolicyType::Never)
            .focusable(false)
            .build();
        frame.add_css_class("results-frame");

        let empty = Label::new(Some("No result"));
        empty.add_css_class("empty-state");
        empty.set_valign(Align::Center);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&frame);
        root.append(&empty);

        let rows = (0..catalog.len())
            .map(|_| {
                let row = ResultRow::new();
                list.append(&row.widget);

                row
            })
            .collect();

        let applications = Rc::new(Self {
            root,
            list,
            frame,
            viewport,
            empty,
            state: RefCell::new(Results {
                catalog,
                history: History::load(),
                rows,
                matches: Vec::new(),
            }),
        });

        let weak = Rc::downgrade(&applications);
        applications.frame.hadjustment().connect_changed(move |_| {
            if let Some(applications) = weak.upgrade() {
                applications.update_height();

                // Width changes arrive during allocation; resize once it
                // finishes.
                let frame = applications.frame.clone();
                gtk::glib::idle_add_local_once(move || frame.queue_resize());
            }
        });

        applications.set_query("");

        applications
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn set_query(&self, query: &str) {
        self.update_results(query, None);
    }

    pub fn record_launch(&self, entry: &Entry, query: &str) {
        let Some(id) = entry.id() else {
            return;
        };

        self.state.borrow_mut().history.record(id.as_str(), query);

        self.set_query(query);
    }

    pub fn clear_history(&self, query: &str) {
        self.state.borrow_mut().history.clear();
        self.set_query(query);
    }

    pub fn replace_catalog(&self, catalog: Catalog, query: &str) {
        let selected = self.selected_entry().and_then(|entry| entry.id());
        let count = catalog.len();

        {
            let mut state = self.state.borrow_mut();
            state.catalog = catalog;

            while state.rows.len() < count {
                let row = ResultRow::new();
                self.list.append(&row.widget);
                state.rows.push(row);
            }

            while state.rows.len() > count {
                let row = state.rows.pop().unwrap();
                self.list.remove(&row.widget);
            }
        }

        self.update_results(query, selected.as_deref());
    }

    fn update_results(&self, query: &str, selected_id: Option<&str>) {
        let mut state = self.state.borrow_mut();
        let mut matches = state.catalog.search(query);
        state.history.sort(&mut matches, query);

        state.matches = matches;
        for (index, row) in state.rows.iter().enumerate() {
            row.bind(state.matches.get(index).map(Rc::as_ref));
        }

        let selected = selected_id.and_then(|id| {
            state
                .matches
                .iter()
                .position(|entry| entry.id().as_deref() == Some(id))
        });
        let row =
            (!state.matches.is_empty()).then(|| state.rows[selected.unwrap_or(0)].widget.clone());
        drop(state);

        self.update_height();
        self.frame.set_visible(row.is_some());
        self.empty.set_visible(row.is_none());

        self.list.select_row(row.as_ref());
        if selected.is_some() {
            self.viewport.scroll_to(row.as_ref().unwrap(), None);
        } else {
            self.frame.vadjustment().set_value(0.0);
        }
    }

    fn update_height(&self) {
        // The viewport's page width already excludes CSS edges and scrollbars.
        let width = self.frame.hadjustment().page_size() as i32;
        let width = if width > 0 { width } else { -1 };

        // ListBox allocates each row at its measured minimum height.
        let height = self
            .state
            .borrow()
            .rows
            .iter()
            .take(VISIBLE_ROWS)
            .map(|row| row.widget.measure(Orientation::Vertical, width).0)
            .sum();

        self.frame.set_max_content_height(height);
    }

    pub fn selected_entry(&self) -> Option<Rc<Entry>> {
        let row = self.list.selected_row()?;

        Some(Rc::clone(
            &self.state.borrow().matches[row.index() as usize],
        ))
    }

    pub fn connect_activate(self: &Rc<Self>, activate: impl Fn(Rc<Entry>) + 'static) {
        let applications = Rc::downgrade(self);

        self.list.connect_row_activated(move |_, row| {
            let Some(applications) = applications.upgrade() else {
                return;
            };

            let entry = Rc::clone(&applications.state.borrow().matches[row.index() as usize]);

            activate(entry);
        });
    }

    pub fn move_selection(&self, offset: i32) {
        let state = self.state.borrow();
        let count = state.matches.len();
        if count == 0 {
            return;
        }

        let next = match self.list.selected_row() {
            Some(row) => (row.index() + offset).rem_euclid(count as i32) as usize,
            None if offset > 0 => 0,
            None => count - 1,
        };
        let row = state.rows[next].widget.clone();
        drop(state);

        self.list.select_row(Some(&row));
        self.viewport.scroll_to(&row, None);
    }
}

struct Results {
    catalog: Catalog,
    history: History,
    rows: Vec<ResultRow>,
    matches: Vec<Rc<Entry>>,
}

struct ResultRow {
    widget: ListBoxRow,
    icon: Image,
    title: Label,
    subtitle: Label,
}

impl ResultRow {
    fn new() -> Self {
        let row = ListBoxRow::new();
        row.set_activatable(true);
        row.set_selectable(true);
        row.set_focusable(false);
        row.set_focus_on_click(false);

        let icon = Image::new();
        icon.set_pixel_size(28);
        icon.add_css_class("app-icon");
        icon.set_valign(Align::Center);

        let title = Label::new(None);
        title.add_css_class("result-title");
        title.set_xalign(0.0);
        title.set_valign(Align::Center);

        let subtitle = Label::new(None);
        subtitle.add_css_class("result-subtitle");
        subtitle.set_xalign(0.0);
        subtitle.set_hexpand(true);
        subtitle.set_ellipsize(EllipsizeMode::End);
        subtitle.set_valign(Align::Center);

        let content = GtkBox::new(Orientation::Horizontal, 10);
        content.set_valign(Align::Center);
        content.append(&icon);
        content.append(&title);
        content.append(&subtitle);

        row.set_child(Some(&content));

        Self {
            widget: row,
            icon,
            title,
            subtitle,
        }
    }

    fn bind(&self, entry: Option<&Entry>) {
        let Some(entry) = entry else {
            self.widget.set_visible(false);

            return;
        };

        match entry.icon() {
            Some(icon) => self.icon.set_from_gicon(icon),
            None => self.icon.clear(),
        }

        self.title.set_text(entry.title());
        if let Some(subtitle) = entry.subtitle() {
            self.subtitle.set_text(subtitle);
            self.subtitle.set_visible(true);
        } else {
            self.subtitle.set_visible(false);
        }
        self.widget.set_visible(true);
    }
}
