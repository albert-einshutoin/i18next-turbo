use anyhow::Result;
use std::io::{self, Write};
use std::path::Path;

use crate::cleanup;
use crate::config::Config;
use crate::extractor::{self, ExtractedKey};

pub fn run(config: &Config, remove: bool, dry_run: bool, locale: Option<String>) -> Result<()> {
    println!("=== i18next-turbo check ===\n");

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

    // First, extract keys from source
    println!("Extracting keys from source files...");
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

    let mut all_keys: Vec<ExtractedKey> = Vec::new();
    for (_file_path, keys) in &extraction.files {
        all_keys.extend(keys.iter().cloned());
    }

    println!("  Found {} keys in source code", all_keys.len());

    // Find dead keys
    println!("\nScanning for dead keys...");
    let locales_path = Path::new(&config.output);
    let dead_keys = cleanup::find_dead_keys(
        locales_path,
        &all_keys,
        config.effective_default_namespace(),
        config.namespace_less_mode(),
        config.merge_namespaces,
        config.preserve_context_variants,
        &config.context_separator,
        check_locale,
    )?;

    if dead_keys.is_empty() {
        println!("\nNo dead keys found. All translation keys are in use!");
        return Ok(());
    }

    println!("\nFound {} dead key(s):", dead_keys.len());
    println!("{}", "-".repeat(60));

    for dk in &dead_keys {
        println!("  [{}] {} -> {}", dk.namespace, dk.key_path, dk.file_path);
    }

    println!("{}", "-".repeat(60));

    // Handle removal
    if remove && !dry_run {
        if !confirm_removal(dead_keys.len()) {
            println!("\nRemoval cancelled.");
            return Ok(());
        }
        println!("\nRemoving dead keys...");
        let removed = cleanup::purge_dead_keys(locales_path, &dead_keys)?;
        println!("  Removed {} key(s)", removed);
    } else if dry_run {
        println!("\n[Dry run] Would remove {} key(s)", dead_keys.len());
        println!("Run with --remove (without --dry-run) to actually remove them.");
    } else {
        println!("\nRun with --remove to delete these keys from locale files.");
        println!("Use --dry-run to preview what would be removed.");
    }

    Ok(())
}

fn confirm_removal(count: usize) -> bool {
    println!(
        "\nThis will permanently remove {} key(s) from your locale files.",
        count
    );
    print!("Proceed? [y/N]: ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}
