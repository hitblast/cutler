use clap::{Parser, Subcommand, ValueEnum};

use super::get_styles;

/// top‐level CLI args for cutler
#[derive(Parser)]
#[command(name = "cutler", styles = get_styles(), version, about)]
pub struct Args {
    /// Increase output verbosity.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Do not restart system services after command execution.
    #[arg(short, long, global = true)]
    pub no_restart_services: bool,

    /// Run in dry-run mode. Commands will be printed but not executed.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Accepts all interactive prompts.
    #[arg(short = 'y', long, global = true)]
    pub accept_all: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Apply the changes written in your config file.
    Apply {
        /// Skip executing external commands at the end.
        #[arg(long)]
        no_exec: bool,
    },
    /// See the list of bundle IDs granted with full-disk access (experimental feature)
    SeeFullDisk,
    /// Homebrew backup-and-restore related commands.
    Brew {
        #[command(subcommand)]
        command: BrewSub,
    },
    /// Run only the external commands written in the config file.
    Exec {
        /// Provide a command name to execute if you only want to run it specifically.
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
    /// Initialize a new config file with sensible defaults.
    Init {
        /// Skip confirmation prompt.
        #[arg(short, long)]
        force: bool,
    },
    /// Unapply the previously applied modifications(s).
    Unapply,
    /// Hard reset domains written in the config file (dangerous).
    Reset {
        /// Skip confirmation prompt.
        #[arg(short, long)]
        force: bool,
    },
    /// Display current status comparing the config and the system.
    Status {
        /// Prompt mode for only notifying if a change is detected. Best suited when the terminal starts.
        #[arg(long, hide = true)]
        prompt: bool,
    },
    /// Manage the configuration file.
    Config {
        #[command(subcommand)]
        command: ConfigSub,
    },
    /// Generate shell completions.
    Completion {
        /// Shell type to generate completions for.
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Check for version updates.
    CheckUpdate,
}

#[derive(Subcommand)]
pub enum ConfigSub {
    /// Display the contents of the configuration file.
    Show,
    /// Delete the configuration file.
    Delete,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Elvish,
    PowerShell,
}

#[derive(Subcommand)]
pub enum BrewSub {
    /// Install Homebrew if not present, then install all formulae and casks from config.
    Install,
    /// Backup current formulae and casks into config file.
    Backup,
}
