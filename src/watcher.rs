use anyhow::{Context, Result};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::config::Config;
use crate::extractor::{self, ExtractedKey};
use crate::json_sync;

/// File watcher with incremental extraction support
pub struct FileWatcher {
    config: Config,
    output_dir: String,
    debounce_duration: Duration,
    /// Cache of extracted keys per file for incremental updates
    file_cache: HashMap<PathBuf, Vec<ExtractedKey>>,
}

impl FileWatcher {
    pub fn new(config: Config, output_dir: Option<String>) -> Self {
        let output = output_dir.unwrap_or_else(|| config.output.clone());
        Self {
            config,
            output_dir: output,
            debounce_duration: Duration::from_millis(300),
            file_cache: HashMap::new(),
        }
    }

    /// Run the file watcher, blocking until interrupted
    pub fn run(&mut self) -> Result<()> {
        let (tx, rx) = channel();

        // Create debouncer
        let mut debouncer = new_debouncer(self.debounce_duration, tx)
            .context("Failed to create file watcher")?;

        // Compute directories to watch from glob patterns
        let watch_dirs = self.compute_watch_dirs();

        if watch_dirs.is_empty() {
            anyhow::bail!("No valid directories found to watch from input patterns");
        }

        // Watch all computed directories
        for dir in &watch_dirs {
            println!("Watching: {}", dir.display());
            debouncer
                .watcher()
                .watch(dir, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch directory: {}", dir.display()))?;
        }

        println!("\nWatching for changes... (Ctrl+C to stop)\n");

        // Initial full extraction
        self.full_extract()?;

        // Process events in a loop
        loop {
            match rx.recv() {
                Ok(result) => {
                    self.handle_events(result)?;
                }
                Err(_) => {
                    // Channel closed, exit
                    break;
                }
            }
        }

        Ok(())
    }

    /// Compute directories to watch from glob patterns
    fn compute_watch_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = HashSet::new();

        for pattern in &self.config.input {
            // Extract the non-glob prefix as the watch directory
            // e.g., "src/**/*.tsx" -> "src"
            let parts: Vec<&str> = pattern.split('/').collect();
            let mut prefix = PathBuf::new();

            for part in parts {
                if part.contains('*') || part.contains('?') || part.contains('[') {
                    break;
                }
                prefix.push(part);
            }

            if prefix.as_os_str().is_empty() {
                prefix.push(".");
            }

            if prefix.exists() && prefix.is_dir() {
                dirs.insert(prefix.canonicalize().unwrap_or(prefix));
            }
        }

        dirs.into_iter().collect()
    }

    /// Check if a file should be processed based on its extension
    fn should_process_file(&self, path: &std::path::Path) -> bool {
        let valid_extensions = ["ts", "tsx", "js", "jsx"];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| valid_extensions.contains(&ext))
            .unwrap_or(false)
    }

    /// Perform initial full extraction of all files
    fn full_extract(&mut self) -> Result<()> {
        println!("--- Initial extraction ---");

        let extraction = extractor::extract_from_glob(&self.config.input, &self.config.functions)?;

        // Populate cache
        for (file_path, keys) in &extraction.files {
            let path = PathBuf::from(file_path);
            self.file_cache.insert(path, keys.clone());
        }

        // Sync to JSON
        let all_keys: Vec<ExtractedKey> = self.file_cache.values().flatten().cloned().collect();
        let sync_results = json_sync::sync_all_locales(&self.config, &all_keys, &self.output_dir)?;

        // Report
        let total_keys: usize = self.file_cache.values().map(|v| v.len()).sum();
        let total_added: usize = sync_results.iter().map(|r| r.added_keys.len()).sum();

        println!("  Files: {}", self.file_cache.len());
        println!("  Keys: {}", total_keys);
        if total_added > 0 {
            println!("  New keys added: {}", total_added);
        }
        if extraction.warning_count > 0 {
            println!("  Warnings: {}", extraction.warning_count);
        }
        println!("--- Ready ---\n");

        Ok(())
    }

    /// Handle debounced file events
    fn handle_events(&mut self, result: DebounceEventResult) -> Result<()> {
        let events = match result {
            Ok(events) => events,
            Err(error) => {
                eprintln!("Watch error: {:?}", error);
                return Ok(());
            }
        };

        let mut changed_files = Vec::new();
        let mut removed_files = Vec::new();

        for event in events {
            let path = event.path;

            // Filter by extension
            if !self.should_process_file(&path) {
                continue;
            }

            if path.exists() {
                changed_files.push(path);
            } else {
                removed_files.push(path);
            }
        }

        // Deduplicate
        changed_files.sort();
        changed_files.dedup();
        removed_files.sort();
        removed_files.dedup();

        // Remove deleted files from cache
        for path in &removed_files {
            self.file_cache.remove(path);
        }

        if changed_files.is_empty() && removed_files.is_empty() {
            return Ok(());
        }

        println!("--- Change detected ---");
        for f in &changed_files {
            println!("  Modified: {}", f.display());
        }
        for f in &removed_files {
            println!("  Removed: {}", f.display());
        }

        // Re-extract only changed files
        self.incremental_extract(&changed_files)?;

        // Merge all cached keys and sync to JSON
        let all_keys: Vec<ExtractedKey> = self.file_cache.values().flatten().cloned().collect();

        let sync_results = json_sync::sync_all_locales(&self.config, &all_keys, &self.output_dir)?;

        let total_added: usize = sync_results.iter().map(|r| r.added_keys.len()).sum();
        if total_added > 0 {
            println!("  Added {} new key(s)", total_added);
        }

        println!("--- Sync complete ---\n");

        Ok(())
    }

    /// Extract keys from only the changed files
    fn incremental_extract(&mut self, changed_files: &[PathBuf]) -> Result<()> {
        use rayon::prelude::*;

        let results: Vec<_> = changed_files
            .par_iter()
            .filter_map(|path| {
                match extractor::extract_from_file(path, &self.config.functions) {
                    Ok(keys) => Some((path.clone(), keys)),
                    Err(e) => {
                        eprintln!("  Warning: {}", e);
                        None
                    }
                }
            })
            .collect();

        // Update cache
        for (path, keys) in results {
            if keys.is_empty() {
                self.file_cache.remove(&path);
            } else {
                self.file_cache.insert(path, keys);
            }
        }

        Ok(())
    }
}
