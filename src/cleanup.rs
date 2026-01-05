use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::path::Path;

use crate::extractor::ExtractedKey;

/// Result of dead key detection
#[derive(Debug, Default)]
pub struct CleanupResult {
    pub dead_keys: Vec<DeadKey>,
    pub removed_count: usize,
}

/// A dead key found in translation files
#[derive(Debug)]
pub struct DeadKey {
    pub file_path: String,
    pub key_path: String,
    pub namespace: String,
}

/// Find dead keys that exist in JSON but not in source code
pub fn find_dead_keys(
    locales_dir: &Path,
    extracted_keys: &[ExtractedKey],
    default_namespace: &str,
    locale: &str,
) -> Result<Vec<DeadKey>> {
    let mut dead_keys = Vec::new();

    // Build a set of extracted key paths (namespace:key format)
    let mut extracted_set: HashSet<String> = HashSet::new();
    for key in extracted_keys {
        let ns = key.namespace.as_deref().unwrap_or(default_namespace);
        // Store both the full key and flattened version
        extracted_set.insert(format!("{}:{}", ns, key.key));
    }

    // Scan locale directory
    let locale_dir = locales_dir.join(locale);
    if !locale_dir.exists() {
        return Ok(dead_keys);
    }

    for entry in std::fs::read_dir(&locale_dir)
        .with_context(|| format!("Failed to read: {}", locale_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let namespace = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("translation")
                .to_string();

            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read: {}", path.display()))?;

            if content.trim().is_empty() {
                continue;
            }

            let json: Value = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse: {}", path.display()))?;

            if let Value::Object(obj) = json {
                let file_path = path.display().to_string();
                find_dead_keys_in_object(
                    &obj,
                    &namespace,
                    "",
                    &extracted_set,
                    &file_path,
                    &mut dead_keys,
                );
            }
        }
    }

    Ok(dead_keys)
}

/// Recursively find dead keys in a JSON object
fn find_dead_keys_in_object(
    obj: &Map<String, Value>,
    namespace: &str,
    prefix: &str,
    extracted_set: &HashSet<String>,
    file_path: &str,
    dead_keys: &mut Vec<DeadKey>,
) {
    for (key, value) in obj {
        let key_path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        match value {
            Value::Object(nested) => {
                // Recurse into nested objects
                find_dead_keys_in_object(
                    nested,
                    namespace,
                    &key_path,
                    extracted_set,
                    file_path,
                    dead_keys,
                );
            }
            Value::String(_) => {
                // Check if this leaf key exists in extracted keys
                let full_key = format!("{}:{}", namespace, key_path);
                if !extracted_set.contains(&full_key) {
                    dead_keys.push(DeadKey {
                        file_path: file_path.to_string(),
                        key_path: key_path.clone(),
                        namespace: namespace.to_string(),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Remove dead keys from locale files (purge mode)
pub fn purge_dead_keys(_locales_dir: &Path, dead_keys: &[DeadKey]) -> Result<usize> {
    use std::collections::HashMap;

    // Group dead keys by file
    let mut keys_by_file: HashMap<&str, Vec<&str>> = HashMap::new();
    for dk in dead_keys {
        keys_by_file
            .entry(dk.file_path.as_str())
            .or_default()
            .push(dk.key_path.as_str());
    }

    let mut removed_count = 0;

    for (file_path, key_paths) in keys_by_file {
        let path = Path::new(file_path);
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(path)?;
        let mut json: Value = serde_json::from_str(&content)?;

        if let Value::Object(ref mut obj) = json {
            for key_path in &key_paths {
                if remove_nested_key(obj, key_path) {
                    removed_count += 1;
                }
            }
        }

        // Write back
        let output = serde_json::to_string_pretty(&json)?;
        std::fs::write(path, format!("{}\n", output))?;
    }

    Ok(removed_count)
}

/// Remove a nested key from a JSON object
fn remove_nested_key(obj: &mut Map<String, Value>, key_path: &str) -> bool {
    let parts: Vec<&str> = key_path.split('.').collect();

    if parts.is_empty() {
        return false;
    }

    if parts.len() == 1 {
        return obj.remove(parts[0]).is_some();
    }

    // Navigate to parent
    let mut current = obj;
    for part in &parts[..parts.len() - 1] {
        match current.get_mut(*part) {
            Some(Value::Object(nested)) => {
                current = nested;
            }
            _ => return false,
        }
    }

    // Remove the final key
    let last_key = parts[parts.len() - 1];
    current.remove(last_key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_nested_key_simple() {
        let mut obj = Map::new();
        obj.insert("hello".to_string(), Value::String("world".to_string()));

        assert!(remove_nested_key(&mut obj, "hello"));
        assert!(!obj.contains_key("hello"));
    }

    #[test]
    fn test_remove_nested_key_deep() {
        let mut inner = Map::new();
        inner.insert("submit".to_string(), Value::String("Submit".to_string()));
        inner.insert("cancel".to_string(), Value::String("Cancel".to_string()));

        let mut obj = Map::new();
        obj.insert("button".to_string(), Value::Object(inner));

        assert!(remove_nested_key(&mut obj, "button.submit"));

        let button = obj.get("button").unwrap().as_object().unwrap();
        assert!(!button.contains_key("submit"));
        assert!(button.contains_key("cancel"));
    }
}
