mod application;
mod cli;
mod clipboard;
mod emoji;
mod image;
mod launcher;
mod store;
mod ui;

use clap::{CommandFactory, Parser};
use gtk::glib;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Command};

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Some(Command::Completions { shell }) => {
            clap_complete::generate(shell, &mut Cli::command(), "relvi", &mut std::io::stdout());

            glib::ExitCode::SUCCESS
        }
        command => application::run(command.as_ref()),
    }
}
