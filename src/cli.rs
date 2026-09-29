use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Subcommand)]
pub enum ClipboardCommand {
    /// Show or hide clipboard history
    Toggle,
    /// Clear saved clipboard history
    Clear,
}

#[derive(Subcommand)]
pub enum Command {
    /// Search clipboard history
    Clipboard {
        #[command(subcommand)]
        command: Option<ClipboardCommand>,
    },
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
