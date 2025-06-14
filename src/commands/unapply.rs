use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use clap::Args;
use defaults_rs::{Domain, preferences::Preferences};
use std::collections::HashMap;
use tokio::fs;

use crate::{
    commands::{GlobalArgs, Runnable},
    defaults::{convert::toml_to_prefvalue, executor},
    snapshot::state::{Snapshot, get_snapshot_path},
    util::logging::{LogLevel, print_log},
};

/// Helper: turn string to TOML value
fn string_to_toml_value(s: &str) -> toml::Value {
    // try bool, int, float, fallback to string
    if s == "true" {
        toml::Value::Boolean(true)
    } else if s == "false" {
        toml::Value::Boolean(false)
    } else if let Ok(i) = s.parse::<i64>() {
        toml::Value::Integer(i)
    } else if let Ok(f) = s.parse::<f64>() {
        toml::Value::Float(f)
    } else {
        toml::Value::String(s.to_string())
    }
}

#[derive(Args, Debug)]
pub struct UnapplyCmd;

#[async_trait]
impl Runnable for UnapplyCmd {
    async fn run(&self, g: &GlobalArgs) -> Result<()> {
        let snap_path = get_snapshot_path();

        if !snap_path.exists() {
            bail!(
                "No snapshot found. Please run `cutler apply` first before unapplying.\n\
                As a fallback, you can use `cutler reset` to reset settings to defaults."
            );
        }

        let verbose = g.verbose;
        let dry_run = g.dry_run;

        // load snapshot from disk
        let snapshot = Snapshot::load(&snap_path)
            .await
            .context(format!("Failed to load snapshot from {:?}", snap_path))?;

        // prepare undo operations, grouping by domain for efficiency
        let mut batch_restores: HashMap<Domain, Vec<(String, defaults_rs::PrefValue)>> =
            HashMap::new();
        let mut batch_deletes: HashMap<Domain, Vec<String>> = HashMap::new();

        // peverse order to undo in correct sequence
        for s in snapshot.settings.into_iter().rev() {
            let domain_obj = if s.domain == "NSGlobalDomain" {
                Domain::Global
            } else {
                Domain::User(s.domain.clone())
            };
            if let Some(orig) = s.original_value {
                let pref_value = toml_to_prefvalue(&string_to_toml_value(&orig))?;
                batch_restores
                    .entry(domain_obj)
                    .or_default()
                    .push((s.key, pref_value));
            } else {
                batch_deletes.entry(domain_obj).or_default().push(s.key);
            }
        }

        // in dry-run mode, just print what would be done
        if dry_run {
            for (domain, restores) in &batch_restores {
                for (key, _) in restores {
                    let domain_str = match domain {
                        Domain::Global => "NSGlobalDomain",
                        Domain::User(s) => s,
                    };
                    print_log(
                        LogLevel::Dry,
                        &format!("Would restore setting '{}' for {}", key, domain_str),
                    );
                }
            }
            for (domain, deletes) in &batch_deletes {
                for key in deletes {
                    let domain_str = match domain {
                        Domain::Global => "NSGlobalDomain",
                        Domain::User(s) => s,
                    };
                    print_log(
                        LogLevel::Dry,
                        &format!("Would remove setting '{}' for {}", key, domain_str),
                    );
                }
            }
        } else {
            // perform batch restores
            if !batch_restores.is_empty() {
                match Preferences::write_batch(batch_restores.into_iter().collect()).await {
                    Ok(_) => {
                        if verbose {
                            print_log(LogLevel::Success, "All settings restored (batch write).");
                        }
                    }
                    Err(e) => {
                        print_log(LogLevel::Error, &format!("Batch restore failed: {e}"));
                    }
                }
            }

            // perform batch deletes
            for (domain, keys) in batch_deletes {
                let domain_str = match &domain {
                    Domain::Global => "NSGlobalDomain",
                    Domain::User(s) => s,
                };

                for key in keys {
                    let _ = executor::delete(domain_str, &key, "Removing", verbose, dry_run).await;
                }
            }
        }

        // warn about external commands (not automatically reverted)
        if !snapshot.external.is_empty() {
            print_log(
                LogLevel::Warning,
                "External commands were executed previously; please revert them manually if needed.",
            );
        }

        // delete the snapshot file
        if dry_run {
            print_log(
                LogLevel::Dry,
                &format!("Would remove snapshot file at {:?}", snap_path),
            );
        } else {
            fs::remove_file(&snap_path)
                .await
                .context(format!("Failed to remove snapshot file at {:?}", snap_path))?;
            if verbose {
                print_log(
                    LogLevel::Success,
                    &format!("Removed snapshot file at {:?}", snap_path),
                );
            }
        }

        // Restart system services if requested
        if !g.no_restart_services {
            crate::util::io::restart_system_services(g).await?;
        }

        Ok(())
    }
}
