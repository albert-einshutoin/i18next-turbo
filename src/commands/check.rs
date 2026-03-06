use anyhow::Result;
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;

use crate::cleanup;
use crate::config::Config;
use crate::extractor::{self, ExtractedKey};

#[derive(Debug, Serialize)]
pub struct DeadKeyReport {
    pub file_path: String,
    pub key_path: String,
    pub namespace: String,
}

#[derive(Debug, Serialize)]
pub struct CheckCommandResult {
    pub dead_keys: Vec<DeadKeyReport>,
    pub removed_count: usize,
}

pub fn run(config: &Config, remove: bool, dry_run: bool, locale: Option<String>) -> Result<()> {
    println!("=== i18next-turbo check ===\n");

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

    println!("Extracting keys from source files...");
    let result = if remove && !dry_run {
        let preview = execute(config, false, true, locale.clone())?;
        if !preview.dead_keys.is_empty() && !confirm_removal(preview.dead_keys.len()) {
            println!("\nRemoval cancelled.");
            return Ok(());
        }
        execute(config, remove, dry_run, locale)?
    } else {
        execute(config, remove, dry_run, locale)?
    };
    print_result(&result, remove, dry_run);
    Ok(())
}

pub fn execute(
    config: &Config,
    remove: bool,
    dry_run: bool,
    locale: Option<String>,
) -> Result<CheckCommandResult> {
    let check_locale = locale
        .as_ref()
        .or(config.locales.first())
        .map(|s| s.as_str())
        .unwrap_or("en");

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
        config.output_format(),
    )?;

    let mut removed_count = 0usize;
    if remove && !dry_run && !dead_keys.is_empty() {
        removed_count = cleanup::purge_dead_keys(locales_path, &dead_keys, config.output_format())?;
    }

    Ok(CheckCommandResult {
        dead_keys: dead_keys
            .into_iter()
            .map(|dk| DeadKeyReport {
                file_path: dk.file_path,
                key_path: dk.key_path,
                namespace: dk.namespace,
            })
            .collect(),
        removed_count,
    })
}

fn print_result(result: &CheckCommandResult, remove: bool, dry_run: bool) {
    if result.dead_keys.is_empty() {
        println!("\nNo dead keys found. All translation keys are in use!");
        return;
    }

    println!("\nFound {} dead key(s):", result.dead_keys.len());
    println!("{}", "-".repeat(60));

    for dk in &result.dead_keys {
        println!("  [{}] {} -> {}", dk.namespace, dk.key_path, dk.file_path);
    }

    println!("{}", "-".repeat(60));

    if remove && !dry_run {
        println!("\nRemoved {} key(s)", result.removed_count);
    } else if dry_run {
        println!("\n[Dry run] Would remove {} key(s)", result.dead_keys.len());
        println!("Run with --remove (without --dry-run) to actually remove them.");
    } else {
        println!("\nRun with --remove to delete these keys from locale files.");
        println!("Use --dry-run to preview what would be removed.");
    }
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
