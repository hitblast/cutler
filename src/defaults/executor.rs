use crate::defaults::lock_for;
use crate::util::logging::{LogLevel, print_log};
use defaults_rs::Domain;
use defaults_rs::preferences::Preferences;

pub async fn delete(
    domain: &str,
    key: &str,
    action: &str,
    verbose: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    let domain_lock = lock_for(domain, verbose);
    let _guard = domain_lock.lock().await;

    if dry_run {
        print_log(
            LogLevel::Dry,
            &format!("Would {} setting '{}' for {}", action, key, domain),
        );
        return Ok(());
    }

    if verbose {
        print_log(
            LogLevel::Info,
            &format!("{} setting '{}' for {}", action, key, domain),
        );
    }

    let domain_obj = if domain == "NSGlobalDomain" {
        Domain::Global
    } else {
        Domain::User(domain.to_string())
    };

    match Preferences::delete(domain_obj, Some(key)).await {
        Ok(_) => {
            if verbose {
                print_log(
                    LogLevel::Success,
                    &format!("{} setting '{}' for {}.", action, key, domain),
                );
            }
        }
        Err(e) => {
            print_log(
                LogLevel::Error,
                &format!(
                    "Failed to {} setting '{}' for {}: {}",
                    action.to_lowercase(),
                    key,
                    domain,
                    e
                ),
            );
        }
    }

    Ok(())
}
