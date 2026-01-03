pub mod cleanup;
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
use crate::config::Config;
#[cfg(feature = "napi")]
use crate::extractor::ExtractedKey;

/// Extract translation keys from source files
///
/// # Arguments
/// * `config_json` - JSON string representation of the Config struct
/// * `options` - Optional extraction options (output, fail_on_warnings, generate_types, types_output)
///
/// # Returns
/// Returns a JSON string with extraction results
#[cfg(feature = "napi")]
#[napi]
pub fn extract(
    config_json: String,
    options: Option<ExtractOptions>,
) -> Result<String> {
    // Parse config from JSON
    let config: Config = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse config: {}", e)))?;

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
/// * `config_json` - JSON string representation of the Config struct
/// * `options` - Optional watch options (output)
///
/// # Note
/// This function runs indefinitely until interrupted. In a Node.js context,
/// this should be called in a separate thread or worker.
#[cfg(feature = "napi")]
#[napi]
pub fn watch(config_json: String, options: Option<WatchOptions>) -> Result<()> {
    // Parse config from JSON
    let config: Config = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse config: {}", e)))?;

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
