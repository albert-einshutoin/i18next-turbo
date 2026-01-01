use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap, Span};
use swc_ecma_ast::{
    CallExpr, Callee, Expr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElementName,
    JSXOpeningElement, Lit, MemberProp, ObjectLit, Prop, PropName, PropOrSpread,
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
    /// Lines disabled via magic comments
    disabled_lines: HashSet<u32>,
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

    /// Extract string literal from the first argument
    fn extract_key_from_args(&self, call: &CallExpr) -> Option<String> {
        call.args.first().and_then(|arg| {
            if let Expr::Lit(Lit::Str(s)) = arg.expr.as_ref() {
                s.value.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
    }

    /// Check if call has count option (for plurals)
    fn has_count_option(&self, call: &CallExpr) -> bool {
        self.get_option_value(call, "count").is_some()
    }

    /// Check if call has context option
    fn get_context_option(&self, call: &CallExpr) -> Option<String> {
        self.get_option_value(call, "context")
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
}

impl Visit for TranslationVisitor {
    fn visit_call_expr(&mut self, call: &CallExpr) {
        // Check magic comments
        if self.is_disabled(call.span) {
            call.visit_children_with(self);
            return;
        }

        if self.is_translation_call(&call.callee) {
            if let Some(key) = self.extract_key_from_args(call) {
                let (namespace, base_key) = self.parse_key_with_namespace(&key);

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
                        namespace: namespace.clone(),
                        default_value: None,
                    });
                    self.keys.push(ExtractedKey {
                        key: key_other,
                        namespace,
                        default_value: None,
                    });
                } else if let Some(ctx) = context {
                    // Context without count
                    self.keys.push(ExtractedKey {
                        key: format!("{}_{}", base_key, ctx),
                        namespace,
                        default_value: None,
                    });
                } else {
                    // Regular key
                    self.keys.push(ExtractedKey {
                        key: base_key,
                        namespace,
                        default_value: None,
                    });
                }
            }
        }

        // Continue visiting child nodes
        call.visit_children_with(self);
    }

    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement) {
        // Check magic comments
        if self.is_disabled(elem.span) {
            elem.visit_children_with(self);
            return;
        }

        // Check if this is a Trans component
        if let JSXElementName::Ident(ident) = &elem.name {
            if self.trans_components.contains(&ident.sym.to_string()) {
                // Extract i18nKey attribute
                if let Some(key) = self.extract_trans_key(elem) {
                    let (namespace, base_key) = self.parse_key_with_namespace(&key);
                    let default_value = self.extract_trans_defaults(elem);

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
    module.visit_with(&mut visitor);

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
}
