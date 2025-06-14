use anyhow::{Result, bail};
use async_trait::async_trait;
use clap::Args;
use defaults_rs::{Domain, preferences::Preferences};
use tokio::fs;

use crate::{
    commands::{GlobalArgs, Runnable},
    config::loader::{get_config_path, load_config},
    domains::{collect, effective, read_current},
    snapshot::state::get_snapshot_path,
    util::{
        io::confirm_action,
        logging::{LogLevel, print_log},
    },
};

#[derive(Args, Debug)]
pub struct ResetCmd {
    /// Forcefully reset configuration.
    #[arg(short, long)]
    force: bool,
}

#[async_trait]
impl Runnable for ResetCmd {
    async fn run(&self, g: &GlobalArgs) -> Result<()> {
        let config_path = get_config_path();
        if !config_path.exists() {
            bail!("No config file found. Please run `cutler init` first, or create a config file.");
        }

        let verbose = g.verbose;
        let quiet = g.quiet;
        let dry_run = g.dry_run;

        if !quiet {
            print_log(
                LogLevel::Warning,
                "This will DELETE all settings defined in your config file.",
            );
            print_log(
                LogLevel::Warning,
                "Settings will be reset to macOS defaults, not to their previous values.",
            );
        }

        if !self.force && !confirm_action("Are you sure you want to continue?")? {
            return Ok(());
        }

        let toml = load_config(&config_path).await?;
        let domains = collect(&toml)?;

        for (domain, table) in domains {
            for (key, _) in table {
                let (eff_dom, eff_key) = effective(&domain, &key);

                // only delete it if currently set
                if read_current(&eff_dom, &eff_key).await.is_some() {
                    let domain_obj = if eff_dom == "NSGlobalDomain" {
                        Domain::Global
                    } else if let Some(rest) = eff_dom.strip_prefix("com.apple.") {
                        Domain::User(format!("com.apple.{}", rest))
                    } else {
                        Domain::User(eff_dom.clone())
                    };

                    if dry_run {
                        print_log(
                            LogLevel::Dry,
                            &format!("Would reset {}.{} to system default", eff_dom, eff_key),
                        );
                    } else {
                        match Preferences::delete(domain_obj, Some(&eff_key)).await {
                            Ok(_) => {
                                if verbose && !quiet {
                                    print_log(
                                        LogLevel::Success,
                                        &format!("Reset {}.{} to system default", eff_dom, eff_key),
                                    );
                                }
                            }
                            Err(e) => {
                                print_log(
                                    LogLevel::Error,
                                    &format!("Failed to reset {}.{}: {}", eff_dom, eff_key, e),
                                );
                            }
                        }
                    }
                } else if verbose && !quiet {
                    print_log(
                        LogLevel::Info,
                        &format!("Skipping {}.{} (not set)", eff_dom, eff_key),
                    );
                }
            }
        }

        // remove snapshot if present
        let snap_path = get_snapshot_path();
        if snap_path.exists() {
            if dry_run {
                if !quiet {
                    print_log(
                        LogLevel::Dry,
                        &format!("Would remove snapshot at {:?}", snap_path),
                    );
                }
            } else if let Err(e) = fs::remove_file(&snap_path).await {
                print_log(
                    LogLevel::Warning,
                    &format!("Failed to remove snapshot: {}", e),
                );
            } else if verbose && !quiet {
                print_log(
                    LogLevel::Success,
                    &format!("Removed snapshot at {:?}", snap_path),
                );
            }
        }

        if !quiet {
            println!("\n🍎 Reset complete. All configured settings have been removed.");
        }

        // restart system services if requested
        if !g.no_restart_services {
            crate::util::io::restart_system_services(g).await?;
        }

        Ok(())
    }
}
