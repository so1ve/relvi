use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Subcommand)]
pub enum Command {
    /// Show or hide the launcher
    Toggle,
    /// Start in the background
    Daemon,
    /// Stop the resident process
    Quit,
    /// Clear launch and query history
    ClearHistory,
    /// Print shell completions
    Completions {
        #[arg(long, value_enum)]
        shell: Shell,
    },
}

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}
