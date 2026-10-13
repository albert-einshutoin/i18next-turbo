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
    finite_index: Arc<FiniteIndex>,
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
            finite_index: Arc::default(),
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
        let valid_extensions = [
            "ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs", "vue", "svelte",
        ];

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
        let (extraction, finite_index) = extractor::extract_from_glob_with_index(
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
        self.finite_index = finite_index;
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
        let events =
            result.context("Watch events lost; incremental extraction cannot continue safely")?;
        let mut notified: Vec<_> = events
            .into_iter()
            .map(|event| normalized(&event.path))
            .filter(|path| self.should_process_file(path))
            .collect();
        notified.sort();
        notified.dedup();
        let mut affected = Arc::make_mut(&mut self.finite_index).refresh(&notified)?;
        if affected.is_empty() {
            return Ok(());
        }

        // Read affected consumers now: rename notifications can arrive after their imports changed.
        // Byte-identical delayed events then produce no second extraction or sync.
        let mut seen: HashSet<_> = notified.into_iter().collect();
        loop {
            let pending: Vec<_> = affected
                .iter()
                .filter(|path| !seen.contains(*path) && self.should_process_file(path))
                .cloned()
                .collect();
            if pending.is_empty() {
                break;
            }
            seen.extend(pending.iter().cloned());
            affected.extend(Arc::make_mut(&mut self.finite_index).refresh(&pending)?);
        }
        let mut changed_files = Vec::new();
        let mut removed_files = Vec::new();
        let mut affected_namespaces = HashSet::new();
        for path in affected {
            if let Some(keys) = self.file_cache.get(&path) {
                for key in keys {
                    affected_namespaces.insert(
                        key.namespace
                            .clone()
                            .unwrap_or_else(|| self.config.default_namespace.clone()),
                    );
                }
            }
            if self.finite_index.source(&path).is_some() {
                changed_files.push(path);
            } else {
                removed_files.push(path);
            }
        }
        changed_files.sort();
        removed_files.sort();

        let finite_index = Arc::clone(&self.finite_index);
        let changed_keys = self.incremental_extract(&changed_files, &finite_index)?;
        // Do not discard deleted keys until every affected source has passed the prune guard.
        for path in &removed_files {
            self.file_cache.remove(path);
        }
        for key in &changed_keys {
            affected_namespaces.insert(
                key.namespace
                    .clone()
                    .unwrap_or_else(|| self.config.default_namespace.clone()),
            );
        }
        println!("--- Change detected ---");
        println!("  Reparsed files: {}", changed_files.len());
        for path in &changed_files {
            println!("  Modified: {}", path.display());
        }
        for path in &removed_files {
            println!("  Removed: {}", path.display());
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
        let unrelated = src.join("Unrelated.ts");
        std::fs::write(&unrelated, "t('unrelated');").unwrap();
        let mut config = make_test_config(vec![format!("{}/*.ts", src.display())], vec![]);
        config.output = tmp.path().join("locales").display().to_string();
        let mut watcher = FileWatcher::new(config, None);
        watcher.full_extract().unwrap();
        // No event was delivered for this file: dependency updates must not reread it.
        std::fs::write(&unrelated, [0xff]).unwrap();
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
        assert_eq!(value["unrelated"], "");
    }

    fn event(path: &Path) -> notify_debouncer_mini::DebouncedEvent {
        notify_debouncer_mini::DebouncedEvent {
            path: path.to_path_buf(),
            kind: notify_debouncer_mini::DebouncedEventKind::Any,
        }
    }

    #[test]
    fn dependency_rename_reads_consumers_before_their_delayed_events() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir(&src).unwrap();
        let old = src.join("labels.ts");
        let new = src.join("moved.ts");
        let app = src.join("App.ts");
        let unrelated = src.join("Other.ts");
        std::fs::write(&old, "export const KEY='kept';").unwrap();
        std::fs::write(&app, "import {KEY} from './labels'; t(KEY);").unwrap();
        std::fs::write(&unrelated, "t('other');").unwrap();
        let mut config = make_test_config(vec![format!("{}/*.ts", src.display())], vec![]);
        config.output = tmp.path().join("locales").display().to_string();
        let mut watcher = FileWatcher::new(config, None);
        watcher.full_extract().unwrap();
        std::fs::rename(&old, &new).unwrap();
        std::fs::write(&app, "import {KEY} from './moved'; t(KEY);").unwrap();
        watcher
            .handle_events(Ok(vec![event(&old), event(&new)]))
            .unwrap();
        let locale = tmp.path().join("locales/en/translation.json");
        let before = std::fs::read(&locale).unwrap();
        std::fs::write(&unrelated, [0xff]).unwrap();
        watcher
            .handle_events(Ok(vec![event(&app), event(&new), event(&old)]))
            .unwrap();
        assert_eq!(std::fs::read(&locale).unwrap(), before);
        // A later real content change must still be processed.
        std::fs::write(&app, "import {KEY} from './moved'; t(KEY); t('later');").unwrap();
        watcher.handle_events(Ok(vec![event(&app)])).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&locale).unwrap()).unwrap();
        assert_eq!(value["kept"], "");
        assert_eq!(value["later"], "");
    }

    #[test]
    fn changed_file_prunes_its_previous_namespace() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let app = tmp.path().join("App.ts");
        std::fs::write(&app, "t('account:old');").unwrap();
        let mut config = make_test_config(vec![app.display().to_string()], vec![]);
        config.output = tmp.path().join("locales").display().to_string();
        let mut watcher = FileWatcher::new(config, None);
        watcher.full_extract().unwrap();
        std::fs::write(&app, "t('common:new');").unwrap();
        watcher.handle_events(Ok(vec![event(&app)])).unwrap();
        let account: serde_json::Value = serde_json::from_slice(
            &std::fs::read(tmp.path().join("locales/en/account.json")).unwrap(),
        )
        .unwrap();
        assert!(account["old"].is_null());
        let common: serde_json::Value = serde_json::from_slice(
            &std::fs::read(tmp.path().join("locales/en/common.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(common["new"], "");
    }

    #[test]
    fn unresolved_dependency_and_removal_refuse_pruning_without_mutating_cache() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir(&src).unwrap();
        let types = src.join("types.ts");
        let removed = src.join("Removed.ts");
        std::fs::write(&types, "export type State='kept';").unwrap();
        std::fs::write(
            src.join("App.ts"),
            "import {State} from './types'; declare const state:State; t(`state.${state}`);",
        )
        .unwrap();
        std::fs::write(&removed, "t('removed');").unwrap();
        let mut config = make_test_config(vec![format!("{}/*.ts", src.display())], vec![]);
        config.output = tmp.path().join("locales").display().to_string();
        let mut watcher = FileWatcher::new(config, None);
        watcher.full_extract().unwrap();
        let locale = tmp.path().join("locales/en/translation.json");
        let before = std::fs::read(&locale).unwrap();
        std::fs::write(&types, "export type State=string;").unwrap();
        std::fs::remove_file(&removed).unwrap();
        let error = watcher
            .handle_events(Ok(vec![event(&types), event(&removed)]))
            .unwrap_err();
        assert!(error.to_string().contains("Cannot prune"));
        assert_eq!(std::fs::read(&locale).unwrap(), before);
        assert!(watcher.file_cache.contains_key(&normalized(&removed)));
    }

    #[test]
    fn adding_an_ambiguous_import_target_refuses_pruning() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("types.ts"), "export type State='kept';").unwrap();
        std::fs::write(
            src.join("App.ts"),
            "import {State} from './types'; declare const state:State; t(`state.${state}`);",
        )
        .unwrap();
        let mut config = make_test_config(vec![format!("{}/*.*", src.display())], vec![]);
        config.output = tmp.path().join("locales").display().to_string();
        let mut watcher = FileWatcher::new(config, None);
        watcher.full_extract().unwrap();
        let locale = tmp.path().join("locales/en/translation.json");
        let before = std::fs::read(&locale).unwrap();
        let added = src.join("types.tsx");
        std::fs::write(&added, "export type State='wrong';").unwrap();
        assert!(watcher.handle_events(Ok(vec![event(&added)])).is_err());
        assert_eq!(std::fs::read(&locale).unwrap(), before);
    }

    #[test]
    fn transitive_consumers_refresh_without_reading_ignored_inputs() {
        let cwd = std::env::current_dir().unwrap();
        let tmp = tempdir_in(&cwd).unwrap();
        let types = tmp.path().join("types.ts");
        let ignored = tmp.path().join("ignored.ts");
        std::fs::write(&types, "export type State='old';").unwrap();
        std::fs::write(
            tmp.path().join("model.ts"),
            "import {State} from './types'; export type Combined=State;",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("App.ts"),
            "import {Combined} from './model'; declare const state:Combined; t(`state.${state}`);",
        )
        .unwrap();
        std::fs::write(&ignored, [0xff]).unwrap();
        let mut config = make_test_config(
            vec![format!("{}/*.ts", tmp.path().display())],
            vec![ignored.display().to_string()],
        );
        config.output = tmp.path().join("locales").display().to_string();
        let mut watcher = FileWatcher::new(config, None);
        watcher.full_extract().unwrap();
        std::fs::write(&types, "export type State='new';").unwrap();
        watcher
            .handle_events(Ok(vec![event(&types), event(&ignored)]))
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(tmp.path().join("locales/en/translation.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(value["state"]["new"], "");
        assert!(value["state"]["old"].is_null());
    }

    #[test]
    fn component_scripts_refresh_imported_constants() {
        for extension in ["vue", "svelte"] {
            let cwd = std::env::current_dir().unwrap();
            let tmp = tempdir_in(&cwd).unwrap();
            let labels = tmp.path().join("labels.ts");
            std::fs::write(&labels, "export const KEY='old';").unwrap();
            std::fs::write(
                tmp.path().join(format!("App.{extension}")),
                "<script lang=\"ts\">import {KEY} from './labels'; t(KEY);</script>",
            )
            .unwrap();
            let mut config = make_test_config(
                vec![format!("{}/*.{{ts,{extension}}}", tmp.path().display())],
                vec![],
            );
            config.output = tmp.path().join("locales").display().to_string();
            let mut watcher = FileWatcher::new(config, None);
            watcher.full_extract().unwrap();
            std::fs::write(&labels, "export const KEY='new';").unwrap();
            watcher.handle_events(Ok(vec![event(&labels)])).unwrap();
            let value: serde_json::Value = serde_json::from_slice(
                &std::fs::read(tmp.path().join("locales/en/translation.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(value["new"], "", "{extension}");
            assert!(value["old"].is_null(), "{extension}");
        }
    }

    #[test]
    fn watcher_errors_invalidate_incremental_state() {
        let watcher_config = make_test_config(vec!["src/*.ts".to_string()], vec![]);
        let mut watcher = FileWatcher::new(watcher_config, None);
        assert!(watcher
            .handle_events(Err(notify_debouncer_mini::notify::Error::generic(
                "events lost"
            )))
            .is_err());
    }
}
