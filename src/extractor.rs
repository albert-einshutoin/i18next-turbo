use crate::config::{PluralConfig, UseTranslationName};
use anyhow::{Context, Result};
use glob::Pattern;
use regex::Regex;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};

/// Normalize a string to NFC form for consistent key handling.
/// This ensures that keys like "が" (NFD: か+゛) and "が" (NFC) are treated as identical.
///
/// Optimization: Uses `is_nfc_quick()` to check if the string is already in NFC form.
/// For most Latin/ASCII text (and pre-normalized CJK), this avoids any allocation.
/// Only strings that actually need normalization will allocate a new String.
fn normalize_key(key: &str) -> Cow<'_, str> {
    // Fast path: check if already normalized (no allocation needed)
    match is_nfc_quick(key.chars()) {
        IsNormalized::Yes => Cow::Borrowed(key),
        // Maybe or No: need to normalize
        _ => Cow::Owned(key.nfc().collect()),
    }
}

// =============================================================================
// Static regex patterns (compiled once via OnceLock for thread-safe lazy init)
// =============================================================================
//
// All regex patterns are validated at compile time via tests (see test_regex_initialization).
// If any pattern is invalid, the test will fail during CI, preventing runtime panics.

/// Pattern for t() calls in comments with single argument.
/// Matches: `t('key')`, `t("key")`, `t(\`key\`)`
/// Captures: Group 1 = the key string
static COMMENT_SINGLE_ARG_REGEX: OnceLock<Regex> = OnceLock::new();

/// Pattern for t() calls in comments with key and default value.
/// Matches: `t('key', 'default')`, `t("key", "default")`
/// Captures: Group 1 = key, Group 2 = default value
static COMMENT_WITH_DEFAULT_REGEX: OnceLock<Regex> = OnceLock::new();

/// Pattern for t() calls in comments with options object containing defaultValue.
/// Matches: `t('key', { defaultValue: 'default' })`
/// Captures: Group 1 = key, Group 2 = default value
static COMMENT_WITH_OPTIONS_REGEX: OnceLock<Regex> = OnceLock::new();

static SCRIPT_BLOCK_REGEX: OnceLock<Regex> = OnceLock::new();
static TEMPLATE_BLOCK_REGEX: OnceLock<Regex> = OnceLock::new();
static STYLE_BLOCK_REGEX: OnceLock<Regex> = OnceLock::new();

/// Returns regex for t() calls in comments with single argument
fn get_comment_single_arg_regex() -> &'static Regex {
    COMMENT_SINGLE_ARG_REGEX.get_or_init(|| {
        // Pattern: non-identifier char (or start), then t, optional whitespace, open paren,
        // optional whitespace, quoted string (single, double, or backtick), close paren
        Regex::new(r#"(?:^|[^a-zA-Z_])t\s*\(\s*['"`]([^'"`]+)['"`]\s*\)"#)
            .expect("COMMENT_SINGLE_ARG_REGEX pattern is invalid - this is a bug")
    })
}

/// Returns regex for t() calls in comments with key and default value
fn get_comment_with_default_regex() -> &'static Regex {
    COMMENT_WITH_DEFAULT_REGEX.get_or_init(|| {
        // Pattern: t('key', 'default') - two consecutive quoted strings separated by comma
        Regex::new(r#"(?:^|[^a-zA-Z_])t\s*\(\s*['"`]([^'"`]+)['"`]\s*,\s*['"`]([^'"`]+)['"`]\s*\)"#)
            .expect("COMMENT_WITH_DEFAULT_REGEX pattern is invalid - this is a bug")
    })
}

/// Returns regex for t() calls with options object (captures key and opening brace)
fn get_comment_with_options_regex() -> &'static Regex {
    COMMENT_WITH_OPTIONS_REGEX.get_or_init(|| {
        Regex::new(r#"(?s)(?:^|[^a-zA-Z_])t\s*\(\s*['"`]([^'"`]+)['"`]\s*,\s*(\{)"#)
            .expect("COMMENT_WITH_OPTIONS_REGEX pattern is invalid - this is a bug")
    })
}

fn get_script_block_regex() -> &'static Regex {
    SCRIPT_BLOCK_REGEX.get_or_init(|| {
        Regex::new(r#"(?is)<script\b[^>]*>(.*?)</script>"#)
            .expect("SCRIPT_BLOCK_REGEX pattern is invalid - this is a bug")
    })
}

fn get_template_block_regex() -> &'static Regex {
    TEMPLATE_BLOCK_REGEX.get_or_init(|| {
        Regex::new(r#"(?is)<template\b[^>]*>(.*?)</template>"#)
            .expect("TEMPLATE_BLOCK_REGEX pattern is invalid - this is a bug")
    })
}

fn get_style_block_regex() -> &'static Regex {
    STYLE_BLOCK_REGEX.get_or_init(|| {
        Regex::new(r#"(?is)<style\b[^>]*>.*?</style>"#)
            .expect("STYLE_BLOCK_REGEX pattern is invalid - this is a bug")
    })
}

#[derive(Debug, Clone)]
struct TagBlock {
    content: String,
    range: Range<usize>,
}

fn extract_tag_blocks(source: &str, regex: &Regex) -> Vec<TagBlock> {
    regex
        .captures_iter(source)
        .filter_map(|caps| {
            let full = caps.get(0)?;
            let inner = caps.get(1).unwrap_or(full);
            Some(TagBlock {
                content: inner.as_str().to_string(),
                range: full.start()..full.end(),
            })
        })
        .collect()
}

#[derive(Default)]
struct CommentOptionsData {
    default_value: Option<String>,
    namespace: Option<String>,
    context: Option<String>,
    has_count: bool,
    has_ordinal: bool,
}

impl CommentOptionsData {
    fn from_text(text: &str) -> Self {
        Self {
            default_value: extract_comment_string_option(text, "defaultValue"),
            namespace: extract_comment_string_option(text, "ns"),
            context: extract_comment_string_option(text, "context"),
            has_count: comment_option_exists(text, "count"),
            has_ordinal: comment_option_truthy(text, "ordinal"),
        }
    }
}

fn extract_braced_block(text: &str, start_index: usize) -> Option<(String, usize)> {
    let mut depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut is_escaped = false;

    for (offset, ch) in text[start_index..].char_indices() {
        let abs_index = start_index + offset;

        if let Some(quote) = in_string {
            if is_escaped {
                is_escaped = false;
                continue;
            }
            if ch == '\\' {
                is_escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' | '`' => {
                in_string = Some(ch);
            }
            '{' => {
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    let end = abs_index + ch.len_utf8();
                    return Some((text[start_index..end].to_string(), end));
                }
            }
            _ => {}
        }
    }

    None
}

fn extract_parenthesized_expression(text: &str, start_index: usize) -> Option<(String, usize)> {
    let mut depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut is_escaped = false;

    for (offset, ch) in text[start_index..].char_indices() {
        let abs_index = start_index + offset;

        if let Some(quote) = in_string {
            if is_escaped {
                is_escaped = false;
                continue;
            }
            if ch == '\\' {
                is_escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' | '`' => in_string = Some(ch),
            '(' => {
                depth += 1;
            }
            ')' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    let end = abs_index + ch.len_utf8();
                    return Some((text[start_index..end].to_string(), end));
                }
            }
            _ => {}
        }
    }

    None
}

fn extract_translation_calls(
    text: &str,
    functions: &[String],
    include_dollar_alias: bool,
) -> Vec<String> {
    let mut names: Vec<String> = functions.to_vec();
    if include_dollar_alias && !names.iter().any(|name| name == "$t") {
        names.push("$t".to_string());
    }
    names.sort_by_key(|b| std::cmp::Reverse(b.len()));
    names.dedup();

    let mut result = Vec::new();
    let mut index = 0usize;
    let len = text.len();

    while index < len {
        let slice = &text[index..];
        let mut matched: Option<&str> = None;
        for name in &names {
            if slice.starts_with(name) && function_boundary_ok(text, index, index + name.len()) {
                matched = Some(name);
                break;
            }
        }

        if let Some(name) = matched {
            let after_name = index + name.len();
            if let Some(open_index) = skip_whitespace_to_paren(text, after_name) {
                if let Some((paren_block, end_index)) =
                    extract_parenthesized_expression(text, open_index)
                {
                    result.push(format!("{}{}", name, paren_block));
                    index = end_index;
                    continue;
                }
            }
        }

        if let Some(ch) = text[index..].chars().next() {
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    result
}

fn skip_whitespace_to_paren(text: &str, mut index: usize) -> Option<usize> {
    while index < text.len() {
        let mut iter = text[index..].char_indices();
        if let Some((offset, ch)) = iter.next() {
            if ch.is_whitespace() {
                index += ch.len_utf8();
                continue;
            }
            if ch == '(' {
                return Some(index + offset);
            }
            return None;
        } else {
            break;
        }
    }
    None
}

fn function_boundary_ok(text: &str, start: usize, end: usize) -> bool {
    if start > 0 {
        if let Some(prev) = text[..start].chars().next_back() {
            if is_identifier_char(prev) {
                return false;
            }
        }
    }

    if end < text.len() {
        if let Some(next) = text[end..].chars().next() {
            if is_identifier_char(next) {
                return false;
            }
        }
    }

    true
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '$' || ch == '.' || ch.is_alphanumeric()
}

fn extract_comment_string_option(text: &str, key: &str) -> Option<String> {
    let base = format!(
        r#"(?s)(?:^|[^a-zA-Z0-9_])["']?{}["']?\s*:\s*"#,
        regex::escape(key)
    );
    let variants = [("'", "[^']+"), ("\"", "[^\"]+"), ("`", "[^`]+")];

    for (quote, inner) in &variants {
        let pattern = format!("{}{}({}){}", base, quote, inner, quote);
        if let Ok(re) = Regex::new(&pattern) {
            if let Some(cap) = re.captures(text) {
                return cap.get(1).map(|m| m.as_str().to_string());
            }
        }
    }

    None
}

fn comment_option_exists(text: &str, key: &str) -> bool {
    let pattern = format!(
        r#"(?s)(?:^|[^a-zA-Z0-9_])["']?{}["']?\s*:"#,
        regex::escape(key)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|re| re.find(text))
        .is_some()
}

fn comment_option_truthy(text: &str, key: &str) -> bool {
    let pattern = format!(
        r#"(?s)(?:^|[^a-zA-Z0-9_])["']?{}["']?\s*:\s*true(?:[^a-zA-Z0-9_]|$)"#,
        regex::escape(key)
    );
    Regex::new(&pattern)
        .ok()
        .and_then(|re| re.find(text))
        .is_some()
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            result.push(value);
        }
    }
    result
}

fn split_top_level_once<'a>(text: &'a str, separator: &str) -> Option<(&'a str, &'a str)> {
    if separator.is_empty() {
        return None;
    }
    let mut depth_paren = 0usize;
    let mut depth_brace = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for (idx, ch) in text.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' | '`' => in_string = Some(ch),
            '(' => depth_paren += 1,
            ')' => depth_paren = depth_paren.saturating_sub(1),
            '{' => depth_brace += 1,
            '}' => depth_brace = depth_brace.saturating_sub(1),
            _ => {}
        }

        if depth_paren == 0 && depth_brace == 0 && text[idx..].starts_with(separator) {
            let right = &text[idx + separator.len()..];
            return Some((&text[..idx], right));
        }
    }
    None
}

fn parse_nested_key_token(token: &str) -> Option<&str> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    if (t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"')) {
        return Some(&t[1..t.len() - 1]);
    }
    Some(t)
}

use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap, Span, Spanned};
use swc_ecma_ast::{
    BinaryOp, CallExpr, Callee, CondExpr, Expr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue,
    JSXElement, JSXElementChild, JSXElementName, JSXExpr, JSXOpeningElement, Lit, MemberProp,
    ObjectLit, ParenExpr, Pat, Prop, PropName, PropOrSpread, Tpl, VarDeclarator,
};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_visit::{Visit, VisitWith};

/// Extracted translation key with metadata
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtractedKey {
    pub key: String,
    pub namespace: Option<String>,
    pub default_value: Option<String>,
}

/// Error encountered during extraction
#[derive(Debug, Clone)]
pub struct ExtractionError {
    pub file_path: String,
    pub message: String,
}

/// Result of extraction from multiple files
#[derive(Debug, Default)]
pub struct ExtractionResult {
    pub files: Vec<(String, Vec<ExtractedKey>)>,
    pub warning_count: usize,
    pub errors: Vec<ExtractionError>,
}

/// Scope information for useTranslation hook
#[derive(Debug, Clone, Default)]
pub struct ScopeInfo {
    /// Namespace from useTranslation('namespace')
    pub namespace: Option<String>,
    /// Key prefix from useTranslation({ keyPrefix: 'prefix' })
    pub key_prefix: Option<String>,
}

#[derive(Debug, Clone)]
struct ContextInfo {
    values: Vec<String>,
    is_dynamic: bool,
}

/// Visitor that traverses the AST and extracts translation keys
pub struct TranslationVisitor {
    /// Set of function names to look for (e.g., "t", "i18n.t")
    functions: HashSet<String>,
    /// Trans component names to look for
    trans_components: HashSet<String>,
    /// Extracted keys
    pub keys: Vec<ExtractedKey>,
    /// Source map for line number lookup
    source_map: Lrc<SourceMap>,
    /// Comments for magic comment detection
    comments: Option<SingleThreadedComments>,
    /// Lines disabled via magic comments (reserved for future use)
    #[allow(dead_code)]
    disabled_lines: HashSet<u32>,
    /// Scope info for variables bound from useTranslation/getFixedT
    scope_bindings: HashMap<String, ScopeInfo>,
    /// Hook-like functions that produce a bound t function.
    use_translation_names: Vec<UseTranslationName>,
    /// File path being processed (for warning messages)
    file_path: Option<String>,
    /// Warning count for non-extractable patterns
    warning_count: usize,
    /// Context separator (e.g., "_" for "friend_male")
    context_separator: String,
    /// Plural separator (e.g., "_" for "item_one")
    plural_separator: String,
    /// Plural suffixes to generate (e.g., ["one", "other"])
    plural_suffixes: Vec<String>,
    /// Whether to generate base key alongside plural keys
    generate_base_plural: bool,
    /// Prefix/suffix settings for nested translation extraction.
    nesting_prefix: String,
    nesting_suffix: String,
    nesting_options_separator: String,
}

impl TranslationVisitor {
    pub fn new(
        functions: Vec<String>,
        trans_components: Vec<String>,
        use_translation_names: Vec<UseTranslationName>,
        source_map: Lrc<SourceMap>,
        comments: Option<SingleThreadedComments>,
        plural_config: PluralConfig,
        nesting_prefix: String,
        nesting_suffix: String,
        nesting_options_separator: String,
    ) -> Self {
        // Parse magic comments to find disabled lines
        let disabled_lines = Self::parse_disabled_lines(&comments);

        Self {
            functions: functions.into_iter().collect(),
            trans_components: trans_components.into_iter().collect(),
            keys: Vec::new(),
            source_map,
            comments,
            disabled_lines,
            scope_bindings: HashMap::new(),
            use_translation_names,
            file_path: None,
            warning_count: 0,
            context_separator: plural_config.context_separator,
            plural_separator: plural_config.separator,
            plural_suffixes: plural_config.suffixes,
            generate_base_plural: plural_config.generate_base,
            nesting_prefix,
            nesting_suffix,
            nesting_options_separator,
        }
    }

    /// Parse comments to find lines with i18next-extract-disable directives
    fn parse_disabled_lines(_comments: &Option<SingleThreadedComments>) -> HashSet<u32> {
        // Note: We handle disable directives in is_disabled() instead
        HashSet::new()
    }

    /// Check if a span is disabled by magic comments
    fn is_disabled(&self, span: Span) -> bool {
        use swc_common::comments::Comments;

        if let Some(comments) = &self.comments {
            // Check leading comments for disable directives
            if let Some(leading) = comments.get_leading(span.lo) {
                for comment in leading {
                    let text = &comment.text;
                    if text.contains("i18next-extract-disable") {
                        return true;
                    }
                }
            }

            // Check trailing comments for disable-line
            if let Some(trailing) = comments.get_trailing(span.hi) {
                for comment in trailing {
                    let text = &comment.text;
                    if text.contains("i18next-extract-disable-line") {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if a call expression matches our target functions
    fn is_translation_call(&self, callee: &Callee) -> bool {
        match callee {
            Callee::Expr(expr) => match expr.as_ref() {
                // Simple function call: t('key')
                Expr::Ident(ident) => self.functions.contains(ident.sym.as_ref()),
                // Member expression: i18n.t('key')
                Expr::Member(member) => {
                    if let MemberProp::Ident(prop) = &member.prop {
                        if let Expr::Ident(obj) = member.obj.as_ref() {
                            let full_name = format!("{}.{}", obj.sym, prop.sym);
                            return self.functions.contains(&full_name);
                        }
                    }
                    false
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Extract string literal or template literal from the first argument
    fn extract_key_from_args(&mut self, call: &CallExpr) -> Option<String> {
        call.args.first().and_then(|arg| {
            match arg.expr.as_ref() {
                // String literal: t('key')
                Expr::Lit(Lit::Str(s)) => s.value.as_str().map(|s| s.to_string()),
                // Template literal: t(`key`)
                Expr::Tpl(tpl) => self.extract_simple_template_literal(tpl, call.span),
                // Selector API: t($ => $.user.profile)
                Expr::Arrow(arrow) => self.extract_selector_key(arrow),
                _ => None,
            }
        })
    }

    fn extract_selector_key(&self, arrow: &swc_ecma_ast::ArrowExpr) -> Option<String> {
        if arrow.params.len() != 1 {
            return None;
        }

        let root_param = match &arrow.params[0] {
            Pat::Ident(ident) => ident.id.sym.to_string(),
            _ => return None,
        };

        let expr = match &*arrow.body {
            swc_ecma_ast::BlockStmtOrExpr::Expr(expr) => expr.as_ref(),
            _ => return None,
        };

        let mut parts = Vec::new();
        if self.collect_selector_parts(expr, &root_param, &mut parts) {
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("."))
            }
        } else {
            None
        }
    }

    fn collect_selector_parts(&self, expr: &Expr, root: &str, parts: &mut Vec<String>) -> bool {
        match expr {
            Expr::Ident(ident) => ident.sym.as_ref() == root,
            Expr::Member(member) => {
                if !self.collect_selector_parts(member.obj.as_ref(), root, parts) {
                    return false;
                }
                match &member.prop {
                    MemberProp::Ident(ident) => parts.push(ident.sym.to_string()),
                    MemberProp::Computed(computed) => {
                        if let Expr::Lit(Lit::Str(s)) = computed.expr.as_ref() {
                            if let Some(value) = s.value.as_str() {
                                parts.push(value.to_string());
                            } else {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }

    /// Extract key from a template literal (only if it's a simple string without expressions)
    fn extract_simple_template_literal(&mut self, tpl: &Tpl, span: Span) -> Option<String> {
        // Only handle simple template literals without expressions
        // e.g., t(`hello`) is OK, but t(`hello ${name}`) is not
        if !tpl.exprs.is_empty() {
            // Warn about dynamic template literals that cannot be extracted
            self.warn_dynamic_template_literal(span);
            return None; // Has interpolations, skip
        }

        // Template literal with no expressions should have exactly one quasi
        if tpl.quasis.len() == 1 {
            let quasi = &tpl.quasis[0];
            if let Some(cooked) = quasi.cooked.as_ref() {
                return cooked.as_str().map(|s| s.to_string());
            }
            return Some(quasi.raw.to_string());
        }

        None
    }

    /// Warn about dynamic template literals that cannot be extracted
    fn warn_dynamic_template_literal(&mut self, span: Span) {
        let loc = self.source_map.lookup_char_pos(span.lo);
        let file_path = self.file_path.as_deref().unwrap_or("<unknown>");
        self.warning_count += 1;
        eprintln!(
            "Warning: Dynamic template literal found at {}:{}:{}. Translation key extraction skipped. Consider using i18next-extract-disable-line if intentional.",
            file_path,
            loc.line,
            loc.col_display + 1
        );
    }

    /// Check if call has count option (for plurals)
    #[allow(dead_code)]
    fn has_count_option(&self, call: &CallExpr) -> bool {
        self.get_option_value(call, "count").is_some()
    }

    /// Generate plural keys based on configuration
    /// Returns a list of keys with the appropriate plural suffixes
    ///
    /// For single-category languages (e.g., Japanese with only "other"),
    /// only the base key is generated without any suffix.
    ///
    /// If `generate_base_plural` is enabled, the base key (without suffix) is also
    /// generated alongside the plural keys.
    fn generate_plural_keys(
        &self,
        base_key: &str,
        context: Option<&str>,
        namespace: Option<String>,
        default_value: Option<String>,
        ordinal: bool,
    ) -> Vec<ExtractedKey> {
        // For single-category languages (only "other"), use base key without suffix
        let is_single_category =
            self.plural_suffixes.len() == 1 && self.plural_suffixes[0] == "other";

        if is_single_category {
            let key = match context {
                Some(ctx) => format!("{}{}{}", base_key, self.context_separator, ctx),
                None => base_key.to_string(),
            };
            return vec![ExtractedKey {
                key,
                namespace,
                default_value,
            }];
        }

        let mut keys: Vec<ExtractedKey> = Vec::new();

        // Optionally generate base key (without plural suffix)
        if self.generate_base_plural {
            let base = match context {
                Some(ctx) => format!("{}{}{}", base_key, self.context_separator, ctx),
                None => base_key.to_string(),
            };
            keys.push(ExtractedKey {
                key: base,
                namespace: namespace.clone(),
                default_value: default_value.clone(),
            });
        }

        // Generate plural keys with suffixes
        keys.extend(self.plural_suffixes.iter().map(|suffix| {
            let suffix = if ordinal {
                format!("ordinal_{}", suffix)
            } else {
                suffix.clone()
            };
            let key = match context {
                Some(ctx) => format!(
                    "{}{}{}{}{}",
                    base_key, self.context_separator, ctx, self.plural_separator, suffix
                ),
                None => format!("{}{}{}", base_key, self.plural_separator, suffix),
            };
            ExtractedKey {
                key,
                namespace: namespace.clone(),
                default_value: default_value.clone(),
            }
        }));

        keys
    }

    fn generate_plural_keys_with_context(
        &mut self,
        base_key: &str,
        namespace: Option<String>,
        default_value: Option<String>,
        context_info: Option<&ContextInfo>,
        ordinal: bool,
    ) {
        match context_info {
            Some(info) if !info.values.is_empty() => {
                for ctx in &info.values {
                    let plural_keys = self.generate_plural_keys(
                        base_key,
                        Some(ctx.as_str()),
                        namespace.clone(),
                        default_value.clone(),
                        ordinal,
                    );
                    self.keys.extend(plural_keys);
                }

                if info.is_dynamic {
                    let plural_keys = self.generate_plural_keys(
                        base_key,
                        None,
                        namespace,
                        default_value,
                        ordinal,
                    );
                    self.keys.extend(plural_keys);
                }
            }
            _ => {
                let plural_keys =
                    self.generate_plural_keys(base_key, None, namespace, default_value, ordinal);
                self.keys.extend(plural_keys);
            }
        }
    }

    fn options_object<'a>(&self, call: &'a CallExpr) -> Option<&'a ObjectLit> {
        if call.args.len() < 2 {
            return None;
        }
        if let Expr::Object(obj) = call.args[1].expr.as_ref() {
            Some(obj)
        } else {
            None
        }
    }

    /// Check if call has context option (supports literal and simple dynamic expressions)
    fn get_context_info(&self, call: &CallExpr) -> Option<ContextInfo> {
        let obj = self.options_object(call)?;
        for prop in &obj.props {
            if let PropOrSpread::Prop(prop) = prop {
                if let Prop::KeyValue(kv) = prop.as_ref() {
                    let prop_key = match &kv.key {
                        PropName::Ident(ident) => Some(ident.sym.to_string()),
                        PropName::Str(s) => s.value.as_str().map(|s| s.to_string()),
                        _ => None,
                    };
                    if prop_key.as_deref() != Some("context") {
                        continue;
                    }

                    if let Expr::Lit(Lit::Str(s)) = kv.value.as_ref() {
                        if let Some(value) = s.value.as_str() {
                            if value.is_empty() {
                                return None;
                            }
                            return Some(ContextInfo {
                                values: vec![value.to_string()],
                                is_dynamic: false,
                            });
                        }
                    }

                    let values = self.resolve_possible_context_values(kv.value.as_ref());
                    return Some(ContextInfo {
                        values,
                        is_dynamic: true,
                    });
                }
            }
        }
        None
    }

    /// Get defaultValue option from t() call
    fn get_default_value_option(&self, call: &CallExpr) -> Option<String> {
        self.get_option_value(call, "defaultValue")
    }

    fn has_return_objects_option(&self, call: &CallExpr) -> bool {
        let Some(obj) = self.options_object(call) else {
            return false;
        };
        self.find_bool_prop(obj, "returnObjects").unwrap_or(false)
    }

    fn has_ordinal_option(&self, call: &CallExpr) -> bool {
        let Some(obj) = self.options_object(call) else {
            return false;
        };
        self.find_bool_prop(obj, "ordinal").unwrap_or(false)
    }

    /// Get a string option value from the second argument object
    fn get_option_value(&self, call: &CallExpr, key: &str) -> Option<String> {
        if call.args.len() < 2 {
            return None;
        }

        if let Expr::Object(obj) = call.args[1].expr.as_ref() {
            return self.find_string_prop(obj, key);
        }
        None
    }

    /// Find a string property in an object literal
    fn find_string_prop(&self, obj: &ObjectLit, key: &str) -> Option<String> {
        for prop in &obj.props {
            if let PropOrSpread::Prop(prop) = prop {
                if let Prop::KeyValue(kv) = prop.as_ref() {
                    let prop_key = match &kv.key {
                        PropName::Ident(ident) => Some(ident.sym.to_string()),
                        PropName::Str(s) => s.value.as_str().map(|s| s.to_string()),
                        _ => None,
                    };

                    if prop_key.as_deref() == Some(key) {
                        if let Expr::Lit(Lit::Str(s)) = kv.value.as_ref() {
                            return s.value.as_str().map(|s| s.to_string());
                        }
                        // For count, just return a marker if it exists
                        if key == "count" {
                            return Some("__count__".to_string());
                        }
                    }
                }
            }
        }
        None
    }

    fn find_bool_prop(&self, obj: &ObjectLit, key: &str) -> Option<bool> {
        for prop in &obj.props {
            if let PropOrSpread::Prop(prop) = prop {
                match prop.as_ref() {
                    Prop::KeyValue(kv) => {
                        let prop_key = match &kv.key {
                            PropName::Ident(ident) => Some(ident.sym.to_string()),
                            PropName::Str(s) => s.value.as_str().map(|s| s.to_string()),
                            _ => None,
                        };
                        if prop_key.as_deref() != Some(key) {
                            continue;
                        }
                        if let Expr::Lit(Lit::Bool(b)) = kv.value.as_ref() {
                            return Some(b.value);
                        }
                        return Some(true);
                    }
                    Prop::Shorthand(ident) if ident.sym.as_ref() == key => return Some(true),
                    _ => {}
                }
            }
        }
        None
    }

    /// Check if an object has a property (for count detection)
    fn has_prop(&self, obj: &ObjectLit, key: &str) -> bool {
        for prop in &obj.props {
            if let PropOrSpread::Prop(prop) = prop {
                if let Prop::KeyValue(kv) = prop.as_ref() {
                    let prop_key = match &kv.key {
                        PropName::Ident(ident) => Some(ident.sym.to_string()),
                        PropName::Str(s) => s.value.as_str().map(|s| s.to_string()),
                        _ => None,
                    };
                    if prop_key.as_deref() == Some(key) {
                        return true;
                    }
                }
                // Handle shorthand: { count }
                if let Prop::Shorthand(ident) = prop.as_ref() {
                    if ident.sym.as_ref() == key {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract nested translation keys from a string value.
    /// Detects patterns like $t(key), $t('key'), or configurable nestingPrefix/suffix forms.
    fn extract_nested_translations(&self, text: &str) -> Vec<ExtractedKey> {
        let mut keys = Vec::new();
        let mut cursor = 0usize;

        while let Some(start_rel) = text[cursor..].find(&self.nesting_prefix) {
            let inner_start = cursor + start_rel + self.nesting_prefix.len();
            let Some(end_rel) = text[inner_start..].find(&self.nesting_suffix) else {
                break;
            };
            let inner_end = inner_start + end_rel;
            let inner = text[inner_start..inner_end].trim();

            let (raw_key_part, raw_options_part) =
                split_top_level_once(inner, self.nesting_options_separator.as_str())
                    .unwrap_or((inner, ""));

            let Some(raw_key) = parse_nested_key_token(raw_key_part.trim()) else {
                cursor = inner_end + self.nesting_suffix.len();
                continue;
            };

            let options = CommentOptionsData::from_text(raw_options_part);
            let (namespace, base_key) = self.parse_key_with_namespace(raw_key);

            if options.has_count {
                keys.extend(self.generate_plural_keys(
                    &base_key,
                    options.context.as_deref(),
                    namespace,
                    None,
                    options.has_ordinal,
                ));
            } else if let Some(ctx) = options.context {
                keys.push(ExtractedKey {
                    key: format!("{}{}{}", base_key, self.context_separator, ctx),
                    namespace,
                    default_value: None,
                });
            } else {
                keys.push(ExtractedKey {
                    key: base_key,
                    namespace,
                    default_value: None,
                });
            }

            cursor = inner_end + self.nesting_suffix.len();
        }

        let mut dedup = HashSet::new();
        keys.into_iter()
            .filter(|k| dedup.insert((k.namespace.clone(), k.key.clone())))
            .collect()
    }

    /// Parse namespace:key format with Unicode normalization
    fn parse_key_with_namespace(&self, key: &str) -> (Option<String>, String) {
        // Normalize the key to NFC form for consistent handling
        let normalized = normalize_key(key);
        if normalized.contains(':') {
            let parts: Vec<&str> = normalized.splitn(2, ':').collect();
            (Some(parts[0].to_string()), parts[1].to_string())
        } else {
            (None, normalized.into_owned())
        }
    }

    /// Extract string from JSX attribute value
    fn extract_jsx_attr_string(&self, value: &JSXAttrValue) -> Option<String> {
        match value {
            // i18nKey="hello"
            JSXAttrValue::Str(s) => s.value.as_str().map(|s| s.to_string()),
            // i18nKey={"hello"}
            JSXAttrValue::JSXExprContainer(container) => {
                if let swc_ecma_ast::JSXExpr::Expr(expr) = &container.expr {
                    if let Expr::Lit(Lit::Str(s)) = expr.as_ref() {
                        return s.value.as_str().map(|s| s.to_string());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn resolve_possible_context_values(&self, expr: &Expr) -> Vec<String> {
        let mut values = self.resolve_possible_string_values(expr);
        values.retain(|v| !v.is_empty());
        dedup_strings(values)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn resolve_possible_string_values(&self, expr: &Expr) -> Vec<String> {
        match expr {
            Expr::Lit(Lit::Str(s)) => s
                .value
                .as_str()
                .map(|s| s.to_string())
                .into_iter()
                .collect(),
            Expr::Tpl(tpl) => {
                if tpl.exprs.is_empty() {
                    let mut text = String::new();
                    for quasi in &tpl.quasis {
                        if let Some(cooked) = &quasi.cooked {
                            text.push_str(&cooked.to_string_lossy());
                        }
                    }
                    if text.is_empty() {
                        Vec::new()
                    } else {
                        vec![text]
                    }
                } else {
                    Vec::new()
                }
            }
            Expr::Paren(ParenExpr { expr, .. }) => {
                self.resolve_possible_string_values(expr.as_ref())
            }
            Expr::Cond(CondExpr { cons, alt, .. }) => {
                let mut values = self.resolve_possible_string_values(cons.as_ref());
                values.extend(self.resolve_possible_string_values(alt.as_ref()));
                values
            }
            Expr::Bin(bin) if matches!(bin.op, BinaryOp::Add) => {
                let left = self.resolve_possible_string_values(bin.left.as_ref());
                let right = self.resolve_possible_string_values(bin.right.as_ref());
                let mut combined = Vec::new();
                for l in &left {
                    for r in &right {
                        combined.push(format!("{}{}", l, r));
                    }
                }
                combined
            }
            _ => Vec::new(),
        }
    }

    /// Extract i18nKey from Trans component attributes
    fn extract_trans_key(&self, elem: &JSXOpeningElement) -> Option<String> {
        for attr in &elem.attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.as_ref() == "i18nKey" {
                        if let Some(value) = &jsx_attr.value {
                            return self.extract_jsx_attr_string(value);
                        }
                    }
                }
            }
        }
        None
    }

    /// Extract ns (namespace) from Trans component attributes
    fn extract_trans_ns(&self, elem: &JSXOpeningElement) -> Option<String> {
        for attr in &elem.attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.as_ref() == "ns" {
                        if let Some(value) = &jsx_attr.value {
                            return self.extract_jsx_attr_string(value);
                        }
                    }
                }
            }
        }
        None
    }

    /// Check if Trans component has count attribute (for plurals)
    fn trans_has_count(&self, elem: &JSXOpeningElement) -> bool {
        for attr in &elem.attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.as_ref() == "count" {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract context attribute info from Trans component (supports dynamic expressions)
    fn extract_trans_context_info(&self, elem: &JSXOpeningElement) -> Option<ContextInfo> {
        for attr in &elem.attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.as_ref() == "context" {
                        if let Some(value) = &jsx_attr.value {
                            if let Some(literal) = self.extract_jsx_attr_string(value) {
                                if literal.is_empty() {
                                    return None;
                                }
                                return Some(ContextInfo {
                                    values: vec![literal],
                                    is_dynamic: false,
                                });
                            }

                            if let JSXAttrValue::JSXExprContainer(container) = value {
                                if let JSXExpr::Expr(expr) = &container.expr {
                                    let values = self.resolve_possible_context_values(expr);
                                    return Some(ContextInfo {
                                        values,
                                        is_dynamic: true,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Extract text content from JSX children
    #[allow(clippy::only_used_in_recursion)]
    fn extract_jsx_children_text(&self, children: &[JSXElementChild]) -> Option<String> {
        let mut text_parts: Vec<String> = Vec::new();

        for child in children {
            match child {
                JSXElementChild::JSXText(text) => {
                    let s = text.value.to_string();
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        text_parts.push(trimmed.to_string());
                    }
                }
                JSXElementChild::JSXExprContainer(container) => {
                    // Handle {variable} - keep as placeholder
                    if let swc_ecma_ast::JSXExpr::Expr(expr) = &container.expr {
                        if let Expr::Ident(ident) = expr.as_ref() {
                            text_parts.push(format!("{{{{{}}}}}", ident.sym));
                        }
                    }
                }
                JSXElementChild::JSXElement(element) => {
                    // Recursively extract text from nested elements
                    // Keep tag names like <strong>, <br>, etc.
                    if let JSXElementName::Ident(ident) = &element.opening.name {
                        let tag = ident.sym.to_string();
                        // For simple inline tags, wrap content
                        if let Some(inner) = self.extract_jsx_children_text(&element.children) {
                            text_parts.push(format!("<{}>{}</{}>", tag, inner, tag));
                        } else if element.closing.is_none() {
                            // Self-closing tag
                            text_parts.push(format!("<{}/>", tag));
                        }
                    }
                }
                _ => {}
            }
        }

        if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(" "))
        }
    }

    /// Extract defaults attribute from Trans component
    fn extract_trans_defaults(&self, elem: &JSXOpeningElement) -> Option<String> {
        for attr in &elem.attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.as_ref() == "defaults" {
                        if let Some(value) = &jsx_attr.value {
                            return self.extract_jsx_attr_string(value);
                        }
                    }
                }
            }
        }
        None
    }

    /// Check if a call is useTranslation and extract scope info
    fn parse_use_translation_call(&self, call: &CallExpr) -> Option<ScopeInfo> {
        let (ns_arg_idx, key_prefix_arg_idx) = match &call.callee {
            Callee::Expr(expr) => match expr.as_ref() {
                Expr::Ident(ident) => {
                    let entry = self
                        .use_translation_names
                        .iter()
                        .find(|entry| entry.name() == ident.sym.as_ref())?;
                    (entry.ns_arg(), entry.key_prefix_arg())
                }
                _ => return None,
            },
            _ => return None,
        };

        let mut scope_info = ScopeInfo::default();
        for (i, arg) in call.args.iter().enumerate() {
            if i == ns_arg_idx {
                if let Expr::Lit(Lit::Str(s)) = arg.expr.as_ref() {
                    scope_info.namespace = s.value.as_str().map(|s| s.to_string());
                } else if let Expr::Object(obj) = arg.expr.as_ref() {
                    if let Some(ns) = self.find_string_prop(obj, "ns") {
                        scope_info.namespace = Some(ns);
                    }
                }
            }
            if i == key_prefix_arg_idx {
                if let Expr::Object(obj) = arg.expr.as_ref() {
                    scope_info.key_prefix = self.find_string_prop(obj, "keyPrefix");
                } else if let Expr::Lit(Lit::Str(s)) = arg.expr.as_ref() {
                    scope_info.key_prefix = s.value.as_str().map(|s| s.to_string());
                }
            }
        }

        if call.args.len() == 1 {
            if let Expr::Object(obj) = call.args[0].expr.as_ref() {
                if let Some(prefix) = self.find_string_prop(obj, "keyPrefix") {
                    scope_info.key_prefix = Some(prefix);
                }
                if let Some(ns) = self.find_string_prop(obj, "ns") {
                    scope_info.namespace = Some(ns);
                }
            }
        }

        Some(scope_info)
    }

    /// Check if a call is getFixedT and extract scope info
    fn parse_get_fixed_t_call(&self, call: &CallExpr) -> Option<ScopeInfo> {
        // Check if this is getFixedT() or i18n.getFixedT()
        let is_get_fixed_t = match &call.callee {
            Callee::Expr(expr) => match expr.as_ref() {
                Expr::Ident(ident) => ident.sym.as_ref() == "getFixedT",
                Expr::Member(member) => {
                    if let MemberProp::Ident(prop) = &member.prop {
                        prop.sym.as_ref() == "getFixedT"
                    } else {
                        false
                    }
                }
                _ => false,
            },
            _ => false,
        };

        if !is_get_fixed_t {
            return None;
        }

        let mut scope_info = ScopeInfo::default();

        // getFixedT(locale, namespace, keyPrefix)
        // or getFixedT(locale, { ns, keyPrefix })
        for (i, arg) in call.args.iter().enumerate() {
            match i {
                0 => {
                    // First arg is locale, skip it
                }
                1 => {
                    // Second arg: namespace (string) or options object
                    if let Expr::Lit(Lit::Str(s)) = arg.expr.as_ref() {
                        scope_info.namespace = s.value.as_str().map(|s| s.to_string());
                    }
                    if let Expr::Object(obj) = arg.expr.as_ref() {
                        if let Some(ns) = self.find_string_prop(obj, "ns") {
                            scope_info.namespace = Some(ns);
                        }
                        scope_info.key_prefix = self.find_string_prop(obj, "keyPrefix");
                    }
                }
                2 => {
                    // Third arg: keyPrefix (string)
                    if let Expr::Lit(Lit::Str(s)) = arg.expr.as_ref() {
                        scope_info.key_prefix = s.value.as_str().map(|s| s.to_string());
                    }
                }
                _ => {}
            }
        }

        Some(scope_info)
    }

    /// Extract bound variable names from a pattern
    fn extract_bound_t_name(&self, pat: &Pat) -> Option<String> {
        match pat {
            // const t = useTranslation()
            Pat::Ident(ident) => Some(ident.id.sym.to_string()),
            // const { t } = useTranslation()
            Pat::Object(obj) => {
                for prop in &obj.props {
                    if let swc_ecma_ast::ObjectPatProp::Assign(assign) = prop {
                        let name = assign.key.sym.to_string();
                        if name == "t" {
                            return Some(name);
                        }
                    }
                    if let swc_ecma_ast::ObjectPatProp::KeyValue(kv) = prop {
                        if let PropName::Ident(key) = &kv.key {
                            if key.sym.as_ref() == "t" {
                                // { t: customName } -> return customName
                                if let Pat::Ident(ident) = kv.value.as_ref() {
                                    return Some(ident.id.sym.to_string());
                                }
                            }
                        }
                    }
                }
                // Check for shorthand { t }
                for prop in &obj.props {
                    if let swc_ecma_ast::ObjectPatProp::Assign(assign) = prop {
                        if assign.key.sym.as_ref() == "t" {
                            return Some("t".to_string());
                        }
                    }
                }
                None
            }
            // const [t] = useTranslation()
            Pat::Array(arr) => {
                if let Some(Pat::Ident(ident)) = arr.elems.first().and_then(|elem| elem.as_ref()) {
                    return Some(ident.id.sym.to_string());
                }
                None
            }
            _ => None,
        }
    }

    /// Apply scope info to a key
    fn apply_scope_to_key(&self, key: &str, func_name: &str) -> (Option<String>, String) {
        if let Some(scope) = self.scope_bindings.get(func_name) {
            let final_key = if let Some(prefix) = &scope.key_prefix {
                format!("{}.{}", prefix, key)
            } else {
                key.to_string()
            };
            (scope.namespace.clone(), final_key)
        } else {
            self.parse_key_with_namespace(key)
        }
    }

    /// Get the function name from a callee
    fn get_callee_name(&self, callee: &Callee) -> Option<String> {
        match callee {
            Callee::Expr(expr) => match expr.as_ref() {
                Expr::Ident(ident) => Some(ident.sym.to_string()),
                Expr::Member(member) => {
                    if let MemberProp::Ident(prop) = &member.prop {
                        if let Expr::Ident(obj) = member.obj.as_ref() {
                            return Some(format!("{}.{}", obj.sym, prop.sym));
                        }
                    }
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Extract keys from comments (e.g., // t('key') or /* t('key', 'default') */)
    pub fn extract_from_comments(&mut self) {
        // Collect all comment texts first to avoid borrow issues
        let comment_texts: Vec<String> = if let Some(comments) = &self.comments {
            let (leading, trailing) = comments.borrow_all();

            let mut texts = Vec::new();

            // Collect leading comments
            for comment_list in leading.values() {
                for comment in comment_list {
                    texts.push(comment.text.to_string());
                }
            }

            // Collect trailing comments
            for comment_list in trailing.values() {
                for comment in comment_list {
                    texts.push(comment.text.to_string());
                }
            }

            texts
        } else {
            Vec::new()
        };

        // Now process the collected texts
        for text in &comment_texts {
            self.extract_keys_from_comment_text(text);
        }
    }

    /// Extract translation keys from a comment string
    fn extract_keys_from_comment_text(&mut self, text: &str) {
        // Look for patterns like t('key'), t("key"), t('key', 'default'), t('key', { defaultValue: '...' })
        // Also support i18n.t('key')

        // Use static regex patterns (compiled once, reused across all calls)
        let single_arg_pattern = get_comment_single_arg_regex();
        let with_default_pattern = get_comment_with_default_regex();
        let with_options_pattern = get_comment_with_options_regex();

        // Extract with options pattern first (most specific)
        for cap in with_options_pattern.captures_iter(text) {
            if let Some(key_match) = cap.get(1) {
                let key = key_match.as_str();
                if let Some(object_match) = cap.get(2) {
                    if let Some((options_text, _)) =
                        extract_braced_block(text, object_match.start())
                    {
                        let CommentOptionsData {
                            default_value,
                            namespace: namespace_override,
                            context,
                            has_count,
                            has_ordinal,
                        } = CommentOptionsData::from_text(&options_text);

                        let (namespace_from_key, base_key) = self.parse_key_with_namespace(key);
                        let namespace = namespace_override.or(namespace_from_key);

                        if has_count {
                            let plural_keys = self.generate_plural_keys(
                                &base_key,
                                context.as_deref(),
                                namespace.clone(),
                                default_value.clone(),
                                has_ordinal,
                            );
                            self.keys.extend(plural_keys);
                            continue;
                        }

                        if let Some(ctx) = context {
                            self.keys.push(ExtractedKey {
                                key: format!("{}{}{}", base_key, self.context_separator, ctx),
                                namespace,
                                default_value,
                            });
                        } else {
                            self.keys.push(ExtractedKey {
                                key: base_key,
                                namespace,
                                default_value,
                            });
                        }
                    }
                }
            }
        }

        // Extract with default pattern
        for cap in with_default_pattern.captures_iter(text) {
            if let Some(key_match) = cap.get(1) {
                let key = key_match.as_str();
                // Check if already captured by options pattern
                let (namespace, base_key) = self.parse_key_with_namespace(key);
                if !self
                    .keys
                    .iter()
                    .any(|k| k.key == base_key && k.namespace == namespace)
                {
                    let default_value = cap.get(2).map(|m| m.as_str().to_string());
                    self.keys.push(ExtractedKey {
                        key: base_key,
                        namespace,
                        default_value,
                    });
                }
            }
        }

        // Extract single arg pattern
        for cap in single_arg_pattern.captures_iter(text) {
            if let Some(key_match) = cap.get(1) {
                let key = key_match.as_str();
                let (namespace, base_key) = self.parse_key_with_namespace(key);
                // Check if already captured
                if !self
                    .keys
                    .iter()
                    .any(|k| k.key == base_key && k.namespace == namespace)
                {
                    self.keys.push(ExtractedKey {
                        key: base_key,
                        namespace,
                        default_value: None,
                    });
                }
            }
        }
    }
}

impl Visit for TranslationVisitor {
    fn visit_var_declarator(&mut self, decl: &VarDeclarator) {
        // Check for useTranslation() or getFixedT() calls
        if let Some(init) = &decl.init {
            if let Expr::Call(call) = init.as_ref() {
                // Try useTranslation first
                if let Some(scope_info) = self.parse_use_translation_call(call) {
                    if let Some(t_name) = self.extract_bound_t_name(&decl.name) {
                        self.scope_bindings.insert(t_name, scope_info);
                    }
                }
                // Try getFixedT
                else if let Some(scope_info) = self.parse_get_fixed_t_call(call) {
                    if let Some(t_name) = self.extract_bound_t_name(&decl.name) {
                        self.scope_bindings.insert(t_name, scope_info);
                    }
                }
            }
        }

        // Continue visiting
        decl.visit_children_with(self);
    }

    fn visit_call_expr(&mut self, call: &CallExpr) {
        // Check magic comments
        if self.is_disabled(call.span) {
            call.visit_children_with(self);
            return;
        }

        if self.is_translation_call(&call.callee) {
            if let Some(key) = self.extract_key_from_args(call) {
                // Check if the callee is bound to a scope
                let callee_name = self.get_callee_name(&call.callee);
                let (namespace_from_scope, base_key) = if let Some(name) = &callee_name {
                    self.apply_scope_to_key(&key, name)
                } else {
                    self.parse_key_with_namespace(&key)
                };

                // Check for count option (plurals)
                let has_count = if call.args.len() >= 2 {
                    if let Expr::Object(obj) = call.args[1].expr.as_ref() {
                        self.has_prop(obj, "count")
                    } else {
                        false
                    }
                } else {
                    false
                };

                // Check for context option
                let context_info = self.get_context_info(call);

                // Check for ordinal option (plural variant naming)
                let is_ordinal = self.has_ordinal_option(call);

                // Check for defaultValue option
                let default_value = self.get_default_value_option(call);

                // returnObjects=true means this key is an object root and should preserve children.
                let has_return_objects = self.has_return_objects_option(call);

                // Extract nested translations from defaultValue (e.g., $t('key'))
                if let Some(ref dv) = default_value {
                    let nested_keys = self.extract_nested_translations(dv);
                    for nested_key in nested_keys {
                        self.keys.push(nested_key);
                    }
                }

                if has_return_objects {
                    self.keys.push(ExtractedKey {
                        key: format!("{}.*", base_key),
                        namespace: namespace_from_scope,
                        default_value: None,
                    });
                } else if has_count {
                    // Generate plural keys based on configuration
                    self.generate_plural_keys_with_context(
                        &base_key,
                        namespace_from_scope,
                        default_value,
                        context_info.as_ref(),
                        is_ordinal,
                    );
                } else if let Some(info) = context_info {
                    if info.values.is_empty() {
                        self.keys.push(ExtractedKey {
                            key: base_key,
                            namespace: namespace_from_scope,
                            default_value,
                        });
                    } else {
                        for ctx in &info.values {
                            self.keys.push(ExtractedKey {
                                key: format!("{}{}{}", base_key, self.context_separator, ctx),
                                namespace: namespace_from_scope.clone(),
                                default_value: default_value.clone(),
                            });
                        }
                        if info.is_dynamic {
                            self.keys.push(ExtractedKey {
                                key: base_key,
                                namespace: namespace_from_scope,
                                default_value,
                            });
                        }
                    }
                } else {
                    // Regular key
                    self.keys.push(ExtractedKey {
                        key: base_key,
                        namespace: namespace_from_scope,
                        default_value,
                    });
                }
            }
        }

        // Continue visiting child nodes
        call.visit_children_with(self);
    }

    fn visit_jsx_element(&mut self, elem: &JSXElement) {
        // Check magic comments
        if self.is_disabled(elem.span) {
            elem.visit_children_with(self);
            return;
        }

        // Check if this is a Trans component
        if let JSXElementName::Ident(ident) = &elem.opening.name {
            if self.trans_components.contains(ident.sym.as_ref()) {
                // Extract i18nKey attribute (primary key source)
                let i18n_key = self.extract_trans_key(&elem.opening);

                // Extract ns attribute
                let ns_from_attr = self.extract_trans_ns(&elem.opening);

                // Extract defaults attribute
                let defaults = self.extract_trans_defaults(&elem.opening);

                // Extract children text (used as key if i18nKey not present, or as default value)
                let children_text = self.extract_jsx_children_text(&elem.children);

                // Check for count attribute (plurals)
                let has_count = self.trans_has_count(&elem.opening);

                // Check for context attribute (supports dynamic expressions)
                let context_info = self.extract_trans_context_info(&elem.opening);

                // Determine the key and default value
                let (key, default_value) = if let Some(key) = i18n_key {
                    // i18nKey is present - use it as key
                    // Use defaults attribute or children as default value
                    let dv = defaults.or(children_text);
                    (key, dv)
                } else if let Some(children) = children_text {
                    // No i18nKey - use children text as key
                    (children.clone(), Some(children))
                } else {
                    // No key available, skip
                    elem.visit_children_with(self);
                    return;
                };

                // Extract nested translations from default value (e.g., $t('key'))
                if let Some(ref dv) = default_value {
                    let nested_keys = self.extract_nested_translations(dv);
                    for nested_key in nested_keys {
                        self.keys.push(nested_key);
                    }
                }

                // Parse namespace from key (e.g., "common:greeting")
                let (namespace_from_key, base_key) = self.parse_key_with_namespace(&key);

                // Use ns attribute if present, otherwise use namespace from key
                let namespace = ns_from_attr.or(namespace_from_key);

                // Generate keys based on count and context attributes
                if has_count {
                    self.generate_plural_keys_with_context(
                        &base_key,
                        namespace.clone(),
                        default_value.clone(),
                        context_info.as_ref(),
                        false,
                    );
                } else if let Some(info) = context_info {
                    if info.values.is_empty() {
                        self.keys.push(ExtractedKey {
                            key: base_key,
                            namespace,
                            default_value,
                        });
                    } else {
                        for ctx in &info.values {
                            self.keys.push(ExtractedKey {
                                key: format!("{}{}{}", base_key, self.context_separator, ctx),
                                namespace: namespace.clone(),
                                default_value: default_value.clone(),
                            });
                        }
                        if info.is_dynamic {
                            self.keys.push(ExtractedKey {
                                key: base_key,
                                namespace,
                                default_value,
                            });
                        }
                    }
                } else {
                    // No modifiers: base key
                    self.keys.push(ExtractedKey {
                        key: base_key,
                        namespace,
                        default_value,
                    });
                }
            }
        }

        // Continue visiting child nodes
        elem.visit_children_with(self);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtractorStrategy {
    JavaScript,
    Vue,
    Svelte,
}

struct StrategyContext<'a> {
    functions: &'a [String],
    trans_components: &'a [String],
    use_translation_names: &'a [UseTranslationName],
    extract_from_comments: bool,
    plural_config: &'a PluralConfig,
    nesting_prefix: &'a str,
    nesting_suffix: &'a str,
    nesting_options_separator: &'a str,
}

impl<'a> StrategyContext<'a> {
    fn new(
        functions: &'a [String],
        trans_components: &'a [String],
        use_translation_names: &'a [UseTranslationName],
        extract_from_comments: bool,
        plural_config: &'a PluralConfig,
        nesting_prefix: &'a str,
        nesting_suffix: &'a str,
        nesting_options_separator: &'a str,
    ) -> Self {
        Self {
            functions,
            trans_components,
            use_translation_names,
            extract_from_comments,
            plural_config,
            nesting_prefix,
            nesting_suffix,
            nesting_options_separator,
        }
    }

    #[allow(clippy::iter_cloned_collect)]
    fn template_functions(&self) -> Vec<String> {
        let mut names: Vec<String> = self.functions.iter().cloned().collect();
        if !names.iter().any(|name| name == "$t") {
            names.push("$t".to_string());
        }
        names.sort();
        names.dedup();
        names
    }
}

impl ExtractorStrategy {
    fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_ascii_lowercase())
        {
            Some(ext) if ext == "vue" => ExtractorStrategy::Vue,
            Some(ext) if ext == "svelte" => ExtractorStrategy::Svelte,
            _ => ExtractorStrategy::JavaScript,
        }
    }

    fn extract(
        &self,
        path: &Path,
        source_code: &str,
        ctx: &StrategyContext,
    ) -> Result<(Vec<ExtractedKey>, usize)> {
        match self {
            ExtractorStrategy::JavaScript => extract_from_source_with_warnings(
                source_code,
                path,
                ctx.functions,
                ctx.trans_components,
                ctx.use_translation_names,
                ctx.extract_from_comments,
                ctx.plural_config,
                ctx.nesting_prefix,
                ctx.nesting_suffix,
                ctx.nesting_options_separator,
            ),
            ExtractorStrategy::Vue => extract_vue_component(path, source_code, ctx),
            ExtractorStrategy::Svelte => extract_svelte_component(path, source_code, ctx),
        }
    }
}

/// Extract translation keys from a TypeScript/JavaScript file
/// Note: This function always extracts from comments for backward compatibility.
pub fn extract_from_file<P: AsRef<Path>>(
    path: P,
    functions: &[String],
    plural_config: &PluralConfig,
) -> Result<Vec<ExtractedKey>> {
    let default_trans_components = vec!["Trans".to_string()];
    let default_use_translation_names =
        vec![UseTranslationName::Name("useTranslation".to_string())];
    let (keys, _) = extract_from_file_with_warnings(
        path,
        functions,
        &default_trans_components,
        &default_use_translation_names,
        true,
        plural_config,
        "$t(",
        ")",
        ",",
    )?;
    Ok(keys)
}

/// Extract translation keys from a file with configurable options
pub fn extract_from_file_with_options<P: AsRef<Path>>(
    path: P,
    functions: &[String],
    extract_from_comments: bool,
    plural_config: &PluralConfig,
) -> Result<Vec<ExtractedKey>> {
    let default_trans_components = vec!["Trans".to_string()];
    let default_use_translation_names =
        vec![UseTranslationName::Name("useTranslation".to_string())];
    let (keys, _) = extract_from_file_with_warnings(
        path,
        functions,
        &default_trans_components,
        &default_use_translation_names,
        extract_from_comments,
        plural_config,
        "$t(",
        ")",
        ",",
    )?;
    Ok(keys)
}

fn extract_from_file_with_warnings<P: AsRef<Path>>(
    path: P,
    functions: &[String],
    trans_components: &[String],
    use_translation_names: &[UseTranslationName],
    extract_from_comments: bool,
    plural_config: &PluralConfig,
    nesting_prefix: &str,
    nesting_suffix: &str,
    nesting_options_separator: &str,
) -> Result<(Vec<ExtractedKey>, usize)> {
    let path = path.as_ref();
    let source_code = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    let strategy = ExtractorStrategy::from_path(path);
    let ctx = StrategyContext::new(
        functions,
        trans_components,
        use_translation_names,
        extract_from_comments,
        plural_config,
        nesting_prefix,
        nesting_suffix,
        nesting_options_separator,
    );
    strategy.extract(path, &source_code, &ctx)
}

/// Extract translation keys from source code string
/// Note: This function always extracts from comments for backward compatibility.
/// Use `extract_from_glob` with config for production use.
pub fn extract_from_source<P: AsRef<Path>>(
    source: &str,
    path: P,
    functions: &[String],
) -> Result<Vec<ExtractedKey>> {
    let plural_config = PluralConfig::default();
    let default_trans_components = vec!["Trans".to_string()];
    let default_use_translation_names =
        vec![UseTranslationName::Name("useTranslation".to_string())];
    let (keys, _) = extract_from_source_with_warnings(
        source,
        path,
        functions,
        &default_trans_components,
        &default_use_translation_names,
        true,
        &plural_config,
        "$t(",
        ")",
        ",",
    )?;
    Ok(keys)
}

/// Extract translation keys from a source string with configurable options
pub fn extract_from_source_with_options<P: AsRef<Path>>(
    source: &str,
    path: P,
    functions: &[String],
    extract_from_comments: bool,
    plural_config: &PluralConfig,
) -> Result<Vec<ExtractedKey>> {
    let default_trans_components = vec!["Trans".to_string()];
    let default_use_translation_names =
        vec![UseTranslationName::Name("useTranslation".to_string())];
    let (keys, _) = extract_from_source_with_warnings(
        source,
        path,
        functions,
        &default_trans_components,
        &default_use_translation_names,
        extract_from_comments,
        plural_config,
        "$t(",
        ")",
        ",",
    )?;
    Ok(keys)
}

fn extract_from_source_with_warnings<P: AsRef<Path>>(
    source: &str,
    path: P,
    functions: &[String],
    trans_components: &[String],
    use_translation_names: &[UseTranslationName],
    should_extract_from_comments: bool,
    plural_config: &PluralConfig,
    nesting_prefix: &str,
    nesting_suffix: &str,
    nesting_options_separator: &str,
) -> Result<(Vec<ExtractedKey>, usize)> {
    let path = path.as_ref();
    let cm: Lrc<SourceMap> = Default::default();

    let fm = cm.new_source_file(
        FileName::Real(path.to_path_buf()).into(),
        source.to_string(),
    );

    // Determine syntax based on file extension
    let is_tsx = path
        .extension()
        .map(|ext| ext == "tsx" || ext == "jsx")
        .unwrap_or(false);

    let syntax = Syntax::Typescript(TsSyntax {
        tsx: is_tsx,
        decorators: true,
        ..Default::default()
    });

    // Create comments container for magic comment detection
    let comments = SingleThreadedComments::default();

    let lexer = Lexer::new(
        syntax,
        Default::default(),
        StringInput::from(&*fm),
        Some(&comments),
    );

    let mut parser = Parser::new_from(lexer);

    // Parse the module, handling errors gracefully with user-friendly error messages
    let module = match parser.parse_module() {
        Ok(module) => module,
        Err(e) => {
            // Get position information from the error span
            let loc = cm.lookup_char_pos(e.span().lo);
            let error_msg = format!("{:?}", e.kind());

            // Format: file:line:column: message
            // This format is recognized by most editors and IDEs for click-to-navigate
            eprintln!(
                "Warning: Parse error in {}:{}:{}: {}",
                path.display(),
                loc.line,
                loc.col_display + 1, // 1-based column for user display
                error_msg
            );
            return Ok((Vec::new(), 0));
        }
    };

    // Visit the AST and extract keys
    let mut visitor = TranslationVisitor::new(
        functions.to_vec(),
        trans_components.to_vec(),
        use_translation_names.to_vec(),
        cm,
        Some(comments),
        plural_config.clone(),
        nesting_prefix.to_string(),
        nesting_suffix.to_string(),
        nesting_options_separator.to_string(),
    );
    visitor.file_path = Some(path.display().to_string());
    module.visit_with(&mut visitor);

    // Also extract keys from comments (if enabled)
    if should_extract_from_comments {
        visitor.extract_from_comments();
    }

    Ok((visitor.keys, visitor.warning_count))
}

fn extract_vue_component(
    file_path: &Path,
    source_code: &str,
    ctx: &StrategyContext,
) -> Result<(Vec<ExtractedKey>, usize)> {
    let mut keys = Vec::new();
    let mut warnings = 0usize;

    let script_blocks = extract_tag_blocks(source_code, get_script_block_regex());
    for (idx, block) in script_blocks.iter().enumerate() {
        let virtual_path = format!("{}#script{}", file_path.display(), idx + 1);
        let (mut script_keys, block_warnings) = extract_from_source_with_warnings(
            block.content.as_str(),
            &virtual_path,
            ctx.functions,
            ctx.trans_components,
            ctx.use_translation_names,
            ctx.extract_from_comments,
            ctx.plural_config,
            ctx.nesting_prefix,
            ctx.nesting_suffix,
            ctx.nesting_options_separator,
        )?;
        keys.append(&mut script_keys);
        warnings += block_warnings;
    }

    let template_blocks = extract_tag_blocks(source_code, get_template_block_regex());
    if !template_blocks.is_empty() {
        let template_functions = ctx.template_functions();
        for (block_idx, block) in template_blocks.iter().enumerate() {
            let exprs = extract_translation_calls(&block.content, &template_functions, true);
            for (expr_idx, expr) in exprs.iter().enumerate() {
                let virtual_source = format!(
                    "function __i18n_tpl_{}() {{ return {}; }}",
                    expr_idx + 1,
                    expr
                );
                let virtual_path = format!(
                    "{}#template{}:{}",
                    file_path.display(),
                    block_idx + 1,
                    expr_idx + 1
                );
                let (mut tpl_keys, tpl_warnings) = extract_from_source_with_warnings(
                    &virtual_source,
                    &virtual_path,
                    &template_functions,
                    ctx.trans_components,
                    ctx.use_translation_names,
                    false,
                    ctx.plural_config,
                    ctx.nesting_prefix,
                    ctx.nesting_suffix,
                    ctx.nesting_options_separator,
                )?;
                keys.append(&mut tpl_keys);
                warnings += tpl_warnings;
            }
        }
    }

    if script_blocks.is_empty() && template_blocks.is_empty() {
        return extract_from_source_with_warnings(
            source_code,
            file_path,
            ctx.functions,
            ctx.trans_components,
            ctx.use_translation_names,
            ctx.extract_from_comments,
            ctx.plural_config,
            ctx.nesting_prefix,
            ctx.nesting_suffix,
            ctx.nesting_options_separator,
        );
    }

    Ok((keys, warnings))
}

fn extract_svelte_component(
    file_path: &Path,
    source_code: &str,
    ctx: &StrategyContext,
) -> Result<(Vec<ExtractedKey>, usize)> {
    let mut keys = Vec::new();
    let mut warnings = 0usize;

    let script_blocks = extract_tag_blocks(source_code, get_script_block_regex());
    for (idx, block) in script_blocks.iter().enumerate() {
        let virtual_path = format!("{}#script{}", file_path.display(), idx + 1);
        let (mut script_keys, block_warnings) = extract_from_source_with_warnings(
            block.content.as_str(),
            &virtual_path,
            ctx.functions,
            ctx.trans_components,
            ctx.use_translation_names,
            ctx.extract_from_comments,
            ctx.plural_config,
            ctx.nesting_prefix,
            ctx.nesting_suffix,
            ctx.nesting_options_separator,
        )?;
        keys.append(&mut script_keys);
        warnings += block_warnings;
    }

    let mut trimmed_template = source_code.to_string();
    let mut removal_ranges: Vec<Range<usize>> = script_blocks
        .iter()
        .map(|block| block.range.clone())
        .collect();
    let style_blocks = extract_tag_blocks(source_code, get_style_block_regex());
    for block in style_blocks {
        removal_ranges.push(block.range);
    }
    removal_ranges.sort_by(|a, b| b.start.cmp(&a.start));
    for range in removal_ranges {
        let len = range.end.saturating_sub(range.start);
        if len == 0 || range.end > trimmed_template.len() {
            continue;
        }
        trimmed_template.replace_range(range, &" ".repeat(len));
    }

    let template_functions = ctx.template_functions();
    let template_exprs = extract_translation_calls(&trimmed_template, &template_functions, true);
    for (idx, expr) in template_exprs.iter().enumerate() {
        let virtual_source = format!("function __svelte_tpl_{}() {{ return {}; }}", idx + 1, expr);
        let virtual_path = format!("{}#template:{}", file_path.display(), idx + 1);
        let (mut tpl_keys, tpl_warnings) = extract_from_source_with_warnings(
            &virtual_source,
            &virtual_path,
            &template_functions,
            ctx.trans_components,
            ctx.use_translation_names,
            false,
            ctx.plural_config,
            ctx.nesting_prefix,
            ctx.nesting_suffix,
            ctx.nesting_options_separator,
        )?;
        keys.append(&mut tpl_keys);
        warnings += tpl_warnings;
    }

    if script_blocks.is_empty() && template_exprs.is_empty() {
        return extract_from_source_with_warnings(
            source_code,
            file_path,
            ctx.functions,
            ctx.trans_components,
            ctx.use_translation_names,
            ctx.extract_from_comments,
            ctx.plural_config,
            ctx.nesting_prefix,
            ctx.nesting_suffix,
            ctx.nesting_options_separator,
        );
    }

    Ok((keys, warnings))
}

/// Result type for a single file extraction (used internally for lock-free processing)
enum FileExtractionResult {
    Success {
        file_path: String,
        keys: Vec<ExtractedKey>,
        warnings: usize,
    },
    Error(ExtractionError),
    Empty {
        warnings: usize,
    },
}

/// Extract keys from multiple files using glob patterns.
///
/// This implementation uses streaming parallel processing:
/// - Uses `par_bridge()` to stream file paths directly into worker threads
/// - No upfront collection of all file paths (O(1) memory for paths)
/// - Lock-free error collection (each thread returns Result enum)
/// - Optimized for large monorepos (millions of files)
pub fn extract_from_glob(
    patterns: &[String],
    ignore_patterns: &[String],
    functions: &[String],
    plural_config: &PluralConfig,
) -> Result<ExtractionResult> {
    let default_trans_components = vec!["Trans".to_string()];
    let default_use_translation_names =
        vec![UseTranslationName::Name("useTranslation".to_string())];
    extract_from_glob_with_options(
        patterns,
        ignore_patterns,
        functions,
        true,
        plural_config,
        &default_trans_components,
        &default_use_translation_names,
        "$t(",
        ")",
        ",",
    )
}

/// Extract keys from multiple files using glob patterns with configurable options.
pub fn extract_from_glob_with_options(
    patterns: &[String],
    ignore_patterns: &[String],
    functions: &[String],
    extract_from_comments: bool,
    plural_config: &PluralConfig,
    trans_components: &[String],
    use_translation_names: &[UseTranslationName],
    nesting_prefix: &str,
    nesting_suffix: &str,
    nesting_options_separator: &str,
) -> Result<ExtractionResult> {
    use rayon::iter::ParallelBridge;
    use rayon::prelude::*;

    let ignore_matchers = Arc::new(compile_ignore_patterns(ignore_patterns)?);
    let trans_components = Arc::new(trans_components.to_vec());
    let use_translation_names = Arc::new(use_translation_names.to_vec());
    let nesting_prefix = Arc::new(nesting_prefix.to_string());
    let nesting_suffix = Arc::new(nesting_suffix.to_string());
    let nesting_options_separator = Arc::new(nesting_options_separator.to_string());

    // Create a streaming iterator that chains all glob patterns
    // This avoids collecting all file paths into memory upfront
    let pattern_refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();

    // Enum to represent either a valid path or an error during glob iteration
    enum GlobItem {
        Path(std::path::PathBuf),
        GlobError { pattern: String, message: String },
        PatternError { pattern: String, message: String },
    }

    // Process files using streaming parallel processing with par_bridge()
    // Files are fed to worker threads as they are discovered by glob
    let file_results: Vec<FileExtractionResult> = pattern_refs
        .into_iter()
        .flat_map(|pattern| {
            let ignore_for_pattern = Arc::clone(&ignore_matchers);
            // Create iterator for this pattern (may error)
            match glob::glob(pattern) {
                Ok(paths) => {
                    // Map each path result to GlobItem
                    paths
                        .filter_map(move |entry| match entry {
                            Ok(path)
                                if path.is_file()
                                    && !matches_ignore_path(&path, ignore_for_pattern.as_ref()) =>
                            {
                                Some(GlobItem::Path(path))
                            }
                            Ok(_) => None, // Skip directories and ignored files
                            Err(e) => Some(GlobItem::GlobError {
                                pattern: pattern.to_string(),
                                message: e.to_string(),
                            }),
                        })
                        .collect::<Vec<_>>()
                }
                Err(e) => {
                    // Return pattern error as a single-element vec
                    vec![GlobItem::PatternError {
                        pattern: pattern.to_string(),
                        message: e.to_string(),
                    }]
                }
            }
        })
        .par_bridge() // Stream directly into parallel processing
        .map({
            let trans_components = Arc::clone(&trans_components);
            let use_translation_names = Arc::clone(&use_translation_names);
            let nesting_prefix = Arc::clone(&nesting_prefix);
            let nesting_suffix = Arc::clone(&nesting_suffix);
            let nesting_options_separator = Arc::clone(&nesting_options_separator);
            move |item| match item {
                GlobItem::Path(path) => {
                    match extract_from_file_with_warnings(
                        &path,
                        functions,
                        &trans_components,
                        &use_translation_names,
                        extract_from_comments,
                        plural_config,
                        &nesting_prefix,
                        &nesting_suffix,
                        &nesting_options_separator,
                    ) {
                        Ok((keys, warnings)) => {
                            if keys.is_empty() {
                                FileExtractionResult::Empty { warnings }
                            } else {
                                FileExtractionResult::Success {
                                    file_path: path.display().to_string(),
                                    keys,
                                    warnings,
                                }
                            }
                        }
                        Err(e) => FileExtractionResult::Error(ExtractionError {
                            file_path: path.display().to_string(),
                            message: e.to_string(),
                        }),
                    }
                }
                GlobItem::GlobError { pattern, message } => {
                    FileExtractionResult::Error(ExtractionError {
                        file_path: pattern,
                        message: format!("Glob error: {}", message),
                    })
                }
                GlobItem::PatternError { pattern, message } => {
                    FileExtractionResult::Error(ExtractionError {
                        file_path: pattern,
                        message: format!("Invalid glob pattern: {}", message),
                    })
                }
            }
        })
        .collect();

    // Aggregate results (single-threaded, but O(n) - no lock contention)
    let mut files: Vec<(String, Vec<ExtractedKey>)> = Vec::new();
    let mut errors: Vec<ExtractionError> = Vec::new();
    let mut warning_count = 0;

    for result in file_results {
        match result {
            FileExtractionResult::Success {
                file_path,
                keys,
                warnings,
            } => {
                warning_count += warnings;
                files.push((file_path, keys));
            }
            FileExtractionResult::Error(err) => {
                warning_count += 1;
                errors.push(err);
            }
            FileExtractionResult::Empty { warnings } => {
                warning_count += warnings;
            }
        }
    }

    Ok(ExtractionResult {
        files,
        warning_count,
        errors,
    })
}

/// Extract keys with early deduplication using fold/reduce pattern.
/// This minimizes memory allocation for large codebases with many duplicate keys.
///
/// Returns a HashMap of unique keys instead of Vec, reducing O(N) to O(unique_keys).
pub fn extract_from_glob_deduplicated(
    patterns: &[String],
    ignore_patterns: &[String],
    functions: &[String],
    plural_config: &PluralConfig,
) -> Result<(HashMap<ExtractedKey, ()>, usize, Vec<ExtractionError>)> {
    let default_trans_components = vec!["Trans".to_string()];
    let default_use_translation_names =
        vec![UseTranslationName::Name("useTranslation".to_string())];
    extract_from_glob_deduplicated_with_options(
        patterns,
        ignore_patterns,
        functions,
        true,
        plural_config,
        &default_trans_components,
        &default_use_translation_names,
        "$t(",
        ")",
        ",",
    )
}

/// Extract keys with early deduplication and configurable comment extraction
pub fn extract_from_glob_deduplicated_with_options(
    patterns: &[String],
    ignore_patterns: &[String],
    functions: &[String],
    extract_from_comments: bool,
    plural_config: &PluralConfig,
    trans_components: &[String],
    use_translation_names: &[UseTranslationName],
    nesting_prefix: &str,
    nesting_suffix: &str,
    nesting_options_separator: &str,
) -> Result<(HashMap<ExtractedKey, ()>, usize, Vec<ExtractionError>)> {
    use rayon::prelude::*;

    let mut all_files: Vec<std::path::PathBuf> = Vec::new();
    let mut glob_errors: Vec<ExtractionError> = Vec::new();
    let ignore_matchers = compile_ignore_patterns(ignore_patterns)?;

    for pattern in patterns {
        let matches =
            glob::glob(pattern).with_context(|| format!("Invalid glob pattern: {}", pattern))?;

        for entry in matches {
            match entry {
                Ok(path) => {
                    if path.is_file() && !matches_ignore_path(&path, &ignore_matchers) {
                        all_files.push(path);
                    }
                }
                Err(e) => {
                    glob_errors.push(ExtractionError {
                        file_path: pattern.clone(),
                        message: format!("Glob error: {}", e),
                    });
                }
            }
        }
    }

    // Use fold + reduce for early deduplication during parallel processing
    // Each thread maintains its own HashSet, then we merge at the end
    type AccumulatorType = (HashMap<ExtractedKey, ()>, usize, Vec<ExtractionError>);

    let initial: AccumulatorType = (HashMap::new(), 0, Vec::new());
    let trans_components = Arc::new(trans_components.to_vec());
    let use_translation_names = Arc::new(use_translation_names.to_vec());
    let nesting_prefix = Arc::new(nesting_prefix.to_string());
    let nesting_suffix = Arc::new(nesting_suffix.to_string());
    let nesting_options_separator = Arc::new(nesting_options_separator.to_string());

    let (unique_keys, warning_count, mut errors) = all_files
        .par_iter()
        .fold(|| initial.clone(), {
            let trans_components = Arc::clone(&trans_components);
            let use_translation_names = Arc::clone(&use_translation_names);
            let nesting_prefix = Arc::clone(&nesting_prefix);
            let nesting_suffix = Arc::clone(&nesting_suffix);
            let nesting_options_separator = Arc::clone(&nesting_options_separator);
            move |mut acc: AccumulatorType, path: &std::path::PathBuf| {
                match extract_from_file_with_warnings(
                    path,
                    functions,
                    &trans_components,
                    &use_translation_names,
                    extract_from_comments,
                    plural_config,
                    &nesting_prefix,
                    &nesting_suffix,
                    &nesting_options_separator,
                ) {
                    Ok((keys, warnings)) => {
                        acc.1 += warnings;
                        // Insert into HashSet for deduplication
                        for key in keys {
                            acc.0.insert(key, ());
                        }
                    }
                    Err(e) => {
                        acc.1 += 1;
                        acc.2.push(ExtractionError {
                            file_path: path.display().to_string(),
                            message: e.to_string(),
                        });
                    }
                }
                acc
            }
        })
        .reduce(
            || initial.clone(),
            |mut a, b| {
                // Merge HashMaps from different threads
                a.0.extend(b.0);
                a.1 += b.1;
                a.2.extend(b.2);
                a
            },
        );

    // Add glob errors
    errors.extend(glob_errors);

    Ok((unique_keys, warning_count, errors))
}

fn matches_ignore_path(path: &Path, patterns: &[Pattern]) -> bool {
    patterns.iter().any(|pattern| pattern.matches_path(path))
}

fn compile_ignore_patterns(patterns: &[String]) -> Result<Vec<Pattern>> {
    let mut compiled = Vec::new();
    for pattern in patterns {
        let matcher = Pattern::new(pattern)
            .with_context(|| format!("Invalid ignore glob pattern: {}", pattern))?;
        compiled.push(matcher);
    }
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_extract_simple_t_call() {
        let source = r#"
            const text = t('hello.world');
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "hello.world");
        assert_eq!(keys[0].namespace, None);
    }

    #[test]
    fn test_extract_i18n_t_call() {
        let source = r#"
            const text = i18n.t('greeting');
        "#;

        let keys = extract_from_source(source, "test.ts", &["i18n.t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
    }

    #[test]
    fn test_extract_with_namespace() {
        let source = r#"
            const text = t('common:button.submit');
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "button.submit");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_extract_multiple_calls() {
        let source = r#"
            const a = t('key1');
            const b = t('key2');
            const c = i18n.t('key3');
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string(), "i18n.t".to_string()])
            .unwrap();

        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_tsx_jsx_support() {
        let source = r#"
            function Component() {
                return <div>{t('jsx.key')}</div>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "jsx.key");
    }

    #[test]
    fn test_trans_component() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="hello">World</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "hello");
    }

    #[test]
    fn test_trans_with_namespace() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="common:greeting" />;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_trans_with_defaults() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="welcome" defaults="Hello there!" />;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "welcome");
        assert_eq!(keys[0].default_value, Some("Hello there!".to_string()));
    }

    #[test]
    fn test_plurals_with_count() {
        let source = r#"
            const text = t('apple', { count: 5 });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "apple_one"));
        assert!(keys.iter().any(|k| k.key == "apple_other"));
    }

    #[test]
    fn test_plurals_with_count_shorthand() {
        let source = r#"
            const count = 5;
            const text = t('item', { count });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "item_one"));
        assert!(keys.iter().any(|k| k.key == "item_other"));
    }

    #[test]
    fn test_context() {
        let source = r#"
            const text = t('friend', { context: 'male' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "friend_male");
    }

    #[test]
    fn test_plurals_with_context() {
        let source = r#"
            const text = t('friend', { count: 2, context: 'female' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "friend_female_one"));
        assert!(keys.iter().any(|k| k.key == "friend_female_other"));
    }

    #[test]
    fn test_template_literal_simple() {
        let source = r#"
            const text = t(`hello.world`);
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "hello.world");
    }

    #[test]
    fn test_template_literal_with_namespace() {
        let source = r#"
            const text = t(`common:button.save`);
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "button.save");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_template_literal_with_interpolation_ignored() {
        let source = r#"
            const key = 'dynamic';
            const text = t(`hello.${key}`);
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        // Template literals with interpolations should be skipped
        assert_eq!(keys.len(), 0);
    }

    #[test]
    fn test_template_literal_dynamic_warning() {
        let source = r#"
            const id = 123;
            const text = t(`key_${id}`);
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        // Dynamic template literals should be skipped (no keys extracted)
        assert_eq!(keys.len(), 0);

        // Note: The warning is printed to stderr via eprintln! in warn_dynamic_template_literal.
        // The warning format is: "Warning: Dynamic template literal found at test.ts:3:XX.
        // Translation key extraction skipped. Consider using i18next-extract-disable-line if intentional."
        // This is verified by manual testing and code review.
    }

    #[test]
    fn test_trans_children_as_key() {
        let source = r#"
            function Component() {
                return <Trans>Hello World</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "Hello World");
        assert_eq!(keys[0].default_value, Some("Hello World".to_string()));
    }

    #[test]
    fn test_trans_children_with_html() {
        let source = r#"
            function Component() {
                return <Trans>Hello <strong>World</strong></Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert!(keys[0].key.contains("Hello"));
        assert!(keys[0].key.contains("<strong>"));
    }

    #[test]
    fn test_trans_ns_attribute() {
        let source = r#"
            function Component() {
                return <Trans ns="common" i18nKey="greeting">Hello</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_trans_count_attribute() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="item" count={5}>items</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "item_one"));
        assert!(keys.iter().any(|k| k.key == "item_other"));
    }

    #[test]
    fn test_trans_children_with_ns_and_count() {
        let source = r#"
            function Component() {
                return <Trans ns="shop" i18nKey="product" count={3}>products</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys
            .iter()
            .any(|k| k.key == "product_one" && k.namespace == Some("shop".to_string())));
        assert!(keys
            .iter()
            .any(|k| k.key == "product_other" && k.namespace == Some("shop".to_string())));
    }

    #[test]
    fn test_trans_context_attribute() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="friend" context="male">Male friend</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "friend_male");
    }

    #[test]
    fn test_trans_context_with_count() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="friend" context="female" count={2}>Female friends</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "friend_female_one"));
        assert!(keys.iter().any(|k| k.key == "friend_female_other"));
    }

    #[test]
    fn test_trans_context_with_ns() {
        let source = r#"
            function Component() {
                return <Trans ns="common" i18nKey="user" context="admin">Admin user</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "user_admin");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_trans_dynamic_context_ternary() {
        let source = r#"
            function Component({ isMale }) {
                return <Trans i18nKey="friend" context={isMale ? 'male' : 'female'}>Friend</Trans>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();
        assert!(keys.iter().any(|k| k.key == "friend_male"));
        assert!(keys.iter().any(|k| k.key == "friend_female"));
        assert!(keys.iter().any(|k| k.key == "friend"));
    }

    #[test]
    fn test_trans_dynamic_context_with_count() {
        let source = r#"
            function Component({ count, variant }) {
                return (
                    <Trans i18nKey="fruit" count={count} context={variant ? 'ripe' : 'fresh'}>
                        Fruit
                    </Trans>
                );
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();
        assert!(keys.iter().any(|k| k.key == "fruit_ripe_one"));
        assert!(keys.iter().any(|k| k.key == "fruit_ripe_other"));
        assert!(keys.iter().any(|k| k.key == "fruit_fresh_one"));
        assert!(keys.iter().any(|k| k.key == "fruit_fresh_other"));
        assert!(keys.iter().any(|k| k.key == "fruit_one"));
        assert!(keys.iter().any(|k| k.key == "fruit_other"));
    }

    #[test]
    fn test_use_translation_with_namespace() {
        let source = r#"
            function Component() {
                const { t } = useTranslation('common');
                return <div>{t('greeting')}</div>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_use_translation_with_key_prefix() {
        let source = r#"
            function Component() {
                const { t } = useTranslation('ns', { keyPrefix: 'user' });
                return <div>{t('name')}</div>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "user.name");
        assert_eq!(keys[0].namespace, Some("ns".to_string()));
    }

    #[test]
    fn test_use_translation_key_prefix_only() {
        let source = r#"
            function Component() {
                const { t } = useTranslation({ keyPrefix: 'settings' });
                return <div>{t('theme')}</div>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "settings.theme");
    }

    #[test]
    fn test_use_translation_array_destructure() {
        let source = r#"
            function Component() {
                const [t] = useTranslation('common');
                return <div>{t('hello')}</div>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "hello");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_use_translation_alias() {
        let source = r#"
            function Component() {
                const { t: translate } = useTranslation('common');
                return <div>{translate('world')}</div>;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["translate".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "world");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_get_fixed_t_with_namespace() {
        let source = r#"
            const t = i18n.getFixedT('en', 'common');
            const text = t('greeting');
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_selector_api_extracts_key_path() {
        let source = r#"
            function Component() {
                return t($ => $.user.profile.name);
            }
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "user.profile.name");
    }

    #[test]
    fn test_use_translation_names_custom_hook() {
        let source = r#"
            function Component() {
                const { t } = loadPageTranslations('common', { keyPrefix: 'user' });
                return t('name');
            }
        "#;
        let plural_config = PluralConfig::default();
        let trans_components = vec!["Trans".to_string()];
        let hooks = vec![UseTranslationName::Detailed(
            crate::config::UseTranslationNameDetails {
                name: "loadPageTranslations".to_string(),
                ns_arg: 0,
                key_prefix_arg: 1,
            },
        )];

        let (keys, _) = extract_from_source_with_warnings(
            source,
            "test.tsx",
            &["t".to_string()],
            &trans_components,
            &hooks,
            true,
            &plural_config,
            "$t(",
            ")",
            ",",
        )
        .unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].namespace, Some("common".to_string()));
        assert_eq!(keys[0].key, "user.name");
    }

    #[test]
    fn test_nested_translation_with_custom_nesting_syntax() {
        let source = r#"
            t('outer', { defaultValue: 'Nested: __nest__("inner.key")' });
        "#;
        let plural_config = PluralConfig::default();
        let trans_components = vec!["Trans".to_string()];
        let hooks = vec![UseTranslationName::Name("useTranslation".to_string())];
        let (keys, _) = extract_from_source_with_warnings(
            source,
            "test.ts",
            &["t".to_string()],
            &trans_components,
            &hooks,
            true,
            &plural_config,
            "__nest__(",
            ")",
            ",",
        )
        .unwrap();

        assert!(keys.iter().any(|k| k.key == "outer"));
        assert!(keys.iter().any(|k| k.key == "inner.key"));
    }

    #[test]
    fn test_ordinal_plural_generation() {
        let source = r#"
            t('rank', { count: n, ordinal: true });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();
        assert!(keys.iter().any(|k| k.key == "rank_ordinal_one"));
        assert!(keys.iter().any(|k| k.key == "rank_ordinal_other"));
    }

    #[test]
    fn test_return_objects_generates_preserve_marker() {
        let source = r#"
            t('countries', { returnObjects: true });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();
        assert!(keys.iter().any(|k| k.key == "countries.*"));
        assert!(!keys.iter().any(|k| k.key == "countries"));
    }

    #[test]
    fn test_get_fixed_t_with_key_prefix() {
        let source = r#"
            const t = getFixedT('en', 'ns', 'user.profile');
            const text = t('name');
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "user.profile.name");
        assert_eq!(keys[0].namespace, Some("ns".to_string()));
    }

    #[test]
    fn test_default_value_extraction() {
        let source = r#"
            const text = t('greeting', { defaultValue: 'Hello World!' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].default_value, Some("Hello World!".to_string()));
    }

    #[test]
    fn test_default_value_with_namespace() {
        let source = r#"
            const text = t('common:welcome', { defaultValue: 'Welcome back!' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "welcome");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
        assert_eq!(keys[0].default_value, Some("Welcome back!".to_string()));
    }

    #[test]
    fn test_default_value_with_count() {
        let source = r#"
            const text = t('item', { count: 5, defaultValue: '{{count}} items' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(
            keys.iter()
                .any(|k| k.key == "item_one"
                    && k.default_value == Some("{{count}} items".to_string()))
        );
        assert!(keys.iter().any(
            |k| k.key == "item_other" && k.default_value == Some("{{count}} items".to_string())
        ));
    }

    #[test]
    fn test_extract_from_single_line_comment() {
        let source = r#"
            // t('comment.key')
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "comment.key");
    }

    #[test]
    fn test_extract_from_block_comment() {
        let source = r#"
            /* t('block.key') */
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "block.key");
    }

    #[test]
    fn test_extract_from_comment_with_default() {
        let source = r#"
            // t('greeting', 'Hello!')
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].default_value, Some("Hello!".to_string()));
    }

    #[test]
    fn test_extract_from_comment_with_options() {
        let source = r#"
            // t('message', { defaultValue: 'Default message' })
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "message");
        assert_eq!(keys[0].default_value, Some("Default message".to_string()));
    }

    #[test]
    fn test_extract_from_comment_with_namespace() {
        let source = r#"
            // t('common:nav.home')
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "nav.home");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_extract_from_comment_with_count_option() {
        let source = r#"
            // t('notification', { count: items.length })
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "notification_one"));
        assert!(keys.iter().any(|k| k.key == "notification_other"));
    }

    #[test]
    fn test_extract_from_comment_with_context_option() {
        let source = r#"
            // t('greeting', { context: 'formal' })
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting_formal");
    }

    #[test]
    fn test_extract_from_comment_with_ns_option() {
        let source = r#"
            // t('button.save', { ns: 'common' })
            const x = 1;
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "button.save");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_nested_translation_in_default_value() {
        let source = r#"
            const text = t('greeting', { defaultValue: 'Hello $t(world)!' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "greeting"));
        assert!(keys.iter().any(|k| k.key == "world"));
    }

    #[test]
    fn test_nested_translation_with_namespace() {
        let source = r#"
            const text = t('message', { defaultValue: 'See $t(common:link)' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "message"));
        assert!(keys
            .iter()
            .any(|k| k.key == "link" && k.namespace == Some("common".to_string())));
    }

    #[test]
    fn test_multiple_nested_translations() {
        let source = r#"
            const text = t('full', { defaultValue: '$t(hello), $t(world)!' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 3);
        assert!(keys.iter().any(|k| k.key == "full"));
        assert!(keys.iter().any(|k| k.key == "hello"));
        assert!(keys.iter().any(|k| k.key == "world"));
    }

    #[test]
    fn test_nested_translation_with_options() {
        let source = r#"
            const text = t('count_msg', { defaultValue: 'You have $t(item, { count: {{count}} })' });
        "#;

        let keys = extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 3);
        assert!(keys.iter().any(|k| k.key == "count_msg"));
        assert!(keys.iter().any(|k| k.key == "item_one"));
        assert!(keys.iter().any(|k| k.key == "item_other"));
    }

    #[test]
    fn test_nested_translation_in_trans_defaults() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="greeting" defaults="Hello $t(name)!" />;
            }
        "#;

        let keys = extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "greeting"));
        assert!(keys.iter().any(|k| k.key == "name"));
    }

    fn extract_from_virtual_file(
        content: &str,
        file_name: &str,
        functions: &[String],
    ) -> Vec<ExtractedKey> {
        let dir = tempdir().unwrap();
        let path = dir.path().join(file_name);
        fs::write(&path, content).unwrap();
        extract_from_file_with_options(&path, functions, true, &PluralConfig::default()).unwrap()
    }

    #[test]
    fn test_vue_component_script_and_template() {
        let source = r#"
            <template>
              <div>{{ $t('template.title') }}</div>
              <button :title="t('template.tooltip', { defaultValue: 'Tooltip' })"></button>
            </template>
            <script setup lang="ts">
            const label = t('script.key');
            </script>
        "#;

        let functions = vec!["t".to_string()];
        let keys = extract_from_virtual_file(source, "component.vue", &functions);

        assert_eq!(keys.len(), 3);
        assert!(keys.iter().any(|k| k.key == "script.key"));
        assert!(keys.iter().any(|k| k.key == "template.title"));
        let tooltip = keys
            .iter()
            .find(|k| k.key == "template.tooltip")
            .expect("template tooltip key");
        assert_eq!(tooltip.default_value.as_deref(), Some("Tooltip"));
    }

    #[test]
    fn test_svelte_component_script_and_markup() {
        let source = r#"
            <script>
              const heading = t('script.value', { defaultValue: 'Value' });
            </script>

            <h1>{$t('template.header')}</h1>
        "#;

        let functions = vec!["t".to_string()];
        let keys = extract_from_virtual_file(source, "component.svelte", &functions);

        assert_eq!(keys.len(), 2);
        let script_key = keys
            .iter()
            .find(|k| k.key == "script.value")
            .expect("script key");
        assert_eq!(script_key.default_value.as_deref(), Some("Value"));
        assert!(keys.iter().any(|k| k.key == "template.header"));
    }

    /// Test that regex-based comment extractors compile successfully.
    #[test]
    fn test_regex_initialization() {
        // Force initialization
        let _ = get_comment_single_arg_regex();
        let _ = get_comment_with_default_regex();
        let _ = get_comment_with_options_regex();

        assert!(get_comment_single_arg_regex().is_match("t('key')"));
        assert!(get_comment_single_arg_regex().is_match("t(\"key\")"));
        assert!(get_comment_single_arg_regex().is_match("t(`key`)"));

        assert!(get_comment_with_default_regex().is_match("t('key', 'default')"));
        assert!(get_comment_with_default_regex().is_match("t(\"key\", \"default\")"));

        assert!(get_comment_with_options_regex().is_match("t('key', { defaultValue: 'value' })"));
        assert!(get_comment_with_options_regex()
            .is_match("t('key', { other: 1, defaultValue: 'value' })"));
    }
}
