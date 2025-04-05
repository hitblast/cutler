use clap::{Parser, Subcommand};
use cutler::{
    commands::{
        apply_defaults, config_delete, config_show, restart_system_services, status_defaults,
        unapply_defaults,
    },
    logging::{print_log, LogLevel},
};

/// Declarative macOS settings management at your fingertips, with speed.
#[derive(Parser)]
#[command(name = "cutler", version, about)]
struct Cli {
    /// Increase output verbosity.
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Run in dry-run mode. Commands will be printed but not executed.
    #[arg(long, global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply defaults from the config file.
    Apply,
    /// Unapply (delete) defaults from the config file.
    Unapply,
    /// Display current status comparing the config vs current defaults.
    Status,
    /// Manage the configuration file.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Display the contents of the configuration file.
    Show,
    /// Delete the configuration file.
    Delete,
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Apply => apply_defaults(cli.verbose, cli.dry_run),
        Commands::Unapply => unapply_defaults(cli.verbose, cli.dry_run),
        Commands::Status => status_defaults(cli.verbose),
        Commands::Config { command } => match command {
            ConfigCommand::Show => config_show(cli.verbose, cli.dry_run),
            ConfigCommand::Delete => config_delete(cli.verbose, cli.dry_run),
        },
    };

    match result {
        Ok(_) => {
            // For commands that modify defaults, restart system services.
            match &cli.command {
                Commands::Apply
                | Commands::Unapply
                | Commands::Config {
                    command: ConfigCommand::Delete,
                } => {
                    if let Err(e) = restart_system_services(cli.verbose, cli.dry_run) {
                        eprintln!("🍎 Manual restart might be required: {}", e);
                    }
                }
                _ => {}
            }
        }
        Err(e) => {
            print_log(LogLevel::Error, &format!("{}", e));
            std::process::exit(1);
        }
    }
}
