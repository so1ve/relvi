mod catalog;
mod cli;
mod history;
mod store;
mod system;
mod ui;

use std::cell::OnceCell;
use std::rc::Rc;

use clap::{CommandFactory, Parser};
use gtk::prelude::*;
use gtk::{Application, gio, glib};

use crate::cli::{Cli, Command};

const APP_ID: &str = "dev.so1ve.Relvi";

fn main() -> glib::ExitCode {
    // handle help, version and completion without launching a full application
    if let Some(Command::Completions { shell }) = Cli::parse().command {
        clap_complete::generate(shell, &mut Cli::command(), "relvi", &mut std::io::stdout());

        return glib::ExitCode::SUCCESS;
    }

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
            let Ok(cli) = Cli::try_parse_from(arguments) else {
                return glib::ExitCode::from(2);
            };

            match cli.command {
                None => application.activate(),
                Some(Command::Toggle) => {
                    ui.get_or_init(|| ui::Ui::new(application)).toggle();
                }
                Some(Command::Daemon) => {
                    ui.get_or_init(|| ui::Ui::new(application));
                    hold.get_or_init(|| application.hold());
                }
                Some(Command::Quit) => application.quit(),
                Some(Command::ClearHistory) => {
                    if let Some(instance) = ui.get() {
                        instance.clear_history();
                    } else {
                        history::History::load().clear();
                    }
                }
                // Completion output belongs to the invoking process.
                Some(Command::Completions { .. }) => return glib::ExitCode::from(2),
            }

            glib::ExitCode::SUCCESS
        }
    ));

    application.run()
}
