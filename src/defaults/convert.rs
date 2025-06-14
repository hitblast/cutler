use defaults_rs::PrefValue;
use std::collections::HashMap;
use toml::Value;

/// Turns a toml::Value into its defaults_rs::PrefValue counterpart.
pub fn toml_to_prefvalue(val: &Value) -> anyhow::Result<PrefValue> {
    Ok(match val {
        Value::String(s) => PrefValue::String(s.clone()),
        Value::Integer(i) => PrefValue::Integer(*i),
        Value::Float(f) => PrefValue::Float(*f),
        Value::Boolean(b) => PrefValue::Boolean(*b),
        Value::Array(arr) => PrefValue::Array(
            arr.iter()
                .map(toml_to_prefvalue)
                .collect::<Result<Vec<_>, anyhow::Error>>()?,
        ),
        Value::Table(tbl) => PrefValue::Dictionary(
            tbl.iter()
                .map(|(k, v)| Ok((k.clone(), toml_to_prefvalue(v)?)))
                .collect::<Result<HashMap<_, _>, anyhow::Error>>()?,
        ),
        _ => anyhow::bail!("Unsupported TOML value for PrefValue"),
    })
}

/// Turns a defaults_rs::PrefValue into its toml::Value counterpart.
pub fn prefvalue_to_toml(val: &PrefValue) -> Value {
    match val {
        PrefValue::String(s) => Value::String(s.clone()),
        PrefValue::Integer(i) => Value::Integer(*i),
        PrefValue::Float(f) => Value::Float(*f),
        PrefValue::Boolean(b) => Value::Boolean(*b),
        PrefValue::Array(arr) => Value::Array(arr.iter().map(prefvalue_to_toml).collect()),
        PrefValue::Dictionary(dict) => {
            let map = dict
                .iter()
                .map(|(k, v)| (k.clone(), prefvalue_to_toml(v)))
                .collect();
            Value::Table(map)
        }
    }
}
