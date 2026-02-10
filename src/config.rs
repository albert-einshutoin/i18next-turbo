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

    /// Default namespace (`false`/empty with `nsSeparator: false` enables namespace-less mode)
    #[serde(
        default = "default_namespace",
        deserialize_with = "deserialize_optional_default_namespace"
    )]
    pub default_namespace: String,

    /// Function names to extract (e.g., ["t", "i18n.t"])
    #[serde(default = "default_functions")]
    pub functions: Vec<String>,

    /// Hook-like function names that return a translation function (t)
    /// Supports string entries or objects with custom argument positions.
    #[serde(default = "default_use_translation_names")]
    pub use_translation_names: Vec<UseTranslationName>,

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

    /// Keep context/plural variants when base key exists (e.g., friend -> friend_male)
    #[serde(default)]
    pub preserve_context_variants: bool,

    /// Whether to remove keys that were not found in source files (default: true)
    #[serde(default = "default_remove_unused_keys")]
    pub remove_unused_keys: bool,

    /// Merge all namespaces into a single locale file
    #[serde(default)]
    pub merge_namespaces: bool,

    /// Default value to use when no explicit defaultValue is provided
    #[serde(default)]
    pub default_value: Option<String>,

    /// Names of Trans components to detect
    #[serde(default = "default_trans_components")]
    pub trans_components: Vec<String>,

    /// HTML tags that should be preserved inside Trans components
    #[serde(default = "default_trans_keep_nodes")]
    pub trans_keep_basic_html_nodes_for: Vec<String>,

    /// Prefix for nested translation calls inside strings (default: "$t(")
    #[serde(default = "default_nesting_prefix")]
    pub nesting_prefix: String,

    /// Suffix for nested translation calls inside strings (default: ")")
    #[serde(default = "default_nesting_suffix")]
    pub nesting_suffix: String,

    /// Separator between nested key and nested options (default: ",")
    #[serde(default = "default_nesting_options_separator")]
    pub nesting_options_separator: String,

    /// Interpolation prefix used in placeholder serialization (default: "{{")
    #[serde(default = "default_interpolation_prefix")]
    pub interpolation_prefix: String,

    /// Interpolation suffix used in placeholder serialization (default: "}}")
    #[serde(default = "default_interpolation_suffix")]
    pub interpolation_suffix: String,

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

    /// Explicit secondary languages for sync operations
    /// When not set, `locales` except primary language are used
    #[serde(default)]
    pub secondary_languages: Option<Vec<String>>,

    /// JSON indentation setting
    /// Examples: 2 (spaces), 4 (spaces), "\t" (tab)
    /// When not set, existing file's indentation is preserved or defaults to 2 spaces
    #[serde(default)]
    pub indentation: Option<Indentation>,

    /// Lint behavior configuration
    #[serde(default)]
    pub lint: LintConfig,

    /// Log level (`error`, `warn`, `info`, `debug`)
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Optional separator configuration
/// Supports both string (e.g., ":") and boolean false (disabled) formats
/// When false is provided, it's converted to an empty string to disable the separator
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OptionalSeparator(pub String);

impl OptionalSeparator {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum UseTranslationName {
    Name(String),
    Detailed(UseTranslationNameDetails),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UseTranslationNameDetails {
    pub name: String,
    #[serde(default = "default_ns_arg")]
    pub ns_arg: usize,
    #[serde(default = "default_key_prefix_arg")]
    pub key_prefix_arg: usize,
}

impl UseTranslationName {
    pub fn name(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Detailed(details) => details.name.as_str(),
        }
    }

    pub fn ns_arg(&self) -> usize {
        match self {
            Self::Name(_) => default_ns_arg(),
            Self::Detailed(details) => details.ns_arg,
        }
    }

    pub fn key_prefix_arg(&self) -> usize {
        match self {
            Self::Name(_) => default_key_prefix_arg(),
            Self::Detailed(details) => details.key_prefix_arg,
        }
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

/// JSON indentation configuration
/// Supports both numeric (spaces) and string (e.g., "\t") formats
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indentation {
    /// Number of spaces for indentation
    Spaces(usize),
    /// Custom indentation string (e.g., "\t")
    Custom(String),
}

impl std::fmt::Display for Indentation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Indentation::Spaces(n) => write!(f, "{}", " ".repeat(*n)),
            Indentation::Custom(s) => write!(f, "{}", s),
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
    pub input: Option<Vec<String>>,
    pub output: Option<String>,
    pub resources_file: Option<String>,
    pub enable_selector: Option<EnableSelector>,
    pub default_locale: Option<String>,
    pub locales_dir: Option<String>,
    pub indentation: Option<Indentation>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum EnableSelector {
    Bool(bool),
    Mode(String),
}

impl EnableSelector {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Bool(v) => *v,
            Self::Mode(mode) => mode == "optimize",
        }
    }

    pub fn optimize(&self) -> bool {
        matches!(self, Self::Mode(mode) if mode == "optimize")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LintConfig {
    #[serde(default = "default_lint_ignored_attributes")]
    pub ignored_attributes: Vec<String>,
    #[serde(default = "default_lint_ignored_tags")]
    pub ignored_tags: Vec<String>,
    #[serde(default = "default_lint_accepted_attributes")]
    pub accepted_attributes: Vec<String>,
    #[serde(default = "default_lint_accepted_tags")]
    pub accepted_tags: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            ignored_attributes: default_lint_ignored_attributes(),
            ignored_tags: default_lint_ignored_tags(),
            accepted_attributes: default_lint_accepted_attributes(),
            accepted_tags: default_lint_accepted_tags(),
            ignore: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocizeConfig {
    pub project_id: String,
    pub api_key: Option<String>,
    pub version: Option<String>,
    pub source_language: Option<String>,
    pub namespaces: Option<Vec<String>>,
    pub update_values: Option<bool>,
    pub source_language_only: Option<bool>,
    pub compare_modification_time: Option<bool>,
    pub cdn_type: Option<String>,
    pub dry_run: Option<bool>,
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
    pub useTranslationNames: Option<Vec<String>>,
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
    pub preserveContextVariants: Option<bool>,
    pub removeUnusedKeys: Option<bool>,
    pub mergeNamespaces: Option<bool>,
    pub defaultValue: Option<String>,
    pub transComponents: Option<Vec<String>>,
    pub transKeepBasicHtmlNodesFor: Option<Vec<String>>,
    pub nestingPrefix: Option<String>,
    pub nestingSuffix: Option<String>,
    pub nestingOptionsSeparator: Option<String>,
    pub interpolationPrefix: Option<String>,
    pub interpolationSuffix: Option<String>,
    pub types: Option<NapiTypesConfig>,
    pub locize: Option<NapiLocizeConfig>,
    pub primaryLanguage: Option<String>,
    pub secondaryLanguages: Option<Vec<String>>,
    /// Indentation: number (spaces) or string (e.g., "\t")
    pub indentation: Option<NapiIndentation>,
    pub logLevel: Option<String>,
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

/// Deserialize defaultNamespace that can be either a string or `false` (namespace-less mode)
fn deserialize_optional_default_namespace<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct OptionalDefaultNamespaceVisitor;

    impl<'de> Visitor<'de> for OptionalDefaultNamespaceVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or boolean false")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v {
                Err(E::custom(
                    "defaultNamespace cannot be true, use a string or false",
                ))
            } else {
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

    deserializer.deserialize_any(OptionalDefaultNamespaceVisitor)
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

fn default_use_translation_names() -> Vec<UseTranslationName> {
    vec![
        UseTranslationName::Name("useTranslation".to_string()),
        UseTranslationName::Name("getT".to_string()),
        UseTranslationName::Name("useT".to_string()),
    ]
}

fn default_ns_arg() -> usize {
    0
}

fn default_key_prefix_arg() -> usize {
    1
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

fn default_nesting_prefix() -> String {
    "$t(".to_string()
}

fn default_nesting_suffix() -> String {
    ")".to_string()
}

fn default_nesting_options_separator() -> String {
    ",".to_string()
}

fn default_interpolation_prefix() -> String {
    "{{".to_string()
}

fn default_interpolation_suffix() -> String {
    "}}".to_string()
}

fn default_types_output() -> String {
    "src/@types/i18next.d.ts".to_string()
}

fn default_lint_ignored_attributes() -> Vec<String> {
    Vec::new()
}

fn default_lint_ignored_tags() -> Vec<String> {
    vec![
        "script".to_string(),
        "style".to_string(),
        "code".to_string(),
        "pre".to_string(),
    ]
}

fn default_lint_accepted_attributes() -> Vec<String> {
    vec![
        "alt".to_string(),
        "title".to_string(),
        "placeholder".to_string(),
        "aria-label".to_string(),
        "aria-description".to_string(),
    ]
}

fn default_lint_accepted_tags() -> Vec<String> {
    vec![
        "p".to_string(),
        "span".to_string(),
        "div".to_string(),
        "button".to_string(),
        "label".to_string(),
        "img".to_string(),
    ]
}

fn default_log_level() -> String {
    "info".to_string()
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
            use_translation_names: default_use_translation_names(),
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
            preserve_context_variants: false,
            remove_unused_keys: default_remove_unused_keys(),
            merge_namespaces: false,
            default_value: None,
            types: TypesConfig::default(),
            trans_components: default_trans_components(),
            trans_keep_basic_html_nodes_for: default_trans_keep_nodes(),
            nesting_prefix: default_nesting_prefix(),
            nesting_suffix: default_nesting_suffix(),
            nesting_options_separator: default_nesting_options_separator(),
            interpolation_prefix: default_interpolation_prefix(),
            interpolation_suffix: default_interpolation_suffix(),
            locize: None,
            primary_language: None,
            secondary_languages: None,
            indentation: None,
            lint: LintConfig::default(),
            log_level: default_log_level(),
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

        if let Some(secondary_languages) = &self.secondary_languages {
            for (i, locale) in secondary_languages.iter().enumerate() {
                if locale.trim().is_empty() {
                    bail!(
                        "Configuration error: 'secondaryLanguages[{}]' is empty.\n\
                         Each locale must be a non-empty string like \"ja\".",
                        i
                    );
                }
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

        for pattern in &self.lint.ignore {
            if pattern.trim().is_empty() {
                bail!(
                    "Configuration error: empty pattern found in 'lint.ignore'.\n\
                     Remove empty entries or provide a glob like \"**/*.stories.tsx\"."
                );
            }
            if let Err(e) = Pattern::new(pattern) {
                bail!(
                    "Configuration error: invalid glob in 'lint.ignore': '{}'.\n\
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

        for (i, hook) in self.use_translation_names.iter().enumerate() {
            if hook.name().trim().is_empty() {
                bail!(
                    "Configuration error: 'useTranslationNames[{}]' must contain a non-empty function name.",
                    i
                );
            }
        }

        // defaultNamespace can be empty only in namespace-less mode (with nsSeparator disabled)
        if self.default_namespace.trim().is_empty() && !self.ns_separator.is_empty() {
            bail!(
                "Configuration error: 'defaultNamespace' is empty but 'nsSeparator' is enabled.\n\
                 Use namespace-less mode with both: \"defaultNamespace\": false and \"nsSeparator\": false."
            );
        }

        if self.nesting_prefix.is_empty() || self.nesting_suffix.is_empty() {
            bail!(
                "Configuration error: 'nestingPrefix' and 'nestingSuffix' must be non-empty strings."
            );
        }
        if self.interpolation_prefix.is_empty() || self.interpolation_suffix.is_empty() {
            bail!(
                "Configuration error: 'interpolationPrefix' and 'interpolationSuffix' must be non-empty strings."
            );
        }

        if let Some(output) = &self.types.output {
            if output.trim().is_empty() {
                bail!("Configuration error: 'types.output' must be a non-empty string when specified.");
            }
        }
        if let Some(resources_file) = &self.types.resources_file {
            if resources_file.trim().is_empty() {
                bail!("Configuration error: 'types.resourcesFile' must be a non-empty string when specified.");
            }
        }
        if let Some(input_patterns) = &self.types.input {
            for (i, pattern) in input_patterns.iter().enumerate() {
                if pattern.trim().is_empty() {
                    bail!(
                        "Configuration error: 'types.input[{}]' must be a non-empty glob pattern.",
                        i
                    );
                }
            }
        }
        if let Some(enable_selector) = &self.types.enable_selector {
            if let EnableSelector::Mode(mode) = enable_selector {
                if mode != "optimize" {
                    bail!(
                        "Configuration error: 'types.enableSelector' must be true, false, or 'optimize'."
                    );
                }
            }
        }

        if let Some(locize) = &self.locize {
            if locize.project_id.trim().is_empty() {
                bail!(
                    "Configuration error: 'locize.projectId' must be a non-empty string when Locize integration is configured."
                );
            }
            if let Some(cdn_type) = &locize.cdn_type {
                if cdn_type != "standard" && cdn_type != "pro" {
                    bail!("Configuration error: 'locize.cdnType' must be 'standard' or 'pro'.");
                }
            }
        }

        match self.log_level.as_str() {
            "error" | "warn" | "info" | "debug" => {}
            _ => bail!("Configuration error: 'logLevel' must be one of: error, warn, info, debug."),
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
            use_translation_names: config
                .useTranslationNames
                .map(|names| names.into_iter().map(UseTranslationName::Name).collect())
                .unwrap_or_else(|| defaults.use_translation_names.clone()),
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
            preserve_context_variants: config
                .preserveContextVariants
                .unwrap_or(defaults.preserve_context_variants),
            remove_unused_keys: config
                .removeUnusedKeys
                .unwrap_or(default_remove_unused_keys()),
            merge_namespaces: config.mergeNamespaces.unwrap_or(defaults.merge_namespaces),
            default_value: config
                .defaultValue
                .or_else(|| defaults.default_value.clone()),
            trans_components: config
                .transComponents
                .unwrap_or_else(|| defaults.trans_components.clone()),
            trans_keep_basic_html_nodes_for: config
                .transKeepBasicHtmlNodesFor
                .unwrap_or_else(|| defaults.trans_keep_basic_html_nodes_for.clone()),
            nesting_prefix: config
                .nestingPrefix
                .unwrap_or_else(|| defaults.nesting_prefix.clone()),
            nesting_suffix: config
                .nestingSuffix
                .unwrap_or_else(|| defaults.nesting_suffix.clone()),
            nesting_options_separator: config
                .nestingOptionsSeparator
                .unwrap_or_else(|| defaults.nesting_options_separator.clone()),
            interpolation_prefix: config
                .interpolationPrefix
                .unwrap_or_else(|| defaults.interpolation_prefix.clone()),
            interpolation_suffix: config
                .interpolationSuffix
                .unwrap_or_else(|| defaults.interpolation_suffix.clone()),
            types: config.types.map(TypesConfig::from).unwrap_or_default(),
            locize: config.locize.and_then(|locize_cfg| {
                locize_cfg.projectId.map(|project_id| LocizeConfig {
                    project_id,
                    api_key: locize_cfg.apiKey,
                    version: locize_cfg.version,
                    source_language: locize_cfg.sourceLanguage,
                    namespaces: locize_cfg.namespaces,
                    update_values: locize_cfg.updateValues,
                    source_language_only: locize_cfg.sourceLanguageOnly,
                    compare_modification_time: locize_cfg.compareModificationTime,
                    cdn_type: locize_cfg.cdnType,
                    dry_run: locize_cfg.dryRun,
                })
            }),
            primary_language: config.primaryLanguage,
            secondary_languages: config.secondaryLanguages,
            indentation: config.indentation.map(Indentation::from),
            lint: defaults.lint.clone(),
            log_level: config
                .logLevel
                .unwrap_or_else(|| defaults.log_level.clone()),
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

    pub fn types_input_patterns(&self) -> Option<Vec<String>> {
        self.types.input.clone()
    }

    pub fn types_resources_file(&self) -> Option<String> {
        self.types.resources_file.clone()
    }

    pub fn types_enable_selector(&self) -> Option<EnableSelector> {
        self.types.enable_selector.clone()
    }

    pub fn types_default_locale(&self) -> Option<String> {
        self.types.default_locale.clone()
    }

    pub fn types_locales_dir(&self) -> Option<String> {
        self.types.locales_dir.clone()
    }

    pub fn types_indentation_string(&self) -> Option<String> {
        self.types.indentation.as_ref().map(|i| i.to_string())
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

    pub fn secondary_languages(&self) -> Vec<String> {
        if let Some(explicit) = &self.secondary_languages {
            let primary = self.primary_language();
            return explicit
                .iter()
                .filter(|locale| locale.as_str() != primary)
                .cloned()
                .collect();
        }

        let primary = self.primary_language();
        self.locales
            .iter()
            .filter(|locale| locale.as_str() != primary)
            .cloned()
            .collect()
    }

    pub fn namespace_less_mode(&self) -> bool {
        self.default_namespace.is_empty()
    }

    pub fn effective_default_namespace(&self) -> &str {
        if self.default_namespace.is_empty() {
            "translation"
        } else {
            self.default_namespace.as_str()
        }
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
    pub input: Option<Vec<String>>,
    pub output: Option<String>,
    pub resourcesFile: Option<String>,
    pub enableSelector: Option<String>,
    pub defaultLocale: Option<String>,
    pub localesDir: Option<String>,
    pub indentation: Option<NapiIndentation>,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct NapiLocizeConfig {
    pub projectId: Option<String>,
    pub apiKey: Option<String>,
    pub version: Option<String>,
    pub sourceLanguage: Option<String>,
    pub namespaces: Option<Vec<String>>,
    pub updateValues: Option<bool>,
    pub sourceLanguageOnly: Option<bool>,
    pub compareModificationTime: Option<bool>,
    pub cdnType: Option<String>,
    pub dryRun: Option<bool>,
}

#[cfg(feature = "napi")]
impl From<NapiTypesConfig> for TypesConfig {
    fn from(value: NapiTypesConfig) -> Self {
        Self {
            input: value.input,
            output: value.output,
            resources_file: value.resourcesFile,
            enable_selector: value.enableSelector.and_then(|raw| {
                let normalized = raw.trim().to_lowercase();
                match normalized.as_str() {
                    "true" => Some(EnableSelector::Bool(true)),
                    "false" => Some(EnableSelector::Bool(false)),
                    "optimize" => Some(EnableSelector::Mode("optimize".to_string())),
                    _ => None,
                }
            }),
            default_locale: value.defaultLocale,
            locales_dir: value.localesDir,
            indentation: value.indentation.map(Indentation::from),
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

    #[test]
    fn parses_use_translation_names_and_nesting_settings() {
        let json = r#"
        {
          "useTranslationNames": [
            "useTranslation",
            { "name": "loadPageTranslations", "nsArg": 1, "keyPrefixArg": 2 }
          ],
          "nestingPrefix": "__nest__(",
          "nestingSuffix": ")",
          "nestingOptionsSeparator": "|"
        }
        "#;
        let config = Config::from_json_string(json).unwrap();
        assert_eq!(config.use_translation_names.len(), 2);
        assert_eq!(
            config.use_translation_names[1].name(),
            "loadPageTranslations"
        );
        assert_eq!(config.use_translation_names[1].ns_arg(), 1);
        assert_eq!(config.use_translation_names[1].key_prefix_arg(), 2);
        assert_eq!(config.nesting_prefix, "__nest__(");
        assert_eq!(config.nesting_suffix, ")");
        assert_eq!(config.nesting_options_separator, "|");
    }

    #[test]
    fn supports_namespace_less_mode_with_default_namespace_false() {
        let json = r#"
        {
          "defaultNamespace": false,
          "nsSeparator": false
        }
        "#;
        let config = Config::from_json_string(json).unwrap();
        assert!(config.namespace_less_mode());
        assert_eq!(config.effective_default_namespace(), "translation");
    }

    #[test]
    fn resolves_secondary_languages_from_explicit_or_locales() {
        let mut config = Config::default();
        config.locales = vec!["en".to_string(), "ja".to_string(), "fr".to_string()];
        config.primary_language = Some("en".to_string());
        assert_eq!(
            config.secondary_languages(),
            vec!["ja".to_string(), "fr".to_string()]
        );

        config.secondary_languages = Some(vec!["fr".to_string()]);
        assert_eq!(config.secondary_languages(), vec!["fr".to_string()]);
    }

    #[test]
    fn parses_types_enable_selector() {
        let json = r#"{ "types": { "enableSelector": "optimize" } }"#;
        let config = Config::from_json_string(json).unwrap();
        assert!(matches!(
            config.types_enable_selector(),
            Some(EnableSelector::Mode(mode)) if mode == "optimize"
        ));
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
