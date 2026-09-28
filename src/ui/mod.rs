mod launcher;
mod scroll;

use std::cell::OnceCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, EventControllerKey, GestureClick, Orientation,
    Overlay, PropagationPhase, gdk, glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use self::launcher::LauncherView;

const VIEW_WIDTH: i32 = 540;

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

/// The long-lived launcher window and its view host.
pub struct Ui {
    window: ApplicationWindow,
    launcher: Rc<LauncherView>,
}

impl Ui {
    pub fn new(application: &Application) -> Self {
        adw::init().unwrap();

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

        let launcher = LauncherView::new(&window);

        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            window,
            #[weak]
            launcher,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                let modifiers = modifiers
                    & (gdk::ModifierType::CONTROL_MASK
                        | gdk::ModifierType::SHIFT_MASK
                        | gdk::ModifierType::ALT_MASK
                        | gdk::ModifierType::SUPER_MASK);

                if key == gdk::Key::Escape && modifiers.is_empty() {
                    if !launcher.cancel_confirmation() {
                        window.set_visible(false);
                    }

                    return glib::Propagation::Stop;
                }

                glib::Propagation::Proceed
            }
        ));
        window.add_controller(keys);

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
        overlay.add_overlay(launcher.widget());

        let anchor_height = OnceCell::new();
        overlay.connect_get_child_position(move |overlay, child| {
            let width = VIEW_WIDTH.min(overlay.width());
            let height = child.measure(Orientation::Vertical, width).1;
            let full_height = *anchor_height.get_or_init(|| height);
            let top = ((overlay.height() - full_height) / 2).max(0);

            Some(gdk::Rectangle::new(
                (overlay.width() - width) / 2,
                top,
                width,
                height.min(overlay.height() - top),
            ))
        });

        window.set_child(Some(&overlay));

        Self { window, launcher }
    }

    pub fn clear_history(&self) {
        self.launcher.clear_history();
    }

    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.window.set_visible(false);
        } else {
            self.present();
        }
    }

    pub fn present(&self) {
        self.window.present();
        self.launcher.focus();
    }
}
