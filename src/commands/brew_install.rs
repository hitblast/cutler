use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Args;
use tokio::process::Command;

use crate::{
    brew::utils::{
        BrewDiff, compare_brew_state, disable_auto_update, ensure_brew, restore_auto_update,
    },
    commands::{GlobalArgs, Runnable},
    config::{get_config_path, load_config},
    util::logging::{LogLevel, print_log},
};

#[derive(Debug, Default, Args)]
pub struct BrewInstallCmd;

#[async_trait]
impl Runnable for BrewInstallCmd {
    async fn run(&self, g: &GlobalArgs) -> Result<()> {
        let cfg_path = get_config_path();
        let quiet = g.quiet;
        let dry_run = g.dry_run;
        let verbose = g.verbose;

        if !cfg_path.exists() {
            if !quiet {
                print_log(
                    LogLevel::Error,
                    "No config file found. Run `cutler init` to start.",
                );
            }
            return Ok(());
        }

        // disable homebrew auto-update
        let prev = disable_auto_update();

        // ensure homebrew installation
        ensure_brew(dry_run).await?;

        let config = load_config(&cfg_path).await?;
        let brew_cfg = config
            .get("brew")
            .and_then(|i| i.as_table())
            .context("No [brew] table found in config")?;

        // check the current brew state, including taps, formulae, and casks
        let brew_diff = match compare_brew_state(brew_cfg).await {
            Ok(diff) => {
                if !diff.extra_formulae.is_empty() {
                    print_log(
                        LogLevel::Warning,
                        &format!(
                            "Extra installed formulae not in config: {:?}",
                            diff.extra_formulae
                        ),
                    );
                }
                if !diff.extra_casks.is_empty() {
                    print_log(
                        LogLevel::Warning,
                        &format!(
                            "Extra installed casks not in config: {:?}",
                            diff.extra_casks
                        ),
                    );
                }
                if !diff.extra_taps.is_empty() {
                    print_log(
                        LogLevel::Warning,
                        &format!("Extra taps not in config: {:?}", diff.extra_taps),
                    );
                }
                if !diff.extra_formulae.is_empty() || !diff.extra_casks.is_empty() {
                    println!(
                        "\nRun `cutler brew backup` to synchronize your config with the system."
                    );
                }
                diff
            }
            Err(e) => {
                print_log(
                    LogLevel::Warning,
                    &format!("Could not check Homebrew status: {e}"),
                );
                // If we cannot compare the state, treat as if nothing is missing.
                BrewDiff {
                    missing_formulae: vec![],
                    extra_formulae: vec![],
                    missing_casks: vec![],
                    extra_casks: vec![],
                    missing_taps: vec![],
                    extra_taps: vec![],
                }
            }
        };

        // tap only the missing taps reported by BrewDiff
        if !brew_diff.missing_taps.is_empty() {
            for tap in brew_diff.missing_taps.iter() {
                if dry_run {
                    print_log(LogLevel::Dry, &format!("Would tap {}", tap));
                } else {
                    print_log(LogLevel::Info, &format!("Tapping: {}", tap));
                    let status = Command::new("brew")
                        .arg("tap")
                        .arg(tap)
                        .stdout(std::process::Stdio::inherit())
                        .stderr(std::process::Stdio::inherit())
                        .stdin(std::process::Stdio::inherit())
                        .status()
                        .await?;
                    if !status.success() {
                        print_log(LogLevel::Error, &format!("Failed to tap: {}", tap));
                    }
                }
            }
        }

        // collect install tasks only for missing formulae and casks
        let mut install_tasks: Vec<Vec<String>> = Vec::new();
        let mut to_fetch_formulae: Vec<String> = Vec::new();
        let mut to_fetch_casks: Vec<String> = Vec::new();

        for name in brew_diff.missing_formulae.iter() {
            install_tasks.push(vec!["install".to_string(), name.to_string()]);
            to_fetch_formulae.push(name.to_string());
        }
        for name in brew_diff.missing_casks.iter() {
            install_tasks.push(vec![
                "install".to_string(),
                "--cask".to_string(),
                name.to_string(),
            ]);
            to_fetch_casks.push(name.to_string());
        }

        if dry_run {
            for args in &install_tasks {
                let display = format!("brew {}", args.join(" "));
                print_log(LogLevel::Dry, &display);
            }
        } else {
            // pre-download everything in parallel
            if !to_fetch_formulae.is_empty() || !to_fetch_casks.is_empty() {
                print_log(LogLevel::Info, "Pre-downloading all formulae and casks...");
            }
            fetch_all(&to_fetch_formulae, &to_fetch_casks, verbose && !quiet).await;

            // sequentially install
            install_sequentially(install_tasks).await?;
        }

        restore_auto_update(prev);
        Ok(())
    }
}

/// Downloads all formulae/casks before installation.
async fn fetch_all(formulae: &[String], casks: &[String], verbose: bool) {
    let mut handles = Vec::new();

    for name in formulae {
        let name = name.clone();
        handles.push(tokio::spawn(async move {
            let mut cmd = Command::new("brew");
            cmd.arg("fetch").arg(&name);
            if verbose {
                print_log(LogLevel::Info, &format!("Fetching formula: {}", name));
            } else {
                cmd.arg("--quiet");
            }
            let _ = cmd.status().await;
        }));
    }
    for name in casks {
        let name = name.clone();
        handles.push(tokio::spawn(async move {
            let mut cmd = Command::new("brew");
            cmd.arg("fetch").arg("--cask").arg(&name);
            if verbose {
                print_log(LogLevel::Info, &format!("Fetching cask: {}", name));
            } else {
                cmd.arg("--quiet");
            }
            let _ = cmd.status().await;
        }));
    }
    for handle in handles {
        let _ = handle.await;
    }
}

/// Install formulae/casks sequentially.
async fn install_sequentially(install_tasks: Vec<Vec<String>>) -> anyhow::Result<()> {
    for args in install_tasks {
        let display = format!("brew {}", args.join(" "));
        print_log(LogLevel::Info, &format!("Installing: {}", display));
        let arg_slices: Vec<&str> = args.iter().map(String::as_str).collect();

        let status = Command::new("brew")
            .args(&arg_slices)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .stdin(std::process::Stdio::inherit())
            .status()
            .await?;

        if !status.success() {
            print_log(LogLevel::Error, &format!("Failed: {}", display));
        }
    }
    Ok(())
}
