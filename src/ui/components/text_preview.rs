use gtk::prelude::*;
use gtk::{Orientation, PolicyType, ScrolledWindow, TextView, WrapMode};

use super::SmoothScroll;

pub struct TextPreview {
    frame: ScrolledWindow,
    text: TextView,
}

impl TextPreview {
    pub fn new() -> Self {
        let text = TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .focusable(false)
            .wrap_mode(WrapMode::WordChar)
            .left_margin(20)
            .right_margin(20)
            .top_margin(12)
            .bottom_margin(20)
            .pixels_above_lines(3)
            .css_classes(["text-preview"])
            .build();

        let frame = ScrolledWindow::builder()
            .child(&text)
            .hscrollbar_policy(PolicyType::Never)
            .vexpand(true)
            .focusable(false)
            .build();
        SmoothScroll::attach(&frame, Orientation::Vertical);

        Self { frame, text }
    }

    pub const fn widget(&self) -> &ScrolledWindow {
        &self.frame
    }

    pub fn set_text(&self, text: &str) {
        let buffer = self.text.buffer();
        buffer.set_text(text);
        self.text
            .scroll_to_iter(&mut buffer.start_iter(), 0.0, false, 0.0, 0.0);
    }
}
