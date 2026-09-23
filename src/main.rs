mod catalog;
mod history;
mod ui;

use std::cell::OnceCell;

use gtk::prelude::*;
use gtk::{Application, gio, glib};

const APP_ID: &str = "io.so1ve.Relvi";

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let ui = OnceCell::<ui::Ui>::new();

    let activate = application.connect_activate(move |application| {
        let instance = ui.get_or_init(|| ui::Ui::new(application));
        instance.present();
    });

    application.connect_command_line(|application, command_line| {
        let arguments = command_line.arguments();

        match &arguments[1..] {
            [] => application.activate(),
            [command] if command == "toggle" => {
                if let Some(window) = application
                    .active_window()
                    .filter(|window| window.is_visible())
                {
                    window.set_visible(false);
                } else {
                    application.activate();
                }
            }
            _ => return glib::ExitCode::from(2),
        }

        glib::ExitCode::SUCCESS
    });

    let exit_code = application.run();

    application.disconnect(activate);

    if exit_code.get() == 2 {
        eprintln!("Usage: relvi [toggle]");
    }

    exit_code
}
