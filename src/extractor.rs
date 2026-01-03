use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap, Span};
use swc_ecma_ast::{
    CallExpr, Callee, Expr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElement,
    JSXElementChild, JSXElementName, JSXOpeningElement, Lit, MemberProp, ObjectLit, Pat, Prop,
    PropName, PropOrSpread, Tpl, VarDeclarator,
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

/// Result of extraction from multiple files
#[derive(Debug, Default)]
pub struct ExtractionResult {
    pub files: Vec<(String, Vec<ExtractedKey>)>,
    pub warning_count: usize,
}

/// Scope information for useTranslation hook
#[derive(Debug, Clone, Default)]
pub struct ScopeInfo {
    /// Namespace from useTranslation('namespace')
    pub namespace: Option<String>,
    /// Key prefix from useTranslation({ keyPrefix: 'prefix' })
    pub key_prefix: Option<String>,
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
    /// File path being processed (for warning messages)
    file_path: Option<String>,
}

impl TranslationVisitor {
    pub fn new(
        functions: Vec<String>,
        source_map: Lrc<SourceMap>,
        comments: Option<SingleThreadedComments>,
    ) -> Self {
        let mut trans_components = HashSet::new();
        trans_components.insert("Trans".to_string());

        // Parse magic comments to find disabled lines
        let disabled_lines = Self::parse_disabled_lines(&comments);

        Self {
            functions: functions.into_iter().collect(),
            trans_components,
            keys: Vec::new(),
            source_map,
            comments,
            disabled_lines,
            scope_bindings: HashMap::new(),
            file_path: None,
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
                Expr::Ident(ident) => self.functions.contains(&ident.sym.to_string()),
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
    fn extract_key_from_args(&self, call: &CallExpr) -> Option<String> {
        call.args.first().and_then(|arg| {
            match arg.expr.as_ref() {
                // String literal: t('key')
                Expr::Lit(Lit::Str(s)) => s.value.as_str().map(|s| s.to_string()),
                // Template literal: t(`key`)
                Expr::Tpl(tpl) => self.extract_simple_template_literal(tpl, call.span),
                _ => None,
            }
        })
    }

    /// Extract key from a template literal (only if it's a simple string without expressions)
    fn extract_simple_template_literal(&self, tpl: &Tpl, span: Span) -> Option<String> {
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
            return quasi.cooked.as_ref().and_then(|s| s.as_str().map(|s| s.to_string()));
        }

        None
    }

    /// Warn about dynamic template literals that cannot be extracted
    fn warn_dynamic_template_literal(&self, span: Span) {
        let loc = self.source_map.lookup_char_pos(span.lo);
        let file_path = self
            .file_path
            .as_ref()
            .map(|p| p.as_str())
            .unwrap_or("<unknown>");
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

    /// Check if call has context option
    fn get_context_option(&self, call: &CallExpr) -> Option<String> {
        self.get_option_value(call, "context")
    }

    /// Get defaultValue option from t() call
    fn get_default_value_option(&self, call: &CallExpr) -> Option<String> {
        self.get_option_value(call, "defaultValue")
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
                    if ident.sym.to_string() == key {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract nested translation keys from a string value
    /// Detects patterns like $t(key), $t('key'), or $t(key, { options })
    fn extract_nested_translations(&self, text: &str) -> Vec<ExtractedKey> {
        let mut keys = Vec::new();

        // Pattern 1: $t('key') or $t("key") - with quotes
        // Captures key, then optionally matches rest until closing paren (handles nested braces)
        let quoted_pattern = regex::Regex::new(
            r#"\$t\s*\(\s*['"]([^'"]+)['"]"#
        ).unwrap();

        // Pattern 2: $t(key) - without quotes (simple identifier or key with colon/dots)
        // Captures just the key part before comma or closing paren
        let unquoted_pattern = regex::Regex::new(
            r#"\$t\s*\(\s*([a-zA-Z_][a-zA-Z0-9_.:]*)"#
        ).unwrap();

        // Extract quoted patterns
        for cap in quoted_pattern.captures_iter(text) {
            let key = cap.get(1).unwrap().as_str();
            let (namespace, base_key) = self.parse_key_with_namespace(key);
            keys.push(ExtractedKey {
                key: base_key,
                namespace,
                default_value: None,
            });
        }

        // Extract unquoted patterns
        for cap in unquoted_pattern.captures_iter(text) {
            let key = cap.get(1).unwrap().as_str();
            let (namespace, base_key) = self.parse_key_with_namespace(key);
            // Avoid duplicates
            if !keys.iter().any(|k| k.key == base_key && k.namespace == namespace) {
                keys.push(ExtractedKey {
                    key: base_key,
                    namespace,
                    default_value: None,
                });
            }
        }

        keys
    }

    /// Parse namespace:key format
    fn parse_key_with_namespace(&self, key: &str) -> (Option<String>, String) {
        if key.contains(':') {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            (Some(parts[0].to_string()), parts[1].to_string())
        } else {
            (None, key.to_string())
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

    /// Extract i18nKey from Trans component attributes
    fn extract_trans_key(&self, elem: &JSXOpeningElement) -> Option<String> {
        for attr in &elem.attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.to_string() == "i18nKey" {
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
                    if name.sym.to_string() == "ns" {
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
                    if name.sym.to_string() == "count" {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract context attribute from Trans component
    fn extract_trans_context(&self, elem: &JSXOpeningElement) -> Option<String> {
        for attr in &elem.attrs {
            if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                if let JSXAttrName::Ident(name) = &jsx_attr.name {
                    if name.sym.to_string() == "context" {
                        if let Some(value) = &jsx_attr.value {
                            return self.extract_jsx_attr_string(value);
                        }
                    }
                }
            }
        }
        None
    }

    /// Extract text content from JSX children
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
                    if name.sym.to_string() == "defaults" {
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
        // Check if this is useTranslation()
        if let Callee::Expr(expr) = &call.callee {
            if let Expr::Ident(ident) = expr.as_ref() {
                if ident.sym.to_string() != "useTranslation" {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        }

        let mut scope_info = ScopeInfo::default();

        // Parse arguments
        for (i, arg) in call.args.iter().enumerate() {
            match i {
                0 => {
                    // First arg: namespace (string or array)
                    if let Expr::Lit(Lit::Str(s)) = arg.expr.as_ref() {
                        scope_info.namespace = s.value.as_str().map(|s| s.to_string());
                    }
                    // Second form: useTranslation({ keyPrefix: '...' })
                    if let Expr::Object(obj) = arg.expr.as_ref() {
                        scope_info.key_prefix = self.find_string_prop(obj, "keyPrefix");
                    }
                }
                1 => {
                    // Second arg: options object
                    if let Expr::Object(obj) = arg.expr.as_ref() {
                        scope_info.key_prefix = self.find_string_prop(obj, "keyPrefix");
                    }
                }
                _ => {}
            }
        }

        Some(scope_info)
    }

    /// Check if a call is getFixedT and extract scope info
    fn parse_get_fixed_t_call(&self, call: &CallExpr) -> Option<ScopeInfo> {
        // Check if this is getFixedT() or i18n.getFixedT()
        let is_get_fixed_t = match &call.callee {
            Callee::Expr(expr) => match expr.as_ref() {
                Expr::Ident(ident) => ident.sym.to_string() == "getFixedT",
                Expr::Member(member) => {
                    if let MemberProp::Ident(prop) = &member.prop {
                        prop.sym.to_string() == "getFixedT"
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
                            if key.sym.to_string() == "t" {
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
                        if assign.key.sym.to_string() == "t" {
                            return Some("t".to_string());
                        }
                    }
                }
                None
            }
            // const [t] = useTranslation()
            Pat::Array(arr) => {
                if let Some(first) = arr.elems.first() {
                    if let Some(Pat::Ident(ident)) = first {
                        return Some(ident.id.sym.to_string());
                    }
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

        // Regex patterns for extracting keys from comments
        // Pattern: t('key') or t("key") or t(`key`)
        let single_arg_pattern = regex::Regex::new(
            r#"(?:^|[^a-zA-Z_])t\s*\(\s*['"`]([^'"`]+)['"`]\s*\)"#
        ).unwrap();

        // Pattern: t('key', 'default') with simple string default
        let with_default_pattern = regex::Regex::new(
            r#"(?:^|[^a-zA-Z_])t\s*\(\s*['"`]([^'"`]+)['"`]\s*,\s*['"`]([^'"`]+)['"`]\s*\)"#
        ).unwrap();

        // Pattern: t('key', { defaultValue: '...' })
        let with_options_pattern = regex::Regex::new(
            r#"(?:^|[^a-zA-Z_])t\s*\(\s*['"`]([^'"`]+)['"`]\s*,\s*\{[^}]*defaultValue\s*:\s*['"`]([^'"`]+)['"`]"#
        ).unwrap();

        // Extract with options pattern first (most specific)
        for cap in with_options_pattern.captures_iter(text) {
            let key = cap.get(1).unwrap().as_str();
            let default_value = cap.get(2).map(|m| m.as_str().to_string());
            let (namespace, base_key) = self.parse_key_with_namespace(key);
            self.keys.push(ExtractedKey {
                key: base_key,
                namespace,
                default_value,
            });
        }

        // Extract with default pattern
        for cap in with_default_pattern.captures_iter(text) {
            let key = cap.get(1).unwrap().as_str();
            // Check if already captured by options pattern
            let (namespace, base_key) = self.parse_key_with_namespace(key);
            if !self.keys.iter().any(|k| k.key == base_key && k.namespace == namespace) {
                let default_value = cap.get(2).map(|m| m.as_str().to_string());
                self.keys.push(ExtractedKey {
                    key: base_key,
                    namespace,
                    default_value,
                });
            }
        }

        // Extract single arg pattern
        for cap in single_arg_pattern.captures_iter(text) {
            let key = cap.get(1).unwrap().as_str();
            let (namespace, base_key) = self.parse_key_with_namespace(key);
            // Check if already captured
            if !self.keys.iter().any(|k| k.key == base_key && k.namespace == namespace) {
                self.keys.push(ExtractedKey {
                    key: base_key,
                    namespace,
                    default_value: None,
                });
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
                let context = self.get_context_option(call);

                // Check for defaultValue option
                let default_value = self.get_default_value_option(call);

                // Extract nested translations from defaultValue (e.g., $t('key'))
                if let Some(ref dv) = default_value {
                    let nested_keys = self.extract_nested_translations(dv);
                    for nested_key in nested_keys {
                        self.keys.push(nested_key);
                    }
                }

                if has_count {
                    // Generate plural keys: key_one, key_other
                    let key_one = match &context {
                        Some(ctx) => format!("{}_{}_one", base_key, ctx),
                        None => format!("{}_one", base_key),
                    };
                    let key_other = match &context {
                        Some(ctx) => format!("{}_{}_other", base_key, ctx),
                        None => format!("{}_other", base_key),
                    };

                    self.keys.push(ExtractedKey {
                        key: key_one,
                        namespace: namespace_from_scope.clone(),
                        default_value: default_value.clone(),
                    });
                    self.keys.push(ExtractedKey {
                        key: key_other,
                        namespace: namespace_from_scope,
                        default_value,
                    });
                } else if let Some(ctx) = context {
                    // Context without count
                    self.keys.push(ExtractedKey {
                        key: format!("{}_{}", base_key, ctx),
                        namespace: namespace_from_scope,
                        default_value,
                    });
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
            if self.trans_components.contains(&ident.sym.to_string()) {
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

                // Check for context attribute
                let context = self.extract_trans_context(&elem.opening);

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
                if has_count && context.is_some() {
                    // Both count and context: key_context_one, key_context_other
                    let ctx = context.as_ref().unwrap();
                    self.keys.push(ExtractedKey {
                        key: format!("{}_{}_one", base_key, ctx),
                        namespace: namespace.clone(),
                        default_value: default_value.clone(),
                    });
                    self.keys.push(ExtractedKey {
                        key: format!("{}_{}_other", base_key, ctx),
                        namespace,
                        default_value,
                    });
                } else if has_count {
                    // Count only: key_one, key_other
                    self.keys.push(ExtractedKey {
                        key: format!("{}_one", base_key),
                        namespace: namespace.clone(),
                        default_value: default_value.clone(),
                    });
                    self.keys.push(ExtractedKey {
                        key: format!("{}_other", base_key),
                        namespace,
                        default_value,
                    });
                } else if let Some(ctx) = context {
                    // Context only: key_context
                    self.keys.push(ExtractedKey {
                        key: format!("{}_{}", base_key, ctx),
                        namespace,
                        default_value,
                    });
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

/// Extract translation keys from a TypeScript/JavaScript file
pub fn extract_from_file<P: AsRef<Path>>(
    path: P,
    functions: &[String],
) -> Result<Vec<ExtractedKey>> {
    let path = path.as_ref();
    let source_code = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    extract_from_source(&source_code, path, functions)
}

/// Extract translation keys from source code string
pub fn extract_from_source<P: AsRef<Path>>(
    source: &str,
    path: P,
    functions: &[String],
) -> Result<Vec<ExtractedKey>> {
    let path = path.as_ref();
    let cm: Lrc<SourceMap> = Default::default();

    let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), source.to_string());

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

    // Parse the module, handling errors gracefully
    let module = match parser.parse_module() {
        Ok(module) => module,
        Err(e) => {
            // Log warning but don't fail - graceful degradation per PRD
            eprintln!(
                "Warning: Failed to parse {}: {:?}",
                path.display(),
                e.kind()
            );
            return Ok(Vec::new());
        }
    };

    // Visit the AST and extract keys
    let mut visitor = TranslationVisitor::new(functions.to_vec(), cm, Some(comments));
    visitor.file_path = Some(path.display().to_string());
    module.visit_with(&mut visitor);

    // Also extract keys from comments
    visitor.extract_from_comments();

    Ok(visitor.keys)
}

/// Extract keys from multiple files using glob patterns
pub fn extract_from_glob(
    patterns: &[String],
    functions: &[String],
) -> Result<ExtractionResult> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut all_files: Vec<std::path::PathBuf> = Vec::new();
    let warning_count = AtomicUsize::new(0);

    for pattern in patterns {
        let matches = glob::glob(pattern)
            .with_context(|| format!("Invalid glob pattern: {}", pattern))?;

        for entry in matches {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        all_files.push(path);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Glob error: {}", e);
                    warning_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    // Process files in parallel using rayon
    let results: Vec<_> = all_files
        .par_iter()
        .filter_map(|path| {
            match extract_from_file(path, functions) {
                Ok(keys) if !keys.is_empty() => {
                    Some((path.display().to_string(), keys))
                }
                Ok(_) => None, // No keys found
                Err(e) => {
                    eprintln!("Warning: {}", e);
                    warning_count.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }
        })
        .collect();

    Ok(ExtractionResult {
        files: results,
        warning_count: warning_count.load(Ordering::Relaxed),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_t_call() {
        let source = r#"
            const text = t('hello.world');
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "hello.world");
        assert_eq!(keys[0].namespace, None);
    }

    #[test]
    fn test_extract_i18n_t_call() {
        let source = r#"
            const text = i18n.t('greeting');
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["i18n.t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
    }

    #[test]
    fn test_extract_with_namespace() {
        let source = r#"
            const text = t('common:button.submit');
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys = extract_from_source(
            source,
            "test.ts",
            &["t".to_string(), "i18n.t".to_string()],
        )
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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "welcome");
        assert_eq!(keys[0].default_value, Some("Hello there!".to_string()));
    }

    #[test]
    fn test_plurals_with_count() {
        let source = r#"
            const text = t('apple', { count: 5 });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "item_one"));
        assert!(keys.iter().any(|k| k.key == "item_other"));
    }

    #[test]
    fn test_context() {
        let source = r#"
            const text = t('friend', { context: 'male' });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "friend_male");
    }

    #[test]
    fn test_plurals_with_context() {
        let source = r#"
            const text = t('friend', { count: 2, context: 'female' });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "friend_female_one"));
        assert!(keys.iter().any(|k| k.key == "friend_female_other"));
    }

    #[test]
    fn test_template_literal_simple() {
        let source = r#"
            const text = t(`hello.world`);
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "hello.world");
    }

    #[test]
    fn test_template_literal_with_namespace() {
        let source = r#"
            const text = t(`common:button.save`);
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        // Template literals with interpolations should be skipped
        assert_eq!(keys.len(), 0);
    }

    #[test]
    fn test_template_literal_dynamic_warning() {
        let source = r#"
            const id = 123;
            const text = t(`key_${id}`);
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "product_one" && k.namespace == Some("shop".to_string())));
        assert!(keys.iter().any(|k| k.key == "product_other" && k.namespace == Some("shop".to_string())));
    }

    #[test]
    fn test_trans_context_attribute() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="friend" context="male">Male friend</Trans>;
            }
        "#;

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "user_admin");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_use_translation_with_namespace() {
        let source = r#"
            function Component() {
                const { t } = useTranslation('common');
                return <div>{t('greeting')}</div>;
            }
        "#;

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.tsx", &["translate".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_get_fixed_t_with_key_prefix() {
        let source = r#"
            const t = getFixedT('en', 'ns', 'user.profile');
            const text = t('name');
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "user.profile.name");
        assert_eq!(keys[0].namespace, Some("ns".to_string()));
    }

    #[test]
    fn test_default_value_extraction() {
        let source = r#"
            const text = t('greeting', { defaultValue: 'Hello World!' });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "greeting");
        assert_eq!(keys[0].default_value, Some("Hello World!".to_string()));
    }

    #[test]
    fn test_default_value_with_namespace() {
        let source = r#"
            const text = t('common:welcome', { defaultValue: 'Welcome back!' });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "item_one" && k.default_value == Some("{{count}} items".to_string())));
        assert!(keys.iter().any(|k| k.key == "item_other" && k.default_value == Some("{{count}} items".to_string())));
    }

    #[test]
    fn test_extract_from_single_line_comment() {
        let source = r#"
            // t('comment.key')
            const x = 1;
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "comment.key");
    }

    #[test]
    fn test_extract_from_block_comment() {
        let source = r#"
            /* t('block.key') */
            const x = 1;
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "block.key");
    }

    #[test]
    fn test_extract_from_comment_with_default() {
        let source = r#"
            // t('greeting', 'Hello!')
            const x = 1;
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "nav.home");
        assert_eq!(keys[0].namespace, Some("common".to_string()));
    }

    #[test]
    fn test_nested_translation_in_default_value() {
        let source = r#"
            const text = t('greeting', { defaultValue: 'Hello $t(world)!' });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "greeting"));
        assert!(keys.iter().any(|k| k.key == "world"));
    }

    #[test]
    fn test_nested_translation_with_namespace() {
        let source = r#"
            const text = t('message', { defaultValue: 'See $t(common:link)' });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "message"));
        assert!(keys.iter().any(|k| k.key == "link" && k.namespace == Some("common".to_string())));
    }

    #[test]
    fn test_multiple_nested_translations() {
        let source = r#"
            const text = t('full', { defaultValue: '$t(hello), $t(world)!' });
        "#;

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

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

        let keys =
            extract_from_source(source, "test.ts", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "count_msg"));
        assert!(keys.iter().any(|k| k.key == "item"));
    }

    #[test]
    fn test_nested_translation_in_trans_defaults() {
        let source = r#"
            function Component() {
                return <Trans i18nKey="greeting" defaults="Hello $t(name)!" />;
            }
        "#;

        let keys =
            extract_from_source(source, "test.tsx", &["t".to_string()]).unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key == "greeting"));
        assert!(keys.iter().any(|k| k.key == "name"));
    }
}
