use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, Image, Label, Orientation, gdk, pango};

use crate::clipboard::{Content, Entry};

pub struct HistoryRow {
    root: GtkBox,
    title: Label,
    detail: Label,
    thumbnail: Image,
}

impl HistoryRow {
    pub fn new() -> Self {
        let title = Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .ellipsize(pango::EllipsizeMode::End)
            .max_width_chars(1)
            .hexpand(true)
            .css_classes(["history-title"])
            .build();
        let detail = Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(pango::EllipsizeMode::End)
            .max_width_chars(1)
            .css_classes(["history-detail"])
            .build();

        let text = GtkBox::new(Orientation::Vertical, 2);
        text.set_valign(Align::Center);
        text.append(&title);
        text.append(&detail);

        let thumbnail = Image::builder()
            .pixel_size(36)
            .valign(Align::Center)
            .css_classes(["history-thumbnail"])
            .build();

        let root = GtkBox::new(Orientation::Horizontal, 10);
        root.add_css_class("history-item");
        root.append(&thumbnail);
        root.append(&text);

        Self {
            root,
            title,
            detail,
            thumbnail,
        }
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn show(&self, entry: Option<&Entry>) {
        let Some(entry) = entry else {
            self.thumbnail.clear();

            return;
        };

        match &entry.content {
            Content::Text(_) => {
                let mut lines = entry
                    .preview
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty());
                self.title.set_text(lines.next().unwrap());

                let detail = lines.next();
                self.title.set_lines(if detail.is_some() { 1 } else { 2 });
                self.detail.set_text(detail.unwrap_or_default());
                self.detail.set_visible(detail.is_some());
                self.thumbnail.set_paintable(gdk::Paintable::NONE);
                self.thumbnail.set_visible(false);
            }
            Content::Image(image) => {
                self.title.set_text("Image");
                self.title.set_lines(1);
                self.detail
                    .set_text(&format!("{} × {}", image.width, image.height));
                self.detail.set_visible(true);
                self.thumbnail.set_paintable(Some(&image.texture));
                self.thumbnail.set_visible(true);
            }
        }
    }
}
