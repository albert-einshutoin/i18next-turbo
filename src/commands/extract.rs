use anyhow::{bail, Result};
use std::collections::HashSet;

use crate::config::Config;
use crate::extractor::{self, ExtractedKey};
use crate::json_sync;
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

    // Extract keys from files
    let extraction = extractor::extract_from_glob(&config.input, &config.functions)?;

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
    for result in &sync_results {
        if !result.added_keys.is_empty() {
            println!(
                "  {} - added {} new key(s)",
                result.file_path,
                result.added_keys.len()
            );
            total_added += result.added_keys.len();
        }
    }

    if total_added == 0 {
        println!("  No new keys added (all keys already exist).");
    }

    // Generate TypeScript types if requested
    if generate_types {
        println!("\nGenerating TypeScript types...");
        let locales_dir = std::path::Path::new(output_dir);
        let types_path = std::path::Path::new(types_output);
        let default_locale = config.locales.first().map(|s| s.as_str()).unwrap_or("en");
        typegen::generate_types(locales_dir, types_path, default_locale)?;
        println!("  Generated: {}", types_output);
    }

    println!("\nDone!");

    // Check fail-on-warnings
    if fail_on_warnings && extraction.warning_count > 0 {
        bail!(
            "{} warning(s) encountered (--fail-on-warnings enabled)",
            extraction.warning_count
        );
    }

    Ok(())
}
