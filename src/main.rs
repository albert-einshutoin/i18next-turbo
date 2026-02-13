#![cfg_attr(test, allow(clippy::field_reassign_with_default))]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use i18next_turbo::commands;
use i18next_turbo::config::Config;
use i18next_turbo::logging::{self, LogLevel};
use i18next_turbo::watcher::FileWatcher;
use std::io::Read;
use std::path::PathBuf;
use std::{fs, path::Path};

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

    /// Log level: error, warn, info, debug
    #[arg(long, global = true)]
    log_level: Option<String>,

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

        /// Sync only primary language locale files
        #[arg(long)]
        sync_primary: bool,

        /// Sync all configured locale files (default behavior)
        #[arg(long)]
        sync_all: bool,
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

        /// Watch mode: re-run lint when files change
        #[arg(long)]
        watch: bool,
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

        /// Run interactive setup wizard
        #[arg(long)]
        interactive: bool,

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

    /// Upload then download translations to keep local and Locize data in sync
    Sync {
        /// Limit sync to a specific locale
        #[arg(long)]
        locale: Option<String>,

        /// Limit sync to a specific namespace
        #[arg(long)]
        namespace: Option<String>,

        /// Preview sync actions without API/file writes
        #[arg(long)]
        dry_run: bool,
    },

    /// Migrate local translation resources to Locize and pull normalized data back
    Migrate {
        /// Limit migrate to a specific locale
        #[arg(long)]
        locale: Option<String>,

        /// Limit migrate to a specific namespace
        #[arg(long)]
        namespace: Option<String>,

        /// Preview migrate actions without API/file writes
        #[arg(long)]
        dry_run: bool,
    },

    /// Interactively set up Locize credentials and save to config
    Setup {
        /// Config file path to save (defaults to detected JSON config or i18next-turbo.json)
        #[arg(long)]
        output: Option<PathBuf>,

        /// Locize project id
        #[arg(long)]
        project_id: Option<String>,

        /// Locize API key
        #[arg(long)]
        api_key: Option<String>,

        /// Locize version (default: latest)
        #[arg(long)]
        version: Option<String>,

        /// Source language (e.g. en)
        #[arg(long)]
        source_language: Option<String>,

        /// Comma-separated namespaces
        #[arg(long)]
        namespaces: Option<String>,

        /// Do not prompt; use provided values and defaults
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let loaded_config = load_config(&cli)?;
    let mut config = loaded_config.config;

    if matches!(loaded_config.source_kind, ConfigSourceKind::Default) {
        auto_detect_config_for_command(&mut config, &cli.command);
    }

    let requested_level = cli
        .log_level
        .as_deref()
        .or(Some(config.log_level.as_str()))
        .unwrap_or("info");
    let level = if cli.verbose {
        LogLevel::Debug
    } else {
        LogLevel::parse(requested_level).unwrap_or(LogLevel::Info)
    };
    logging::set_level(level);
    logging::debug(&format!("resolved log level: {:?}", level));

    match cli.command {
        Commands::Extract {
            output,
            fail_on_warnings,
            generate_types,
            types_output,
            dry_run,
            ci,
            sync_primary,
            sync_all,
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
                sync_primary,
                sync_all,
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
        Commands::Lint {
            fail_on_error,
            watch,
        } => {
            commands::lint::run(&config, fail_on_error, watch)?;
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
            interactive,
            input,
            output,
            locales,
            namespace,
            functions,
        } => {
            commands::init::run(
                force,
                interactive,
                &input,
                &output,
                &locales,
                &namespace,
                &functions,
            )?;
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
            LocizeCommands::Sync {
                locale,
                namespace,
                dry_run,
            } => {
                commands::locize::sync(&config, locale, namespace, dry_run)?;
            }
            LocizeCommands::Migrate {
                locale,
                namespace,
                dry_run,
            } => {
                commands::locize::migrate(&config, locale, namespace, dry_run)?;
            }
            LocizeCommands::Setup {
                output,
                project_id,
                api_key,
                version,
                source_language,
                namespaces,
                yes,
            } => {
                commands::locize::setup(
                    &config,
                    loaded_config.source_path.as_deref(),
                    output,
                    project_id,
                    api_key,
                    version,
                    source_language,
                    namespaces,
                    yes,
                )?;
            }
        },
    }

    Ok(())
}

fn auto_detect_config_for_command(config: &mut Config, command: &Commands) {
    let should_detect = matches!(
        command,
        Commands::Status { .. } | Commands::Lint { .. } | Commands::Check { .. }
    );
    if !should_detect {
        return;
    }

    if let Some(output) = detect_locales_output_dir() {
        logging::debug(&format!("auto-detected locales output: {}", output));
        config.output = output;
    }
    let detected_locales = detect_locale_codes(Path::new(&config.output));
    if !detected_locales.is_empty() {
        logging::debug(&format!("auto-detected locales: {:?}", detected_locales));
        config.locales = detected_locales;
    }

    let inputs = detect_source_globs(&config.output);
    if !inputs.is_empty() {
        logging::debug(&format!("auto-detected input patterns: {:?}", inputs));
        config.input = inputs;
    }
}

fn detect_locales_output_dir() -> Option<String> {
    let candidates = ["locales", "public/locales", "src/locales", "app/locales"];
    for dir in candidates {
        let path = Path::new(dir);
        if path.exists() && path.is_dir() && has_locale_json_subdir(path) {
            return Some(path.to_string_lossy().to_string());
        }
    }

    let discovered = discover_locale_output_dirs(Path::new("."), 4);
    choose_best_locale_output(discovered)
}

fn choose_best_locale_output(mut paths: Vec<PathBuf>) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    paths.sort_by_key(|p| {
        let normalized = p.to_string_lossy().replace('\\', "/");
        let depth = normalized.split('/').filter(|s| !s.is_empty()).count();
        let has_locales_segment = normalized.split('/').any(|seg| seg == "locales");
        (if has_locales_segment { 0 } else { 1 }, depth, normalized)
    });
    let best = paths.into_iter().next()?;
    Some(normalize_relative_path(&best))
}

fn normalize_relative_path(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    if raw == "." {
        ".".to_string()
    } else {
        raw.strip_prefix("./").unwrap_or(&raw).to_string()
    }
}

fn discover_locale_output_dirs(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
        if depth > max_depth {
            return;
        }
        if has_locale_json_subdir(dir) {
            out.push(dir.to_path_buf());
        }

        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if matches!(
                    name,
                    "node_modules" | ".git" | "target" | ".next" | "dist" | "build"
                ) {
                    continue;
                }
            }
            walk(&path, depth + 1, max_depth, out);
        }
    }

    let mut out = Vec::new();
    walk(root, 0, max_depth, &mut out);
    out
}

fn has_locale_json_subdir(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    for entry in entries.flatten() {
        let sub = entry.path();
        if !sub.is_dir() {
            continue;
        }
        let Some(locale_name) = sub.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !looks_like_locale_code(locale_name) {
            continue;
        }
        if let Ok(files) = fs::read_dir(&sub) {
            for file in files.flatten() {
                let p = file.path();
                if p.is_file() && has_locale_extension(&p) {
                    return true;
                }
            }
        }
    }
    false
}

fn has_locale_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("json" | "json5" | "js" | "ts")
    )
}

fn looks_like_locale_code(name: &str) -> bool {
    let len = name.len();
    (2..=12).contains(&len)
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn detect_locale_codes(output_dir: &Path) -> Vec<String> {
    let mut locales = Vec::new();
    let Ok(entries) = fs::read_dir(output_dir) else {
        return locales;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let has_json = fs::read_dir(&path)
            .ok()
            .map(|iter| {
                iter.flatten().any(|f| {
                    let p = f.path();
                    p.is_file() && has_locale_extension(&p)
                })
            })
            .unwrap_or(false);
        if has_json {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                locales.push(name.to_string());
            }
        }
    }
    locales.sort();
    locales.dedup();
    locales
}

fn detect_source_globs(locales_output: &str) -> Vec<String> {
    let candidates = ["src", "app", "components", "pages", "lib"];
    let mut globs = Vec::new();
    let excluded_root = Path::new(locales_output)
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or_default()
        .to_string();
    for dir in candidates {
        let path = Path::new(dir);
        if path.exists() && path.is_dir() && dir != excluded_root {
            globs.push(format!("{}/**/*.{{ts,tsx,js,jsx}}", dir));
        }
    }
    if globs.is_empty() && Path::new(".").exists() {
        globs.push("**/*.{ts,tsx,js,jsx}".to_string());
    }
    globs
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn cwd_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        fn change_to(path: &Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    #[test]
    fn detect_locale_codes_reads_subdirectories_with_json() {
        let tmp = tempdir().unwrap();
        let out = tmp.path().join("locales");
        std::fs::create_dir_all(out.join("en")).unwrap();
        std::fs::create_dir_all(out.join("ja")).unwrap();
        std::fs::write(out.join("en").join("translation.json"), "{}").unwrap();
        std::fs::write(out.join("ja").join("translation.json"), "{}").unwrap();

        let locales = detect_locale_codes(&out);
        assert_eq!(locales, vec!["en".to_string(), "ja".to_string()]);
    }

    #[test]
    fn auto_detect_config_updates_status_inputs_and_output() {
        let _lock = cwd_test_lock().lock().unwrap();
        let tmp = tempdir().unwrap();
        let _guard = CwdGuard::change_to(tmp.path());
        std::fs::create_dir_all("locales/en").unwrap();
        std::fs::write("locales/en/translation.json", "{}").unwrap();
        std::fs::create_dir_all("src").unwrap();

        let mut config = Config::default();
        config.input = vec!["x/**/*.ts".to_string()];
        config.output = "xlocales".to_string();

        let cmd = Commands::Status {
            locale: None,
            fail_on_incomplete: false,
            namespace: None,
        };
        auto_detect_config_for_command(&mut config, &cmd);

        assert_eq!(config.output, "locales");
        assert!(config.locales.contains(&"en".to_string()));
        assert!(config.input.iter().any(|g| g == "src/**/*.{ts,tsx,js,jsx}"));
    }

    #[test]
    fn auto_detect_config_skips_extract_command() {
        let _lock = cwd_test_lock().lock().unwrap();
        let tmp = tempdir().unwrap();
        let _guard = CwdGuard::change_to(tmp.path());
        std::fs::create_dir_all("locales/en").unwrap();
        std::fs::write("locales/en/translation.json", "{}").unwrap();
        std::fs::create_dir_all("src").unwrap();

        let mut config = Config::default();
        config.output = "custom-output".to_string();
        config.input = vec!["custom/**/*.ts".to_string()];

        let cmd = Commands::Extract {
            output: None,
            fail_on_warnings: false,
            generate_types: false,
            types_output: None,
            dry_run: false,
            ci: false,
            sync_primary: false,
            sync_all: false,
        };
        auto_detect_config_for_command(&mut config, &cmd);

        assert_eq!(config.output, "custom-output");
        assert_eq!(config.input, vec!["custom/**/*.ts".to_string()]);
    }

    #[test]
    fn detect_locale_codes_accepts_json5_files() {
        let tmp = tempdir().unwrap();
        let out = tmp.path().join("locales");
        std::fs::create_dir_all(out.join("en")).unwrap();
        std::fs::write(out.join("en").join("translation.json5"), "{}").unwrap();

        let locales = detect_locale_codes(&out);
        assert_eq!(locales, vec!["en".to_string()]);
    }

    #[test]
    fn auto_detect_config_applies_to_check_command() {
        let _lock = cwd_test_lock().lock().unwrap();
        let tmp = tempdir().unwrap();
        let _guard = CwdGuard::change_to(tmp.path());
        std::fs::create_dir_all("public/locales/en").unwrap();
        std::fs::write("public/locales/en/translation.json", "{}").unwrap();
        std::fs::create_dir_all("src").unwrap();

        let mut config = Config::default();
        config.output = "xlocales".to_string();

        let cmd = Commands::Check {
            remove: false,
            dry_run: true,
            locale: None,
        };
        auto_detect_config_for_command(&mut config, &cmd);
        assert_eq!(config.output, "public/locales");
        assert!(config.locales.contains(&"en".to_string()));
    }

    #[test]
    fn detect_source_globs_falls_back_to_workspace_glob() {
        let globs = detect_source_globs("src");
        assert!(!globs.is_empty());
    }
}
