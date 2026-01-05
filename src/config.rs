use anyhow::{bail, Context, Result};
use icu_locid::Locale;
use icu_plurals::{PluralCategory, PluralRules};
use serde::Deserialize;
use std::collections::BTreeSet;
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

    /// Plural suffixes to generate (e.g., ["one", "other"] for English)
    /// Supported values: zero, one, two, few, many, other
    /// Examples:
    ///   - English: ["one", "other"]
    ///   - Arabic: ["zero", "one", "two", "few", "many", "other"]
    ///   - Russian: ["one", "few", "many", "other"]
    ///   - Japanese: ["other"] (no plural forms)
    #[serde(default = "default_plural_suffixes")]
    pub plural_suffixes: Vec<String>,

    /// Whether to extract keys from comments (e.g., // t('key'))
    /// Default: true
    #[serde(default = "default_extract_from_comments")]
    pub extract_from_comments: bool,

    /// Whether to auto-detect plural categories from locale rules
    #[serde(default = "default_use_locale_plural_rules")]
    pub use_locale_plural_rules: bool,

    /// Type generation configuration
    #[serde(default)]
    pub types: TypesConfig,
}

#[derive(Debug, Clone)]
pub struct PluralConfig {
    pub separator: String,
    pub suffixes: Vec<String>,
}

impl Default for PluralConfig {
    fn default() -> Self {
        Self {
            separator: "_".to_string(),
            suffixes: vec!["one".to_string(), "other".to_string()],
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TypesConfig {
    pub output: Option<String>,
    pub default_locale: Option<String>,
    pub locales_dir: Option<String>,
}

#[cfg(feature = "napi")]
use napi_derive::napi;

#[cfg(feature = "napi")]
#[napi(object)]
#[allow(non_snake_case)]
pub struct NapiConfig {
    pub input: Option<Vec<String>>,
    pub output: Option<String>,
    pub locales: Option<Vec<String>>,
    pub defaultNamespace: Option<String>,
    pub functions: Option<Vec<String>>,
    pub keySeparator: Option<String>,
    pub nsSeparator: Option<String>,
    pub contextSeparator: Option<String>,
    pub pluralSeparator: Option<String>,
    pub pluralSuffixes: Option<Vec<String>>,
    pub extractFromComments: Option<bool>,
    pub useLocalePluralRules: Option<bool>,
    pub types: Option<NapiTypesConfig>,
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

fn default_plural_suffixes() -> Vec<String> {
    vec!["one".to_string(), "other".to_string()]
}

fn default_extract_from_comments() -> bool {
    true
}

fn default_use_locale_plural_rules() -> bool {
    true
}

fn default_types_output() -> String {
    "src/@types/i18next.d.ts".to_string()
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
            plural_suffixes: default_plural_suffixes(),
            extract_from_comments: default_extract_from_comments(),
            use_locale_plural_rules: default_use_locale_plural_rules(),
            types: TypesConfig::default(),
        }
    }
}

impl Config {
    pub fn plural_config(&self) -> PluralConfig {
        let suffixes = if self.use_locale_plural_rules {
            compute_plural_suffixes_from_locales(&self.locales)
        } else {
            self.plural_suffixes.clone()
        };

        let mut final_suffixes = if suffixes.is_empty() {
            vec!["one".to_string(), "other".to_string()]
        } else {
            suffixes
        };

        if !final_suffixes.iter().any(|s| s == "other") {
            final_suffixes.push("other".to_string());
        }

        PluralConfig {
            separator: self.plural_separator.clone(),
            suffixes: final_suffixes,
        }
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Check locales is not empty
        if self.locales.is_empty() {
            bail!(
                "Configuration error: 'locales' must contain at least one locale.\n\
                 Example: \"locales\": [\"en\", \"ja\"]"
            );
        }

        // Check for empty locale strings
        for (i, locale) in self.locales.iter().enumerate() {
            if locale.trim().is_empty() {
                bail!(
                    "Configuration error: 'locales[{}]' is empty.\n\
                     Each locale must be a non-empty string like \"en\" or \"ja\".",
                    i
                );
            }
        }

        // Check input patterns are not empty
        if self.input.is_empty() {
            bail!(
                "Configuration error: 'input' must contain at least one glob pattern.\n\
                 Example: \"input\": [\"src/**/*.tsx\", \"src/**/*.ts\"]"
            );
        }

        // Validate each input pattern is a valid glob
        for pattern in &self.input {
            if pattern.trim().is_empty() {
                bail!(
                    "Configuration error: empty input pattern found.\n\
                     Each input pattern must be a non-empty glob like \"src/**/*.tsx\"."
                );
            }
            // Try to compile the glob pattern to catch syntax errors early
            if let Err(e) = glob::Pattern::new(pattern) {
                bail!(
                    "Configuration error: invalid glob pattern '{}'.\n\
                     Glob error: {}\n\
                     Example of valid patterns: \"src/**/*.tsx\", \"lib/*.js\"",
                    pattern,
                    e
                );
            }
        }

        // Check output is not empty
        if self.output.trim().is_empty() {
            bail!(
                "Configuration error: 'output' must be a non-empty directory path.\n\
                 Example: \"output\": \"locales\""
            );
        }

        // Check for potentially problematic output path characters
        let invalid_chars = ['<', '>', '|', '\0'];
        for c in invalid_chars {
            if self.output.contains(c) {
                bail!(
                    "Configuration error: 'output' contains invalid character '{}'.\n\
                     Please use a valid directory path.",
                    c
                );
            }
        }

        // Check functions is not empty
        if self.functions.is_empty() {
            bail!(
                "Configuration error: 'functions' must contain at least one function name.\n\
                 Example: \"functions\": [\"t\", \"i18n.t\"]"
            );
        }

        // Check default_namespace is not empty
        if self.default_namespace.trim().is_empty() {
            bail!(
                "Configuration error: 'defaultNamespace' must be a non-empty string.\n\
                 Example: \"defaultNamespace\": \"translation\""
            );
        }

        if let Some(output) = &self.types.output {
            if output.trim().is_empty() {
                bail!("Configuration error: 'types.output' must be a non-empty string when specified.");
            }
        }

        Ok(())
    }

    /// Load configuration from a JSON file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a JSON string
    pub fn from_json_string(json_str: &str) -> Result<Self> {
        let config: Config =
            serde_json::from_str(json_str).with_context(|| "Failed to parse config JSON string")?;
        config.validate()?;
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
                    // Default config is pre-validated, no need to validate again
                    Ok(Self::default())
                }
            }
        }
    }

    #[cfg(feature = "napi")]
    pub fn from_napi(config: NapiConfig) -> Result<Self> {
        let defaults = Config::default();
        let config = Config {
            input: config.input.unwrap_or(defaults.input),
            output: config.output.unwrap_or(defaults.output),
            locales: config.locales.unwrap_or(defaults.locales),
            default_namespace: config
                .defaultNamespace
                .unwrap_or(defaults.default_namespace),
            functions: config.functions.unwrap_or(defaults.functions),
            key_separator: config.keySeparator.unwrap_or(defaults.key_separator),
            ns_separator: config.nsSeparator.unwrap_or(defaults.ns_separator),
            context_separator: config
                .contextSeparator
                .unwrap_or(defaults.context_separator),
            plural_separator: config.pluralSeparator.unwrap_or(defaults.plural_separator),
            plural_suffixes: config.pluralSuffixes.unwrap_or(defaults.plural_suffixes),
            extract_from_comments: config
                .extractFromComments
                .unwrap_or(defaults.extract_from_comments),
            use_locale_plural_rules: config
                .useLocalePluralRules
                .unwrap_or(default_use_locale_plural_rules()),
            types: config.types.map(TypesConfig::from).unwrap_or_default(),
        };
        config.validate()?;
        Ok(config)
    }
}

impl Config {
    pub fn types_output_path(&self) -> String {
        self.types
            .output
            .clone()
            .unwrap_or_else(default_types_output)
    }

    pub fn types_default_locale(&self) -> Option<String> {
        self.types.default_locale.clone()
    }

    pub fn types_locales_dir(&self) -> Option<String> {
        self.types.locales_dir.clone()
    }

    pub fn default_types_output() -> String {
        default_types_output()
    }
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct NapiTypesConfig {
    pub output: Option<String>,
    pub defaultLocale: Option<String>,
    pub localesDir: Option<String>,
}

#[cfg(feature = "napi")]
impl From<NapiTypesConfig> for TypesConfig {
    fn from(value: NapiTypesConfig) -> Self {
        Self {
            output: value.output,
            default_locale: value.defaultLocale,
            locales_dir: value.localesDir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_output_defaults_to_standard_path() {
        let config = Config::default();
        assert_eq!(config.types_output_path(), Config::default_types_output());
    }

    #[test]
    fn types_output_can_be_overridden_via_json() {
        let json = r#"{ "types": { "output": "generated/types.d.ts" } }"#;
        let config = Config::from_json_string(json).unwrap();
        assert_eq!(config.types_output_path(), "generated/types.d.ts");
    }

    #[test]
    fn plural_config_uses_locale_rules_when_enabled() {
        let mut config = Config::default();
        config.locales = vec!["ru".to_string()];
        config.use_locale_plural_rules = true;
        let plural = config.plural_config();
        assert!(plural.suffixes.contains(&"few".to_string()));
        assert!(plural.suffixes.contains(&"many".to_string()));
        assert!(plural.suffixes.contains(&"one".to_string()));
        assert!(plural.suffixes.contains(&"other".to_string()));
    }

    #[test]
    fn plural_config_uses_explicit_suffixes_when_disabled() {
        let mut config = Config::default();
        config.use_locale_plural_rules = false;
        config.plural_suffixes = vec!["zero".to_string(), "other".to_string()];
        let plural = config.plural_config();
        assert_eq!(
            plural.suffixes,
            vec!["zero".to_string(), "other".to_string()]
        );
    }
}

fn compute_plural_suffixes_from_locales(locales: &[String]) -> Vec<String> {
    let mut categories = BTreeSet::new();

    for locale in locales {
        if let Some(locale_categories) = categories_for_locale(locale) {
            for cat in locale_categories {
                categories.insert(cat);
            }
        }
    }

    if categories.is_empty() {
        return vec!["one".to_string(), "other".to_string()];
    }

    if !categories.contains("other") {
        categories.insert("other".to_string());
    }

    categories.into_iter().collect()
}

fn categories_for_locale(locale: &str) -> Option<Vec<String>> {
    let trimmed = locale.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed: Locale = trimmed.parse().ok()?;
    let data_locale = parsed.into();
    let rules = PluralRules::try_new_cardinal(&data_locale).ok()?;
    let supported: Vec<PluralCategory> = rules.categories().collect();

    let mut result = Vec::new();
    for category in [
        PluralCategory::Zero,
        PluralCategory::One,
        PluralCategory::Two,
        PluralCategory::Few,
        PluralCategory::Many,
        PluralCategory::Other,
    ] {
        if supported.contains(&category) {
            result.push(plural_category_to_str(category).to_string());
        }
    }
    Some(result)
}

fn plural_category_to_str(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}
