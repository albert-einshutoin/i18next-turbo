use anyhow::{bail, Result};
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

pub fn run(
    force: bool,
    interactive: bool,
    input: &str,
    output: &str,
    locales: &str,
    namespace: &str,
    functions: &str,
) -> Result<()> {
    println!("=== i18next-turbo init ===\n");

    let config_path = Path::new("i18next-turbo.json");

    // Check if config already exists
    if config_path.exists() && !force {
        bail!(
            "Configuration file already exists: {}. Use --force to overwrite.",
            config_path.display()
        );
    }

    let mut resolved_input = input.to_string();
    let mut resolved_output = output.to_string();
    let mut resolved_locales = locales.to_string();

    if output == "locales" {
        if let Some(detected_output) = detect_output_dir() {
            resolved_output = detected_output;
            println!("Detected output directory: {}", resolved_output);
        }
    }

    if locales == "en,ja" {
        if let Some(detected_locales) = detect_locales_csv(&resolved_output) {
            resolved_locales = detected_locales;
            println!("Detected locales: {}", resolved_locales);
        }
    }

    if input == "src/**/*.{ts,tsx,js,jsx}" {
        if let Some(detected_input) = detect_input_glob() {
            resolved_input = detected_input;
            println!("Detected input pattern: {}", resolved_input);
        }
    }

    let mut resolved_namespace = namespace.to_string();
    let mut resolved_functions = functions.to_string();

    if interactive {
        println!("\nInteractive setup wizard (press Enter to keep default):");
        resolved_input = prompt_with_default("Input patterns (comma-separated)", &resolved_input)?;
        resolved_output = prompt_with_default("Output directory", &resolved_output)?;
        resolved_locales = prompt_with_default("Locales (comma-separated)", &resolved_locales)?;
        resolved_namespace = prompt_with_default("Default namespace", &resolved_namespace)?;
        resolved_functions =
            prompt_with_default("Functions (comma-separated)", &resolved_functions)?;
    }

    // Parse comma-separated values
    let input_patterns = split_csv(&resolved_input);
    let locales_vec = split_csv(&resolved_locales);
    let functions_vec = split_csv(&resolved_functions);

    // Create config JSON
    let config = serde_json::json!({
        "input": input_patterns,
        "output": resolved_output,
        "locales": locales_vec,
        "defaultNamespace": resolved_namespace,
        "functions": functions_vec,
        "keySeparator": ".",
        "nsSeparator": ":"
    });

    // Write config file
    let config_str = serde_json::to_string_pretty(&config)?;
    std::fs::write(config_path, format!("{}\n", config_str))?;

    println!("Created configuration file: {}\n", config_path.display());
    println!("Configuration:");
    println!("  Input patterns: {:?}", input_patterns);
    println!("  Output: {}", resolved_output);
    println!("  Locales: {:?}", locales_vec);
    println!("  Default namespace: {}", resolved_namespace);
    println!("  Functions: {:?}", functions_vec);

    println!("\nNext steps:");
    println!("  1. Run 'i18next-turbo extract' to extract translation keys");
    println!("  2. Run 'i18next-turbo watch' for continuous extraction");
    println!("  3. Run 'i18next-turbo typegen' to generate TypeScript types");

    // Create output directories for each locale
    println!("\nCreating locale directories...");
    for locale in &locales_vec {
        let locale_dir = Path::new(&resolved_output).join(locale);
        if !locale_dir.exists() {
            std::fs::create_dir_all(&locale_dir)?;
            println!("  Created: {}", locale_dir.display());
        }
    }

    println!("\nDone!");
    Ok(())
}

fn prompt_with_default(label: &str, default_value: &str) -> Result<String> {
    print!("{} [{}]: ", label, default_value);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let v = buf.trim();
    if v.is_empty() {
        Ok(default_value.to_string())
    } else {
        Ok(v.to_string())
    }
}

fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn detect_output_dir() -> Option<String> {
    let candidates = [
        "locales",
        "public/locales",
        "src/locales",
        "app/locales",
        "resources/locales",
    ];

    for candidate in candidates {
        let path = Path::new(candidate);
        if path.is_dir() && has_locale_json_subdir(path) {
            return Some(candidate.to_string());
        }
    }

    None
}

fn has_locale_json_subdir(base: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(base) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Ok(locale_entries) = std::fs::read_dir(path) else {
            continue;
        };
        for locale_entry in locale_entries.flatten() {
            let file_path = locale_entry.path();
            if !file_path.is_file() {
                continue;
            }
            let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if matches!(ext, "json" | "json5" | "js" | "ts") {
                return true;
            }
        }
    }

    false
}

fn detect_locales_csv(output: &str) -> Option<String> {
    let base = Path::new(output);
    let Ok(entries) = std::fs::read_dir(base) else {
        return None;
    };

    let mut locales = BTreeSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(locale) = path.file_name().and_then(|n| n.to_str()) {
                if looks_like_locale(locale) {
                    locales.insert(locale.to_string());
                }
            }
        }
    }

    if locales.is_empty() {
        None
    } else {
        Some(locales.into_iter().collect::<Vec<_>>().join(","))
    }
}

fn looks_like_locale(locale: &str) -> bool {
    let len = locale.len();
    (2..=12).contains(&len)
        && locale
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn detect_input_glob() -> Option<String> {
    let candidates = [
        "src/**/*.{ts,tsx,js,jsx}",
        "app/**/*.{ts,tsx,js,jsx}",
        "pages/**/*.{ts,tsx,js,jsx}",
        "components/**/*.{ts,tsx,js,jsx}",
    ];

    for candidate in candidates {
        let root = candidate.split('/').next().unwrap_or_default();
        if Path::new(root).is_dir() {
            return Some(candidate.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

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

    fn cwd_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn looks_like_locale_accepts_common_patterns() {
        assert!(looks_like_locale("en"));
        assert!(looks_like_locale("ja"));
        assert!(looks_like_locale("en-US"));
        assert!(looks_like_locale("pt_BR"));
        assert!(!looks_like_locale("x"));
        assert!(!looks_like_locale("ja JP"));
    }

    #[test]
    fn detect_locales_csv_from_output_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("en")).unwrap();
        std::fs::create_dir_all(tmp.path().join("ja")).unwrap();
        std::fs::create_dir_all(tmp.path().join("not a locale")).unwrap();

        let detected = detect_locales_csv(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(detected, "en,ja");
    }

    #[test]
    fn detect_output_dir_prefers_existing_public_locales() {
        let _lock = cwd_test_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let _guard = CwdGuard::change_to(tmp.path());
        std::fs::create_dir_all("public/locales/en").unwrap();
        std::fs::write("public/locales/en/translation.json", "{}").unwrap();

        let detected = detect_output_dir();
        assert_eq!(detected.as_deref(), Some("public/locales"));
    }

    #[test]
    fn detect_input_glob_falls_back_to_app_when_no_src() {
        let _lock = cwd_test_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let _guard = CwdGuard::change_to(tmp.path());
        std::fs::create_dir_all("app").unwrap();

        let detected = detect_input_glob();
        assert_eq!(detected.as_deref(), Some("app/**/*.{ts,tsx,js,jsx}"));
    }

    #[test]
    fn split_csv_trims_and_skips_empty_entries() {
        let values = split_csv(" t , ,i18n.t ,");
        assert_eq!(values, vec!["t".to_string(), "i18n.t".to_string()]);
    }
}
