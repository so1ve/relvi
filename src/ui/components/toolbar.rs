use gtk::prelude::*;
use gtk::{Box as GtkBox, Label, Orientation, Widget};

pub fn toolbar(title: &Label, controls: &[&Widget]) -> GtkBox {
    title.set_xalign(0.0);
    title.set_hexpand(true);

    let row = GtkBox::new(Orientation::Horizontal, 4);
    row.add_css_class("pane-toolbar");
    row.append(title);

    for control in controls {
        control.set_focusable(false);
        row.append(*control);
    }

    row
}
