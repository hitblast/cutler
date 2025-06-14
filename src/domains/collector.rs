use defaults_rs::{Domain, PrefValue, ReadResult, preferences::Preferences};
use std::collections::HashMap;
use toml::Value;

/// Recursively flatten nested TOML tables into (domain, settings‑table) pairs.
fn flatten_domains(
    prefix: Option<String>,
    table: &toml::value::Table,
    dest: &mut Vec<(String, toml::value::Table)>,
) {
    let mut flat = toml::value::Table::new();

    for (k, v) in table {
        if let Value::Table(inner) = v {
            // descend into nested table
            let new_prefix = match &prefix {
                Some(p) if !p.is_empty() => format!("{}.{}", p, k),
                _ => k.clone(),
            };
            flatten_domains(Some(new_prefix), inner, dest);
        } else {
            flat.insert(k.clone(), v.clone());
        }
    }

    if !flat.is_empty() {
        dest.push((prefix.unwrap_or_default(), flat));
    }
}

/// Collect all tables in `[set]`, flatten them, and return a map domain → settings.
pub fn collect(parsed: &Value) -> Result<HashMap<String, toml::value::Table>, anyhow::Error> {
    let root = parsed
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("Config is not a TOML table"))?;
    let mut out = HashMap::new();

    for (key, val) in root {
        if key == "set" {
            if let Value::Table(set_inner) = val {
                for (domain_key, domain_val) in set_inner {
                    if let Value::Table(inner) = domain_val {
                        let mut flat = Vec::with_capacity(inner.len());

                        flatten_domains(Some(domain_key.clone()), inner, &mut flat);

                        for (domain, tbl) in flat {
                            out.insert(domain, tbl);
                        }
                    }
                }
            }
            continue;
        }
    }
    Ok(out)
}

/// Given a config‑domain and key, return the effective “defaults” domain + key.
pub fn effective(domain: &str, key: &str) -> (String, String) {
    if domain == "NSGlobalDomain" {
        ("NSGlobalDomain".into(), key.into())
    } else if let Some(rest) = domain.strip_prefix("NSGlobalDomain.") {
        if rest.is_empty() {
            ("NSGlobalDomain".into(), key.into())
        } else {
            ("NSGlobalDomain".into(), format!("{}.{}", rest, key))
        }
    } else {
        (format!("com.apple.{}", domain), key.into())
    }
}

/// do we need to prefix “com.apple.” on this domain?
pub fn needs_prefix(domain: &str) -> bool {
    !domain.starts_with("NSGlobalDomain") && !domain.starts_with("com.apple.")
}

/// Check whether a domain exists.
pub async fn check_domain_exists(full_domain: &str) -> Result<(), anyhow::Error> {
    let domains = Preferences::list_domains().await.unwrap();

    if domains.contains(&full_domain.to_owned()) {
        Ok(())
    } else {
        anyhow::bail!("Domain \"{}\" does not exist!", full_domain)
    }
}

/// Read the current value of a defaults key, if any.
pub async fn read_current(eff_domain: &str, eff_key: &str) -> Option<String> {
    let domain_obj = if eff_domain == "NSGlobalDomain" {
        Domain::Global
    } else if let Some(rest) = eff_domain.strip_prefix("com.apple.") {
        Domain::User(format!("com.apple.{}", rest))
    } else {
        Domain::User(eff_domain.to_string())
    };

    fn prefvalue_to_cutler_string(val: &PrefValue) -> String {
        match val {
            PrefValue::Boolean(b) => {
                if *b {
                    "1".into()
                } else {
                    "0".into()
                }
            }
            PrefValue::Integer(i) => i.to_string(),
            PrefValue::Float(f) => f.to_string(),
            PrefValue::String(s) => s.clone(),
            PrefValue::Array(arr) => {
                let inner = arr
                    .iter()
                    .map(prefvalue_to_cutler_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", inner)
            }
            PrefValue::Dictionary(dict) => {
                let inner = dict
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, prefvalue_to_cutler_string(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", inner)
            }
        }
    }

    match Preferences::read(domain_obj, Some(eff_key)).await {
        Ok(result) => match result {
            ReadResult::Value(val) => Some(prefvalue_to_cutler_string(&val)),
            ReadResult::Plist(plist_val) => Some(format!("{plist_val:?}")),
        },
        Err(_) => None,
    }
}
