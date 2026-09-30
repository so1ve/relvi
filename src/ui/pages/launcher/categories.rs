use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GtkBox, Orientation, PolicyType, ScrolledWindow, ToggleButton, glib};

use crate::ui::components::SmoothScroll;

type Changed = Box<dyn Fn()>;

pub struct Categories {
    frame: ScrolledWindow,
    scroll: SmoothScroll,
    row: GtkBox,
    buttons: RefCell<Vec<(Option<&'static str>, ToggleButton)>>,
    changed: RefCell<Option<Changed>>,
}

impl Categories {
    pub fn new(available: &[&'static str]) -> Rc<Self> {
        let row = GtkBox::new(Orientation::Horizontal, 4);
        let frame = ScrolledWindow::builder()
            .child(&row)
            .hscrollbar_policy(PolicyType::External)
            .vscrollbar_policy(PolicyType::Never)
            .propagate_natural_height(true)
            .focusable(false)
            .build();
        frame.add_css_class("category-bar");
        frame.set_tooltip_text(Some(
            "Ctrl+H / Shift+Tab: previous category\nCtrl+L / Tab: next category",
        ));

        let scroll = SmoothScroll::attach(&frame, Orientation::Horizontal);

        let categories = Rc::new(Self {
            frame,
            scroll,
            row,
            buttons: RefCell::new(Vec::new()),
            changed: RefCell::new(None),
        });
        categories.set_categories(available);

        categories
    }

    pub const fn widget(&self) -> &ScrolledWindow {
        &self.frame
    }

    pub fn selected(&self) -> Option<&'static str> {
        self.buttons
            .borrow()
            .iter()
            .find(|(_, button)| button.is_active())
            .and_then(|(category, _)| *category)
    }

    pub fn set_categories(self: &Rc<Self>, available: &[&'static str]) {
        self.scroll.stop();

        let selected = self
            .selected()
            .filter(|category| available.contains(category));
        self.buttons.borrow_mut().clear();

        while let Some(button) = self.row.first_child() {
            self.row.remove(&button);
        }

        let mut buttons = Vec::with_capacity(available.len() + 1);

        for category in std::iter::once(None).chain(available.iter().copied().map(Some)) {
            let label = match category {
                None => "All",
                Some("AudioVideo") => "Media",
                Some("Game") => "Games",
                Some("Network") => "Internet",
                Some("Utility") => "Utilities",
                Some(name) => name,
            };
            let button = ToggleButton::with_label(label);
            button.add_css_class("category-tab");
            button.set_focusable(false);
            button.set_focus_on_click(false);

            if let Some((_, first)) = buttons.first() {
                button.set_group(Some(first));
            }
            button.set_active(category == selected);
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = categories)]
                self,
                move |button| {
                    if !button.is_active() {
                        return;
                    }

                    categories.reveal(button);

                    if let Some(changed) = categories.changed.borrow().as_ref() {
                        changed();
                    }
                }
            ));

            self.row.append(&button);
            buttons.push((category, button));
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

    pub fn connect_changed(&self, changed: impl Fn() + 'static) {
        self.changed.replace(Some(Box::new(changed)));
    }

    fn reveal(&self, button: &ToggleButton) {
        self.scroll.stop();

        if let Some(bounds) = button.compute_bounds(&self.row) {
            self.frame.hadjustment().clamp_page(
                f64::from(bounds.x()),
                f64::from(bounds.x() + bounds.width()),
            );
        }
    }
}
