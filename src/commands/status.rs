use crate::{
    brew::utils::{BrewDiff, compare_brew_state, ensure_brew},
    commands::{GlobalArgs, Runnable},
    config::loader::{get_config_path, load_config},
    defaults::normalize,
    domains::{collect, effective, read_current},
    util::logging::{BOLD, GREEN, LogLevel, RED, RESET, print_log},
};
use anyhow::{Result, bail};
use async_trait::async_trait;
use clap::Args;

#[derive(Args, Debug)]
pub struct StatusCmd;

#[async_trait]
impl Runnable for StatusCmd {
    async fn run(&self, g: &GlobalArgs) -> Result<()> {
        let quiet = g.quiet;
        let verbose = g.verbose;

        let config_path = get_config_path();
        if !config_path.exists() {
            bail!("No config file found. Please run `cutler init` first, or create a config file.");
        }

        let toml = load_config(&config_path).await?;
        let domains = collect(&toml)?;

        // flatten all settings into a list
        let entries: Vec<(String, String, toml::Value)> = domains
            .into_iter()
            .flat_map(|(domain, table)| {
                table
                    .into_iter()
                    .map(move |(key, value)| (domain.clone(), key.clone(), value.clone()))
            })
            .collect();

        // collect results
        let mut outcomes = Vec::with_capacity(entries.len());
        for (domain, key, value) in entries.iter() {
            let (eff_dom, eff_key) = effective(domain, key);
            let desired = normalize(value);
            let current = read_current(&eff_dom, &eff_key)
                .await
                .unwrap_or_else(|| "Not set".into());
            let is_diff = current != desired;
            outcomes.push((eff_dom, eff_key, desired, current, is_diff));
        }

        let mut any_diff = false;
        for (eff_dom, eff_key, desired, current, is_diff) in outcomes {
            if is_diff {
                any_diff = true;
                if !quiet {
                    println!(
                        "{}{}.{}: should be {} (currently {}{}{}){}",
                        BOLD, eff_dom, eff_key, desired, RED, current, RESET, RESET,
                    );
                }
            } else if verbose && !quiet {
                println!(
                    "{}{}.{}: {} (matches desired){}",
                    GREEN, eff_dom, eff_key, current, RESET
                );
            }
        }

        if !quiet {
            if !any_diff {
                println!("\n🍎 All settings already match your configuration.");
            } else {
                println!("\nRun `cutler apply` to apply these changes from your config.");
            }
        }

        // brew status reporting
        if let Some(brew_val) = toml.get("brew").and_then(|v| v.as_table()) {
            if !quiet {
                println!("\n🍺 Homebrew status:");
            }

            // ensure homebrew is installed (skip if not)
            if let Err(e) = ensure_brew(g.dry_run).await {
                print_log(LogLevel::Warning, &format!("Homebrew not available: {e}"));
            } else {
                match compare_brew_state(brew_val).await {
                    Ok(BrewDiff {
                        missing_formulae,
                        extra_formulae,
                        missing_casks,
                        extra_casks,
                        missing_taps,
                        extra_taps,
                    }) => {
                        let mut any_brew_diff = false;
                        if !missing_formulae.is_empty() {
                            any_brew_diff = true;
                            println!(
                                "{}Formulae missing:{} {}",
                                RED,
                                RESET,
                                missing_formulae.join(", ")
                            );
                        }
                        if !extra_formulae.is_empty() {
                            any_brew_diff = true;
                            println!(
                                "{}Extra installed formulae:{} {}",
                                RED,
                                RESET,
                                extra_formulae.join(", ")
                            );
                        }
                        if !missing_casks.is_empty() {
                            any_brew_diff = true;
                            println!(
                                "{}Casks missing:{} {}",
                                RED,
                                RESET,
                                missing_casks.join(", ")
                            );
                        }
                        if !extra_casks.is_empty() {
                            any_brew_diff = true;
                            println!(
                                "{}Extra installed casks:{} {}",
                                RED,
                                RESET,
                                extra_casks.join(", ")
                            );
                        }
                        if !missing_taps.is_empty() {
                            any_brew_diff = true;
                            println!("{}Taps missing:{} {}", RED, RESET, missing_taps.join(", "));
                        }
                        if !extra_taps.is_empty() {
                            any_brew_diff = true;
                            println!("{}Extra tapped:{} {}", RED, RESET, extra_taps.join(", "));
                        }
                        if !any_brew_diff && !quiet {
                            println!("All Homebrew things match your configuration.");
                        }
                    }
                    Err(e) => {
                        print_log(
                            LogLevel::Warning,
                            &format!("Could not check Homebrew status: {e}"),
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
