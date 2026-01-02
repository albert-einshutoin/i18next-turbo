use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::Path;

use crate::config::Config;
use crate::extractor::ExtractedKey;

/// Result of syncing keys to a locale file
#[derive(Debug, Default)]
pub struct SyncResult {
    pub file_path: String,
    pub added_keys: Vec<String>,
    pub existing_keys: usize,
}

/// Read a JSON locale file, returning an empty map if it doesn't exist
pub fn read_locale_file(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read locale file: {}", path.display()))?;

    // Handle empty files
    if content.trim().is_empty() {
        return Ok(Map::new());
    }

    let map: Map<String, Value> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON in: {}", path.display()))?;

    Ok(map)
}

/// Insert a nested key path, creating intermediate objects as needed.
/// Returns true if the key was newly added, false if it already existed.
fn insert_nested_key(obj: &mut Map<String, Value>, path: &[&str], default_value: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    if path.len() == 1 {
        // Leaf node
        if obj.contains_key(path[0]) {
            false
        } else {
            obj.insert(
                path[0].to_string(),
                Value::String(default_value.to_string()),
            );
            true
        }
    } else {
        // Intermediate node - ensure it's an object
        let entry = obj
            .entry(path[0].to_string())
            .or_insert_with(|| Value::Object(Map::new()));

        if let Value::Object(ref mut nested) = entry {
            insert_nested_key(nested, &path[1..], default_value)
        } else {
            // Key exists but is not an object - conflict, skip
            false
        }
    }
}

/// Recursively sort all keys in a JSON object alphabetically
pub fn sort_keys_alphabetically(map: &Map<String, Value>) -> Map<String, Value> {
    let mut sorted = Map::new();
    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();

    for key in keys {
        let value = map.get(key).unwrap();
        let sorted_value = match value {
            Value::Object(nested) => Value::Object(sort_keys_alphabetically(nested)),
            other => other.clone(),
        };
        sorted.insert(key.clone(), sorted_value);
    }

    sorted
}

/// Merge extracted keys into an existing translation map.
/// - New keys are added with default_value if available, otherwise empty string
/// - Existing keys are preserved (translations are kept)
pub fn merge_keys(
    existing: &mut Map<String, Value>,
    keys: &[ExtractedKey],
    target_namespace: &str,
    default_namespace: &str,
) -> SyncResult {
    let mut result = SyncResult::default();

    for key in keys {
        // Determine which namespace this key belongs to
        let key_namespace = key
            .namespace
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(default_namespace);

        // Skip keys that don't belong to this namespace
        if key_namespace != target_namespace {
            continue;
        }

        // Handle nested keys: "button.submit" -> {"button": {"submit": ""}}
        let parts: Vec<&str> = key.key.split('.').collect();

        // Use default_value if available, otherwise empty string
        let value = key.default_value.as_deref().unwrap_or("");

        if insert_nested_key(existing, &parts, value) {
            result.added_keys.push(key.key.clone());
        } else {
            result.existing_keys += 1;
        }
    }

    result
}

/// Write JSON to file atomically using temp file + rename pattern
pub fn write_locale_file(path: &Path, content: &Map<String, Value>) -> Result<()> {
    use std::io::Write;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write to temp file first
    let temp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(content)?;

    {
        let mut file = std::fs::File::create(&temp_path)
            .with_context(|| format!("Failed to create temp file: {}", temp_path.display()))?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?; // Trailing newline
        file.sync_all()?;
    }

    // Atomic rename
    std::fs::rename(&temp_path, path)
        .with_context(|| format!("Failed to rename temp file to: {}", path.display()))?;

    Ok(())
}

/// Sync extracted keys to all locale files
pub fn sync_all_locales(
    config: &Config,
    keys: &[ExtractedKey],
    output_dir: &str,
) -> Result<Vec<SyncResult>> {
    let mut results = Vec::new();

    // Collect all namespaces from keys
    let mut namespaces: std::collections::HashSet<String> = std::collections::HashSet::new();
    namespaces.insert(config.default_namespace.clone());

    for key in keys {
        if let Some(ns) = &key.namespace {
            namespaces.insert(ns.clone());
        }
    }

    // Process each locale and namespace combination
    for locale in &config.locales {
        for namespace in &namespaces {
            let file_path = Path::new(output_dir)
                .join(locale)
                .join(format!("{}.json", namespace));

            // Read existing file
            let mut content = read_locale_file(&file_path)?;

            // Merge new keys
            let mut sync_result =
                merge_keys(&mut content, keys, namespace, &config.default_namespace);
            sync_result.file_path = file_path.display().to_string();

            // Sort keys alphabetically
            let sorted = sort_keys_alphabetically(&content);

            // Write back to file
            write_locale_file(&file_path, &sorted)?;

            results.push(sync_result);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_nested_key_simple() {
        let mut map = Map::new();
        let added = insert_nested_key(&mut map, &["hello"], "");

        assert!(added);
        assert_eq!(map.get("hello"), Some(&Value::String("".to_string())));
    }

    #[test]
    fn test_insert_nested_key_deep() {
        let mut map = Map::new();
        let added = insert_nested_key(&mut map, &["button", "submit"], "");

        assert!(added);
        let button = map.get("button").unwrap().as_object().unwrap();
        assert_eq!(button.get("submit"), Some(&Value::String("".to_string())));
    }

    #[test]
    fn test_insert_nested_key_existing() {
        let mut map = Map::new();
        map.insert("hello".to_string(), Value::String("world".to_string()));

        let added = insert_nested_key(&mut map, &["hello"], "");

        assert!(!added);
        assert_eq!(map.get("hello"), Some(&Value::String("world".to_string())));
    }

    #[test]
    fn test_sort_keys_alphabetically() {
        let mut map = Map::new();
        map.insert("zebra".to_string(), Value::String("z".to_string()));
        map.insert("apple".to_string(), Value::String("a".to_string()));
        map.insert("mango".to_string(), Value::String("m".to_string()));

        let sorted = sort_keys_alphabetically(&map);
        let keys: Vec<_> = sorted.keys().collect();

        assert_eq!(keys, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn test_sort_nested_objects() {
        let mut inner = Map::new();
        inner.insert("z".to_string(), Value::String("1".to_string()));
        inner.insert("a".to_string(), Value::String("2".to_string()));

        let mut map = Map::new();
        map.insert("nested".to_string(), Value::Object(inner));

        let sorted = sort_keys_alphabetically(&map);
        let nested = sorted.get("nested").unwrap().as_object().unwrap();
        let keys: Vec<_> = nested.keys().collect();

        assert_eq!(keys, vec!["a", "z"]);
    }

    #[test]
    fn test_merge_keys() {
        let mut existing = Map::new();
        existing.insert(
            "existing".to_string(),
            Value::String("translated".to_string()),
        );

        let keys = vec![
            ExtractedKey {
                key: "existing".to_string(),
                namespace: None,
                default_value: None,
            },
            ExtractedKey {
                key: "new.key".to_string(),
                namespace: None,
                default_value: None,
            },
        ];

        let result = merge_keys(&mut existing, &keys, "translation", "translation");

        assert_eq!(result.added_keys.len(), 1);
        assert_eq!(result.added_keys[0], "new.key");
        assert_eq!(result.existing_keys, 1);
        // Existing translation is preserved
        assert_eq!(
            existing.get("existing"),
            Some(&Value::String("translated".to_string()))
        );
    }

    #[test]
    fn test_merge_keys_with_default_value() {
        let mut existing = Map::new();

        let keys = vec![
            ExtractedKey {
                key: "greeting".to_string(),
                namespace: None,
                default_value: Some("Hello World!".to_string()),
            },
            ExtractedKey {
                key: "no_default".to_string(),
                namespace: None,
                default_value: None,
            },
        ];

        let result = merge_keys(&mut existing, &keys, "translation", "translation");

        assert_eq!(result.added_keys.len(), 2);
        // Key with default_value should use that value
        assert_eq!(
            existing.get("greeting"),
            Some(&Value::String("Hello World!".to_string()))
        );
        // Key without default_value should use empty string
        assert_eq!(
            existing.get("no_default"),
            Some(&Value::String("".to_string()))
        );
    }
}
