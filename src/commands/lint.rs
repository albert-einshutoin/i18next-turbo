use anyhow::{bail, Result};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::config::Config;
use crate::lint::{self, LintOptions};

pub fn run(config: &Config, fail_on_error: bool, watch: bool) -> Result<()> {
    if watch {
        return run_watch(config, fail_on_error);
    }

    println!("=== i18next-turbo lint ===\n");

    println!("Configuration:");
    println!("  Input patterns: {:?}", config.input);
    println!();

    println!("Scanning for hardcoded strings...");
    let lint_options = LintOptions {
        ignored_attributes: config.lint.ignored_attributes.clone(),
        ignored_tags: config.lint.ignored_tags.clone(),
        accepted_attributes: config.lint.accepted_attributes.clone(),
        accepted_tags: config.lint.accepted_tags.clone(),
        ignore_patterns: config.lint.ignore.clone(),
    };
    let result = lint::lint_from_glob_with_options(&config.input, &lint_options)?;

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
        println!("\n{}:{}:{}", issue.file_path, issue.line, issue.column);
        println!("  {}", issue.message);
        println!("  Text: \"{}\"", issue.text);
    }

    println!("\n{}", "=".repeat(60));
    println!("Total: {} issue(s)", result.issues.len());

    if fail_on_error {
        bail!(
            "{} lint issue(s) found (--fail-on-error enabled)",
            result.issues.len()
        );
    }

    Ok(())
}

fn run_watch(config: &Config, fail_on_error: bool) -> Result<()> {
    println!("=== i18next-turbo lint (watch) ===\n");
    run_once(config, fail_on_error)?;

    let (tx, rx) = channel();
    let mut debouncer = new_debouncer(Duration::from_millis(300), tx)?;

    let watch_dirs = compute_watch_dirs(&config.input);
    for dir in &watch_dirs {
        debouncer.watcher().watch(dir, RecursiveMode::Recursive)?;
    }

    println!("Watching for changes... (Ctrl+C to stop)\n");
    while let Ok(result) = rx.recv() {
        handle_watch_events(result, config, fail_on_error)?;
    }
    Ok(())
}

fn handle_watch_events(
    result: DebounceEventResult,
    config: &Config,
    fail_on_error: bool,
) -> Result<()> {
    match result {
        Ok(events) => {
            if events.is_empty() {
                return Ok(());
            }
            println!("--- lint re-run ---");
            run_once(config, fail_on_error)?;
        }
        Err(err) => {
            eprintln!("Watch error: {:?}", err);
        }
    }
    Ok(())
}

fn run_once(config: &Config, fail_on_error: bool) -> Result<()> {
    let lint_options = LintOptions {
        ignored_attributes: config.lint.ignored_attributes.clone(),
        ignored_tags: config.lint.ignored_tags.clone(),
        accepted_attributes: config.lint.accepted_attributes.clone(),
        accepted_tags: config.lint.accepted_tags.clone(),
        ignore_patterns: config.lint.ignore.clone(),
    };
    let result = lint::lint_from_glob_with_options(&config.input, &lint_options)?;

    println!("  Files checked: {}", result.files_checked);
    println!("  Issues found: {}", result.issues.len());
    if result.issues.is_empty() {
        println!("No hardcoded strings found. All text appears to be translated!\n");
        return Ok(());
    }

    for issue in &result.issues {
        println!(
            "{}:{}:{} {}",
            issue.file_path, issue.line, issue.column, issue.message
        );
    }
    println!();

    if fail_on_error {
        bail!(
            "{} lint issue(s) found (--fail-on-error enabled)",
            result.issues.len()
        );
    }
    Ok(())
}

fn compute_watch_dirs(patterns: &[String]) -> Vec<PathBuf> {
    let mut dirs = HashSet::new();
    for pattern in patterns {
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
        if Path::new(&prefix).exists() {
            dirs.insert(prefix);
        }
    }
    dirs.into_iter().collect()
}
