use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Configuration for i18next-turbo
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Glob patterns for input files (e.g., ["src/**/*.tsx", "src/**/*.ts"])
    #[serde(default = "default_input")]
    pub input: Vec<String>,

    /// Output directory for translation files
    #[serde(default = "default_output")]
    pub output: String,

    /// List of language codes (e.g., ["en", "ja"])
    #[serde(default = "default_locales")]
    pub locales: Vec<String>,

    /// Default namespace
    #[serde(default = "default_namespace")]
    pub default_namespace: String,

    /// Function names to extract (e.g., ["t", "i18n.t"])
    #[serde(default = "default_functions")]
    pub functions: Vec<String>,

    /// Key separator (e.g., "." for "button.submit")
    #[serde(default = "default_key_separator")]
    pub key_separator: String,

    /// Namespace separator (e.g., ":" for "common:greeting")
    #[serde(default = "default_ns_separator")]
    pub ns_separator: String,

    /// Context separator (e.g., "_" for "friend_male")
    #[serde(default = "default_context_separator")]
    pub context_separator: String,

    /// Plural separator (e.g., "_" for "item_one")
    #[serde(default = "default_plural_separator")]
    pub plural_separator: String,
}

fn default_input() -> Vec<String> {
    vec!["src/**/*.{ts,tsx,js,jsx}".to_string()]
}

fn default_output() -> String {
    "locales".to_string()
}

fn default_locales() -> Vec<String> {
    vec!["en".to_string()]
}

fn default_namespace() -> String {
    "translation".to_string()
}

fn default_functions() -> Vec<String> {
    vec!["t".to_string()]
}

fn default_key_separator() -> String {
    ".".to_string()
}

fn default_ns_separator() -> String {
    ":".to_string()
}

fn default_context_separator() -> String {
    "_".to_string()
}

fn default_plural_separator() -> String {
    "_".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: default_input(),
            output: default_output(),
            locales: default_locales(),
            default_namespace: default_namespace(),
            functions: default_functions(),
            key_separator: default_key_separator(),
            ns_separator: default_ns_separator(),
            context_separator: default_context_separator(),
            plural_separator: default_plural_separator(),
        }
    }
}

impl Config {
    /// Load configuration from a JSON file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Try to load from default config file, or return default config
    pub fn load_or_default<P: AsRef<Path>>(path: Option<P>) -> Result<Self> {
        match path {
            Some(p) => Self::load(p),
            None => {
                let default_path = Path::new("i18next-turbo.json");
                if default_path.exists() {
                    Self::load(default_path)
                } else {
                    Ok(Self::default())
                }
            }
        }
    }
}
