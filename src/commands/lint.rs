use anyhow::{bail, Result};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::config::Config;
use crate::lint::{self, LintOptions};

#[derive(Debug, Serialize)]
pub struct LintIssueReport {
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct LintCommandResult {
    pub files_checked: usize,
    pub issues: Vec<LintIssueReport>,
}

pub fn run(config: &Config, fail_on_error: bool, watch: bool) -> Result<()> {
    if watch {
        return run_watch(config, fail_on_error);
    }

    println!("=== i18next-turbo lint ===\n");
    println!("Configuration:");
    println!("  Input patterns: {:?}", config.input);
    println!();

    println!("Scanning for hardcoded strings...");
    let result = execute(config, fail_on_error)?;
    print_result(&result);
    Ok(())
}

pub fn execute(config: &Config, fail_on_error: bool) -> Result<LintCommandResult> {
    let lint_options = LintOptions {
        ignored_attributes: config.lint.ignored_attributes.clone(),
        ignored_tags: config.lint.ignored_tags.clone(),
        accepted_attributes: config.lint.accepted_attributes.clone(),
        accepted_tags: config.lint.accepted_tags.clone(),
        ignore_patterns: config.lint.ignore.clone(),
    };
    let result = lint::lint_from_glob_with_options(&config.input, &lint_options)?;
    let issues: Vec<LintIssueReport> = result
        .issues
        .iter()
        .map(|issue| LintIssueReport {
            file_path: issue.file_path.clone(),
            line: issue.line,
            column: issue.column,
            message: issue.message.clone(),
            text: issue.text.clone(),
        })
        .collect();

    if fail_on_error && !issues.is_empty() {
        bail!(
            "{} lint issue(s) found (--fail-on-error enabled)",
            issues.len()
        );
    }

    Ok(LintCommandResult {
        files_checked: result.files_checked,
        issues,
    })
}

fn print_result(result: &LintCommandResult) {
    println!("  Files checked: {}", result.files_checked);
    println!("  Issues found: {}", result.issues.len());
    println!();

    if result.issues.is_empty() {
        println!("No hardcoded strings found. All text appears to be translated!");
        return;
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
    let result = execute(config, fail_on_error)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn compute_watch_dirs_extracts_existing_prefix() {
        let dirs = compute_watch_dirs(&["./**/*.ts".to_string()]);
        assert!(dirs.contains(&PathBuf::from(".")));
    }

    #[test]
    fn handle_watch_events_accepts_error_result() {
        let config = Config::default();
        let result: DebounceEventResult =
            Err(notify_debouncer_mini::notify::Error::generic("watch error"));
        let handled = handle_watch_events(result, &config, false);
        assert!(handled.is_ok());
    }
}
