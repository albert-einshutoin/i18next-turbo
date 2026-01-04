use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

use crate::cleanup;
use crate::config::Config;
use crate::extractor::{self, ExtractedKey};

pub fn run(config: &Config, locale: Option<String>) -> Result<()> {
    println!("=== i18next-turbo status ===\n");

    // Determine locale to check
    let check_locale = locale
        .as_ref()
        .or(config.locales.first())
        .map(|s| s.as_str())
        .unwrap_or("en");

    println!("Configuration:");
    println!("  Locales directory: {}", config.output);
    println!("  Checking locale: {}", check_locale);
    println!("  Default namespace: {}", config.default_namespace);
    println!();

    // Extract keys from source
    println!("Scanning source files...");
    let extraction = extractor::extract_from_glob(&config.input, &config.functions)?;

    let mut source_keys: HashSet<String> = HashSet::new();
    let mut all_keys: Vec<ExtractedKey> = Vec::new();

    for (_file_path, keys) in &extraction.files {
        for key in keys {
            let full_key = match &key.namespace {
                Some(ns) => format!("{}:{}", ns, key.key),
                None => format!("{}:{}", config.default_namespace, key.key),
            };
            source_keys.insert(full_key);
            all_keys.push(key.clone());
        }
    }

    println!("  Source files: {}", extraction.files.len());
    println!("  Keys in source: {}", source_keys.len());

    // Count keys in locale files
    let locales_path = Path::new(&config.output);
    let locale_dir = locales_path.join(check_locale);

    let mut locale_keys: HashSet<String> = HashSet::new();

    if locale_dir.exists() {
        for entry in std::fs::read_dir(&locale_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let namespace = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("translation");

                let content = std::fs::read_to_string(&path)?;
                if content.trim().is_empty() {
                    continue;
                }

                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    count_json_keys(&json, namespace, "", &mut locale_keys);
                }
            }
        }
    }

    println!("  Keys in locale: {}", locale_keys.len());

    // Find dead keys
    let dead_keys =
        cleanup::find_dead_keys(locales_path, &all_keys, &config.default_namespace, check_locale)?;

    // Find missing keys (in source but not in locale)
    let missing_count = source_keys
        .iter()
        .filter(|k| !locale_keys.contains(*k))
        .count();

    // Summary
    println!("\n{}", "=".repeat(40));
    println!("Summary:");
    println!("{}", "=".repeat(40));

    if dead_keys.is_empty() && missing_count == 0 {
        println!("  All keys are synchronized!");
    } else {
        if missing_count > 0 {
            println!(
                "  Missing keys (in source, not in locale): {}",
                missing_count
            );
        }
        if !dead_keys.is_empty() {
            println!(
                "  Dead keys (in locale, not in source): {}",
                dead_keys.len()
            );
        }
        println!();
        println!("Run 'i18next-turbo extract' to add missing keys.");
        if !dead_keys.is_empty() {
            println!("Run 'i18next-turbo check --remove' to remove dead keys.");
        }
    }

    Ok(())
}

/// Count all leaf keys in a JSON structure
fn count_json_keys(value: &Value, namespace: &str, prefix: &str, keys: &mut HashSet<String>) {
    match value {
        Value::Object(obj) => {
            for (k, v) in obj {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                count_json_keys(v, namespace, &path, keys);
            }
        }
        Value::String(_) => {
            keys.insert(format!("{}:{}", namespace, prefix));
        }
        _ => {}
    }
}
