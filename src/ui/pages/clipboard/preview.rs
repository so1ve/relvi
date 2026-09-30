use gtk::prelude::*;
use gtk::{Box as GtkBox, Label, Orientation, Stack, Widget, gdk};

use crate::clipboard::Content;
use crate::ui::components::{ImagePreview, TextPreview, toolbar};

pub struct Preview {
    root: GtkBox,
    heading: Label,
    dimensions: Label,
    content: Stack,
    text: TextPreview,
    image: ImagePreview,
    empty: Label,
}

impl Preview {
    pub fn new() -> Self {
        let heading = Label::new(Some("Preview"));
        heading.add_css_class("pane-title");

        let dimensions = Label::new(None);
        dimensions.set_visible(false);

        let header = toolbar(&heading, &[dimensions.upcast_ref()]);
        let text = TextPreview::new();
        let image = ImagePreview::new();

        let empty = Label::new(Some("No preview"));
        empty.add_css_class("empty-state");

        let content = Stack::builder().hexpand(true).vexpand(true).build();
        content.add_child(text.widget());
        content.add_child(image.widget());
        content.add_child(&empty);
        content.set_visible_child(&empty);

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&header);
        root.append(&content);

        Self {
            root,
            heading,
            dimensions,
            content,
            text,
            image,
            empty,
        }
    }

    pub const fn widget(&self) -> &GtkBox {
        &self.root
    }

    pub fn show(&self, content: Option<&Content>) {
        let child: &Widget = match content {
            Some(Content::Text(text)) => {
                self.heading.set_text("Text");
                self.dimensions.set_visible(false);
                self.text.set_text(text);
                self.image.set_paintable(gdk::Paintable::NONE);

                self.text.widget().upcast_ref()
            }
            Some(Content::Image(image)) => {
                self.heading.set_text("Image");
                self.dimensions
                    .set_text(&format!("{} × {}", image.width, image.height));
                self.dimensions.set_visible(true);
                self.text.set_text("");
                self.image.set_paintable(Some(&image.texture));

                self.image.widget().upcast_ref()
            }
            None => {
                self.heading.set_text("Preview");
                self.dimensions.set_visible(false);
                self.text.set_text("");
                self.image.set_paintable(gdk::Paintable::NONE);

                self.empty.upcast_ref()
            }
        };

        self.content.set_visible_child(child);
    }
}
