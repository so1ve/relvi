use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GtkBox, Orientation, PolicyType, ScrolledWindow, ToggleButton, glib};

use super::SmoothScroll;

type Changed = Box<dyn Fn()>;

pub struct CategoryBar<T> {
    frame: ScrolledWindow,
    scroll: SmoothScroll,
    row: GtkBox,
    buttons: RefCell<Vec<(T, ToggleButton)>>,
    changed: Rc<RefCell<Option<Changed>>>,
}

impl<T: Copy + Eq> CategoryBar<T> {
    pub fn new<'a>(items: impl IntoIterator<Item = (T, &'a str)>) -> Self {
        let row = GtkBox::new(Orientation::Horizontal, 4);
        let frame = ScrolledWindow::builder()
            .child(&row)
            .hscrollbar_policy(PolicyType::External)
            .vscrollbar_policy(PolicyType::Never)
            .propagate_natural_height(true)
            .focusable(false)
            .build();
        frame.add_css_class("category-bar");

        let scroll = SmoothScroll::attach(&frame, Orientation::Horizontal);

        let bar = Self {
            frame,
            scroll,
            row,
            buttons: RefCell::new(Vec::new()),
            changed: Rc::new(RefCell::new(None)),
        };
        bar.set_items(items);

        bar
    }

    pub const fn widget(&self) -> &ScrolledWindow {
        &self.frame
    }

    pub fn selected(&self) -> T {
        self.buttons
            .borrow()
            .iter()
            .find(|(_, button)| button.is_active())
            .map(|(category, _)| *category)
            .unwrap()
    }

    /// Replace a nonempty list without emitting a selection change.
    pub fn set_items<'a>(&self, items: impl IntoIterator<Item = (T, &'a str)>) {
        self.scroll.stop();

        let selected = self
            .buttons
            .borrow()
            .iter()
            .find(|(_, button)| button.is_active())
            .map(|(category, _)| *category);
        let items = items.into_iter();
        let mut buttons = Vec::with_capacity(items.size_hint().0);

        for (category, label) in items {
            let button = ToggleButton::builder()
                .label(label)
                .css_classes(["category-tab"])
                .focusable(false)
                .focus_on_click(false)
                .build();

            if let Some((_, first)) = buttons.first() {
                button.set_group(Some(first));
            }
            buttons.push((category, button));
        }

        let selected = buttons
            .iter()
            .position(|(category, _)| Some(*category) == selected)
            .unwrap_or(0);
        buttons[selected].1.set_active(true);

        while let Some(button) = self.row.first_child() {
            self.row.remove(&button);
        }

        let row = &self.row;
        let scroll = &self.scroll;
        let adjustment = self.frame.hadjustment();
        let changed = &self.changed;

        for (_, button) in &buttons {
            button.connect_toggled(glib::clone!(
                #[weak]
                row,
                #[strong]
                scroll,
                #[strong]
                adjustment,
                #[strong]
                changed,
                move |button| {
                    if !button.is_active() {
                        return;
                    }

                    scroll.stop();

                    if let Some(bounds) = button.compute_bounds(&row) {
                        adjustment.clamp_page(
                            f64::from(bounds.x()),
                            f64::from(bounds.x() + bounds.width()),
                        );
                    }

                    if let Some(changed) = changed.borrow().as_ref() {
                        changed();
                    }
                }
            ));

            self.row.append(button);
        }

        self.buttons.replace(buttons);
    }

    pub fn cycle(&self, offset: i32) {
        let button = {
            let buttons = self.buttons.borrow();
            let current = buttons
                .iter()
                .position(|(_, button)| button.is_active())
                .unwrap();
            let next = (current as i32 + offset).rem_euclid(buttons.len() as i32) as usize;

            buttons[next].1.clone()
        };
        button.set_active(true);
    }

    pub fn reset(&self) {
        let first = self.buttons.borrow()[0].1.clone();
        first.set_active(true);
        self.scroll.stop();
        self.frame.hadjustment().set_value(0.0);
    }

    pub fn connect_changed(&self, changed: impl Fn() + 'static) {
        self.changed.replace(Some(Box::new(changed)));
    }
}
