use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::Path;

use crate::config::Config;
use crate::json_sync;

pub fn run(config: &Config, remove_unused: bool, dry_run: bool) -> Result<()> {
    println!("=== i18next-turbo sync ===\n");

    if config.locales.len() < 2 {
        println!("Sync requires at least 2 locales configured.");
        return Ok(());
    }

    let primary_locale = &config.locales[0];
    let secondary_locales: Vec<&String> = config.locales[1..].iter().collect();

    println!("Configuration:");
    println!("  Locales directory: {}", config.output);
    println!("  Primary locale: {}", primary_locale);
    println!("  Secondary locales: {:?}", secondary_locales);
    if dry_run {
        println!("  Mode: Dry run (no files will be modified)");
    }
    println!();

    let locales_path = Path::new(&config.output);
    let extension = config.output_extension();
    let output_format = config.output_format();

    // Read all namespaces from primary locale
    let primary_dir = locales_path.join(primary_locale);
    if !primary_dir.exists() {
        println!(
            "Primary locale directory does not exist: {}",
            primary_dir.display()
        );
        return Ok(());
    }

    let mut total_added = 0;
    let mut total_removed = 0;

    // Process each namespace file in primary locale
    for entry in std::fs::read_dir(&primary_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext == extension)
            .unwrap_or(false)
        {
            let namespace = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("translation");

            let primary_content = std::fs::read_to_string(&path)?;
            if primary_content.trim().is_empty() {
                continue;
            }

            let primary_json =
                json_sync::parse_locale_value_str(&primary_content, output_format, &path)
                    .with_context(|| format!("Failed to parse primary file: {}", path.display()))?;

            // Sync to each secondary locale
            for secondary_locale in &secondary_locales {
                let secondary_path = locales_path
                    .join(secondary_locale)
                    .join(format!("{}.{}", namespace, extension));

                let mut secondary_json = if secondary_path.exists() {
                    let content = std::fs::read_to_string(&secondary_path)?;
                    json_sync::parse_locale_value_str(&content, output_format, &secondary_path)
                        .with_context(|| {
                            format!(
                                "Failed to parse secondary file: {}",
                                secondary_path.display()
                            )
                        })?
                } else {
                    Value::Object(Map::new())
                };

                // Sync keys
                let (added, removed) =
                    sync_json_keys(&primary_json, &mut secondary_json, remove_unused);

                if added > 0 || removed > 0 {
                    println!(
                        "  {}/{}.{}: +{} added, -{} removed",
                        secondary_locale, namespace, extension, added, removed
                    );

                    if !dry_run {
                        // Ensure directory exists
                        if let Some(parent) = secondary_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        // Sort and write
                        if let Some(obj) = secondary_json.as_object() {
                            let sorted = json_sync::sort_keys_alphabetically(obj);
                            json_sync::write_locale_file(
                                &secondary_path,
                                &sorted,
                                output_format,
                                None,
                            )?;
                        }
                    }

                    total_added += added;
                    total_removed += removed;
                }
            }
        }
    }

    println!();
    if total_added == 0 && total_removed == 0 {
        println!("All locales are already in sync!");
    } else {
        println!("Summary:");
        println!("  Keys added: {}", total_added);
        if remove_unused {
            println!("  Keys removed: {}", total_removed);
        }
        if dry_run {
            println!("\n[Dry run] No files were modified.");
        } else {
            println!("\nDone!");
        }
    }

    Ok(())
}

/// Sync JSON keys from primary to secondary, returning (added, removed) counts
fn sync_json_keys(primary: &Value, secondary: &mut Value, remove_unused: bool) -> (usize, usize) {
    let mut added = 0;
    let mut removed = 0;

    if let (Value::Object(primary_obj), Value::Object(secondary_obj)) = (primary, secondary) {
        // Add missing keys from primary
        for (key, primary_value) in primary_obj {
            if !secondary_obj.contains_key(key) {
                // Add key with empty string or nested object
                let new_value = create_empty_structure(primary_value);
                secondary_obj.insert(key.clone(), new_value);
                added += count_leaf_keys(primary_value);
            } else if let Value::Object(_) = primary_value {
                // Recursively sync nested objects
                if let Some(secondary_value) = secondary_obj.get_mut(key) {
                    let (a, r) = sync_json_keys(primary_value, secondary_value, remove_unused);
                    added += a;
                    removed += r;
                }
            }
        }

        // Remove keys that don't exist in primary
        if remove_unused {
            let keys_to_remove: Vec<String> = secondary_obj
                .keys()
                .filter(|k| !primary_obj.contains_key(*k))
                .cloned()
                .collect();

            for key in keys_to_remove {
                if let Some(value) = secondary_obj.remove(&key) {
                    removed += count_leaf_keys(&value);
                }
            }
        }
    }

    (added, removed)
}

/// Create an empty structure matching the primary's structure
fn create_empty_structure(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut new_obj = Map::new();
            for (k, v) in obj {
                new_obj.insert(k.clone(), create_empty_structure(v));
            }
            Value::Object(new_obj)
        }
        _ => Value::String(String::new()),
    }
}

/// Count the number of leaf keys in a JSON structure
fn count_leaf_keys(value: &Value) -> usize {
    match value {
        Value::Object(obj) => obj.values().map(count_leaf_keys).sum(),
        Value::String(_) => 1,
        _ => 0,
    }
}
