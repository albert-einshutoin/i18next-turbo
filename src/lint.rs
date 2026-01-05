use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap, Span};
use swc_ecma_ast::{
    JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElement, JSXElementChild, JSXElementName,
    JSXText,
};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_visit::{Visit, VisitWith};

/// A lint issue found in source code
#[derive(Debug)]
pub struct LintIssue {
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub text: String,
}

/// Result of linting multiple files
#[derive(Debug, Default)]
pub struct LintResult {
    pub issues: Vec<LintIssue>,
    pub files_checked: usize,
}

/// Visitor that finds hardcoded strings in JSX
pub struct LintVisitor {
    /// Source map for line number lookup
    source_map: Lrc<SourceMap>,
    /// Lint issues found
    pub issues: Vec<LintIssue>,
    /// File path being linted
    file_path: String,
    /// Tags to ignore (e.g., script, style)
    ignored_tags: HashSet<String>,
    /// Attributes to check (e.g., alt, title, placeholder)
    checked_attributes: HashSet<String>,
    /// Whether we're inside a Trans component
    in_trans: bool,
}

impl LintVisitor {
    pub fn new(source_map: Lrc<SourceMap>, file_path: String) -> Self {
        let mut ignored_tags = HashSet::new();
        ignored_tags.insert("script".to_string());
        ignored_tags.insert("style".to_string());
        ignored_tags.insert("code".to_string());
        ignored_tags.insert("pre".to_string());

        let mut checked_attributes = HashSet::new();
        checked_attributes.insert("alt".to_string());
        checked_attributes.insert("title".to_string());
        checked_attributes.insert("placeholder".to_string());
        checked_attributes.insert("aria-label".to_string());
        checked_attributes.insert("aria-description".to_string());

        Self {
            source_map,
            issues: Vec::new(),
            file_path,
            ignored_tags,
            checked_attributes,
            in_trans: false,
        }
    }

    /// Get line and column from span
    fn get_location(&self, span: Span) -> (usize, usize) {
        let loc = self.source_map.lookup_char_pos(span.lo);
        (loc.line, loc.col_display + 1)
    }

    /// Check if text looks like it should be translated
    fn should_be_translated(&self, text: &str) -> bool {
        let trimmed = text.trim();

        // Skip empty or whitespace-only
        if trimmed.is_empty() {
            return false;
        }

        // Skip if it's just punctuation or numbers
        if trimmed.chars().all(|c| !c.is_alphabetic()) {
            return false;
        }

        // Skip very short strings (likely not user-facing)
        if trimmed.len() < 2 {
            return false;
        }

        // Skip if it looks like a variable or code
        if trimmed.starts_with('{') || trimmed.starts_with('$') {
            return false;
        }

        // Skip common non-translatable patterns
        let skip_patterns = [
            "className",
            "onClick",
            "onChange",
            "onSubmit",
            "px",
            "em",
            "rem",
            "%",
            "vh",
            "vw",
        ];
        if skip_patterns.contains(&trimmed) {
            return false;
        }

        true
    }
}

impl Visit for LintVisitor {
    fn visit_jsx_element(&mut self, elem: &JSXElement) {
        // Check if this is a Trans component
        let is_trans = if let JSXElementName::Ident(ident) = &elem.opening.name {
            ident.sym.as_ref() == "Trans"
        } else {
            false
        };

        // Check if this is an ignored tag
        let is_ignored = if let JSXElementName::Ident(ident) = &elem.opening.name {
            self.ignored_tags
                .contains(&ident.sym.to_string().to_lowercase())
        } else {
            false
        };

        if is_ignored {
            return;
        }

        let was_in_trans = self.in_trans;
        if is_trans {
            self.in_trans = true;
        }

        // Check attributes for hardcoded strings
        if !self.in_trans {
            for attr in &elem.opening.attrs {
                if let JSXAttrOrSpread::JSXAttr(jsx_attr) = attr {
                    if let JSXAttrName::Ident(name) = &jsx_attr.name {
                        let attr_name = name.sym.to_string();
                        if self.checked_attributes.contains(&attr_name) {
                            if let Some(JSXAttrValue::Str(s)) = &jsx_attr.value {
                                if let Some(text) = s.value.as_str().map(|v| v.to_string()) {
                                    if self.should_be_translated(&text) {
                                        let (line, column) = self.get_location(s.span);
                                        self.issues.push(LintIssue {
                                            file_path: self.file_path.clone(),
                                            line,
                                            column,
                                            message: format!(
                                                "Hardcoded string in '{}' attribute should be translated",
                                                attr_name
                                            ),
                                            text,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check children for hardcoded text
        if !self.in_trans {
            for child in &elem.children {
                if let JSXElementChild::JSXText(text) = child {
                    self.check_jsx_text(text);
                }
            }
        }

        // Visit children
        elem.visit_children_with(self);

        self.in_trans = was_in_trans;
    }
}

impl LintVisitor {
    fn check_jsx_text(&mut self, text: &JSXText) {
        let value = text.value.to_string();
        if self.should_be_translated(&value) {
            let (line, column) = self.get_location(text.span);
            self.issues.push(LintIssue {
                file_path: self.file_path.clone(),
                line,
                column,
                message: "Hardcoded text in JSX should be translated".to_string(),
                text: value.trim().to_string(),
            });
        }
    }
}

/// Lint a single file for hardcoded strings
pub fn lint_file<P: AsRef<Path>>(path: P) -> Result<Vec<LintIssue>> {
    let path = path.as_ref();
    let source_code = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    lint_source(&source_code, path)
}

/// Lint source code string
pub fn lint_source<P: AsRef<Path>>(source: &str, path: P) -> Result<Vec<LintIssue>> {
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

    // Only lint JSX files
    if !is_tsx {
        return Ok(Vec::new());
    }

    let syntax = Syntax::Typescript(TsSyntax {
        tsx: is_tsx,
        decorators: true,
        ..Default::default()
    });

    let lexer = Lexer::new(syntax, Default::default(), StringInput::from(&*fm), None);

    let mut parser = Parser::new_from(lexer);

    let module = match parser.parse_module() {
        Ok(module) => module,
        Err(e) => {
            eprintln!(
                "Warning: Failed to parse {}: {:?}",
                path.display(),
                e.kind()
            );
            return Ok(Vec::new());
        }
    };

    let mut visitor = LintVisitor::new(cm, path.display().to_string());
    module.visit_with(&mut visitor);

    Ok(visitor.issues)
}

/// Lint multiple files using glob patterns
pub fn lint_from_glob(patterns: &[String]) -> Result<LintResult> {
    let mut result = LintResult::default();

    for pattern in patterns {
        let matches =
            glob::glob(pattern).with_context(|| format!("Invalid glob pattern: {}", pattern))?;

        for entry in matches {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        result.files_checked += 1;
                        match lint_file(&path) {
                            Ok(issues) => result.issues.extend(issues),
                            Err(e) => eprintln!("Warning: {}", e),
                        }
                    }
                }
                Err(e) => eprintln!("Warning: Glob error: {}", e),
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_hardcoded_text() {
        let source = r#"
            function Component() {
                return <div>Hello World</div>;
            }
        "#;

        let issues = lint_source(source, "test.tsx").unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].text.contains("Hello World"));
    }

    #[test]
    fn test_lint_trans_ignored() {
        let source = r#"
            function Component() {
                return <Trans>Hello World</Trans>;
            }
        "#;

        let issues = lint_source(source, "test.tsx").unwrap();
        assert_eq!(issues.len(), 0);
    }

    #[test]
    fn test_lint_attribute() {
        let source = r#"
            function Component() {
                return <img alt="A beautiful image" />;
            }
        "#;

        let issues = lint_source(source, "test.tsx").unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("alt"));
    }

    #[test]
    fn test_lint_empty_text_ignored() {
        let source = r#"
            function Component() {
                return <div>   </div>;
            }
        "#;

        let issues = lint_source(source, "test.tsx").unwrap();
        assert_eq!(issues.len(), 0);
    }
}
