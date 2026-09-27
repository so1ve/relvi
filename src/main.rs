mod applications;
mod store;
mod ui;

use std::cell::OnceCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Application, gio, glib};

const APP_ID: &str = "dev.so1ve.Relvi";

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let ui = Rc::new(OnceCell::<ui::Ui>::new());
    let hold = OnceCell::new();

    application.connect_activate(glib::clone!(
        #[weak]
        ui,
        move |application| {
            ui.get_or_init(|| ui::Ui::new(application)).present();
        }
    ));

    application.connect_command_line(glib::clone!(
        #[weak]
        ui,
        #[upgrade_or]
        glib::ExitCode::FAILURE,
        move |application, command_line| {
            let arguments = command_line.arguments();

            match &arguments[1..] {
                [] => application.activate(),
                [command] if command == "toggle" => {
                    ui.get_or_init(|| ui::Ui::new(application)).toggle();
                }
                [command] if command == "daemon" => {
                    ui.get_or_init(|| ui::Ui::new(application));
                    hold.get_or_init(|| application.hold());
                }
                [command] if command == "quit" => application.quit(),
                [command] if command == "clear-history" => {
                    if let Some(instance) = ui.get() {
                        instance.clear_history();
                    } else {
                        applications::clear_history();
                    }
                }
                _ => return glib::ExitCode::from(2),
            }

            glib::ExitCode::SUCCESS
        }
    ));

    let exit_code = application.run();

    if exit_code.get() == 2 {
        eprintln!("Usage: relvi [toggle|daemon|quit|clear-history]");
    }

    exit_code
}
