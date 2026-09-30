use gtk::{Stack, gdk};

use crate::clipboard::Content;
use crate::ui::components::{ImagePreview, TextPreview};

pub struct Preview {
    root: Stack,
    text: TextPreview,
    image: ImagePreview,
}

impl Preview {
    pub fn new() -> Self {
        let text = TextPreview::new();
        let image = ImagePreview::new();

        let root = Stack::builder().hexpand(true).vexpand(true).build();
        root.add_child(text.widget());
        root.add_child(image.widget());

        Self { root, text, image }
    }

    pub const fn widget(&self) -> &Stack {
        &self.root
    }

    pub fn show(&self, content: Option<&Content>) {
        let child = match content {
            Some(Content::Text(text)) => {
                self.text.set_text(text);
                self.image.set_paintable(gdk::Paintable::NONE);

                self.text.widget()
            }
            Some(Content::Image(image)) => {
                self.text.set_text("");
                self.image.set_paintable(Some(&image.texture));

                self.image.widget()
            }
            None => {
                self.text.set_text("");
                self.image.set_paintable(gdk::Paintable::NONE);

                self.text.widget()
            }
        };

        self.root.set_visible_child(child);
    }
}
