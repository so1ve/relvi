use std::error::Error;
use std::ffi::OsStr;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{Widget, Window, gio, glib};
use gtk4_layer_shell::{KeyboardMode, LayerShell};

pub async fn insert(widget: &impl IsA<Widget>, text: &str) -> Result<(), Box<dyn Error>> {
    let window = widget.root().unwrap().downcast::<Window>().unwrap();
    if !window.is_layer_window() {
        return Err("Automatic insertion requires a Wayland layer-shell window".into());
    }

    // Keep the picker visible, but let the compositor focus the previous app
    // before sending any text.
    let mode = window.keyboard_mode();
    let (sender, receiver) = async_channel::bounded(1);
    let handler = window.connect_is_active_notify(move |window| {
        if !window.is_active() {
            let _ = sender.try_send(());
        }
    });
    window.set_keyboard_mode(KeyboardMode::None);

    let released = if window.is_active() {
        glib::future_with_timeout(Duration::from_secs(1), receiver.recv())
            .await
            .map(Result::unwrap)
    } else {
        Ok(())
    };
    window.disconnect(handler);

    let result = async {
        released.map_err(|_| "The compositor did not release keyboard focus")?;
        if !window.is_visible() {
            return Err("Picker closed before insertion".into());
        }

        let process = gio::Subprocess::newv(
            &[OsStr::new("wtype"), OsStr::new("--"), OsStr::new(text)],
            gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_PIPE,
        )?;
        let (_, stderr) = process.communicate_utf8_future(None).await?;
        if !process.is_successful() {
            return Err(format!("wtype: {}", stderr.unwrap_or_default().trim()).into());
        }

        Ok(())
    }
    .await;

    window.set_keyboard_mode(mode);

    result
}
