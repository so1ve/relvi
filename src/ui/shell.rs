use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, GestureClick, Orientation, Overlay, Stack, gdk,
    glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use super::pages::Page;

pub fn create_window(application: &Application) -> ApplicationWindow {
    adw::init().unwrap();

    let window = ApplicationWindow::builder()
        .application(application)
        .title("Relvi")
        .decorated(false)
        .hide_on_close(true)
        .default_width(720)
        .default_height(520)
        .css_classes(["relvi-window"])
        .build();

    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../../resources/style.css"));
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(&window),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_namespace(Some("relvi"));
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        window.set_exclusive_zone(0);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }
    }

    window
}

pub struct Shell {
    window: ApplicationWindow,
    stack: Stack,
    width: Rc<Cell<i32>>,
}

impl Shell {
    pub fn new(window: ApplicationWindow, pages: &[&dyn Page]) -> Self {
        let stack = Stack::builder()
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();
        stack.add_css_class("palette");
        for page in pages {
            stack.add_child(page.widget());
        }

        let width = Rc::new(Cell::new(pages[0].width()));

        let backdrop = GtkBox::new(Orientation::Vertical, 0);
        backdrop.set_hexpand(true);
        backdrop.set_vexpand(true);

        let dismiss = GestureClick::new();
        dismiss.connect_released(glib::clone!(
            #[weak]
            window,
            move |_, _, _, _| window.set_visible(false)
        ));
        backdrop.add_controller(dismiss);

        let overlay = Overlay::new();
        overlay.set_child(Some(&backdrop));
        overlay.add_overlay(&stack);
        window.set_child(Some(&overlay));

        // Prepare the renderer while hidden and keep the initial top edge
        // fixed when pages or result counts change.
        WidgetExt::realize(&window);

        let anchor_height = stack.measure(Orientation::Vertical, width.get()).1;
        overlay.connect_get_child_position(glib::clone!(
            #[weak]
            width,
            #[upgrade_or]
            None,
            move |overlay, child| {
                let width = width.get().min(overlay.width());
                let height = child.measure(Orientation::Vertical, width).1;
                let top = ((overlay.height() - anchor_height) / 2).max(0);

                Some(gdk::Rectangle::new(
                    (overlay.width() - width) / 2,
                    top,
                    width,
                    height.min(overlay.height() - top),
                ))
            }
        ));

        Self {
            window,
            stack,
            width,
        }
    }

    pub fn present(&self, page: &dyn Page) {
        self.width.set(page.width());
        self.stack.set_visible_child(page.widget());
        self.window.present();
        page.present();
    }

    pub fn toggle(&self, page: &dyn Page) {
        if self.window.is_visible() && self.stack.visible_child().as_ref() == Some(page.widget()) {
            self.window.set_visible(false);
        } else {
            self.present(page);
        }
    }
}
