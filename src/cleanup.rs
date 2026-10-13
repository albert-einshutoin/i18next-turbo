#![allow(clippy::too_many_arguments)]

use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::path::Path;

use crate::config::OutputFormat;
use crate::extractor::ExtractedKey;
use crate::json_sync;

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
    namespace_less_mode: bool,
    merge_namespaces: bool,
    preserve_context_variants: bool,
    context_separator: &str,
    locale: &str,
    format: OutputFormat,
) -> Result<Vec<DeadKey>> {
    let mut dead_keys = Vec::new();

    // Build a set of extracted key paths (namespace:key format)
    let mut extracted_set: HashSet<String> = HashSet::new();
    let mut object_root_set: HashSet<String> = HashSet::new();
    for key in extracted_keys {
        let ns = key.namespace.as_deref().unwrap_or(default_namespace);
        if let Some(root) = key.key.strip_suffix(".*") {
            object_root_set.insert(format_key_id(ns, root, namespace_less_mode));
        } else {
            extracted_set.insert(format_key_id(ns, &key.key, namespace_less_mode));
        }
    }

    // Scan locale directory
    let locale_dir = locales_dir.join(locale);
    if !locale_dir.exists() {
        return Ok(dead_keys);
    }

    for path in json_sync::list_locale_files(locales_dir, locale, format)? {
        let namespace = json_sync::locale_namespace_from_path(&path)
            .unwrap_or_else(|| "translation".to_string());
        let json = json_sync::read_locale_value(&path, format)
            .with_context(|| format!("Failed to parse: {}", path.display()))?;

        if let Value::Object(obj) = json {
            let file_path = path.display().to_string();
            if merge_namespaces && !namespace_less_mode {
                for (root_ns, value) in obj {
                    match value {
                        Value::Object(nested) => {
                            find_dead_keys_in_object(
                                &nested,
                                &root_ns,
                                "",
                                &extracted_set,
                                &object_root_set,
                                namespace_less_mode,
                                preserve_context_variants,
                                context_separator,
                                &file_path,
                                &mut dead_keys,
                            );
                        }
                        Value::String(_) => {
                            let full_key = format_key_id(&root_ns, "", namespace_less_mode);
                            if !extracted_set.contains(&full_key) {
                                dead_keys.push(DeadKey {
                                    file_path: file_path.clone(),
                                    key_path: root_ns.clone(),
                                    namespace: root_ns.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                find_dead_keys_in_object(
                    &obj,
                    &namespace,
                    "",
                    &extracted_set,
                    &object_root_set,
                    namespace_less_mode,
                    preserve_context_variants,
                    context_separator,
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
    object_root_set: &HashSet<String>,
    namespace_less_mode: bool,
    preserve_context_variants: bool,
    context_separator: &str,
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
                    object_root_set,
                    namespace_less_mode,
                    preserve_context_variants,
                    context_separator,
                    file_path,
                    dead_keys,
                );
            }
            Value::String(_) => {
                // Check if this leaf key exists in extracted keys
                let full_key = format_key_id(namespace, &key_path, namespace_less_mode);
                let covered_by_object_root = object_root_set
                    .iter()
                    .any(|root| full_key == *root || full_key.starts_with(&format!("{}.", root)));
                let covered_by_context_variant = preserve_context_variants
                    && is_covered_by_context_variant(
                        namespace,
                        &key_path,
                        extracted_set,
                        namespace_less_mode,
                        context_separator,
                    );
                if !extracted_set.contains(&full_key)
                    && !covered_by_object_root
                    && !covered_by_context_variant
                {
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

fn is_covered_by_context_variant(
    namespace: &str,
    key_path: &str,
    extracted_set: &HashSet<String>,
    namespace_less_mode: bool,
    context_separator: &str,
) -> bool {
    if context_separator.is_empty() {
        return false;
    }

    let mut candidate = key_path.to_string();
    while let Some((base, _)) = candidate.rsplit_once(context_separator) {
        let full_base = format_key_id(namespace, base, namespace_less_mode);
        if extracted_set.contains(&full_base) {
            return true;
        }
        candidate = base.to_string();
    }
    false
}

fn format_key_id(namespace: &str, key_path: &str, namespace_less_mode: bool) -> String {
    if namespace_less_mode {
        key_path.to_string()
    } else {
        format!("{}:{}", namespace, key_path)
    }
}

/// Remove dead keys from locale files (purge mode)
pub fn purge_dead_keys(
    _locales_dir: &Path,
    dead_keys: &[DeadKey],
    format: OutputFormat,
) -> Result<usize> {
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

        let mut json = json_sync::read_locale_value(path, format)?;

        if let Value::Object(ref mut obj) = json {
            for key_path in &key_paths {
                if remove_nested_key(obj, key_path) {
                    removed_count += 1;
                }
            }
        }

        // Write back
        if let Value::Object(ref obj) = json {
            let sorted = json_sync::sort_keys_alphabetically(obj);
            json_sync::write_locale_file(path, &sorted, format, None)?;
        }
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

    #[test]
    fn test_context_variant_is_preserved_when_base_key_exists() {
        let mut extracted_set = HashSet::new();
        extracted_set.insert("common:friend".to_string());

        assert!(is_covered_by_context_variant(
            "common",
            "friend_male",
            &extracted_set,
            false,
            "_",
        ));
        assert!(is_covered_by_context_variant(
            "common",
            "friend_male_one",
            &extracted_set,
            false,
            "_",
        ));
    }

    #[test]
    fn test_find_dead_keys_with_merge_namespaces_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let locale_dir = tmp.path().join("en");
        std::fs::create_dir_all(&locale_dir).unwrap();
        std::fs::write(
            locale_dir.join("all.json"),
            r#"{
  "common": { "hello": "Hello", "stale": "Old" },
  "home": { "title": "Title" }
}"#,
        )
        .unwrap();

        let extracted_keys = vec![
            ExtractedKey {
                locales: None,
                key: "hello".to_string(),
                namespace: Some("common".to_string()),
                default_value: None,
            },
            ExtractedKey {
                locales: None,
                key: "title".to_string(),
                namespace: Some("home".to_string()),
                default_value: None,
            },
        ];

        let dead = find_dead_keys(
            tmp.path(),
            &extracted_keys,
            "translation",
            false,
            true,
            false,
            "_",
            "en",
            crate::config::OutputFormat::Json,
        )
        .unwrap();

        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].namespace, "common");
        assert_eq!(dead[0].key_path, "stale");
    }
}
