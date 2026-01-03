use anyhow::Result;
use clap::{Parser, Subcommand};
use i18next_turbo::cleanup;
use i18next_turbo::config::Config;
use i18next_turbo::extractor::{self, ExtractedKey};
use i18next_turbo::json_sync;
use i18next_turbo::lint;
use i18next_turbo::typegen;
use i18next_turbo::watcher::FileWatcher;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "i18next-turbo")]
#[command(author, version, about = "High-performance i18n key extraction tool", long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract translation keys from source files
    Extract {
        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<String>,

        /// Fail on warnings
        #[arg(long)]
        fail_on_warnings: bool,

        /// Generate TypeScript type definitions after extraction
        #[arg(long)]
        generate_types: bool,

        /// TypeScript output path (only used with --generate-types)
        #[arg(long, default_value = "src/@types/i18next.d.ts")]
        types_output: String,
    },

    /// Watch for file changes and extract keys automatically
    Watch {
        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Generate TypeScript type definitions from existing locale files
    Typegen {
        /// TypeScript output path
        #[arg(short, long, default_value = "src/@types/i18next.d.ts")]
        output: String,

        /// Default locale to use for type generation
        #[arg(short, long)]
        default_locale: Option<String>,

        /// Locales directory (overrides config)
        #[arg(short, long)]
        locales_dir: Option<String>,
    },

    /// Check for dead (unused) translation keys
    Check {
        /// Remove dead keys from locale files
        #[arg(long)]
        remove: bool,

        /// Show what would be removed without actually removing
        #[arg(long)]
        dry_run: bool,

        /// Locale to check (defaults to first locale in config)
        #[arg(short, long)]
        locale: Option<String>,
    },

    /// Show translation status summary
    Status {
        /// Locale to check (defaults to first locale in config)
        #[arg(short, long)]
        locale: Option<String>,
    },

    /// Sync translation keys across locales
    Sync {
        /// Remove keys that don't exist in primary locale
        #[arg(long)]
        remove_unused: bool,

        /// Preview changes without writing files
        #[arg(long)]
        dry_run: bool,
    },

    /// Lint source files for hardcoded strings that should be translated
    Lint {
        /// Fail on lint errors (useful for CI)
        #[arg(long)]
        fail_on_error: bool,
    },

    /// Rename a translation key in source files and locale files
    RenameKey {
        /// The old key to rename
        old_key: String,

        /// The new key name
        new_key: String,

        /// Preview changes without modifying files
        #[arg(long)]
        dry_run: bool,

        /// Only rename in locale files (skip source files)
        #[arg(long)]
        locales_only: bool,
    },

    /// Initialize a new i18next-turbo configuration file
    Init {
        /// Overwrite existing config file
        #[arg(long)]
        force: bool,

        /// Input glob patterns (comma-separated)
        #[arg(short, long, default_value = "src/**/*.{ts,tsx,js,jsx}")]
        input: String,

        /// Output directory for locale files
        #[arg(short, long, default_value = "locales")]
        output: String,

        /// Locales (comma-separated)
        #[arg(short, long, default_value = "en,ja")]
        locales: String,

        /// Default namespace
        #[arg(short, long, default_value = "translation")]
        namespace: String,

        /// Functions to extract (comma-separated)
        #[arg(short, long, default_value = "t")]
        functions: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration
    let config = Config::load_or_default(cli.config.as_ref())?;

    match cli.command {
        Commands::Extract {
            output,
            fail_on_warnings,
            generate_types,
            types_output,
        } => {
            run_extract(&config, output, fail_on_warnings, generate_types, &types_output)?;
        }
        Commands::Watch { output } => {
            println!("=== i18next-turbo watch ===\n");
            let mut watcher = FileWatcher::new(config, output);
            watcher.run()?;
        }
        Commands::Typegen {
            output,
            default_locale,
            locales_dir,
        } => {
            run_typegen(&config, &output, default_locale, locales_dir)?;
        }
        Commands::Check {
            remove,
            dry_run,
            locale,
        } => {
            run_check(&config, remove, dry_run, locale)?;
        }
        Commands::Status { locale } => {
            run_status(&config, locale)?;
        }
        Commands::Sync {
            remove_unused,
            dry_run,
        } => {
            run_sync(&config, remove_unused, dry_run)?;
        }
        Commands::Lint { fail_on_error } => {
            run_lint(&config, fail_on_error)?;
        }
        Commands::RenameKey {
            old_key,
            new_key,
            dry_run,
            locales_only,
        } => {
            run_rename_key(&config, &old_key, &new_key, dry_run, locales_only)?;
        }
        Commands::Init {
            force,
            input,
            output,
            locales,
            namespace,
            functions,
        } => {
            run_init(force, &input, &output, &locales, &namespace, &functions)?;
        }
    }

    Ok(())
}

fn run_extract(
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

    if extraction.files.is_empty() {
        println!("No translation keys found.");
        if fail_on_warnings && extraction.warning_count > 0 {
            eprintln!(
                "\nFailed: {} warning(s) encountered (--fail-on-warnings enabled)",
                extraction.warning_count
            );
            std::process::exit(1);
        }
        return Ok(());
    }

    // Collect all keys (with deduplication for display)
    let mut unique_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
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
        eprintln!(
            "\nFailed: {} warning(s) encountered (--fail-on-warnings enabled)",
            extraction.warning_count
        );
        std::process::exit(1);
    }

    Ok(())
}

fn run_typegen(
    config: &Config,
    output: &str,
    default_locale: Option<String>,
    locales_dir: Option<String>,
) -> Result<()> {
    println!("=== i18next-turbo typegen ===\n");

    let locales_dir_path = locales_dir.as_ref().unwrap_or(&config.output);
    let default_locale = default_locale
        .as_ref()
        .or(config.locales.first())
        .map(|s| s.as_str())
        .unwrap_or("en");

    println!("Configuration:");
    println!("  Locales directory: {}", locales_dir_path);
    println!("  Default locale: {}", default_locale);
    println!("  Output: {}", output);
    println!();

    let locales_path = std::path::Path::new(locales_dir_path);
    let output_path = std::path::Path::new(output);

    typegen::generate_types(locales_path, output_path, default_locale)?;

    println!("TypeScript types generated successfully!");
    println!("  Output: {}", output);

    Ok(())
}

fn run_check(
    config: &Config,
    remove: bool,
    dry_run: bool,
    locale: Option<String>,
) -> Result<()> {
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
    let extraction = extractor::extract_from_glob(&config.input, &config.functions)?;

    let mut all_keys: Vec<ExtractedKey> = Vec::new();
    for (_file_path, keys) in &extraction.files {
        all_keys.extend(keys.iter().cloned());
    }

    println!("  Found {} keys in source code", all_keys.len());

    // Find dead keys
    println!("\nScanning for dead keys...");
    let locales_path = std::path::Path::new(&config.output);
    let dead_keys = cleanup::find_dead_keys(locales_path, &all_keys, &config.default_namespace, check_locale)?;

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

fn run_status(config: &Config, locale: Option<String>) -> Result<()> {
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

    let mut source_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
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
    let locales_path = std::path::Path::new(&config.output);
    let locale_dir = locales_path.join(check_locale);

    let mut locale_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

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

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    count_json_keys(&json, namespace, "", &mut locale_keys);
                }
            }
        }
    }

    println!("  Keys in locale: {}", locale_keys.len());

    // Find dead keys
    let dead_keys = cleanup::find_dead_keys(locales_path, &all_keys, &config.default_namespace, check_locale)?;

    // Find missing keys (in source but not in locale)
    let missing_count = source_keys.iter().filter(|k| !locale_keys.contains(*k)).count();

    // Summary
    println!("\n{}", "=".repeat(40));
    println!("Summary:");
    println!("{}", "=".repeat(40));

    if dead_keys.is_empty() && missing_count == 0 {
        println!("  ✓ All keys are synchronized!");
    } else {
        if missing_count > 0 {
            println!("  ⚠ Missing keys (in source, not in locale): {}", missing_count);
        }
        if !dead_keys.is_empty() {
            println!("  ⚠ Dead keys (in locale, not in source): {}", dead_keys.len());
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
fn count_json_keys(
    value: &serde_json::Value,
    namespace: &str,
    prefix: &str,
    keys: &mut std::collections::HashSet<String>,
) {
    match value {
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                count_json_keys(v, namespace, &path, keys);
            }
        }
        serde_json::Value::String(_) => {
            keys.insert(format!("{}:{}", namespace, prefix));
        }
        _ => {}
    }
}

fn run_sync(config: &Config, remove_unused: bool, dry_run: bool) -> Result<()> {
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

    let locales_path = std::path::Path::new(&config.output);

    // Read all namespaces from primary locale
    let primary_dir = locales_path.join(primary_locale);
    if !primary_dir.exists() {
        println!("Primary locale directory does not exist: {}", primary_dir.display());
        return Ok(());
    }

    let mut total_added = 0;
    let mut total_removed = 0;

    // Process each namespace file in primary locale
    for entry in std::fs::read_dir(&primary_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let namespace = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("translation");

            let primary_content = std::fs::read_to_string(&path)?;
            if primary_content.trim().is_empty() {
                continue;
            }

            let primary_json: serde_json::Value = serde_json::from_str(&primary_content)?;

            // Sync to each secondary locale
            for secondary_locale in &secondary_locales {
                let secondary_path = locales_path
                    .join(secondary_locale)
                    .join(format!("{}.json", namespace));

                let mut secondary_json = if secondary_path.exists() {
                    let content = std::fs::read_to_string(&secondary_path)?;
                    if content.trim().is_empty() {
                        serde_json::Value::Object(serde_json::Map::new())
                    } else {
                        serde_json::from_str(&content)?
                    }
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                };

                // Sync keys
                let (added, removed) = sync_json_keys(
                    &primary_json,
                    &mut secondary_json,
                    remove_unused,
                );

                if added > 0 || removed > 0 {
                    println!(
                        "  {}/{}.json: +{} added, -{} removed",
                        secondary_locale, namespace, added, removed
                    );

                    if !dry_run {
                        // Ensure directory exists
                        if let Some(parent) = secondary_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        // Sort and write
                        if let Some(obj) = secondary_json.as_object() {
                            let sorted = json_sync::sort_keys_alphabetically(obj);
                            let output = serde_json::to_string_pretty(&sorted)?;
                            std::fs::write(&secondary_path, format!("{}\n", output))?;
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
fn sync_json_keys(
    primary: &serde_json::Value,
    secondary: &mut serde_json::Value,
    remove_unused: bool,
) -> (usize, usize) {
    let mut added = 0;
    let mut removed = 0;

    if let (serde_json::Value::Object(primary_obj), serde_json::Value::Object(secondary_obj)) =
        (primary, secondary)
    {
        // Add missing keys from primary
        for (key, primary_value) in primary_obj {
            if !secondary_obj.contains_key(key) {
                // Add key with empty string or nested object
                let new_value = create_empty_structure(primary_value);
                secondary_obj.insert(key.clone(), new_value);
                added += count_leaf_keys(primary_value);
            } else if let serde_json::Value::Object(_) = primary_value {
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
fn create_empty_structure(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(obj) => {
            let mut new_obj = serde_json::Map::new();
            for (k, v) in obj {
                new_obj.insert(k.clone(), create_empty_structure(v));
            }
            serde_json::Value::Object(new_obj)
        }
        _ => serde_json::Value::String(String::new()),
    }
}

/// Count the number of leaf keys in a JSON structure
fn count_leaf_keys(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(obj) => obj.values().map(count_leaf_keys).sum(),
        serde_json::Value::String(_) => 1,
        _ => 0,
    }
}

fn run_lint(config: &Config, fail_on_error: bool) -> Result<()> {
    println!("=== i18next-turbo lint ===\n");

    println!("Configuration:");
    println!("  Input patterns: {:?}", config.input);
    println!();

    println!("Scanning for hardcoded strings...");
    let result = lint::lint_from_glob(&config.input)?;

    println!("  Files checked: {}", result.files_checked);
    println!("  Issues found: {}", result.issues.len());
    println!();

    if result.issues.is_empty() {
        println!("No hardcoded strings found. All text appears to be translated!");
        return Ok(());
    }

    println!("{}", "=".repeat(60));
    println!("Issues:");
    println!("{}", "=".repeat(60));

    for issue in &result.issues {
        println!(
            "\n{}:{}:{}",
            issue.file_path, issue.line, issue.column
        );
        println!("  {}", issue.message);
        println!("  Text: \"{}\"", issue.text);
    }

    println!("\n{}", "=".repeat(60));
    println!("Total: {} issue(s)", result.issues.len());

    if fail_on_error {
        std::process::exit(1);
    }

    Ok(())
}

fn run_rename_key(
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
                                .replace(&format!("'{}'", search_key), &format!("'{}'", replace_key))
                                .replace(&format!("\"{}\"", search_key), &format!("\"{}\"", replace_key))
                                .replace(&format!("`{}`", search_key), &format!("`{}`", replace_key));

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

        let mut json: serde_json::Value = serde_json::from_str(&content)?;

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
                        serde_json::Value::Object(serde_json::Map::new())
                    } else {
                        serde_json::from_str(&new_content)?
                    }
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
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
fn get_nested_value(json: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
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
fn remove_nested_key(json: &mut serde_json::Value, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.len() == 1 {
        if let serde_json::Value::Object(obj) = json {
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
    if let serde_json::Value::Object(obj) = current {
        if let Some(last) = parts.last() {
            obj.remove(*last);
        }
    }
}

/// Set a nested value in JSON using dot notation
fn set_nested_value(json: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last part - set the value
            if let serde_json::Value::Object(obj) = current {
                obj.insert((*part).to_string(), value);
            }
            return;
        }

        // Navigate or create intermediate objects
        if let serde_json::Value::Object(obj) = current {
            if !obj.contains_key(*part) {
                obj.insert((*part).to_string(), serde_json::Value::Object(serde_json::Map::new()));
            }
            if let Some(val) = obj.get_mut(*part) {
                current = val;
            } else {
                // This should not happen since we just inserted the key above
                // But we handle it gracefully to avoid panics
                return;
            }
        }
    }
}

fn run_init(
    force: bool,
    input: &str,
    output: &str,
    locales: &str,
    namespace: &str,
    functions: &str,
) -> Result<()> {
    println!("=== i18next-turbo init ===\n");

    let config_path = std::path::Path::new("i18next-turbo.json");

    // Check if config already exists
    if config_path.exists() && !force {
        eprintln!("Configuration file already exists: {}", config_path.display());
        eprintln!("Use --force to overwrite.");
        std::process::exit(1);
    }

    // Parse comma-separated values
    let input_patterns: Vec<String> = input.split(',').map(|s| s.trim().to_string()).collect();
    let locales_vec: Vec<String> = locales.split(',').map(|s| s.trim().to_string()).collect();
    let functions_vec: Vec<String> = functions.split(',').map(|s| s.trim().to_string()).collect();

    // Create config JSON
    let config = serde_json::json!({
        "input": input_patterns,
        "output": output,
        "locales": locales_vec,
        "defaultNamespace": namespace,
        "functions": functions_vec,
        "keySeparator": ".",
        "nsSeparator": ":"
    });

    // Write config file
    let config_str = serde_json::to_string_pretty(&config)?;
    std::fs::write(config_path, format!("{}\n", config_str))?;

    println!("Created configuration file: {}\n", config_path.display());
    println!("Configuration:");
    println!("  Input patterns: {:?}", input_patterns);
    println!("  Output: {}", output);
    println!("  Locales: {:?}", locales_vec);
    println!("  Default namespace: {}", namespace);
    println!("  Functions: {:?}", functions_vec);

    println!("\nNext steps:");
    println!("  1. Run 'i18next-turbo extract' to extract translation keys");
    println!("  2. Run 'i18next-turbo watch' for continuous extraction");
    println!("  3. Run 'i18next-turbo typegen' to generate TypeScript types");

    // Create output directories for each locale
    println!("\nCreating locale directories...");
    for locale in &locales_vec {
        let locale_dir = std::path::Path::new(output).join(locale);
        if !locale_dir.exists() {
            std::fs::create_dir_all(&locale_dir)?;
            println!("  Created: {}", locale_dir.display());
        }
    }

    println!("\nDone!");
    Ok(())
}
