use anyhow::Result;
use clap::{Parser, Subcommand};
use i18next_turbo::config::Config;
use i18next_turbo::extractor::{self, ExtractedKey};
use i18next_turbo::json_sync;
use i18next_turbo::watcher::FileWatcher;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "i18next-turbo")]
#[command(author, version, about = "High-performance i18n key extraction tool", long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract translation keys from source files
    Extract {
        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<String>,

        /// Fail on warnings
        #[arg(long)]
        fail_on_warnings: bool,
    },

    /// Watch for file changes and extract keys automatically
    Watch {
        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration
    let config = Config::load_or_default(cli.config.as_ref())?;

    match cli.command {
        Commands::Extract {
            output,
            fail_on_warnings,
        } => {
            run_extract(&config, output, fail_on_warnings)?;
        }
        Commands::Watch { output } => {
            println!("=== i18next-turbo watch ===\n");
            let mut watcher = FileWatcher::new(config, output);
            watcher.run()?;
        }
    }

    Ok(())
}

fn run_extract(config: &Config, output: Option<String>, fail_on_warnings: bool) -> Result<()> {
    println!("=== i18next-turbo extract ===\n");

    // Determine output directory
    let output_dir = output.as_ref().unwrap_or(&config.output);

    println!("Configuration:");
    println!("  Input patterns: {:?}", config.input);
    println!("  Output: {}", output_dir);
    println!("  Locales: {:?}", config.locales);
    println!("  Functions: {:?}", config.functions);
    println!();

    // Extract keys from files
    let extraction = extractor::extract_from_glob(&config.input, &config.functions)?;

    if extraction.files.is_empty() {
        println!("No translation keys found.");
        if fail_on_warnings && extraction.warning_count > 0 {
            eprintln!(
                "\nFailed: {} warning(s) encountered (--fail-on-warnings enabled)",
                extraction.warning_count
            );
            std::process::exit(1);
        }
        return Ok(());
    }

    // Collect all keys (with deduplication for display)
    let mut unique_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut all_keys: Vec<ExtractedKey> = Vec::new();

    println!("Extracted keys by file:");
    println!("{}", "-".repeat(60));

    for (file_path, keys) in &extraction.files {
        println!("\n{}", file_path);
        for key in keys {
            let full_key = match &key.namespace {
                Some(ns) => format!("{}:{}", ns, key.key),
                None => key.key.clone(),
            };
            println!("  - {}", full_key);
            unique_keys.insert(full_key);
            all_keys.push(key.clone());
        }
    }

    println!("\n{}", "-".repeat(60));
    println!("\nExtraction Summary:");
    println!("  Files processed: {}", extraction.files.len());
    println!("  Unique keys found: {}", unique_keys.len());
    if extraction.warning_count > 0 {
        println!("  Warnings: {}", extraction.warning_count);
    }

    // Sync to JSON files
    println!("\nSyncing to locale files...");
    let sync_results = json_sync::sync_all_locales(config, &all_keys, output_dir)?;

    // Report sync results
    let mut total_added = 0;
    for result in &sync_results {
        if !result.added_keys.is_empty() {
            println!(
                "  {} - added {} new key(s)",
                result.file_path,
                result.added_keys.len()
            );
            total_added += result.added_keys.len();
        }
    }

    if total_added == 0 {
        println!("  No new keys added (all keys already exist).");
    }

    println!("\nDone!");

    // Check fail-on-warnings
    if fail_on_warnings && extraction.warning_count > 0 {
        eprintln!(
            "\nFailed: {} warning(s) encountered (--fail-on-warnings enabled)",
            extraction.warning_count
        );
        std::process::exit(1);
    }

    Ok(())
}
