use anyhow::{bail, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

use crate::cleanup;
use crate::config::Config;
use crate::extractor::{self, ExtractedKey};

pub fn run(
    config: &Config,
    locale: Option<String>,
    fail_on_incomplete: bool,
    namespace: Option<String>,
) -> Result<()> {
    println!("=== i18next-turbo status ===\n");

    // Determine locale to check
    let check_locale = locale
        .as_ref()
        .or(config.locales.first())
        .map(|s| s.as_str())
        .unwrap_or("en");

    let namespace_filter = namespace.as_deref();

    println!("Configuration:");
    println!("  Locales directory: {}", config.output);
    println!("  Checking locale: {}", check_locale);
    println!(
        "  Default namespace: {}",
        config.effective_default_namespace()
    );
    if let Some(ns) = namespace_filter {
        println!("  Namespace filter: {}", ns);
    }
    println!();

    // Extract keys from source
    println!("Scanning source files...");
    let plural_config = config.plural_config();
    let extraction = extractor::extract_from_glob_with_options(
        &config.input,
        &config.ignore,
        &config.functions,
        config.extract_from_comments,
        &plural_config,
        &config.trans_components,
        &config.trans_keep_basic_html_nodes_for,
        &config.use_translation_names,
        &config.nesting_prefix,
        &config.nesting_suffix,
        &config.nesting_options_separator,
        &config.interpolation_prefix,
        &config.interpolation_suffix,
    )?;

    let mut source_keys: HashSet<String> = HashSet::new();
    let mut all_keys: Vec<ExtractedKey> = Vec::new();
    let namespace_less_mode = config.namespace_less_mode();

    for (_file_path, keys) in &extraction.files {
        for key in keys {
            let namespace = key
                .namespace
                .as_deref()
                .unwrap_or(config.effective_default_namespace());
            if namespace_filter.is_none_or(|filter| filter == namespace) {
                let full_key = if namespace_less_mode {
                    key.key.clone()
                } else {
                    format!("{}:{}", namespace, key.key)
                };
                source_keys.insert(full_key);
            }
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

                if let Some(filter) = namespace_filter {
                    if namespace != filter {
                        continue;
                    }
                }

                let content = std::fs::read_to_string(&path)?;
                if content.trim().is_empty() {
                    continue;
                }

                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    count_json_keys(&json, namespace, "", namespace_less_mode, &mut locale_keys);
                }
            }
        }
    }

    println!("  Keys in locale: {}", locale_keys.len());

    // Find dead keys
    let dead_keys = cleanup::find_dead_keys(
        locales_path,
        &all_keys,
        config.effective_default_namespace(),
        namespace_less_mode,
        config.preserve_context_variants,
        &config.context_separator,
        check_locale,
    )?;
    let dead_keys: Vec<_> = dead_keys
        .into_iter()
        .filter(|dk| namespace_filter.is_none_or(|ns| dk.namespace == ns))
        .collect();

    // Find missing keys (in source but not in locale)
    let missing_count = source_keys
        .iter()
        .filter(|k| !locale_keys.contains(*k))
        .count();

    let total_keys = source_keys.len();
    let completed = total_keys.saturating_sub(missing_count);
    println!("  Progress: {}", format_progress_bar(completed, total_keys));

    // Summary
    println!("\n{}", "=".repeat(40));
    println!("Summary:");
    println!("{}", "=".repeat(40));

    let is_incomplete = missing_count > 0 || !dead_keys.is_empty();

    if !is_incomplete {
        println!("  \x1b[32m✓\x1b[0m All keys are synchronized!");
    } else {
        if missing_count > 0 {
            println!(
                "  \x1b[33m!\x1b[0m Missing keys (in source, not in locale): {}",
                missing_count
            );
        }
        if !dead_keys.is_empty() {
            println!(
                "  \x1b[33m!\x1b[0m Dead keys (in locale, not in source): {}",
                dead_keys.len()
            );
        }
        println!();
        println!("Run 'i18next-turbo extract' to add missing keys.");
        if !dead_keys.is_empty() {
            println!("Run 'i18next-turbo check --remove' to remove dead keys.");
        }
    }

    // Fail if incomplete and --fail-on-incomplete is set
    if fail_on_incomplete && is_incomplete {
        bail!(
            "Translations are incomplete: {} missing, {} dead (--fail-on-incomplete enabled)",
            missing_count,
            dead_keys.len()
        );
    }

    Ok(())
}

/// Count all leaf keys in a JSON structure
fn count_json_keys(
    value: &Value,
    namespace: &str,
    prefix: &str,
    namespace_less_mode: bool,
    keys: &mut HashSet<String>,
) {
    match value {
        Value::Object(obj) => {
            for (k, v) in obj {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                count_json_keys(v, namespace, &path, namespace_less_mode, keys);
            }
        }
        Value::String(_) => {
            if namespace_less_mode {
                keys.insert(prefix.to_string());
            } else {
                keys.insert(format!("{}:{}", namespace, prefix));
            }
        }
        _ => {}
    }
}

fn format_progress_bar(completed: usize, total: usize) -> String {
    const BAR_WIDTH: usize = 30;

    if total == 0 {
        return format!("[{}] 0.0% (0/0)", "-".repeat(BAR_WIDTH));
    }

    let ratio = completed as f64 / total as f64;
    let filled = ((ratio * BAR_WIDTH as f64).round() as usize).min(BAR_WIDTH);
    let bar = format!("[{}{}]", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled));
    format!("{} {:>5.1}% ({}/{})", bar, ratio * 100.0, completed, total)
}
