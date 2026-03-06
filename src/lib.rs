#![cfg_attr(test, allow(clippy::field_reassign_with_default))]

pub mod cleanup;
pub mod commands;
pub mod config;
pub mod error;
pub mod extractor;
pub mod fs;
pub mod json_sync;
pub mod lint;
pub mod logging;
pub mod typegen;
pub mod watcher;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi_derive::napi;

#[cfg(feature = "napi")]
use crate::config::{Config, NapiConfig};

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
    /// Number of stale keys removed from locale files
    pub keys_removed: u32,
    /// List of updated locale files
    pub updated_files: Vec<String>,
    /// Number of warnings encountered
    pub warnings: u32,
    /// Number of extraction warnings encountered before sync
    pub extraction_warnings: u32,
    /// Number of key conflicts encountered during sync
    pub conflicts: u32,
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
        .unwrap_or_else(Config::default_types_output);

    let result = commands::extract::execute(
        &config,
        output.cloned(),
        fail_on_warnings,
        generate_types,
        &types_output,
        false,
        false,
        false,
        false,
    )
    .map_err(|e| napi::Error::from_reason(format!("Extraction failed: {}", e)))?;

    Ok(ExtractResult {
        success: result.success,
        files_processed: result.files_processed as u32,
        unique_keys: result.unique_keys as u32,
        keys_added: result.keys_added as u32,
        keys_removed: result.keys_removed as u32,
        updated_files: result.updated_files,
        warnings: result.warnings as u32,
        extraction_warnings: result.extraction_warnings as u32,
        conflicts: result.conflicts as u32,
        message: result.message,
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

    let result = commands::lint::execute(&config, fail_on_error)
        .map_err(|e| napi::Error::from_reason(format!("Lint failed: {}", e)))?;

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
    let locale = options.as_ref().and_then(|o| o.locale.as_ref()).cloned();
    let result = commands::check::execute(&config, remove, dry_run, locale)
        .map_err(|e| napi::Error::from_reason(format!("Check failed: {}", e)))?;

    Ok(CheckResult {
        dead_keys: result
            .dead_keys
            .iter()
            .map(|dk| DeadKeyInfo {
                file_path: dk.file_path.clone(),
                key_path: dk.key_path.clone(),
                namespace: dk.namespace.clone(),
            })
            .collect(),
        removed_count: result.removed_count as u32,
    })
}
