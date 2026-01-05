use anyhow::{Context, Result};
use fs2::FileExt;
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::config::Config;
use crate::extractor::ExtractedKey;
use crate::fs::FileSystem;

/// Result of syncing keys to a locale file
#[derive(Debug, Default)]
pub struct SyncResult {
    pub file_path: String,
    pub added_keys: Vec<String>,
    pub existing_keys: usize,
}

/// Read a JSON locale file, returning an empty map if it doesn't exist
pub fn read_locale_file(path: &Path) -> Result<Map<String, Value>> {
    read_locale_file_with_fs(path, &crate::fs::RealFileSystem)
}

/// Read a JSON locale file using the provided FileSystem
pub fn read_locale_file_with_fs<F: FileSystem>(path: &Path, fs: &F) -> Result<Map<String, Value>> {
    if !fs.exists(path) {
        return Ok(Map::new());
    }

    let content = fs
        .read_to_string(path)
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
///
/// This function uses iterative approach instead of recursion to prevent
/// stack overflow with deeply nested keys (DoS protection).
fn insert_nested_key(obj: &mut Map<String, Value>, path: &[&str], default_value: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    // Use iterative approach to prevent stack overflow with deep nesting
    let mut current = obj;

    for (i, key) in path.iter().enumerate() {
        let is_last = i == path.len() - 1;

        if is_last {
            // Leaf node - insert the value
            if current.contains_key(*key) {
                return false;
            } else {
                current.insert(
                    (*key).to_string(),
                    Value::String(default_value.to_string()),
                );
                return true;
            }
        } else {
            // Intermediate node - ensure it's an object
            // We need to use a raw pointer trick to satisfy the borrow checker
            // because we're iterating while mutating
            let entry = current
                .entry((*key).to_string())
                .or_insert_with(|| Value::Object(Map::new()));

            match entry {
                Value::Object(ref mut nested) => {
                    current = nested;
                }
                _ => {
                    // Key exists but is not an object - conflict, skip
                    return false;
                }
            }
        }
    }

    false
}

/// Sort all keys in a JSON object alphabetically (including nested objects).
///
/// Uses a controlled recursion with explicit depth limit to prevent stack overflow
/// from malicious inputs (DoS protection). Maximum depth is 100 levels.
pub fn sort_keys_alphabetically(map: &Map<String, Value>) -> Map<String, Value> {
    const MAX_DEPTH: usize = 100;
    sort_keys_with_depth(map, 0, MAX_DEPTH)
}

/// Internal function with depth tracking to prevent stack overflow
fn sort_keys_with_depth(map: &Map<String, Value>, depth: usize, max_depth: usize) -> Map<String, Value> {
    let mut sorted = Map::new();
    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();

    for key in keys {
        if let Some(value) = map.get(key) {
            let sorted_value = match value {
                Value::Object(nested) if depth < max_depth => {
                    Value::Object(sort_keys_with_depth(nested, depth + 1, max_depth))
                }
                Value::Object(nested) => {
                    // At max depth, just clone without sorting deeper
                    Value::Object(nested.clone())
                }
                other => other.clone(),
            };
            sorted.insert(key.clone(), sorted_value);
        }
    }

    sorted
}

/// Merge extracted keys into an existing translation map.
/// - New keys are added with default_value if available, otherwise empty string
/// - Existing keys are preserved (translations are kept)
/// - If key_separator is empty, keys are stored flat (not nested)
pub fn merge_keys(
    existing: &mut Map<String, Value>,
    keys: &[ExtractedKey],
    target_namespace: &str,
    default_namespace: &str,
    key_separator: &str,
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

        // Use default_value if available, otherwise empty string
        let value = key.default_value.as_deref().unwrap_or("");

        // If key_separator is empty, use flat keys
        if key_separator.is_empty() {
            // Flat key mode: store as-is
            if existing.contains_key(&key.key) {
                result.existing_keys += 1;
            } else {
                existing.insert(key.key.clone(), Value::String(value.to_string()));
                result.added_keys.push(key.key.clone());
            }
        } else {
            // Handle nested keys: "button.submit" -> {"button": {"submit": ""}}
            let parts: Vec<&str> = key.key.split(key_separator).collect();

            if insert_nested_key(existing, &parts, value) {
                result.added_keys.push(key.key.clone());
            } else {
                result.existing_keys += 1;
            }
        }
    }

    result
}

/// Write JSON to file atomically using temp file + rename pattern
pub fn write_locale_file(path: &Path, content: &Map<String, Value>) -> Result<()> {
    write_locale_file_with_fs(path, content, &crate::fs::RealFileSystem)
}

/// Write JSON to file using the provided FileSystem
pub fn write_locale_file_with_fs<F: FileSystem>(
    path: &Path,
    content: &Map<String, Value>,
    fs: &F,
) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write to temp file first
    let temp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(content)?;
    let json_with_newline = format!("{}\n", json);

    fs.write(&temp_path, &json_with_newline)
        .with_context(|| format!("Failed to write temp file: {}", temp_path.display()))?;

    // Atomic rename
    fs.rename(&temp_path, path)
        .with_context(|| format!("Failed to rename temp file to: {}", path.display()))?;

    Ok(())
}

/// Atomically read, modify, and write a locale file with exclusive file locking.
/// This prevents data corruption when multiple processes access the same file.
///
/// The lock is held for the entire read-modify-write cycle to ensure ACID-like
/// transaction guarantees.
pub fn sync_locale_file_locked(
    path: &Path,
    keys: &[ExtractedKey],
    target_namespace: &str,
    default_namespace: &str,
    key_separator: &str,
) -> Result<SyncResult> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Create or open the file for reading and writing
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("Failed to open locale file: {}", path.display()))?;

    // Acquire exclusive lock (blocks until available)
    file.lock_exclusive()
        .with_context(|| format!("Failed to acquire lock on: {}", path.display()))?;

    // Read existing content with BufReader for efficiency
    let mut content = {
        let mut reader = BufReader::new(&file);
        let mut content_str = String::new();
        reader.read_to_string(&mut content_str)
            .with_context(|| format!("Failed to read locale file: {}", path.display()))?;

        if content_str.trim().is_empty() {
            Map::new()
        } else {
            serde_json::from_str(&content_str)
                .with_context(|| format!("Failed to parse JSON in: {}", path.display()))?
        }
    };

    // Merge new keys
    let mut sync_result = merge_keys(
        &mut content,
        keys,
        target_namespace,
        default_namespace,
        key_separator,
    );
    sync_result.file_path = path.display().to_string();

    // Only write if there were changes
    if !sync_result.added_keys.is_empty() {
        // Sort keys alphabetically
        let sorted = sort_keys_alphabetically(&content);

        // Write to temp file first (atomic write pattern)
        let temp_path = path.with_extension("json.tmp");
        {
            let temp_file = File::create(&temp_path)
                .with_context(|| format!("Failed to create temp file: {}", temp_path.display()))?;
            let mut writer = BufWriter::new(temp_file);
            serde_json::to_writer_pretty(&mut writer, &sorted)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }

        // Atomic rename
        std::fs::rename(&temp_path, path)
            .with_context(|| format!("Failed to rename temp file to: {}", path.display()))?;
    }

    // Lock is automatically released when file is dropped
    Ok(sync_result)
}

/// Collect unique namespaces from a set of extracted keys
pub fn collect_namespaces(keys: &[ExtractedKey], default_namespace: &str) -> std::collections::HashSet<String> {
    let mut namespaces = std::collections::HashSet::new();
    namespaces.insert(default_namespace.to_string());

    for key in keys {
        if let Some(ns) = &key.namespace {
            namespaces.insert(ns.clone());
        }
    }

    namespaces
}

/// Sync extracted keys to specific namespace files only (for incremental updates)
/// This is more efficient when only a subset of namespaces have changed.
///
/// Uses file locking to prevent data corruption when multiple processes
/// (e.g., watch mode + manual extract) access the same files.
pub fn sync_namespaces(
    config: &Config,
    keys: &[ExtractedKey],
    output_dir: &str,
    namespaces: &std::collections::HashSet<String>,
) -> Result<Vec<SyncResult>> {
    let mut results = Vec::new();

    // Process only the specified namespace files
    for locale in &config.locales {
        for namespace in namespaces {
            let file_path = Path::new(output_dir)
                .join(locale)
                .join(format!("{}.json", namespace));

            // Use locked sync for data integrity
            let sync_result = sync_locale_file_locked(
                &file_path,
                keys,
                namespace,
                &config.default_namespace,
                &config.key_separator,
            )?;

            results.push(sync_result);
        }
    }

    Ok(results)
}

/// Sync extracted keys to all locale files
pub fn sync_all_locales(
    config: &Config,
    keys: &[ExtractedKey],
    output_dir: &str,
) -> Result<Vec<SyncResult>> {
    // Collect all namespaces from keys
    let namespaces = collect_namespaces(keys, &config.default_namespace);

    // Use the namespace-specific sync
    sync_namespaces(config, keys, output_dir, &namespaces)
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
        let button = map
            .get("button")
            .expect("button should exist after insert_nested_key")
            .as_object()
            .expect("button should be an object after insert_nested_key");
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
        let nested = sorted
            .get("nested")
            .expect("nested should exist after sort_keys_alphabetically")
            .as_object()
            .expect("nested should be an object in sort_keys_alphabetically");
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

        let result = merge_keys(&mut existing, &keys, "translation", "translation", ".");

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

        let result = merge_keys(&mut existing, &keys, "translation", "translation", ".");

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

    #[test]
    fn test_merge_keys_flat_mode() {
        let mut existing = Map::new();

        let keys = vec![
            ExtractedKey {
                key: "button.submit".to_string(),
                namespace: None,
                default_value: Some("Submit".to_string()),
            },
            ExtractedKey {
                key: "form.validation.required".to_string(),
                namespace: None,
                default_value: None,
            },
        ];

        // Empty separator = flat key mode
        let result = merge_keys(&mut existing, &keys, "translation", "translation", "");

        assert_eq!(result.added_keys.len(), 2);
        // Keys should be stored as-is, not nested
        assert_eq!(
            existing.get("button.submit"),
            Some(&Value::String("Submit".to_string()))
        );
        assert_eq!(
            existing.get("form.validation.required"),
            Some(&Value::String("".to_string()))
        );
        // Should NOT have nested structure
        assert!(existing.get("button").is_none());
        assert!(existing.get("form").is_none());
    }
}
