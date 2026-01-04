use anyhow::Result;
use serde_json::{Map, Value};

use crate::config::Config;
use crate::json_sync;

pub fn run(
    config: &Config,
    old_key: &str,
    new_key: &str,
    dry_run: bool,
    locales_only: bool,
) -> Result<()> {
    println!("=== i18next-turbo rename-key ===\n");

    // Parse namespace from keys
    let (old_ns, old_key_path) = parse_key_with_ns(old_key, &config.default_namespace);
    let (new_ns, new_key_path) = parse_key_with_ns(new_key, &config.default_namespace);

    println!("Renaming key:");
    println!("  From: {}:{}", old_ns, old_key_path);
    println!("  To:   {}:{}", new_ns, new_key_path);
    if dry_run {
        println!("  Mode: Dry run (no files will be modified)");
    }
    println!();

    let mut source_changes = 0;
    let mut locale_changes = 0;

    // Step 1: Rename in source files (unless locales_only)
    if !locales_only {
        println!("Scanning source files...");

        for pattern in &config.input {
            let matches = glob::glob(pattern)?;
            for entry in matches {
                if let Ok(path) = entry {
                    if path.is_file() {
                        let content = std::fs::read_to_string(&path)?;

                        // Build the full old key for search
                        let search_key = if old_ns == config.default_namespace {
                            old_key_path.clone()
                        } else {
                            format!("{}:{}", old_ns, old_key_path)
                        };

                        // Build the full new key for replacement
                        let replace_key = if new_ns == config.default_namespace {
                            new_key_path.clone()
                        } else {
                            format!("{}:{}", new_ns, new_key_path)
                        };

                        // Check if file contains the old key
                        if content.contains(&format!("'{}'", search_key))
                            || content.contains(&format!("\"{}\"", search_key))
                            || content.contains(&format!("`{}`", search_key))
                        {
                            let new_content = content
                                .replace(
                                    &format!("'{}'", search_key),
                                    &format!("'{}'", replace_key),
                                )
                                .replace(
                                    &format!("\"{}\"", search_key),
                                    &format!("\"{}\"", replace_key),
                                )
                                .replace(
                                    &format!("`{}`", search_key),
                                    &format!("`{}`", replace_key),
                                );

                            if new_content != content {
                                println!("  {}", path.display());
                                source_changes += 1;

                                if !dry_run {
                                    std::fs::write(&path, new_content)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        if source_changes == 0 {
            println!("  No source files contain the key.");
        }
    }

    // Step 2: Rename in locale files
    println!("\nUpdating locale files...");
    let locales_path = std::path::Path::new(&config.output);

    for locale in &config.locales {
        let ns_file = locales_path
            .join(locale)
            .join(format!("{}.json", old_ns));

        if !ns_file.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&ns_file)?;
        if content.trim().is_empty() {
            continue;
        }

        let mut json: Value = serde_json::from_str(&content)?;

        // Get the value at old key path
        let old_value = get_nested_value(&json, &old_key_path);

        if let Some(value) = old_value {
            // Remove old key
            remove_nested_key(&mut json, &old_key_path);

            // If namespace changed, we need to write to a different file
            if old_ns != new_ns {
                // Write updated old namespace file
                if !dry_run {
                    if let Some(obj) = json.as_object() {
                        let sorted = json_sync::sort_keys_alphabetically(obj);
                        let output = serde_json::to_string_pretty(&sorted)?;
                        std::fs::write(&ns_file, format!("{}\n", output))?;
                    }
                }

                // Add to new namespace file
                let new_ns_file = locales_path
                    .join(locale)
                    .join(format!("{}.json", new_ns));

                let mut new_json = if new_ns_file.exists() {
                    let new_content = std::fs::read_to_string(&new_ns_file)?;
                    if new_content.trim().is_empty() {
                        Value::Object(Map::new())
                    } else {
                        serde_json::from_str(&new_content)?
                    }
                } else {
                    Value::Object(Map::new())
                };

                set_nested_value(&mut new_json, &new_key_path, value);

                if !dry_run {
                    if let Some(obj) = new_json.as_object() {
                        let sorted = json_sync::sort_keys_alphabetically(obj);
                        let output = serde_json::to_string_pretty(&sorted)?;
                        std::fs::write(&new_ns_file, format!("{}\n", output))?;
                    }
                }

                println!("  {}/{}.json -> {}/{}.json", locale, old_ns, locale, new_ns);
            } else {
                // Same namespace, just rename key path
                set_nested_value(&mut json, &new_key_path, value);

                if !dry_run {
                    if let Some(obj) = json.as_object() {
                        let sorted = json_sync::sort_keys_alphabetically(obj);
                        let output = serde_json::to_string_pretty(&sorted)?;
                        std::fs::write(&ns_file, format!("{}\n", output))?;
                    }
                }

                println!("  {}/{}.json", locale, old_ns);
            }

            locale_changes += 1;
        }
    }

    if locale_changes == 0 {
        println!("  Key not found in any locale files.");
    }

    // Summary
    println!("\n{}", "=".repeat(40));
    println!("Summary:");
    if !locales_only {
        println!("  Source files updated: {}", source_changes);
    }
    println!("  Locale files updated: {}", locale_changes);

    if dry_run {
        println!("\n[Dry run] No files were modified.");
    } else if source_changes > 0 || locale_changes > 0 {
        println!("\nDone!");
    }

    Ok(())
}

/// Parse a key that may contain namespace (ns:key.path)
fn parse_key_with_ns(key: &str, default_ns: &str) -> (String, String) {
    if key.contains(':') {
        let parts: Vec<&str> = key.splitn(2, ':').collect();
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (default_ns.to_string(), key.to_string())
    }
}

/// Get a nested value from JSON using dot notation
fn get_nested_value(json: &Value, path: &str) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for part in parts {
        match current.get(part) {
            Some(v) => current = v,
            None => return None,
        }
    }

    Some(current.clone())
}

/// Remove a nested key from JSON using dot notation
fn remove_nested_key(json: &mut Value, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.len() == 1 {
        if let Value::Object(obj) = json {
            obj.remove(parts[0]);
        }
        return;
    }

    // Navigate to parent
    let mut current = json;
    for part in &parts[..parts.len() - 1] {
        match current.get_mut(*part) {
            Some(v) => current = v,
            None => return,
        }
    }

    // Remove the last key
    if let Value::Object(obj) = current {
        if let Some(last) = parts.last() {
            obj.remove(*last);
        }
    }
}

/// Set a nested value in JSON using dot notation
fn set_nested_value(json: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last part - set the value
            if let Value::Object(obj) = current {
                obj.insert((*part).to_string(), value);
            }
            return;
        }

        // Navigate or create intermediate objects
        if let Value::Object(obj) = current {
            if !obj.contains_key(*part) {
                obj.insert((*part).to_string(), Value::Object(Map::new()));
            }
            if let Some(val) = obj.get_mut(*part) {
                current = val;
            } else {
                return;
            }
        }
    }
}
