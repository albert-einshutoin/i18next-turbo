use anyhow::{bail, Result};

use crate::config::Config;
use crate::lint;

pub fn run(config: &Config, fail_on_error: bool) -> Result<()> {
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
