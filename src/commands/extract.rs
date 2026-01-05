use anyhow::{bail, Result};
use std::collections::HashSet;

use crate::config::Config;
use crate::extractor::{self, ExtractedKey};
use crate::json_sync::{self, KeyConflict};
use crate::typegen;

pub fn run(
    config: &Config,
    output: Option<String>,
    fail_on_warnings: bool,
    generate_types: bool,
    types_output: &str,
) -> Result<()> {
    println!("=== i18next-turbo extract ===\n");

    // Determine output directory
    let output_dir = output.as_ref().unwrap_or(&config.output);

    println!("Configuration:");
    println!("  Input patterns: {:?}", config.input);
    println!("  Output: {}", output_dir);
    println!("  Locales: {:?}", config.locales);
    println!("  Functions: {:?}", config.functions);
    println!();

    let plural_config = config.plural_config();

    // Extract keys from files
    let extraction = extractor::extract_from_glob_with_options(
        &config.input,
        &config.ignore,
        &config.functions,
        config.extract_from_comments,
        &plural_config,
    )?;

    // Report any errors encountered during extraction
    if !extraction.errors.is_empty() {
        eprintln!("\nExtraction errors:");
        for error in &extraction.errors {
            eprintln!("  {}: {}", error.file_path, error.message);
        }
        eprintln!();
    }

    if extraction.files.is_empty() {
        println!("No translation keys found.");
        if fail_on_warnings && extraction.warning_count > 0 {
            bail!(
                "{} warning(s) encountered (--fail-on-warnings enabled)",
                extraction.warning_count
            );
        }
        return Ok(());
    }

    // Collect all keys (with deduplication for display)
    let mut unique_keys: HashSet<String> = HashSet::new();
    let mut all_keys: Vec<ExtractedKey> = Vec::new();

    println!("Extracted keys by file:");
    println!("{}", "-".repeat(60));

    for (file_path, keys) in &extraction.files {
        println!("\n{}", file_path);
        for key in keys {
            let full_key = match &key.namespace {
                Some(ns) => format!("{}:{}", ns, key.key),
                None => key.key.clone(),
            };
            println!("  - {}", full_key);
            unique_keys.insert(full_key);
            all_keys.push(key.clone());
        }
    }

    println!("\n{}", "-".repeat(60));
    println!("\nExtraction Summary:");
    println!("  Files processed: {}", extraction.files.len());
    println!("  Unique keys found: {}", unique_keys.len());
    if extraction.warning_count > 0 {
        println!("  Warnings: {}", extraction.warning_count);
    }

    // Sync to JSON files
    println!("\nSyncing to locale files...");
    let sync_results = json_sync::sync_all_locales(config, &all_keys, output_dir)?;

    // Report sync results
    let mut total_added = 0;
    let mut total_removed = 0;
    let mut total_conflicts = 0;
    let mut all_conflicts: Vec<(String, KeyConflict)> = Vec::new();

    for result in &sync_results {
        if !result.added_keys.is_empty() {
            println!(
                "  {} - added {} new key(s)",
                result.file_path,
                result.added_keys.len()
            );
            total_added += result.added_keys.len();
        }

        if !result.removed_keys.is_empty() {
            println!(
                "  {} - removed {} stale key(s)",
                result.file_path,
                result.removed_keys.len()
            );
            total_removed += result.removed_keys.len();
        }

        // Collect conflicts for reporting
        if !result.conflicts.is_empty() {
            total_conflicts += result.conflicts.len();
            for conflict in &result.conflicts {
                all_conflicts.push((result.file_path.clone(), conflict.clone()));
            }
        }
    }

    if total_added == 0 {
        println!("  No new keys added (all keys already exist).");
    }
    if total_removed > 0 {
        println!("  Removed stale keys: {}", total_removed);
    }

    // Report conflicts with user-friendly messages
    if !all_conflicts.is_empty() {
        eprintln!();
        eprintln!(
            "\x1b[33m⚠ Warning: {} key(s) were skipped due to conflicts:\x1b[0m",
            total_conflicts
        );
        for (file_path, conflict) in &all_conflicts {
            match conflict {
                KeyConflict::ValueIsNotObject {
                    key_path,
                    existing_value,
                } => {
                    eprintln!("  \x1b[31m✗\x1b[0m {} in {}", key_path, file_path);
                    eprintln!(
                        "    Cannot create nested key: '{}' already exists as scalar value: {}",
                        key_path.split('.').next().unwrap_or(key_path),
                        existing_value
                    );
                }
                KeyConflict::ObjectIsValue { key_path } => {
                    eprintln!("  \x1b[31m✗\x1b[0m {} in {}", key_path, file_path);
                    eprintln!(
                        "    Cannot set scalar value: '{}' already exists as an object with nested keys",
                        key_path
                    );
                }
            }
        }
        eprintln!();
        eprintln!(
            "  \x1b[90mTo fix: manually update the conflicting keys in your locale files,\x1b[0m"
        );
        eprintln!("  \x1b[90mor rename the keys in your source code to avoid collision.\x1b[0m");
    }

    // Generate TypeScript types if requested
    if generate_types {
        println!("\nGenerating TypeScript types...");
        let locales_dir_override = config
            .types_locales_dir()
            .unwrap_or_else(|| output_dir.to_string());
        let locales_dir_path = std::path::Path::new(&locales_dir_override);
        let types_path = std::path::Path::new(types_output);
        let default_locale_owned = config
            .types_default_locale()
            .or_else(|| config.locales.first().cloned())
            .unwrap_or_else(|| "en".to_string());
        typegen::generate_types(locales_dir_path, types_path, &default_locale_owned)?;
        println!("  Generated: {}", types_output);
    }

    println!("\nDone!");

    // Check fail-on-warnings (includes extraction warnings and key conflicts)
    let total_warnings = extraction.warning_count + total_conflicts;
    if fail_on_warnings && total_warnings > 0 {
        bail!(
            "{} warning(s) encountered (--fail-on-warnings enabled): {} extraction warnings, {} key conflicts",
            total_warnings,
            extraction.warning_count,
            total_conflicts
        );
    }

    Ok(())
}
