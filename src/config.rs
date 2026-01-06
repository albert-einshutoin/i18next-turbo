use anyhow::{bail, Context, Result};
use glob::Pattern;
use icu_locid::Locale;
use icu_plurals::{PluralCategory, PluralRules};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// Configuration for i18next-turbo
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Glob patterns for input files (e.g., ["src/**/*.tsx", "src/**/*.ts"])
    #[serde(default = "default_input")]
    pub input: Vec<String>,

    /// Output directory for translation files
    #[serde(default = "default_output")]
    pub output: String,

    /// Output format for translation files (json, json5, ...)
    #[serde(default)]
    pub output_format: OutputFormat,

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
    /// Set to false or "" to disable namespace separation
    #[serde(
        default = "default_ns_separator",
        deserialize_with = "deserialize_optional_separator"
    )]
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

    /// Whether to completely disable plural key generation
    /// When true, keys with `count` will not generate `_one`, `_other` etc.
    /// Default: false
    #[serde(default)]
    pub disable_plurals: bool,

    /// Whether to generate base plural forms (key without suffix) alongside plural keys
    /// When true, generates both "item" and "item_one", "item_other"
    /// Default: false
    #[serde(default)]
    pub generate_base_plural_forms: bool,

    /// Whether to extract keys from comments (e.g., // t('key'))
    /// Default: true
    #[serde(default = "default_extract_from_comments")]
    pub extract_from_comments: bool,

    /// Whether to auto-detect plural categories from locale rules
    #[serde(default = "default_use_locale_plural_rules")]
    pub use_locale_plural_rules: bool,

    /// Files/globs to ignore when extracting
    #[serde(default)]
    pub ignore: Vec<String>,

    /// Glob patterns for keys that should always be preserved when pruning
    #[serde(default)]
    pub preserve_patterns: Vec<String>,

    /// Whether to remove keys that were not found in source files (default: true)
    #[serde(default = "default_remove_unused_keys")]
    pub remove_unused_keys: bool,

    /// Default value to use when no explicit defaultValue is provided
    #[serde(default)]
    pub default_value: Option<String>,

    /// Names of Trans components to detect
    #[serde(default = "default_trans_components")]
    pub trans_components: Vec<String>,

    /// HTML tags that should be preserved inside Trans components
    #[serde(default = "default_trans_keep_nodes")]
    pub trans_keep_basic_html_nodes_for: Vec<String>,

    /// Type generation configuration
    #[serde(default)]
    pub types: TypesConfig,

    /// Locize integration settings
    #[serde(default)]
    pub locize: Option<LocizeConfig>,

    /// Primary language for type generation and sync operations
    /// When not set, the first locale in the `locales` array is used
    #[serde(default)]
    pub primary_language: Option<String>,

    /// JSON indentation setting
    /// Examples: 2 (spaces), 4 (spaces), "\t" (tab)
    /// When not set, existing file's indentation is preserved or defaults to 2 spaces
    #[serde(default)]
    pub indentation: Option<Indentation>,
}

/// Optional separator configuration
/// Supports both string (e.g., ":") and boolean false (disabled) formats
/// When false is provided, it's converted to an empty string to disable the separator
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSeparator(pub String);

impl OptionalSeparator {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> serde::Deserialize<'de> for OptionalSeparator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct OptionalSeparatorVisitor;

        impl<'de> Visitor<'de> for OptionalSeparatorVisitor {
            type Value = OptionalSeparator;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or boolean false")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v {
                    Err(E::custom("separator cannot be true, use a string or false"))
                } else {
                    // false means disabled (empty string)
                    Ok(OptionalSeparator(String::new()))
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OptionalSeparator(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OptionalSeparator(v))
            }
        }

        deserializer.deserialize_any(OptionalSeparatorVisitor)
    }
}

impl serde::Serialize for OptionalSeparator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.is_empty() {
            serializer.serialize_bool(false)
        } else {
            serializer.serialize_str(&self.0)
        }
    }
}

impl Default for OptionalSeparator {
    fn default() -> Self {
        Self(String::new())
    }
}

/// JSON indentation configuration
/// Supports both numeric (spaces) and string (e.g., "\t") formats
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indentation {
    /// Number of spaces for indentation
    Spaces(usize),
    /// Custom indentation string (e.g., "\t")
    Custom(String),
}

impl Indentation {
    /// Convert to indentation string
    pub fn to_string(&self) -> String {
        match self {
            Indentation::Spaces(n) => " ".repeat(*n),
            Indentation::Custom(s) => s.clone(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Indentation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct IndentationVisitor;

        impl<'de> Visitor<'de> for IndentationVisitor {
            type Value = Indentation;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a positive integer or a string")
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v >= 0 {
                    Ok(Indentation::Spaces(v as usize))
                } else {
                    Err(E::custom("indentation must be non-negative"))
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Indentation::Spaces(v as usize))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Indentation::Custom(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Indentation::Custom(v))
            }
        }

        deserializer.deserialize_any(IndentationVisitor)
    }
}

impl serde::Serialize for Indentation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Indentation::Spaces(n) => serializer.serialize_u64(*n as u64),
            Indentation::Custom(s) => serializer.serialize_str(s),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    Json,
    Json5,
    #[serde(alias = "js")]
    JsEsm,
    JsCjs,
    Ts,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Json => "json",
            OutputFormat::Json5 => "json5",
            OutputFormat::JsEsm | OutputFormat::JsCjs => "js",
            OutputFormat::Ts => "ts",
        }
    }

    pub fn parse_str(value: &str) -> Result<Self> {
        match value.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "json5" => Ok(OutputFormat::Json5),
            "js" | "js-esm" => Ok(OutputFormat::JsEsm),
            "js-cjs" => Ok(OutputFormat::JsCjs),
            "ts" => Ok(OutputFormat::Ts),
            other => bail!(
                "Configuration error: unsupported outputFormat '{}'. Supported: json, json5, js, js-esm, js-cjs, ts",
                other
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluralConfig {
    pub separator: String,
    pub suffixes: Vec<String>,
    /// Whether to generate base key alongside plural keys
    pub generate_base: bool,
    /// Context separator (e.g., "_" for "friend_male")
    pub context_separator: String,
}

impl Default for PluralConfig {
    fn default() -> Self {
        Self {
            separator: "_".to_string(),
            suffixes: vec!["one".to_string(), "other".to_string()],
            generate_base: false,
            context_separator: "_".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TypesConfig {
    pub output: Option<String>,
    pub default_locale: Option<String>,
    pub locales_dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocizeConfig {
    pub project_id: String,
    pub api_key: Option<String>,
    pub version: Option<String>,
    pub source_language: Option<String>,
    pub namespaces: Option<Vec<String>>,
}

#[cfg(feature = "napi")]
use napi_derive::napi;

#[cfg(feature = "napi")]
#[napi(object)]
#[allow(non_snake_case)]
pub struct NapiConfig {
    pub input: Option<Vec<String>>,
    pub output: Option<String>,
    pub outputFormat: Option<String>,
    pub locales: Option<Vec<String>>,
    pub defaultNamespace: Option<String>,
    pub functions: Option<Vec<String>>,
    pub keySeparator: Option<String>,
    pub nsSeparator: Option<String>,
    pub contextSeparator: Option<String>,
    pub pluralSeparator: Option<String>,
    pub pluralSuffixes: Option<Vec<String>>,
    pub disablePlurals: Option<bool>,
    pub generateBasePluralForms: Option<bool>,
    pub extractFromComments: Option<bool>,
    pub useLocalePluralRules: Option<bool>,
    pub ignore: Option<Vec<String>>,
    pub preservePatterns: Option<Vec<String>>,
    pub removeUnusedKeys: Option<bool>,
    pub defaultValue: Option<String>,
    pub types: Option<NapiTypesConfig>,
    pub locize: Option<NapiLocizeConfig>,
    pub primaryLanguage: Option<String>,
    /// Indentation: number (spaces) or string (e.g., "\t")
    pub indentation: Option<NapiIndentation>,
}

/// NAPI-compatible indentation type
/// Can be either a number (spaces) or a string (custom indentation)
#[cfg(feature = "napi")]
#[napi(object)]
pub struct NapiIndentation {
    /// Number of spaces (mutually exclusive with `custom`)
    pub spaces: Option<u32>,
    /// Custom indentation string (mutually exclusive with `spaces`)
    pub custom: Option<String>,
}

#[cfg(feature = "napi")]
impl From<NapiIndentation> for Indentation {
    fn from(value: NapiIndentation) -> Self {
        if let Some(spaces) = value.spaces {
            Indentation::Spaces(spaces as usize)
        } else if let Some(custom) = value.custom {
            Indentation::Custom(custom)
        } else {
            Indentation::Spaces(2) // default
        }
    }
}

/// Deserialize a separator that can be either a string or `false` (disabled)
/// When `false` is provided, it's converted to an empty string
fn deserialize_optional_separator<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct OptionalSeparatorVisitor;

    impl<'de> Visitor<'de> for OptionalSeparatorVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or boolean false")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v {
                Err(E::custom("separator cannot be true, use a string or false"))
            } else {
                // false means disabled (empty string)
                Ok(String::new())
            }
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v)
        }
    }

    deserializer.deserialize_any(OptionalSeparatorVisitor)
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

fn default_remove_unused_keys() -> bool {
    true
}

fn default_trans_components() -> Vec<String> {
    vec!["Trans".to_string()]
}

fn default_trans_keep_nodes() -> Vec<String> {
    vec!["br".to_string(), "strong".to_string(), "i".to_string()]
}

fn default_types_output() -> String {
    "src/@types/i18next.d.ts".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: default_input(),
            output: default_output(),
            output_format: OutputFormat::default(),
            locales: default_locales(),
            default_namespace: default_namespace(),
            functions: default_functions(),
            key_separator: default_key_separator(),
            ns_separator: default_ns_separator(),
            context_separator: default_context_separator(),
            plural_separator: default_plural_separator(),
            plural_suffixes: default_plural_suffixes(),
            disable_plurals: false,
            generate_base_plural_forms: false,
            extract_from_comments: default_extract_from_comments(),
            use_locale_plural_rules: default_use_locale_plural_rules(),
            ignore: Vec::new(),
            preserve_patterns: Vec::new(),
            remove_unused_keys: default_remove_unused_keys(),
            default_value: None,
            types: TypesConfig::default(),
            trans_components: default_trans_components(),
            trans_keep_basic_html_nodes_for: default_trans_keep_nodes(),
            locize: None,
            primary_language: None,
            indentation: None,
        }
    }
}

impl Config {
    pub fn plural_config(&self) -> PluralConfig {
        // If plurals are disabled, return empty suffixes
        if self.disable_plurals {
            return PluralConfig {
                separator: self.plural_separator.clone(),
                suffixes: Vec::new(),
                generate_base: false,
                context_separator: self.context_separator.clone(),
            };
        }

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
            generate_base: self.generate_base_plural_forms,
            context_separator: self.context_separator.clone(),
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

        // Validate ignore patterns
        for pattern in &self.ignore {
            if pattern.trim().is_empty() {
                bail!(
                    "Configuration error: empty pattern found in 'ignore'.\n\
                     Remove empty entries or provide a glob like \"**/*.test.tsx\"."
                );
            }
            if let Err(e) = Pattern::new(pattern) {
                bail!(
                    "Configuration error: invalid glob in 'ignore': '{}'.\n\
                     Glob error: {}",
                    pattern,
                    e
                );
            }
        }

        // Validate preservePatterns entries
        for pattern in &self.preserve_patterns {
            if pattern.trim().is_empty() {
                bail!(
                    "Configuration error: empty entry found in 'preservePatterns'.\n\
                     Example: \"common:*\" or \"auth.login.*\""
                );
            }
            if let Err(e) = Pattern::new(pattern) {
                bail!(
                    "Configuration error: invalid glob in 'preservePatterns': '{}'.\n\
                     Glob error: {}",
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

        if let Some(locize) = &self.locize {
            if locize.project_id.trim().is_empty() {
                bail!(
                    "Configuration error: 'locize.projectId' must be a non-empty string when Locize integration is configured."
                );
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
            input: config.input.unwrap_or_else(|| defaults.input.clone()),
            output: config.output.unwrap_or_else(|| defaults.output.clone()),
            output_format: config
                .outputFormat
                .as_deref()
                .map(OutputFormat::from_str)
                .transpose()?
                .unwrap_or(defaults.output_format),
            locales: config.locales.unwrap_or_else(|| defaults.locales.clone()),
            default_namespace: config
                .defaultNamespace
                .unwrap_or_else(|| defaults.default_namespace.clone()),
            functions: config
                .functions
                .unwrap_or_else(|| defaults.functions.clone()),
            key_separator: config
                .keySeparator
                .unwrap_or_else(|| defaults.key_separator.clone()),
            ns_separator: config
                .nsSeparator
                .unwrap_or_else(|| defaults.ns_separator.clone()),
            context_separator: config
                .contextSeparator
                .unwrap_or_else(|| defaults.context_separator.clone()),
            plural_separator: config
                .pluralSeparator
                .unwrap_or_else(|| defaults.plural_separator.clone()),
            plural_suffixes: config
                .pluralSuffixes
                .unwrap_or_else(|| defaults.plural_suffixes.clone()),
            disable_plurals: config.disablePlurals.unwrap_or(false),
            generate_base_plural_forms: config.generateBasePluralForms.unwrap_or(false),
            extract_from_comments: config
                .extractFromComments
                .unwrap_or(defaults.extract_from_comments),
            use_locale_plural_rules: config
                .useLocalePluralRules
                .unwrap_or(default_use_locale_plural_rules()),
            ignore: config.ignore.unwrap_or_else(|| defaults.ignore.clone()),
            preserve_patterns: config
                .preservePatterns
                .unwrap_or_else(|| defaults.preserve_patterns.clone()),
            remove_unused_keys: config
                .removeUnusedKeys
                .unwrap_or(default_remove_unused_keys()),
            default_value: config
                .defaultValue
                .or_else(|| defaults.default_value.clone()),
            trans_components: default_trans_components(),
            trans_keep_basic_html_nodes_for: default_trans_keep_nodes(),
            types: config.types.map(TypesConfig::from).unwrap_or_default(),
            locize: config.locize.and_then(|locize_cfg| {
                locize_cfg.projectId.map(|project_id| LocizeConfig {
                    project_id,
                    api_key: locize_cfg.apiKey,
                    version: locize_cfg.version,
                    source_language: locize_cfg.sourceLanguage,
                    namespaces: locize_cfg.namespaces,
                })
            }),
            primary_language: config.primaryLanguage,
            indentation: config.indentation.map(Indentation::from),
        };
        config.validate()?;
        Ok(config)
    }
}

impl Config {
    pub fn output_format(&self) -> OutputFormat {
        self.output_format
    }

    pub fn output_extension(&self) -> &'static str {
        self.output_format.extension()
    }

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

    /// Get the primary language for this configuration
    /// Returns `primary_language` if set, otherwise the first locale
    pub fn primary_language(&self) -> &str {
        self.primary_language
            .as_deref()
            .unwrap_or_else(|| self.locales.first().map(|s| s.as_str()).unwrap_or("en"))
    }

    /// Get the indentation string for JSON output
    /// Returns the configured indentation or None if not set
    pub fn indentation_string(&self) -> Option<String> {
        self.indentation.as_ref().map(|i| i.to_string())
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
#[napi(object)]
pub struct NapiLocizeConfig {
    pub projectId: Option<String>,
    pub apiKey: Option<String>,
    pub version: Option<String>,
    pub sourceLanguage: Option<String>,
    pub namespaces: Option<Vec<String>>,
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

    #[test]
    fn plural_config_returns_empty_when_disable_plurals_is_true() {
        let mut config = Config::default();
        config.disable_plurals = true;
        let plural = config.plural_config();
        assert!(plural.suffixes.is_empty());
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
