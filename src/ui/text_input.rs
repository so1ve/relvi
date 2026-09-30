mod keyboard;

use std::error::Error;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{Widget, Window, gio, glib};
use gtk4_layer_shell::{KeyboardMode, LayerShell};

pub async fn paste(widget: &impl IsA<Widget>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let window = widget.root().unwrap().downcast::<Window>().unwrap();
    if !window.is_layer_window() {
        return Err("Automatic insertion requires a Wayland layer-shell window".into());
    }

    let result = async {
        set_keyboard_focus(&window, false).await?;
        if !window.is_visible() {
            return Err("Picker closed before insertion".into());
        }

        gio::spawn_blocking(keyboard::paste).await.unwrap()
    }
    .await;

    // Finish the focus handoff before the next queued insertion can copy.
    set_keyboard_focus(&window, true).await?;

    result
}

async fn set_keyboard_focus(
    window: &Window,
    focused: bool,
) -> Result<(), glib::FutureWithTimeoutError> {
    let (sender, receiver) = async_channel::bounded(1);
    let handler = window.connect_is_active_notify(move |window| {
        if window.is_active() == focused {
            let _ = sender.try_send(());
        }
    });
    window.set_keyboard_mode(if focused {
        KeyboardMode::Exclusive
    } else {
        KeyboardMode::None
    });

    let result = if window.is_visible() && window.is_active() != focused {
        glib::future_with_timeout(Duration::from_secs(1), receiver.recv())
            .await
            .map(Result::unwrap)
    } else {
        Ok(())
    };
    window.disconnect(handler);

    result
}
