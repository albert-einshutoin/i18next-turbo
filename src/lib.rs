pub mod cleanup;
pub mod commands;
pub mod config;
pub mod extractor;
pub mod fs;
pub mod json_sync;
pub mod lint;
pub mod typegen;
pub mod watcher;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi_derive::napi;

#[cfg(feature = "napi")]
use crate::cleanup as cleanup_mod;
#[cfg(feature = "napi")]
use crate::config::{Config, NapiConfig};
#[cfg(feature = "napi")]
use crate::extractor::ExtractedKey;
#[cfg(feature = "napi")]
use crate::lint as lint_mod;

// ============================================
// NAPI Result Types (zero-copy JS interop)
// ============================================

/// Result of extraction operation
#[cfg(feature = "napi")]
#[napi(object)]
pub struct ExtractResult {
    /// Whether the extraction was successful
    pub success: bool,
    /// Number of files processed
    pub files_processed: u32,
    /// Number of unique keys found
    pub unique_keys: u32,
    /// Number of new keys added to locale files
    pub keys_added: u32,
    /// List of updated locale files
    pub updated_files: Vec<String>,
    /// Number of warnings encountered
    pub warnings: u32,
    /// Optional message (e.g., "No translation keys found.")
    pub message: Option<String>,
}

/// Result of lint operation
#[cfg(feature = "napi")]
#[napi(object)]
pub struct LintResult {
    /// Number of files checked
    pub files_checked: u32,
    /// List of lint issues found
    pub issues: Vec<LintIssueInfo>,
}

/// Information about a lint issue
#[cfg(feature = "napi")]
#[napi(object)]
pub struct LintIssueInfo {
    /// File path where the issue was found
    pub file_path: String,
    /// Line number
    pub line: u32,
    /// Column number
    pub column: u32,
    /// Issue message
    pub message: String,
    /// The hardcoded text that should be translated
    pub text: String,
}

/// Result of check operation
#[cfg(feature = "napi")]
#[napi(object)]
pub struct CheckResult {
    /// List of dead (unused) keys
    pub dead_keys: Vec<DeadKeyInfo>,
    /// Number of keys removed (if remove option was used)
    pub removed_count: u32,
}

/// Information about a dead key
#[cfg(feature = "napi")]
#[napi(object)]
pub struct DeadKeyInfo {
    /// File path where the key is defined
    pub file_path: String,
    /// Key path (e.g., "button.submit")
    pub key_path: String,
    /// Namespace of the key
    pub namespace: String,
}

/// Extract translation keys from source files
///
/// # Arguments
/// * `config` - Configuration object
/// * `options` - Optional extraction options (output, fail_on_warnings, generate_types, types_output)
///
/// # Returns
/// Returns extraction results directly as a JavaScript object (zero-copy)
#[napi]
#[cfg(feature = "napi")]
pub fn extract(config: NapiConfig, options: Option<ExtractOptions>) -> Result<ExtractResult> {
    let config: Config = Config::from_napi(config)
        .map_err(|e| napi::Error::from_reason(format!("Config validation failed: {}", e)))?;

    // Extract options
    let output = options.as_ref().and_then(|o| o.output.as_ref());
    let fail_on_warnings = options
        .as_ref()
        .and_then(|o| o.fail_on_warnings)
        .unwrap_or(false);
    let generate_types = options
        .as_ref()
        .and_then(|o| o.generate_types)
        .unwrap_or(false);
    let types_output = options
        .as_ref()
        .and_then(|o| o.types_output.as_ref())
        .map(|s| s.to_string())
        .or_else(|| config.types.output.clone())
        .unwrap_or_else(|| Config::default_types_output());

    // Determine output directory
    let output_dir = output.unwrap_or(&config.output);

    let plural_config = config.plural_config();

    // Extract keys from files
    let extraction = crate::extractor::extract_from_glob_with_options(
        &config.input,
        &config.ignore,
        &config.functions,
        config.extract_from_comments,
        &plural_config,
        &config.trans_components,
        &config.use_translation_names,
        &config.nesting_prefix,
        &config.nesting_suffix,
        &config.nesting_options_separator,
    )
    .map_err(|e| napi::Error::from_reason(format!("Extraction failed: {}", e)))?;

    if extraction.files.is_empty() {
        if fail_on_warnings && extraction.warning_count > 0 {
            return Err(napi::Error::from_reason(format!(
                "Failed: {} warning(s) encountered (fail_on_warnings enabled)",
                extraction.warning_count
            )));
        }
        return Ok(ExtractResult {
            success: true,
            files_processed: 0,
            unique_keys: 0,
            keys_added: 0,
            updated_files: vec![],
            warnings: extraction.warning_count as u32,
            message: Some("No translation keys found.".to_string()),
        });
    }

    // Collect all keys
    let mut unique_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut all_keys: Vec<ExtractedKey> = Vec::new();

    for (_file_path, keys) in &extraction.files {
        for key in keys {
            let full_key = match &key.namespace {
                Some(ns) => format!("{}:{}", ns, key.key),
                None => key.key.clone(),
            };
            unique_keys.insert(full_key);
            all_keys.push(key.clone());
        }
    }

    // Sync to JSON files
    let sync_results = crate::json_sync::sync_all_locales(&config, &all_keys, output_dir, false)
        .map_err(|e| napi::Error::from_reason(format!("Sync failed: {}", e)))?;

    // Report sync results
    let mut total_added = 0usize;
    let mut updated_files: Vec<String> = Vec::new();
    for result in &sync_results {
        if !result.added_keys.is_empty() {
            total_added += result.added_keys.len();
            updated_files.push(result.file_path.clone());
        }
    }

    // Generate TypeScript types if requested
    if generate_types {
        let locales_dir = config.types.locales_dir.as_deref().unwrap_or(output_dir);
        let locales_dir = std::path::Path::new(locales_dir);
        let types_path = std::path::Path::new(&types_output);
        let default_locale = config
            .types
            .default_locale
            .as_deref()
            .or_else(|| config.locales.first().map(|s| s.as_str()))
            .unwrap_or("en");
        crate::typegen::generate_types(locales_dir, types_path, default_locale)
            .map_err(|e| napi::Error::from_reason(format!("Type generation failed: {}", e)))?;
    }

    // Check fail-on-warnings
    if fail_on_warnings && extraction.warning_count > 0 {
        return Err(napi::Error::from_reason(format!(
            "Failed: {} warning(s) encountered (fail_on_warnings enabled)",
            extraction.warning_count
        )));
    }

    Ok(ExtractResult {
        success: true,
        files_processed: extraction.files.len() as u32,
        unique_keys: unique_keys.len() as u32,
        keys_added: total_added as u32,
        updated_files,
        warnings: extraction.warning_count as u32,
        message: None,
    })
}

/// Watch for file changes and extract keys automatically
///
/// # Arguments
/// * `config` - Configuration object
/// * `options` - Optional watch options (output)
///
/// # Note
/// This function runs indefinitely until interrupted. In a Node.js context,
/// this should be called in a separate thread or worker.
#[napi]
#[cfg(feature = "napi")]
pub fn watch(config: NapiConfig, options: Option<WatchOptions>) -> Result<()> {
    let config: Config = Config::from_napi(config)
        .map_err(|e| napi::Error::from_reason(format!("Config validation failed: {}", e)))?;

    // Extract options
    let output = options.as_ref().and_then(|o| o.output.as_ref());

    // Create watcher
    let mut watcher = crate::watcher::FileWatcher::new(config, output.cloned());

    // Run watcher (this blocks)
    watcher
        .run()
        .map_err(|e| napi::Error::from_reason(format!("Watch failed: {}", e)))?;

    Ok(())
}

/// Extract options
#[cfg(feature = "napi")]
#[napi(object)]
pub struct ExtractOptions {
    /// Output directory (overrides config)
    pub output: Option<String>,
    /// Fail on warnings
    pub fail_on_warnings: Option<bool>,
    /// Generate TypeScript type definitions after extraction
    pub generate_types: Option<bool>,
    /// TypeScript output path (only used with generate_types)
    pub types_output: Option<String>,
}

/// Watch options
#[cfg(feature = "napi")]
#[napi(object)]
pub struct WatchOptions {
    /// Output directory (overrides config)
    pub output: Option<String>,
}

/// Lint options
#[cfg(feature = "napi")]
#[napi(object)]
pub struct LintOptions {
    /// Fail on lint errors
    pub fail_on_error: Option<bool>,
}

/// Check options
#[cfg(feature = "napi")]
#[napi(object)]
pub struct CheckOptions {
    /// Remove dead keys from locale files
    pub remove: Option<bool>,
    /// Preview changes without modifying files
    pub dry_run: Option<bool>,
    /// Locale to check (defaults to first locale in config)
    pub locale: Option<String>,
}

/// Lint source files for hardcoded strings
#[cfg(feature = "napi")]
#[napi]
pub fn lint(config: NapiConfig, options: Option<LintOptions>) -> Result<LintResult> {
    let config: Config = Config::from_napi(config)
        .map_err(|e| napi::Error::from_reason(format!("Config validation failed: {}", e)))?;
    let fail_on_error = options
        .as_ref()
        .and_then(|o| o.fail_on_error)
        .unwrap_or(false);

    let result = lint_mod::lint_from_glob(&config.input)
        .map_err(|e| napi::Error::from_reason(format!("Lint failed: {}", e)))?;

    if fail_on_error && !result.issues.is_empty() {
        return Err(napi::Error::from_reason(format!(
            "Lint failed: {} issue(s) found",
            result.issues.len()
        )));
    }

    Ok(LintResult {
        files_checked: result.files_checked as u32,
        issues: result
            .issues
            .iter()
            .map(|issue| LintIssueInfo {
                file_path: issue.file_path.clone(),
                line: issue.line as u32,
                column: issue.column as u32,
                message: issue.message.clone(),
                text: issue.text.clone(),
            })
            .collect(),
    })
}

/// Check for dead (unused) translation keys
#[cfg(feature = "napi")]
#[napi]
pub fn check(config: NapiConfig, options: Option<CheckOptions>) -> Result<CheckResult> {
    let config: Config = Config::from_napi(config)
        .map_err(|e| napi::Error::from_reason(format!("Config validation failed: {}", e)))?;
    let remove = options.as_ref().and_then(|o| o.remove).unwrap_or(false);
    let dry_run = options.as_ref().and_then(|o| o.dry_run).unwrap_or(false);
    let locale = options
        .as_ref()
        .and_then(|o| o.locale.as_ref())
        .map(|s| s.as_str())
        .or(config.locales.first().map(|s| s.as_str()))
        .unwrap_or("en");

    let plural_config = config.plural_config();
    let extraction = crate::extractor::extract_from_glob_with_options(
        &config.input,
        &config.ignore,
        &config.functions,
        config.extract_from_comments,
        &plural_config,
        &config.trans_components,
        &config.use_translation_names,
        &config.nesting_prefix,
        &config.nesting_suffix,
        &config.nesting_options_separator,
    )
    .map_err(|e| napi::Error::from_reason(format!("Extraction failed: {}", e)))?;

    let mut all_keys: Vec<ExtractedKey> = Vec::new();
    for (_file_path, keys) in &extraction.files {
        all_keys.extend(keys.iter().cloned());
    }

    let locales_path = std::path::Path::new(&config.output);
    let dead_keys =
        cleanup_mod::find_dead_keys(locales_path, &all_keys, &config.default_namespace, locale)
            .map_err(|e| napi::Error::from_reason(format!("Check failed: {}", e)))?;

    let mut removed_count = 0usize;
    if remove && !dry_run && !dead_keys.is_empty() {
        removed_count = cleanup_mod::purge_dead_keys(locales_path, &dead_keys)
            .map_err(|e| napi::Error::from_reason(format!("Cleanup failed: {}", e)))?;
    }

    Ok(CheckResult {
        dead_keys: dead_keys
            .iter()
            .map(|dk| DeadKeyInfo {
                file_path: dk.file_path.clone(),
                key_path: dk.key_path.clone(),
                namespace: dk.namespace.clone(),
            })
            .collect(),
        removed_count: removed_count as u32,
    })
}
