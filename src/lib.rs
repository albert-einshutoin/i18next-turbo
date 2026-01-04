pub mod cleanup;
pub mod commands;
pub mod config;
pub mod extractor;
pub mod json_sync;
pub mod lint;
pub mod typegen;
pub mod watcher;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi_derive::napi;
#[cfg(feature = "napi")]
use serde_json;

#[cfg(feature = "napi")]
use crate::config::{Config, NapiConfig};
#[cfg(feature = "napi")]
use crate::cleanup as cleanup_mod;
#[cfg(feature = "napi")]
use crate::extractor::ExtractedKey;
#[cfg(feature = "napi")]
use crate::lint as lint_mod;

/// Extract translation keys from source files
///
/// # Arguments
/// * `config` - Configuration object
/// * `options` - Optional extraction options (output, fail_on_warnings, generate_types, types_output)
///
/// # Returns
/// Returns a JSON string with extraction results
#[napi]
#[cfg(feature = "napi")]
pub fn extract(
    config: NapiConfig,
    options: Option<ExtractOptions>,
) -> Result<String> {
    let config: Config = Config::from_napi(config)
        .map_err(|e| napi::Error::from_reason(format!("Config validation failed: {}", e)))?;

    // Extract options
    let output = options.as_ref().and_then(|o| o.output.as_ref());
    let fail_on_warnings = options.as_ref().and_then(|o| o.fail_on_warnings).unwrap_or(false);
    let generate_types = options.as_ref().and_then(|o| o.generate_types).unwrap_or(false);
    let types_output = options
        .as_ref()
        .and_then(|o| o.types_output.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("src/@types/i18next.d.ts");

    // Determine output directory
    let output_dir = output.unwrap_or(&config.output);

    // Extract keys from files
    let extraction = crate::extractor::extract_from_glob(&config.input, &config.functions)
        .map_err(|e| napi::Error::from_reason(format!("Extraction failed: {}", e)))?;

    if extraction.files.is_empty() {
        if fail_on_warnings && extraction.warning_count > 0 {
            return Err(napi::Error::from_reason(format!(
                "Failed: {} warning(s) encountered (fail_on_warnings enabled)",
                extraction.warning_count
            )));
        }
        return Ok(serde_json::json!({
            "success": true,
            "files_processed": 0,
            "unique_keys": 0,
            "warnings": extraction.warning_count,
            "message": "No translation keys found."
        })
        .to_string());
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
    let sync_results = crate::json_sync::sync_all_locales(&config, &all_keys, output_dir)
        .map_err(|e| napi::Error::from_reason(format!("Sync failed: {}", e)))?;

    // Report sync results
    let mut total_added = 0;
    let mut updated_files: Vec<String> = Vec::new();
    for result in &sync_results {
        if !result.added_keys.is_empty() {
            total_added += result.added_keys.len();
            updated_files.push(result.file_path.clone());
        }
    }

    // Generate TypeScript types if requested
    if generate_types {
        let locales_dir = std::path::Path::new(output_dir);
        let types_path = std::path::Path::new(types_output);
        let default_locale = config.locales.first().map(|s| s.as_str()).unwrap_or("en");
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

    Ok(serde_json::json!({
        "success": true,
        "files_processed": extraction.files.len(),
        "unique_keys": unique_keys.len(),
        "keys_added": total_added,
        "updated_files": updated_files,
        "warnings": extraction.warning_count,
    })
    .to_string())
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
    watcher.run()
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
pub fn lint(config: NapiConfig, options: Option<LintOptions>) -> Result<String> {
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

    Ok(serde_json::json!({
        "files_checked": result.files_checked,
        "issues": result.issues.iter().map(|issue| serde_json::json!({
            "file_path": issue.file_path,
            "line": issue.line,
            "column": issue.column,
            "message": issue.message,
            "text": issue.text,
        })).collect::<Vec<_>>(),
    })
    .to_string())
}

/// Check for dead (unused) translation keys
#[cfg(feature = "napi")]
#[napi]
pub fn check(config: NapiConfig, options: Option<CheckOptions>) -> Result<String> {
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

    let extraction = crate::extractor::extract_from_glob(&config.input, &config.functions)
        .map_err(|e| napi::Error::from_reason(format!("Extraction failed: {}", e)))?;

    let mut all_keys: Vec<ExtractedKey> = Vec::new();
    for (_file_path, keys) in &extraction.files {
        all_keys.extend(keys.iter().cloned());
    }

    let locales_path = std::path::Path::new(&config.output);
    let dead_keys = cleanup_mod::find_dead_keys(
        locales_path,
        &all_keys,
        &config.default_namespace,
        locale,
    )
    .map_err(|e| napi::Error::from_reason(format!("Check failed: {}", e)))?;

    let mut removed_count = 0usize;
    if remove && !dry_run && !dead_keys.is_empty() {
        removed_count = cleanup_mod::purge_dead_keys(locales_path, &dead_keys)
            .map_err(|e| napi::Error::from_reason(format!("Cleanup failed: {}", e)))?;
    }

    Ok(serde_json::json!({
        "dead_keys": dead_keys.iter().map(|dk| serde_json::json!({
            "file_path": dk.file_path,
            "key_path": dk.key_path,
            "namespace": dk.namespace,
        })).collect::<Vec<_>>(),
        "removed_count": removed_count,
    })
    .to_string())
}
