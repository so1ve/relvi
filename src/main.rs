mod catalog;
mod history;
mod ui;

use std::cell::OnceCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Application, gio, glib};

const APP_ID: &str = "io.so1ve.Relvi";

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let ui = Rc::new(OnceCell::<ui::Ui>::new());
    let activation_ui = Rc::clone(&ui);

    let activate = application.connect_activate(move |application| {
        let instance = activation_ui.get_or_init(|| ui::Ui::new(application));
        instance.present();
    });

    application.connect_command_line(move |application, command_line| {
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
            [command] if command == "clear-history" => {
                if let Some(instance) = ui.get() {
                    instance.clear_history();
                } else {
                    history::History::load().clear();
                }
            }
            _ => return glib::ExitCode::from(2),
        }

        glib::ExitCode::SUCCESS
    });

    let exit_code = application.run();

    application.disconnect(activate);

    if exit_code.get() == 2 {
        eprintln!("Usage: relvi [toggle|clear-history]");
    }

    exit_code
}
