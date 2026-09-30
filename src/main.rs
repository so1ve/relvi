mod application;
mod catalog;
mod cli;
mod clipboard;
mod history;
mod image;
mod store;
mod system;
mod ui;

use clap::{CommandFactory, Parser};
use gtk::glib;

use crate::cli::{Cli, Command};

fn main() -> glib::ExitCode {
    match Cli::parse().command {
        Some(Command::Completions { shell }) => {
            clap_complete::generate(shell, &mut Cli::command(), "relvi", &mut std::io::stdout());

            glib::ExitCode::SUCCESS
        }
        command => application::run(command.as_ref()),
    }
}
