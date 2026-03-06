use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::HashSet;

use crate::config::Config;
use crate::extractor::{self, ExtractedKey};
use crate::json_sync::{self, KeyConflict};
use crate::typegen;

#[derive(Debug, Serialize)]
pub struct ExtractCommandResult {
    pub success: bool,
    pub files_processed: usize,
    pub unique_keys: usize,
    pub keys_added: usize,
    pub keys_removed: usize,
    pub updated_files: Vec<String>,
    pub warnings: usize,
    pub extraction_warnings: usize,
    pub conflicts: usize,
    pub message: Option<String>,
}

#[derive(Debug)]
struct ExtractExecution {
    result: ExtractCommandResult,
    file_keys: Vec<(String, Vec<String>)>,
    conflict_details: Vec<(String, KeyConflict)>,
    extraction_errors: Vec<(String, String)>,
    output_dir: String,
    dry_run: bool,
    generate_types: bool,
    types_output: String,
    had_changes: bool,
    extraction_error_count: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    config: &Config,
    output: Option<String>,
    fail_on_warnings: bool,
    generate_types: bool,
    types_output: &str,
    dry_run: bool,
    ci: bool,
    sync_primary: bool,
    sync_all: bool,
    verbose: bool,
) -> Result<()> {
    let execution = execute_internal(
        config,
        output,
        fail_on_warnings,
        generate_types,
        types_output,
        dry_run,
        ci,
        sync_primary,
        sync_all,
    )?;
    print_execution(config, &execution, verbose);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn execute(
    config: &Config,
    output: Option<String>,
    fail_on_warnings: bool,
    generate_types: bool,
    types_output: &str,
    dry_run: bool,
    ci: bool,
    sync_primary: bool,
    sync_all: bool,
) -> Result<ExtractCommandResult> {
    Ok(execute_internal(
        config,
        output,
        fail_on_warnings,
        generate_types,
        types_output,
        dry_run,
        ci,
        sync_primary,
        sync_all,
    )?
    .result)
}

#[allow(clippy::too_many_arguments)]
fn execute_internal(
    config: &Config,
    output: Option<String>,
    fail_on_warnings: bool,
    generate_types: bool,
    types_output: &str,
    dry_run: bool,
    ci: bool,
    sync_primary: bool,
    sync_all: bool,
) -> Result<ExtractExecution> {
    if sync_primary && sync_all {
        bail!("--sync-primary and --sync-all cannot be used together");
    }

    let output_dir = output.as_ref().unwrap_or(&config.output);
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
    let first_extraction_error = extraction.errors.first().map(|error| error.message.clone());

    let extraction_error_count = extraction.errors.len();
    if extraction.files.is_empty() {
        let warnings = extraction.warning_count;
        if extraction_error_count > 0 {
            bail!(
                "{} extraction error(s) encountered: {}",
                extraction_error_count,
                first_extraction_error.unwrap_or_else(|| "unknown extraction error".to_string())
            );
        }
        if fail_on_warnings && warnings > 0 {
            bail!(
                "{} warning(s) encountered (--fail-on-warnings enabled)",
                warnings
            );
        }
        return Ok(ExtractExecution {
            result: ExtractCommandResult {
                success: true,
                files_processed: 0,
                unique_keys: 0,
                keys_added: 0,
                keys_removed: 0,
                updated_files: Vec::new(),
                warnings,
                extraction_warnings: warnings,
                conflicts: 0,
                message: Some("No translation keys found.".to_string()),
            },
            file_keys: Vec::new(),
            conflict_details: Vec::new(),
            extraction_errors: extraction
                .errors
                .iter()
                .map(|error| (error.file_path.clone(), error.message.clone()))
                .collect(),
            output_dir: output_dir.to_string(),
            dry_run,
            generate_types,
            types_output: types_output.to_string(),
            had_changes: false,
            extraction_error_count,
        });
    }

    let mut unique_keys: HashSet<String> = HashSet::new();
    let mut all_keys: Vec<ExtractedKey> = Vec::new();
    let mut file_keys = Vec::new();

    for (file_path, keys) in &extraction.files {
        let mut display_keys = Vec::new();
        for key in keys {
            let full_key = match &key.namespace {
                Some(ns) => format!("{}:{}", ns, key.key),
                None => key.key.clone(),
            };
            display_keys.push(full_key.clone());
            unique_keys.insert(full_key);
            all_keys.push(key.clone());
        }
        file_keys.push((file_path.clone(), display_keys));
    }

    let sync_results = if sync_primary {
        let locales = vec![config.primary_language().to_string()];
        json_sync::sync_locales(config, &all_keys, output_dir, &locales, dry_run)?
    } else {
        json_sync::sync_all_locales(config, &all_keys, output_dir, dry_run)?
    };

    let mut total_added = 0usize;
    let mut total_removed = 0usize;
    let mut conflict_details = Vec::new();
    let mut updated_files = Vec::new();

    for result in &sync_results {
        if !result.added_keys.is_empty() || !result.removed_keys.is_empty() {
            updated_files.push(result.file_path.clone());
        }
        total_added += result.added_keys.len();
        total_removed += result.removed_keys.len();
        for conflict in &result.conflicts {
            conflict_details.push((result.file_path.clone(), conflict.clone()));
        }
    }

    if generate_types && !dry_run {
        let locales_dir_override = config
            .types_locales_dir()
            .unwrap_or_else(|| output_dir.to_string());
        let locales_dir_path = std::path::Path::new(&locales_dir_override);
        let types_path = std::path::Path::new(types_output);
        let default_locale_owned = config
            .types_default_locale()
            .or_else(|| config.locales.first().cloned())
            .unwrap_or_else(|| "en".to_string());
        let indentation = config.types_indentation_string();
        let input_patterns = config.types_input_patterns();
        let resources_file = config.types_resources_file();
        let enable_selector = config.types_enable_selector();
        typegen::generate_types_with_options(
            locales_dir_path,
            types_path,
            &default_locale_owned,
            indentation.as_deref(),
            input_patterns.as_deref(),
            resources_file.as_deref().map(std::path::Path::new),
            enable_selector.as_ref(),
            config.merge_namespaces,
        )?;
    }

    let total_warnings = extraction.warning_count + conflict_details.len();
    if fail_on_warnings && total_warnings > 0 {
        bail!(
            "{} warning(s) encountered (--fail-on-warnings enabled): {} extraction warnings, {} key conflicts",
            total_warnings,
            extraction.warning_count,
            conflict_details.len()
        );
    }

    let had_changes = total_added > 0 || total_removed > 0;
    if ci && had_changes {
        bail!(
            "Locale files {} out of sync (--ci enabled): {} keys added, {} keys removed",
            if dry_run { "are" } else { "were" },
            total_added,
            total_removed
        );
    }
    if extraction_error_count > 0 {
        bail!(
            "{} extraction error(s) encountered: {}",
            extraction_error_count,
            first_extraction_error.unwrap_or_else(|| "unknown extraction error".to_string())
        );
    }

    Ok(ExtractExecution {
        result: ExtractCommandResult {
            success: true,
            files_processed: extraction.files.len(),
            unique_keys: unique_keys.len(),
            keys_added: total_added,
            keys_removed: total_removed,
            updated_files,
            warnings: total_warnings,
            extraction_warnings: extraction.warning_count,
            conflicts: conflict_details.len(),
            message: None,
        },
        file_keys,
        conflict_details,
        extraction_errors: extraction
            .errors
            .iter()
            .map(|error| (error.file_path.clone(), error.message.clone()))
            .collect(),
        output_dir: output_dir.to_string(),
        dry_run,
        generate_types,
        types_output: types_output.to_string(),
        had_changes,
        extraction_error_count,
    })
}

fn print_execution(config: &Config, execution: &ExtractExecution, verbose: bool) {
    if execution.dry_run {
        println!("=== i18next-turbo extract (dry-run) ===\n");
    } else {
        println!("=== i18next-turbo extract ===\n");
    }

    println!("Configuration:");
    println!("  Input patterns: {:?}", config.input);
    println!("  Output: {}", execution.output_dir);
    println!("  Locales: {:?}", config.locales);
    println!("  Functions: {:?}", config.functions);
    if verbose {
        println!("  Key separator: {:?}", config.key_separator);
        println!("  NS separator: {:?}", config.ns_separator);
        println!("  Plural separator: {:?}", config.plural_separator);
        println!("  Context separator: {:?}", config.context_separator);
        println!("  Remove unused keys: {}", config.remove_unused_keys);
        println!("  Disable plurals: {}", config.disable_plurals);
        if !config.ignore.is_empty() {
            println!("  Ignore patterns: {:?}", config.ignore);
        }
        if !config.preserve_patterns.is_empty() {
            println!("  Preserve patterns: {:?}", config.preserve_patterns);
        }
    }
    println!();

    if let Some(message) = &execution.result.message {
        println!("{}", message);
    } else {
        println!("Extracted keys by file:");
        println!("{}", "-".repeat(60));
        for (file_path, keys) in &execution.file_keys {
            println!("\n{}", file_path);
            for key in keys {
                println!("  - {}", key);
            }
        }
        println!("\n{}", "-".repeat(60));
        println!("\nExtraction Summary:");
        println!("  Files processed: {}", execution.result.files_processed);
        println!("  Unique keys found: {}", execution.result.unique_keys);
        if execution.result.extraction_warnings > 0 {
            println!("  Warnings: {}", execution.result.extraction_warnings);
        }

        if execution.dry_run {
            println!("\nPreviewing changes (dry-run mode)...");
        } else {
            println!("\nSyncing to locale files...");
        }

        if execution.result.keys_added == 0 {
            println!(
                "  No new keys {} added (all keys already exist).",
                if execution.dry_run { "would be" } else { "" }
            );
        } else {
            for file_path in &execution.result.updated_files {
                println!("  {}", file_path);
            }
        }

        if execution.result.keys_removed > 0 {
            println!(
                "  {} stale keys: {}",
                if execution.dry_run {
                    "Would remove"
                } else {
                    "Removed"
                },
                execution.result.keys_removed
            );
        }
    }

    if !execution.extraction_errors.is_empty() {
        eprintln!("\nExtraction errors:");
        for (file_path, message) in &execution.extraction_errors {
            eprintln!("  {}: {}", file_path, message);
        }
        eprintln!();
    }

    if !execution.conflict_details.is_empty() {
        eprintln!();
        eprintln!(
            "\x1b[33m⚠ Warning: {} key(s) were skipped due to conflicts:\x1b[0m",
            execution.conflict_details.len()
        );
        for (file_path, conflict) in &execution.conflict_details {
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

    if execution.generate_types && !execution.dry_run {
        println!("\nGenerating TypeScript types...");
        println!("  Generated: {}", execution.types_output);
    } else if execution.generate_types && execution.dry_run {
        println!("\n(Skipping type generation in dry-run mode)");
    }

    if execution.dry_run {
        println!("\nDry-run complete. No files were modified.");
    } else if execution.had_changes || execution.extraction_error_count == 0 {
        println!("\nDone!");
    }
}
