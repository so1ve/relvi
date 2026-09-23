mod applications;
mod palette;

use std::cell::OnceCell;

use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, GestureClick, Orientation, Overlay, Stack, gdk,
    glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use self::palette::Palette;

const VIEW_WIDTH: i32 = 540;

/// The long-lived launcher window and its view host.
pub struct Ui {
    window: ApplicationWindow,
    palette: Palette,
}

impl Ui {
    pub fn new(application: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(application)
            .decorated(false)
            .default_width(720)
            .default_height(520)
            .build();

        let provider = gtk::CssProvider::new();
        provider.load_from_string(include_str!("../../resources/style.css"));
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(&window),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        window.set_title(Some("Relvi"));
        window.set_hide_on_close(true);
        window.add_css_class("relvi-window");
        configure_layer_surface(&window);

        let palette = Palette::new(&window);

        // The stack is the extension point for additional launcher views.
        // The application palette is the first view; future views can be
        // added here without changing the window or layer-shell plumbing.
        let views = Stack::new();
        views.add_named(palette.widget(), Some("applications"));

        // The layer surface covers the whole output, so a click that misses
        // the active view lands on this backdrop and dismisses the launcher.
        let backdrop = GtkBox::new(gtk::Orientation::Vertical, 0);
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
        overlay.add_overlay(&views);
        let expanded_height = OnceCell::new();
        overlay.connect_get_child_position(move |overlay, child| {
            let width = VIEW_WIDTH.min(overlay.width());
            let height = child.measure(Orientation::Vertical, width).1;
            // Keep the initial height as the anchor once wrapped rows
            // have been allocated the actual viewport width.
            let full_height = if child.width() > 0 {
                *expanded_height.get_or_init(|| height)
            } else {
                height
            };
            let top = ((overlay.height() - full_height) / 2).max(0);

            Some(gdk::Rectangle::new(
                (overlay.width() - width) / 2,
                top,
                width,
                height.min(overlay.height() - top),
            ))
        });
        window.set_child(Some(&overlay));

        Self { window, palette }
    }

    pub fn present(&self) {
        self.window.present();
        self.palette.focus();
    }
}

fn configure_layer_surface(window: &ApplicationWindow) {
    if !gtk4_layer_shell::is_supported() {
        return;
    }

    window.init_layer_shell();
    window.set_namespace(Some("relvi"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    window.set_exclusive_zone(0);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(edge, true);
    }
}
