use crate::commands::{BrewInstallCmd, GlobalArgs, Runnable};
use crate::{
    config::loader::load_config,
    defaults::{convert::toml_to_prefvalue, flags},
    domains::collector,
    external::runner,
    snapshot::state::{SettingState, Snapshot},
    util::logging::{LogLevel, print_log},
};
use anyhow::Result;
use async_trait::async_trait;
use clap::Args;
use defaults_rs::{Domain, preferences::Preferences};
use std::collections::HashMap;
use toml::Value;

#[derive(Args, Debug)]
pub struct ApplyCmd {
    /// Skip executing external commands at the end.
    #[arg(long)]
    pub no_exec: bool,

    /// Invoke `cutler brew install` after applying defaults.
    #[arg(long)]
    pub with_brew: bool,

    /// Risky: Disables check for domain existence before applying modification
    #[arg(long)]
    pub disable_checks: bool,
}

/// Represents an apply command job.
#[derive(Debug)]
struct Job {
    domain: String,
    key: String,
    toml_value: Value,
    action: &'static str,
    original: Option<String>,
    new_value: String,
}

#[async_trait]
impl Runnable for ApplyCmd {
    async fn run(&self, g: &GlobalArgs) -> Result<()> {
        let verbose = g.verbose;
        let dry_run = g.dry_run;

        let config_path_opt = crate::util::config::ensure_config_exists_or_init(g).await?;
        let config_path = match config_path_opt {
            Some(path) => path,
            None => anyhow::bail!("Aborted."),
        };

        // parse + flatten domains
        let toml = load_config(&config_path).await?;
        let domains = collector::collect(&toml)?;

        // load the old snapshot (if any), otherwise create a new instance
        let snap_path = crate::snapshot::state::get_snapshot_path();
        let snap = if snap_path.exists() {
            Snapshot::load(&snap_path).await.unwrap_or_else(|e| {
                print_log(
                    LogLevel::Warning,
                    &format!("Bad snapshot: {}; starting new", e),
                );
                Snapshot::new()
            })
        } else {
            Snapshot::new()
        };

        // turn the old snapshot into a hashmap for a quick lookup
        let mut existing: std::collections::HashMap<_, _> = snap
            .settings
            .into_iter()
            .map(|s| ((s.domain.clone(), s.key.clone()), s))
            .collect();

        let mut jobs: Vec<Job> = Vec::new();

        for (dom, table) in domains.into_iter() {
            // if we need to insert the com.apple prefix, check once
            if collector::needs_prefix(&dom) && !self.disable_checks {
                collector::check_domain_exists(&format!("com.apple.{}", dom)).await?;
            }

            for (key, toml_value) in table.into_iter() {
                let (eff_dom, eff_key) = collector::effective(&dom, &key);
                let desired = flags::normalize(&toml_value);

                // read the current value from the system
                // then, check if changed
                let current = collector::read_current(&eff_dom, &eff_key)
                    .await
                    .unwrap_or_default();
                let changed = current != desired;

                // grab the old snapshot entry if it exists
                let old_entry = existing.get(&(eff_dom.clone(), eff_key.clone())).cloned();

                if changed {
                    existing.remove(&(eff_dom.clone(), eff_key.clone()));
                    let original = old_entry.as_ref().and_then(|e| e.original_value.clone());

                    // decide “applying” vs “updating”
                    let action = if old_entry.is_some() {
                        "Updating"
                    } else {
                        "Applying"
                    };

                    jobs.push(Job {
                        domain: eff_dom.clone(),
                        key: eff_key.clone(),
                        toml_value: toml_value.clone(),
                        action,
                        original,
                        new_value: desired.clone(),
                    });
                } else if verbose {
                    print_log(
                        LogLevel::Info,
                        &format!("Skipping unchanged {}:{}", eff_dom, eff_key),
                    );
                }
            }
        }

        // use defaults-rs batch write API for all changed settings
        // group jobs by domain for batch write
        let mut batch: HashMap<Domain, Vec<(String, defaults_rs::PrefValue)>> = HashMap::new();

        for job in &jobs {
            let domain_obj = if job.domain == "NSGlobalDomain" {
                Domain::Global
            } else {
                Domain::User(job.domain.clone())
            };

            if verbose && !dry_run {
                print_log(
                    LogLevel::Info,
                    &format!(
                        "{} {}:{} -> {}",
                        job.action, job.domain, job.key, job.new_value
                    ),
                );
            }
            let pref_value = toml_to_prefvalue(&job.toml_value)?;
            batch
                .entry(domain_obj)
                .or_default()
                .push((job.key.clone(), pref_value));
        }

        // perform batch write
        if !dry_run {
            match Preferences::write_batch(batch.into_iter().collect()).await {
                Ok(_) => {
                    if verbose {
                        print_log(LogLevel::Success, "All settings applied (batch write).");
                    }
                }
                Err(e) => {
                    print_log(LogLevel::Error, &format!("Batch write failed: {e}"));
                }
            }
        } else {
            for job in &jobs {
                print_log(
                    LogLevel::Dry,
                    &format!(
                        "Would {} setting '{}' for {}",
                        job.action, job.key, job.domain
                    ),
                );
            }
        }

        let mut new_snap = Snapshot::new();
        for ((_, _), mut old_entry) in existing.into_iter() {
            old_entry.new_value = old_entry.new_value.clone();
            new_snap.settings.push(old_entry);
        }
        // now append all the newly applied/updated settings
        for job in jobs {
            new_snap.settings.push(SettingState {
                domain: job.domain,
                key: job.key,
                original_value: job.original.clone(),
                new_value: job.new_value,
            });
        }
        new_snap.external = runner::extract(&toml);

        if !dry_run {
            new_snap.save(&snap_path).await?;
            if verbose {
                print_log(
                    LogLevel::Success,
                    &format!("Snapshot saved: {:?}", snap_path),
                );
            }
        } else {
            print_log(LogLevel::Dry, "Would save snapshot");
        }

        // exec external commands
        if !self.no_exec {
            let _ = runner::run_all(&toml, verbose, dry_run).await;
        }

        // run brew
        if self.with_brew {
            BrewInstallCmd.run(g).await?;
        }

        // restart system services if requested
        if !g.no_restart_services {
            crate::util::io::restart_system_services(g).await?;
        }

        Ok(())
    }
}
