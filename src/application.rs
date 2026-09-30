use std::cell::OnceCell;
use std::rc::Rc;

use clap::Parser;
use gtk::prelude::*;
use gtk::{Application, gio, glib};
use tracing::{debug, error};

use crate::cli::{Cli, ClipboardCommand, Command};
use crate::ui::{self, ClipboardPage, LauncherPage, Shell};
use crate::{clipboard, history};

const APP_ID: &str = "dev.so1ve.Relvi";

struct App {
    shell: Shell,
    launcher: Rc<LauncherPage>,
    clipboard: Rc<ClipboardPage>,
    hold: OnceCell<gio::ApplicationHoldGuard>,
}

impl App {
    fn new(application: &Application) -> Self {
        let window = ui::create_window(application);
        let launcher = LauncherPage::new(&window);
        let clipboard = ClipboardPage::new(&window);
        let shell = Shell::new(window, &[launcher.as_ref(), clipboard.as_ref()]);

        Self {
            shell,
            launcher,
            clipboard,
            hold: OnceCell::new(),
        }
    }

    fn command_line(
        &self,
        application: &Application,
        command_line: &gio::ApplicationCommandLine,
    ) -> glib::ExitCode {
        let arguments = command_line.arguments();
        let Ok(cli) = Cli::try_parse_from(arguments) else {
            return glib::ExitCode::from(2);
        };

        match cli.command {
            None => application.activate(),
            Some(Command::Toggle) => self.shell.toggle(self.launcher.as_ref()),
            Some(Command::Clipboard { command }) => match command {
                None => self.shell.present(self.clipboard.as_ref()),
                Some(ClipboardCommand::Toggle) => self.shell.toggle(self.clipboard.as_ref()),
                Some(ClipboardCommand::Clear) => self.clipboard.clear(),
            },
            Some(Command::Daemon) => {
                self.hold.get_or_init(|| application.hold());
            }
            Some(Command::Quit) => application.quit(),
            Some(Command::ClearHistory) => self.launcher.clear_history(),
            // Completion output belongs to the invoking process.
            Some(Command::Completions { .. }) => return glib::ExitCode::from(2),
        }

        glib::ExitCode::SUCCESS
    }
}

pub fn run(command: Option<&Command>) -> glib::ExitCode {
    debug!("Registering application");

    let application = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    if let Err(error) = application.register(gio::Cancellable::NONE) {
        error!(%error, "Could not register Relvi");

        return glib::ExitCode::FAILURE;
    }

    if application.is_remote() {
        debug!("Forwarding command to resident process");

        return application.run();
    }

    // With no resident process, control commands need no window or catalog.
    match command {
        Some(Command::Quit) => return glib::ExitCode::SUCCESS,
        Some(Command::ClearHistory) => {
            history::History::load().clear();

            return glib::ExitCode::SUCCESS;
        }
        Some(Command::Clipboard {
            command: Some(ClipboardCommand::Clear),
        }) => {
            clipboard::History::load().clear();

            return glib::ExitCode::SUCCESS;
        }
        _ => {}
    }

    let app = Rc::new(App::new(&application));
    application.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_| app.shell.present(app.launcher.as_ref())
    ));
    application.connect_command_line(glib::clone!(
        #[weak]
        app,
        #[upgrade_or]
        glib::ExitCode::FAILURE,
        move |application, command_line| app.command_line(application, command_line)
    ));

    debug!("Application ready");

    application.run()
}
