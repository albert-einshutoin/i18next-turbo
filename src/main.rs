use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use i18next_turbo::commands;
use i18next_turbo::config::Config;
use i18next_turbo::watcher::FileWatcher;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "i18next-turbo")]
#[command(author, version, about = "High-performance i18n key extraction tool", long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Configuration as JSON string (used by Node.js wrapper for JS/TS config files)
    #[arg(long, global = true, hide = true)]
    config_json: Option<String>,

    /// Read configuration JSON from stdin (avoids ARG_MAX limits for large configs)
    #[arg(long, global = true, hide = true)]
    config_stdin: bool,

    /// Hint for the original config path (provided by Node wrapper for JS/TS configs)
    #[arg(long, global = true, hide = true)]
    config_path_hint: Option<PathBuf>,

    /// Enable verbose output for detailed logging
    #[arg(short, long, global = true)]
    verbose: bool,

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

        /// Generate TypeScript type definitions after extraction
        #[arg(long)]
        generate_types: bool,

        /// TypeScript output path (only used with --generate-types)
        #[arg(long)]
        types_output: Option<String>,

        /// Preview changes without writing files
        #[arg(long)]
        dry_run: bool,

        /// Exit with non-zero code if locale files would be updated (useful for CI)
        #[arg(long)]
        ci: bool,
    },

    /// Watch for file changes and extract keys automatically
    Watch {
        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Generate TypeScript type definitions from existing locale files
    Typegen {
        /// TypeScript output path
        #[arg(short, long)]
        output: Option<String>,

        /// Default locale to use for type generation
        #[arg(short, long)]
        default_locale: Option<String>,

        /// Locales directory (overrides config)
        #[arg(short, long)]
        locales_dir: Option<String>,
    },

    /// Check for dead (unused) translation keys
    Check {
        /// Remove dead keys from locale files
        #[arg(long)]
        remove: bool,

        /// Show what would be removed without actually removing
        #[arg(long)]
        dry_run: bool,

        /// Locale to check (defaults to first locale in config)
        #[arg(short, long)]
        locale: Option<String>,
    },

    /// Show translation status summary
    Status {
        /// Locale to check (defaults to first locale in config)
        #[arg(short, long)]
        locale: Option<String>,

        /// Exit with non-zero code if translations are incomplete (useful for CI)
        #[arg(long)]
        fail_on_incomplete: bool,

        /// Only include keys from the specified namespace
        #[arg(long)]
        namespace: Option<String>,
    },

    /// Sync translation keys across locales
    Sync {
        /// Remove keys that don't exist in primary locale
        #[arg(long)]
        remove_unused: bool,

        /// Preview changes without writing files
        #[arg(long)]
        dry_run: bool,
    },

    /// Lint source files for hardcoded strings that should be translated
    Lint {
        /// Fail on lint errors (useful for CI)
        #[arg(long)]
        fail_on_error: bool,
    },

    /// Rename a translation key in source files and locale files
    RenameKey {
        /// The old key to rename
        old_key: String,

        /// The new key name
        new_key: String,

        /// Preview changes without modifying files
        #[arg(long)]
        dry_run: bool,

        /// Only rename in locale files (skip source files)
        #[arg(long)]
        locales_only: bool,
    },

    /// Initialize a new i18next-turbo configuration file
    Init {
        /// Overwrite existing config file
        #[arg(long)]
        force: bool,

        /// Input glob patterns (comma-separated)
        #[arg(short, long, default_value = "src/**/*.{ts,tsx,js,jsx}")]
        input: String,

        /// Output directory for locale files
        #[arg(short, long, default_value = "locales")]
        output: String,

        /// Locales (comma-separated)
        #[arg(short, long, default_value = "en,ja")]
        locales: String,

        /// Default namespace
        #[arg(short, long, default_value = "translation")]
        namespace: String,

        /// Functions to extract (comma-separated)
        #[arg(short, long, default_value = "t")]
        functions: String,
    },

    /// Migrate existing i18next/i18next-parser config files to i18next-turbo.json
    Migrate {
        /// Output path for the generated i18next-turbo.json
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Automatically confirm prompts and overwrite existing files
        #[arg(long)]
        yes: bool,

        /// Show the converted config without writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Locize integration commands
    Locize {
        #[command(subcommand)]
        command: LocizeCommands,
    },
}

#[derive(Subcommand)]
enum LocizeCommands {
    /// Upload local translation files to Locize
    Upload {
        /// Limit upload to a specific locale
        #[arg(long)]
        locale: Option<String>,

        /// Limit upload to a specific namespace
        #[arg(long)]
        namespace: Option<String>,

        /// Preview the upload without calling the API
        #[arg(long)]
        dry_run: bool,
    },

    /// Download translations from Locize into the local project
    Download {
        /// Limit download to a specific locale
        #[arg(long)]
        locale: Option<String>,

        /// Limit download to a specific namespace
        #[arg(long)]
        namespace: Option<String>,

        /// Preview the download without writing files
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let loaded_config = load_config(&cli)?;
    let config = loaded_config.config;

    match cli.command {
        Commands::Extract {
            output,
            fail_on_warnings,
            generate_types,
            types_output,
            dry_run,
            ci,
        } => {
            let resolved_types_output = types_output.unwrap_or_else(|| config.types_output_path());
            commands::extract::run(
                &config,
                output,
                fail_on_warnings,
                generate_types,
                &resolved_types_output,
                dry_run,
                ci,
                cli.verbose,
            )?;
        }
        Commands::Watch { output } => {
            println!("=== i18next-turbo watch ===\n");
            let mut watcher = FileWatcher::new(config.clone(), output);
            watcher.run()?;
        }
        Commands::Typegen {
            output,
            default_locale,
            locales_dir,
        } => {
            let resolved_output = output.unwrap_or_else(|| config.types_output_path());
            let resolved_default_locale = default_locale.or_else(|| config.types_default_locale());
            let resolved_locales_dir = locales_dir.or_else(|| config.types_locales_dir());
            commands::typegen::run(
                &config,
                &resolved_output,
                resolved_default_locale,
                resolved_locales_dir,
            )?;
        }
        Commands::Check {
            remove,
            dry_run,
            locale,
        } => {
            commands::check::run(&config, remove, dry_run, locale)?;
        }
        Commands::Status {
            locale,
            fail_on_incomplete,
            namespace,
        } => {
            commands::status::run(&config, locale, fail_on_incomplete, namespace)?;
        }
        Commands::Sync {
            remove_unused,
            dry_run,
        } => {
            commands::sync::run(&config, remove_unused, dry_run)?;
        }
        Commands::Lint { fail_on_error } => {
            commands::lint::run(&config, fail_on_error)?;
        }
        Commands::RenameKey {
            old_key,
            new_key,
            dry_run,
            locales_only,
        } => {
            commands::rename_key::run(&config, &old_key, &new_key, dry_run, locales_only)?;
        }
        Commands::Init {
            force,
            input,
            output,
            locales,
            namespace,
            functions,
        } => {
            commands::init::run(force, &input, &output, &locales, &namespace, &functions)?;
        }
        Commands::Migrate {
            output,
            yes,
            dry_run,
        } => {
            commands::migrate::run(
                &config,
                output,
                yes,
                dry_run,
                loaded_config.source_path.as_deref(),
                matches!(loaded_config.source_kind, ConfigSourceKind::InlineJson),
            )?;
        }
        Commands::Locize { command } => match command {
            LocizeCommands::Upload {
                locale,
                namespace,
                dry_run,
            } => {
                commands::locize::upload(&config, locale, namespace, dry_run)?;
            }
            LocizeCommands::Download {
                locale,
                namespace,
                dry_run,
            } => {
                commands::locize::download(&config, locale, namespace, dry_run)?;
            }
        },
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfigSourceKind {
    Default,
    File,
    InlineJson,
}

struct LoadedConfig {
    config: Config,
    source_kind: ConfigSourceKind,
    source_path: Option<PathBuf>,
}

fn load_config(cli: &Cli) -> Result<LoadedConfig> {
    if cli.config_stdin {
        let mut stdin_content = String::new();
        std::io::stdin()
            .read_to_string(&mut stdin_content)
            .context("Failed to read config from stdin")?;
        let config = Config::from_json_string(&stdin_content)?;
        return Ok(LoadedConfig {
            config,
            source_kind: ConfigSourceKind::InlineJson,
            source_path: cli.config_path_hint.clone(),
        });
    }

    if let Some(config_json) = &cli.config_json {
        let config = Config::from_json_string(config_json)?;
        return Ok(LoadedConfig {
            config,
            source_kind: ConfigSourceKind::InlineJson,
            source_path: cli.config_path_hint.clone(),
        });
    }

    if let Some(config_path) = &cli.config {
        let config = Config::load(config_path)?;
        return Ok(LoadedConfig {
            config,
            source_kind: ConfigSourceKind::File,
            source_path: Some(config_path.clone()),
        });
    }

    let default_path = Path::new("i18next-turbo.json");
    if default_path.exists() {
        let config = Config::load(default_path)?;
        return Ok(LoadedConfig {
            config,
            source_kind: ConfigSourceKind::File,
            source_path: Some(default_path.to_path_buf()),
        });
    }

    Ok(LoadedConfig {
        config: Config::default(),
        source_kind: ConfigSourceKind::Default,
        source_path: None,
    })
}
