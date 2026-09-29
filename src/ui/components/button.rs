use gtk::builders::ButtonBuilder;
use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Image, Label, Orientation};

pub fn text(label: &str) -> ButtonBuilder {
    Button::builder()
        .label(label)
        .css_classes(["action-button"])
}

pub fn icon(name: &str) -> ButtonBuilder {
    Button::builder()
        .icon_name(name)
        .css_classes(["action-button"])
}

pub fn icon_text(name: &str, label: &str) -> ButtonBuilder {
    let content = GtkBox::new(Orientation::Horizontal, 6);
    content.append(&Image::from_icon_name(name));
    content.append(&Label::new(Some(label)));

    Button::builder()
        .child(&content)
        .css_classes(["action-button"])
}
