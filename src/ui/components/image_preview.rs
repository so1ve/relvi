use gtk::prelude::*;
use gtk::{ContentFit, Picture, PolicyType, ScrolledWindow, gdk};

pub struct ImagePreview {
    frame: ScrolledWindow,
    picture: Picture,
}

impl ImagePreview {
    pub fn new() -> Self {
        let picture = Picture::builder()
            .can_shrink(true)
            .content_fit(ContentFit::Contain)
            .hexpand(true)
            .vexpand(true)
            .css_classes(["image-preview"])
            .build();
        let frame = ScrolledWindow::builder()
            .child(&picture)
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Never)
            .build();

        Self { frame, picture }
    }

    pub const fn widget(&self) -> &ScrolledWindow {
        &self.frame
    }

    pub fn set_paintable(&self, image: Option<&impl IsA<gdk::Paintable>>) {
        self.picture.set_paintable(image);
    }
}
