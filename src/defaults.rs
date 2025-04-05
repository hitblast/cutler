use std::process::Command;
use toml::Value;

use crate::logging::{print_log, LogLevel};

/// For a given TOML value, returns the flag and string. Booleans become "-bool" with "true"/"false".
pub fn get_flag_and_value(
    value: &Value,
) -> Result<(&'static str, String), Box<dyn std::error::Error>> {
    match value {
        Value::Boolean(b) => Ok(("-bool", if *b { "true".into() } else { "false".into() })),
        Value::Integer(_) => Ok(("-int", value.to_string())),
        Value::Float(_) => Ok(("-float", value.to_string())),
        Value::String(_) => Ok(("-string", value.as_str().unwrap().to_string())),
        _ => Err("Unsupported type encountered in configuration".into()),
    }
}

/// Executes a "defaults write" command with the given parameters.
pub fn execute_defaults_write(
    eff_domain: &str,
    eff_key: &str,
    flag: &str,
    value_str: &str,
    action: &str,
    verbose: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if dry_run {
        print_log(
            LogLevel::Info,
            &format!(
                "Dry-run: Would execute: defaults write {} \"{}\" {} \"{}\"",
                eff_domain, eff_key, flag, value_str
            ),
        );
        return Ok(());
    }
    if verbose {
        print_log(
            LogLevel::Info,
            &format!(
                "{}: defaults write {} \"{}\" {} \"{}\"",
                action, eff_domain, eff_key, flag, value_str
            ),
        );
    }
    let output = Command::new("defaults")
        .arg("write")
        .arg(eff_domain)
        .arg(eff_key)
        .arg(flag)
        .arg(value_str)
        .output()?;
    if !output.status.success() {
        print_log(
            LogLevel::Error,
            &format!(
                "Failed to {} setting '{}' for {}.",
                action.to_lowercase(),
                eff_key,
                eff_domain
            ),
        );
    } else if verbose {
        print_log(
            LogLevel::Success,
            &format!("{} setting '{}' for {}.", action, eff_key, eff_domain),
        );
    }
    Ok(())
}

/// Executes a "defaults delete" command with the specified parameters.
pub fn execute_defaults_delete(
    eff_domain: &str,
    eff_key: &str,
    action: &str,
    verbose: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if dry_run {
        print_log(
            LogLevel::Info,
            &format!(
                "Dry-run: Would execute: defaults delete {} \"{}\"",
                eff_domain, eff_key
            ),
        );
        return Ok(());
    }
    if verbose {
        print_log(
            LogLevel::Info,
            &format!("{}: defaults delete {} \"{}\"", action, eff_domain, eff_key),
        );
    }
    let output = Command::new("defaults")
        .arg("delete")
        .arg(eff_domain)
        .arg(eff_key)
        .output()?;
    if !output.status.success() {
        print_log(
            LogLevel::Error,
            &format!(
                "Failed to {} setting '{}' for {}.",
                action.to_lowercase(),
                eff_key,
                eff_domain
            ),
        );
    } else if verbose {
        print_log(
            LogLevel::Success,
            &format!("{} setting '{}' for {}.", action, eff_key, eff_domain),
        );
    }
    Ok(())
}

/// Checks whether a given domain exists using the "defaults" command.
pub fn check_domain_exists(full_domain: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("defaults").arg("domains").output()?;
    if !output.status.success() {
        return Err("Failed to retrieve domains.".into());
    }
    let domains_str = String::from_utf8_lossy(&output.stdout);
    if domains_str.split(',').any(|d| d.trim() == full_domain) {
        Ok(())
    } else {
        Err(format!("Domain '{}' does not exist. Aborting.", full_domain).into())
    }
}

/// Helper: Reads the current value from defaults (if any) for a given effective domain and key.
pub fn get_current_value(eff_domain: &str, eff_key: &str) -> Option<String> {
    let output = Command::new("defaults")
        .arg("read")
        .arg(eff_domain)
        .arg(eff_key)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Normalizes the desired value for comparison.
/// For booleans: maps true → "1" and false → "0". For strings, simply returns the inner string.
pub fn normalize_desired(value: &Value) -> String {
    match value {
        Value::Boolean(b) => {
            if *b {
                "1".into()
            } else {
                "0".into()
            }
        }
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}
