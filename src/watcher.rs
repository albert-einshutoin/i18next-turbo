use anyhow::{Context, Result};
use glob::Pattern;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::extractor::{self, ExtractedKey};
use crate::finite_values::{normalized, FiniteIndex};
use crate::json_sync;

/// File watcher with incremental extraction support
pub struct FileWatcher {
    config: Config,
    output_dir: String,
    debounce_duration: Duration,
    /// Cache of extracted keys per file for incremental updates
    file_cache: HashMap<PathBuf, Vec<ExtractedKey>>,
    requires_project_scan: bool,
    ignore_patterns: Vec<Pattern>,
    input_patterns: Vec<Pattern>,
}

impl FileWatcher {
    pub fn new(config: Config, output_dir: Option<String>) -> Self {
        let output = output_dir.unwrap_or_else(|| config.output.clone());
        let ignore_patterns = config
            .ignore
            .iter()
            .filter_map(|pattern| Pattern::new(pattern).ok())
            .collect();
        Self {
            input_patterns: config
                .input
                .iter()
                .flat_map(|pattern| extractor::expand_brace_patterns(pattern))
                .filter_map(|pattern| Pattern::new(&pattern).ok())
                .collect(),
            config,
            output_dir: output,
            debounce_duration: Duration::from_millis(300),
            file_cache: HashMap::new(),
            requires_project_scan: false,
            ignore_patterns,
        }
    }

    /// Run the file watcher, blocking until interrupted
    pub fn run(&mut self) -> Result<()> {
        let (tx, rx) = channel();

        // Create debouncer
        let mut debouncer =
            new_debouncer(self.debounce_duration, tx).context("Failed to create file watcher")?;

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
        while let Ok(result) = rx.recv() {
            self.handle_events(result)?;
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
    fn should_process_file(&self, path: &Path) -> bool {
        let valid_extensions = ["ts", "tsx", "js", "jsx"];

        if self.is_ignored(path)
            || !self
                .input_patterns
                .iter()
                .any(|pattern| self.matches_path(pattern, path))
        {
            return false;
        }

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| valid_extensions.contains(&ext))
            .unwrap_or(false)
    }

    fn is_ignored(&self, path: &Path) -> bool {
        self.ignore_patterns
            .iter()
            .any(|pattern| self.matches_path(pattern, path))
    }

    fn matches_path(&self, pattern: &Pattern, path: &Path) -> bool {
        pattern.matches_path(path)
            || std::env::current_dir()
                .ok()
                .and_then(|cwd| {
                    path.strip_prefix(cwd)
                        .ok()
                        .map(|relative| pattern.matches_path(relative))
                })
                .unwrap_or(false)
    }

    /// Perform initial full extraction of all files
    fn full_extract(&mut self) -> Result<()> {
        println!("--- Initial extraction ---");

        let plural_config = self.config.plural_config();
        let extraction = extractor::extract_from_glob_with_options(
            &self.config.input,
            &self.config.ignore,
            &self.config.functions,
            self.config.extract_from_comments,
            &plural_config,
            &self.config.trans_components,
            &self.config.trans_keep_basic_html_nodes_for,
            &self.config.use_translation_names,
            &self.config.nesting_prefix,
            &self.config.nesting_suffix,
            &self.config.nesting_options_separator,
            &self.config.interpolation_prefix,
            &self.config.interpolation_suffix,
        )?;

        if self.config.remove_unused_keys && extraction.warning_count > 0 {
            anyhow::bail!("Cannot prune translations while source extraction is incomplete");
        }
        self.requires_project_scan = extraction.requires_project_scan;
        // Populate cache
        self.file_cache.clear();
        for (file_path, keys) in &extraction.files {
            let path = normalized(Path::new(file_path));
            self.file_cache.insert(path, keys.clone());
        }

        // Sync to JSON
        let all_keys: Vec<ExtractedKey> = self.file_cache.values().flatten().cloned().collect();
        let sync_results =
            json_sync::sync_all_locales(&self.config, &all_keys, &self.output_dir, false)?;

        // Report
        let total_keys: usize = self.file_cache.values().map(|v| v.len()).sum();
        let total_added: usize = sync_results.iter().map(|r| r.added_keys.len()).sum();
        let total_removed: usize = sync_results.iter().map(|r| r.removed_keys.len()).sum();

        println!("  Files: {}", self.file_cache.len());
        println!("  Keys: {}", total_keys);
        if total_added > 0 {
            println!("  New keys added: {}", total_added);
        }
        if total_removed > 0 {
            println!("  Keys removed: {}", total_removed);
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
            let path = normalized(&event.path);

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

        let finite_index = Arc::new(FiniteIndex::load(&changed_files)?);
        if (self.requires_project_scan || finite_index.has_dependencies())
            && (!changed_files.is_empty() || !removed_files.is_empty())
        {
            // Imported types/constants can change consumers even when their source is untouched.
            return self.full_extract();
        }

        // Collect namespaces from removed files before removing from cache
        let mut affected_namespaces = HashSet::new();
        for path in &removed_files {
            if let Some(keys) = self.file_cache.get(path) {
                for key in keys {
                    if let Some(ns) = &key.namespace {
                        affected_namespaces.insert(ns.clone());
                    } else {
                        affected_namespaces.insert(self.config.default_namespace.clone());
                    }
                }
            }
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

        // Re-extract only changed files and collect their namespaces
        let changed_keys = self.incremental_extract(&changed_files, &finite_index)?;
        for key in &changed_keys {
            if let Some(ns) = &key.namespace {
                affected_namespaces.insert(ns.clone());
            } else {
                affected_namespaces.insert(self.config.default_namespace.clone());
            }
        }

        // Merge all cached keys but only sync affected namespaces
        let all_keys: Vec<ExtractedKey> = self.file_cache.values().flatten().cloned().collect();

        // Only sync the affected namespaces (IO optimization)
        let sync_results = json_sync::sync_namespaces(
            &self.config,
            &all_keys,
            &self.output_dir,
            &affected_namespaces,
            false, // dry_run (watch mode always writes)
        )?;

        let total_added: usize = sync_results.iter().map(|r| r.added_keys.len()).sum();
        let total_removed: usize = sync_results.iter().map(|r| r.removed_keys.len()).sum();
        if total_added > 0 {
            println!("  Added {} new key(s)", total_added);
        }
        if total_removed > 0 {
            println!("  Removed {} stale key(s)", total_removed);
        }

        println!("--- Sync complete ---\n");

        Ok(())
    }

    /// Extract keys from only the changed files
    /// Returns all extracted keys from the changed files
    fn incremental_extract(
        &mut self,
        changed_files: &[PathBuf],
        finite_index: &Arc<FiniteIndex>,
    ) -> Result<Vec<ExtractedKey>> {
        use rayon::prelude::*;

        let plural_config = self.config.plural_config();
        let results: Result<Vec<_>> = changed_files
            .par_iter()
            .filter(|path| self.should_process_file(path))
            .map(|path| {
                let (keys, warnings) = extractor::extract_from_file_with_warnings(
                    path,
                    &self.config.functions,
                    &self.config.trans_components,
                    &self.config.trans_keep_basic_html_nodes_for,
                    &self.config.use_translation_names,
                    self.config.extract_from_comments,
                    &plural_config,
                    &self.config.nesting_prefix,
                    &self.config.nesting_suffix,
                    &self.config.nesting_options_separator,
                    &self.config.interpolation_prefix,
                    &self.config.interpolation_suffix,
                    Some(finite_index),
                )?;
                if self.config.remove_unused_keys && warnings > 0 {
                    anyhow::bail!(
                        "Cannot prune translations while source extraction is incomplete"
                    );
                }
                Ok((normalized(path), keys))
            })
            .collect();

        // Update cache and collect all extracted keys
        let mut all_extracted_keys = Vec::new();
        for (path, keys) in results? {
            if keys.is_empty() {
                self.file_cache.remove(&path);
            } else {
                all_extracted_keys.extend(keys.clone());
                self.file_cache.insert(path, keys);
            }
        }

        Ok(all_extracted_keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir_in;

    fn make_test_config(input: Vec<String>, ignore: Vec<String>) -> Config {
        Config {
            input,
            ignore,
            output: "locales".to_string(),
            locales: vec!["en".to_string()],
            functions: vec!["t".to_string()],
            ..Config::default()
        }
    }

    #[test]
    fn compute_watch_dirs_from_glob_prefixes() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let relative = src_dir
            .strip_prefix(&cwd)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let config = make_test_config(vec![format!("{}/**/*.ts", relative)], vec![]);
        let watcher = FileWatcher::new(config, None);

        let dirs = watcher.compute_watch_dirs();
        assert!(
            dirs.iter().any(|d| d.ends_with(&relative)),
            "watch dirs should include source dir; got: {:?}",
            dirs
        );
    }

    #[test]
    fn should_process_file_respects_extension_and_ignore_patterns() {
        let config = make_test_config(
            vec!["src/**/*.ts".to_string()],
            vec!["**/*.spec.ts".to_string()],
        );
        let watcher = FileWatcher::new(config, None);

        assert!(watcher.should_process_file(Path::new("src/app.ts")));
        assert!(!watcher.should_process_file(Path::new("src/app.tsx")));
        assert!(!watcher.should_process_file(Path::new("other/app.ts")));
        assert!(!watcher.should_process_file(Path::new("src/app.md")));
        assert!(!watcher.should_process_file(Path::new("src/app.spec.ts")));
    }

    #[test]
    fn incremental_extract_updates_cache_for_changed_files() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file = src_dir.join("app.ts");
        std::fs::write(&file, "t('watch.key');").unwrap();

        let relative = src_dir
            .strip_prefix(&cwd)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let config = make_test_config(vec![format!("{}/**/*.ts", relative)], vec![]);
        let mut watcher = FileWatcher::new(config, None);

        let keys = watcher
            .incremental_extract(
                std::slice::from_ref(&file),
                &Arc::new(FiniteIndex::load(std::slice::from_ref(&file)).unwrap()),
            )
            .unwrap();
        assert!(keys.iter().any(|k| k.key == "watch.key"));

        let cached = watcher.file_cache.get(&file).unwrap();
        assert!(cached.iter().any(|k| k.key == "watch.key"));
    }

    #[test]
    fn imported_type_changes_refresh_consumer_keys() {
        use notify_debouncer_mini::{DebouncedEvent, DebouncedEventKind};
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir(&src).unwrap();
        let types = src.join("types.ts");
        std::fs::write(&types, "export type State='old';").unwrap();
        std::fs::write(
            src.join("App.ts"),
            "import {State} from './types'; declare const state:State; t(`state.${state}`);",
        )
        .unwrap();
        let mut config = make_test_config(vec![format!("{}/*.ts", src.display())], vec![]);
        config.output = tmp.path().join("locales").display().to_string();
        let mut watcher = FileWatcher::new(config, None);
        watcher.full_extract().unwrap();
        assert!(watcher.requires_project_scan);
        std::fs::write(&types, "export type State='new';").unwrap();
        watcher
            .handle_events(Ok(vec![DebouncedEvent {
                path: types,
                kind: DebouncedEventKind::Any,
            }]))
            .unwrap();
        let locale = tmp.path().join("locales/en/translation.json");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(locale).unwrap()).unwrap();
        assert_eq!(value["state"]["new"], "");
        assert!(value["state"]["old"].is_null());
    }
}
