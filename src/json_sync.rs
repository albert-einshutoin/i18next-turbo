#![allow(clippy::too_many_arguments)]

use anyhow::{bail, Context, Result};
use glob::Pattern;
use serde::Serialize;
use serde_json::ser::{Formatter, Serializer};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;

use crate::config::{Config, OutputFormat};
use crate::extractor::ExtractedKey;
use crate::fs::FileSystem;

fn effective_namespace(default_namespace: &str) -> &str {
    if default_namespace.is_empty() {
        "translation"
    } else {
        default_namespace
    }
}

fn merged_namespace_filename(config: &Config) -> String {
    config
        .merged_namespace_filename
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .map(|name| name.to_string())
        .unwrap_or_else(|| effective_namespace(&config.default_namespace).to_string())
}

fn locale_namespace_file_path(
    config: &Config,
    output_dir: &str,
    locale: &str,
    namespace: &str,
) -> std::path::PathBuf {
    let output_ext = config.output_extension();
    let file_stem = if config.merge_namespaces {
        config
            .merged_namespace_filename
            .clone()
            .or_else(|| detect_existing_merged_filename(output_dir, locale, output_ext))
            .unwrap_or_else(|| merged_namespace_filename(config))
    } else {
        namespace.to_string()
    };
    Path::new(output_dir)
        .join(locale)
        .join(format!("{}.{}", file_stem, output_ext))
}

fn detect_existing_merged_filename(output_dir: &str, locale: &str, ext: &str) -> Option<String> {
    let locale_dir = Path::new(output_dir).join(locale);
    let entries = std::fs::read_dir(locale_dir).ok()?;

    let mut stems: Vec<String> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let file_ext = path.extension().and_then(|e| e.to_str())?;
            if file_ext != ext {
                return None;
            }
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    if stems.is_empty() {
        return None;
    }
    stems.sort();
    stems.dedup();

    // Existing merged layouts often use one shared file name per locale (e.g. all.json).
    if stems.len() == 1 {
        return stems.into_iter().next();
    }

    if stems.iter().any(|s| s == "translation") {
        return Some("translation".to_string());
    }

    None
}

fn merge_namespace_key(config: &Config, namespace: &str, key: &str) -> String {
    let separator = if config.key_separator.is_empty() {
        "."
    } else {
        config.key_separator.as_str()
    };
    if key.is_empty() {
        namespace.to_string()
    } else {
        format!("{}{}{}", namespace, separator, key)
    }
}

// =============================================================================
// JSON Style Detection and Custom Formatting
// =============================================================================

/// Detected JSON formatting style from existing file
#[derive(Debug, Clone)]
pub struct JsonStyle {
    /// Indentation string (e.g., "  ", "    ", "\t")
    pub indent: String,
    /// Whether the file uses CRLF line endings
    pub use_crlf: bool,
    /// Whether the file ends with a trailing newline
    pub trailing_newline: bool,
}

impl Default for JsonStyle {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(), // 2 spaces is serde_json default
            use_crlf: false,
            trailing_newline: true,
        }
    }
}

/// Detect JSON formatting style from file content
pub fn detect_json_style(content: &str) -> JsonStyle {
    let mut style = JsonStyle {
        use_crlf: content.contains("\r\n"),
        trailing_newline: content.ends_with('\n') || content.ends_with("\r\n"),
        ..JsonStyle::default()
    };

    // Detect indentation by looking at the first indented line
    // JSON objects start with "{" and the first key is indented
    for line in content.lines() {
        // Skip empty lines and the opening brace
        if line.trim().is_empty() || line.trim() == "{" || line.trim() == "[" {
            continue;
        }

        // Count leading whitespace
        let trimmed = line.trim_start();
        if trimmed.starts_with('"') || trimmed.starts_with('}') || trimmed.starts_with(']') {
            let indent_len = line.len() - trimmed.len();
            if indent_len > 0 {
                style.indent = line[..indent_len].to_string();
                break;
            }
        }
    }

    style
}

/// Custom JSON formatter that respects detected style
struct StylePreservingFormatter {
    indent: Vec<u8>,
    newline: Vec<u8>,
    current_indent: usize,
}

impl StylePreservingFormatter {
    fn new(style: &JsonStyle) -> Self {
        Self {
            indent: style.indent.as_bytes().to_vec(),
            newline: if style.use_crlf {
                b"\r\n".to_vec()
            } else {
                b"\n".to_vec()
            },
            current_indent: 0,
        }
    }
}

impl Formatter for StylePreservingFormatter {
    fn begin_array<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent += 1;
        writer.write_all(b"[")
    }

    fn end_array<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent -= 1;
        writer.write_all(&self.newline)?;
        for _ in 0..self.current_indent {
            writer.write_all(&self.indent)?;
        }
        writer.write_all(b"]")
    }

    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        if first {
            writer.write_all(&self.newline)?;
        } else {
            writer.write_all(b",")?;
            writer.write_all(&self.newline)?;
        }
        for _ in 0..self.current_indent {
            writer.write_all(&self.indent)?;
        }
        Ok(())
    }

    fn end_array_value<W>(&mut self, _writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        Ok(())
    }

    fn begin_object<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent += 1;
        writer.write_all(b"{")
    }

    fn end_object<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.current_indent -= 1;
        writer.write_all(&self.newline)?;
        for _ in 0..self.current_indent {
            writer.write_all(&self.indent)?;
        }
        writer.write_all(b"}")
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        if first {
            writer.write_all(&self.newline)?;
        } else {
            writer.write_all(b",")?;
            writer.write_all(&self.newline)?;
        }
        for _ in 0..self.current_indent {
            writer.write_all(&self.indent)?;
        }
        Ok(())
    }

    fn end_object_key<W>(&mut self, _writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        Ok(())
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        writer.write_all(b": ")
    }

    fn end_object_value<W>(&mut self, _writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + Write,
    {
        Ok(())
    }
}

/// Serialize JSON with style preservation
fn serialize_with_style<W: Write>(writer: W, value: &Value, style: &JsonStyle) -> Result<()> {
    let formatter = StylePreservingFormatter::new(style);
    let mut serializer = Serializer::with_formatter(writer, formatter);
    value.serialize(&mut serializer)?;
    Ok(())
}

/// Represents a conflict when inserting a key into the translation map.
/// Conflicts occur when the key path collides with existing data structures.
#[derive(Debug, Clone)]
pub enum KeyConflict {
    /// Attempted to create a nested structure at a path that already has a scalar value.
    /// Example: trying to add "button.submit" when "button" already exists as a string.
    ValueIsNotObject {
        /// The key path where the conflict occurred (e.g., "button")
        key_path: String,
        /// String representation of the existing value
        existing_value: String,
    },
    /// Attempted to set a scalar value at a path that already has nested children.
    /// Example: trying to add "button" as a string when "button.submit" already exists.
    ObjectIsValue {
        /// The key path where the conflict occurred
        key_path: String,
    },
}

impl std::fmt::Display for KeyConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyConflict::ValueIsNotObject {
                key_path,
                existing_value,
            } => {
                write!(
                    f,
                    "Cannot create nested key at '{}': existing value is '{}' (not an object)",
                    key_path, existing_value
                )
            }
            KeyConflict::ObjectIsValue { key_path } => {
                write!(
                    f,
                    "Cannot set scalar value at '{}': path contains nested objects",
                    key_path
                )
            }
        }
    }
}

/// Result of syncing keys to a locale file
#[derive(Debug, Default)]
pub struct SyncResult {
    pub file_path: String,
    pub added_keys: Vec<String>,
    pub existing_keys: usize,
    /// Keys that were skipped due to conflicts with existing data structures
    pub conflicts: Vec<KeyConflict>,
    pub removed_keys: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct PreserveMatcher {
    key_patterns: Vec<Pattern>,
    namespaced_patterns: Vec<Pattern>,
    ns_separator: String,
}

impl PreserveMatcher {
    fn new(patterns: &[String], ns_separator: &str) -> Result<Self> {
        let mut key_patterns = Vec::new();
        let mut namespaced_patterns = Vec::new();

        for pattern in patterns {
            let compiled = Pattern::new(pattern)
                .with_context(|| format!("Invalid preserve pattern: {}", pattern))?;
            if !ns_separator.is_empty() && pattern.contains(ns_separator) {
                namespaced_patterns.push(compiled);
            } else {
                key_patterns.push(compiled);
            }
        }

        Ok(Self {
            key_patterns,
            namespaced_patterns,
            ns_separator: ns_separator.to_string(),
        })
    }

    fn matches(&self, namespace: &str, key: &str) -> bool {
        if self.key_patterns.iter().any(|pattern| pattern.matches(key)) {
            return true;
        }

        if self.namespaced_patterns.is_empty() {
            return false;
        }

        let namespaced_key = if self.ns_separator.is_empty() {
            format!("{}{}", namespace, key)
        } else {
            format!("{}{}{}", namespace, self.ns_separator, key)
        };

        self.namespaced_patterns
            .iter()
            .any(|pattern| pattern.matches(&namespaced_key))
    }
}

/// Read a JSON locale file, returning an empty map if it doesn't exist
pub fn read_locale_file(path: &Path) -> Result<Map<String, Value>> {
    read_locale_file_with_fs(path, &crate::fs::RealFileSystem)
}

/// Read a JSON locale file using the provided FileSystem
pub fn read_locale_file_with_fs<F: FileSystem>(path: &Path, fs: &F) -> Result<Map<String, Value>> {
    if !fs.exists(path) {
        return Ok(Map::new());
    }

    let content = fs
        .read_to_string(path)
        .with_context(|| format!("Failed to read locale file: {}", path.display()))?;

    // Handle empty files
    if content.trim().is_empty() {
        return Ok(Map::new());
    }

    let map: Map<String, Value> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON in: {}", path.display()))?;

    Ok(map)
}

/// Result of inserting a nested key
enum InsertResult {
    /// Key was newly added
    Added,
    /// Key already existed (not modified)
    Existed,
    /// Conflict occurred (data structure mismatch)
    Conflict(KeyConflict),
}

/// Insert a nested key path, creating intermediate objects as needed.
/// Returns InsertResult indicating whether the key was added, existed, or conflicted.
///
/// This function uses iterative approach instead of recursion to prevent
/// stack overflow with deeply nested keys (DoS protection).
fn insert_nested_key(
    obj: &mut Map<String, Value>,
    path: &[&str],
    default_value: &str,
) -> InsertResult {
    if path.is_empty() {
        return InsertResult::Existed;
    }

    // Use iterative approach to prevent stack overflow with deep nesting
    let mut current = obj;
    let mut current_path = Vec::new();

    for (i, key) in path.iter().enumerate() {
        current_path.push(*key);
        let is_last = i == path.len() - 1;

        if is_last {
            // Leaf node - insert the value
            if let Some(existing) = current.get(*key) {
                // Check if we're trying to set a scalar where an object exists
                if existing.is_object() {
                    return InsertResult::Conflict(KeyConflict::ObjectIsValue {
                        key_path: current_path.join("."),
                    });
                }
                return InsertResult::Existed;
            } else {
                current.insert((*key).to_string(), Value::String(default_value.to_string()));
                return InsertResult::Added;
            }
        } else {
            // Intermediate node - ensure it's an object
            let entry = current
                .entry((*key).to_string())
                .or_insert_with(|| Value::Object(Map::new()));

            match entry {
                Value::Object(ref mut nested) => {
                    current = nested;
                }
                other => {
                    // Key exists but is not an object - conflict!
                    return InsertResult::Conflict(KeyConflict::ValueIsNotObject {
                        key_path: current_path.join("."),
                        existing_value: format!("{}", other),
                    });
                }
            }
        }
    }

    InsertResult::Existed
}

/// Sort all keys in a JSON object alphabetically (including nested objects).
///
/// Uses a controlled recursion with explicit depth limit to prevent stack overflow
/// from malicious inputs (DoS protection). Maximum depth is 100 levels.
pub fn sort_keys_alphabetically(map: &Map<String, Value>) -> Map<String, Value> {
    const MAX_DEPTH: usize = 100;
    sort_keys_with_depth(map, 0, MAX_DEPTH)
}

/// Internal function with depth tracking to prevent stack overflow
fn sort_keys_with_depth(
    map: &Map<String, Value>,
    depth: usize,
    max_depth: usize,
) -> Map<String, Value> {
    let mut sorted = Map::new();
    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();

    for key in keys {
        if let Some(value) = map.get(key) {
            let sorted_value = match value {
                Value::Object(nested) if depth < max_depth => {
                    Value::Object(sort_keys_with_depth(nested, depth + 1, max_depth))
                }
                Value::Object(nested) => {
                    // At max depth, just clone without sorting deeper
                    Value::Object(nested.clone())
                }
                other => other.clone(),
            };
            sorted.insert(key.clone(), sorted_value);
        }
    }

    sorted
}

/// Merge extracted keys into an existing translation map.
/// - New keys are added with default values (explicit or config-level fallback)
/// - Existing keys are preserved unless removal is requested
/// - preservePatterns keep dynamic keys even when removal is enabled
pub(crate) fn merge_keys(
    existing: &mut Map<String, Value>,
    keys: &[ExtractedKey],
    target_namespace: &str,
    config: &Config,
    preserve_matcher: &PreserveMatcher,
) -> SyncResult {
    let mut result = SyncResult::default();
    let mut seen_paths: HashSet<String> = HashSet::new();
    let mut seen_object_roots: Vec<String> = Vec::new();
    let default_namespace = effective_namespace(&config.default_namespace);
    let fallback_default = config.default_value.as_deref();
    let key_separator = config.key_separator.as_str();

    for key in keys {
        let key_namespace = key.namespace.as_deref().unwrap_or(default_namespace);

        if !config.merge_namespaces && key_namespace != target_namespace {
            continue;
        }

        let effective_key = if config.merge_namespaces {
            merge_namespace_key(config, key_namespace, &key.key)
        } else {
            key.key.clone()
        };

        if let Some(root) = effective_key.strip_suffix(".*") {
            seen_paths.insert(root.to_string());
            seen_object_roots.push(root.to_string());
            continue;
        }

        let value = key
            .default_value
            .as_deref()
            .or(fallback_default)
            .unwrap_or("");

        seen_paths.insert(effective_key.clone());

        if key_separator.is_empty() {
            if let Some(existing_value) = existing.get(&effective_key) {
                if existing_value.is_object() {
                    result.conflicts.push(KeyConflict::ObjectIsValue {
                        key_path: effective_key.clone(),
                    });
                } else {
                    result.existing_keys += 1;
                }
            } else {
                existing.insert(effective_key.clone(), Value::String(value.to_string()));
                result.added_keys.push(effective_key.clone());
            }
        } else {
            let parts: Vec<&str> = effective_key.split(key_separator).collect();
            match insert_nested_key(existing, &parts, value) {
                InsertResult::Added => {
                    result.added_keys.push(effective_key.clone());
                }
                InsertResult::Existed => {
                    result.existing_keys += 1;
                }
                InsertResult::Conflict(conflict) => {
                    result.conflicts.push(conflict);
                }
            }
        }
    }

    if config.remove_unused_keys {
        let mut removed = Vec::new();
        prune_unused_keys(
            existing,
            "",
            key_separator,
            target_namespace,
            &seen_paths,
            &seen_object_roots,
            preserve_matcher,
            &mut removed,
        );
        result.removed_keys = removed;
    }

    result
}

fn prune_unused_keys(
    node: &mut Map<String, Value>,
    parent_path: &str,
    key_separator: &str,
    namespace: &str,
    seen_paths: &HashSet<String>,
    seen_object_roots: &[String],
    preserve_matcher: &PreserveMatcher,
    removed: &mut Vec<String>,
) -> bool {
    let mut keys_to_remove = Vec::new();

    for (key, value) in node.iter_mut() {
        let current_path = if parent_path.is_empty() || key_separator.is_empty() {
            key.clone()
        } else {
            format!("{}{}{}", parent_path, key_separator, key)
        };

        let keep = seen_paths.contains(&current_path)
            || seen_object_roots.iter().any(|root| {
                current_path == *root
                    || (!root.is_empty()
                        && if key_separator.is_empty() {
                            current_path.starts_with(root)
                        } else {
                            current_path.starts_with(&format!("{}{}", root, key_separator))
                        })
            })
            || preserve_matcher.matches(namespace, &current_path);

        if let Some(obj) = value.as_object_mut() {
            let child_empty = prune_unused_keys(
                obj,
                &current_path,
                key_separator,
                namespace,
                seen_paths,
                seen_object_roots,
                preserve_matcher,
                removed,
            );
            if child_empty && !keep {
                keys_to_remove.push((key.clone(), current_path));
            }
        } else if !keep {
            keys_to_remove.push((key.clone(), current_path));
        }
    }

    for (key, path) in keys_to_remove {
        node.remove(&key);
        removed.push(path);
    }

    node.is_empty()
}

pub fn parse_locale_value_str(content: &str, format: OutputFormat, path: &Path) -> Result<Value> {
    let map = parse_locale_map(content, format, path)?;
    Ok(Value::Object(map))
}

fn parse_locale_map(
    content: &str,
    format: OutputFormat,
    path: &Path,
) -> Result<Map<String, Value>> {
    if content.trim().is_empty() {
        return Ok(Map::new());
    }

    let map: Map<String, Value> = match format {
        OutputFormat::Json => serde_json::from_str(content)
            .with_context(|| format!("Failed to parse JSON in: {}", path.display()))?,
        OutputFormat::Json5 => json5::from_str(content)
            .with_context(|| format!("Failed to parse JSON5 in: {}", path.display()))?,
        OutputFormat::JsEsm | OutputFormat::JsCjs | OutputFormat::Ts => {
            let fragment = extract_json_fragment(content)
                .with_context(|| format!("Failed to locate JSON object in: {}", path.display()))?;
            serde_json::from_str(&fragment).with_context(|| {
                format!("Failed to parse JSON in JS/TS module: {}", path.display())
            })?
        }
    };

    Ok(map)
}

fn write_json_locale_with_fs<F: FileSystem>(
    path: &Path,
    content: &Map<String, Value>,
    style: Option<&JsonStyle>,
    fs: &F,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let style = if let Some(style) = style.cloned() {
        style
    } else if fs.exists(path) {
        let existing = fs
            .read_to_string(path)
            .with_context(|| format!("Failed to read existing file: {}", path.display()))?;
        detect_json_style(&existing)
    } else {
        JsonStyle::default()
    };
    let mut buffer = Vec::new();
    serialize_with_style(&mut buffer, &Value::Object(content.clone()), &style)?;
    if style.trailing_newline {
        buffer.extend_from_slice(if style.use_crlf { b"\r\n" } else { b"\n" });
    }

    fs.atomic_write(path, &buffer)
        .with_context(|| format!("Failed to write locale file: {}", path.display()))
}

fn write_json5_locale_with_fs<F: FileSystem>(
    path: &Path,
    content: &Map<String, Value>,
    fs: &F,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let existing = if fs.exists(path) {
        fs.read_to_string(path).ok()
    } else {
        None
    };
    let (prefix_comments, suffix_comments, prefer_trailing_comma) = existing
        .as_deref()
        .map(extract_json5_preservation_hints)
        .unwrap_or_default();
    let preserved_numbers = existing
        .as_deref()
        .map(|current| build_json5_numeric_preservation_map(current, content))
        .unwrap_or_default();

    let mut json_body = render_json5_with_preserved_numbers(content, &preserved_numbers)?;
    if prefer_trailing_comma {
        json_body = add_trailing_commas_to_pretty_json(&json_body);
    }

    let mut output = String::new();
    if !prefix_comments.is_empty() {
        output.push_str(&prefix_comments);
        if !prefix_comments.ends_with('\n') {
            output.push('\n');
        }
    }
    output.push_str(&json_body);
    if !suffix_comments.is_empty() {
        output.push('\n');
        output.push_str(&suffix_comments);
    }
    output.push('\n');
    let buffer = output.into_bytes();
    fs.atomic_write(path, &buffer)
        .with_context(|| format!("Failed to write locale file: {}", path.display()))
}

fn extract_json5_preservation_hints(content: &str) -> (String, String, bool) {
    let first_brace = content.find('{');
    let last_brace = content.rfind('}');
    let prefix = first_brace
        .map(|idx| content[..idx].trim_end().to_string())
        .unwrap_or_default();
    let suffix = last_brace
        .map(|idx| content[idx + 1..].trim_start().to_string())
        .unwrap_or_default();
    let prefer_trailing_comma = content.contains(",\n}") || content.contains(",\n]");
    (prefix, suffix, prefer_trailing_comma)
}

fn add_trailing_commas_to_pretty_json(pretty_json: &str) -> String {
    let mut lines: Vec<String> = pretty_json.lines().map(|l| l.to_string()).collect();
    for i in 1..lines.len() {
        let trimmed = lines[i].trim();
        if trimmed == "}" || trimmed == "]" {
            // find previous non-empty line
            let mut prev_idx = None;
            for j in (0..i).rev() {
                if !lines[j].trim().is_empty() {
                    prev_idx = Some(j);
                    break;
                }
            }
            if let Some(j) = prev_idx {
                let prev_trimmed = lines[j].trim();
                if !(prev_trimmed.ends_with(',')
                    || prev_trimmed.ends_with('{')
                    || prev_trimmed.ends_with('['))
                {
                    lines[j].push(',');
                }
            }
        }
    }
    lines.join("\n")
}

fn render_json5_with_preserved_numbers(
    content: &Map<String, Value>,
    preserved_numbers: &HashMap<String, String>,
) -> Result<String> {
    let root = Value::Object(content.clone());
    let mut out = String::new();
    write_json5_value(&root, "", 0, preserved_numbers, &mut out)?;
    Ok(out)
}

fn write_json5_value(
    value: &Value,
    pointer: &str,
    depth: usize,
    preserved_numbers: &HashMap<String, String>,
    out: &mut String,
) -> Result<()> {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            out.push('{');
            out.push('\n');
            let mut iter = map.iter().peekable();
            while let Some((key, v)) = iter.next() {
                out.push_str(&"  ".repeat(depth + 1));
                out.push_str(&serde_json::to_string(key)?);
                out.push_str(": ");
                let child_pointer = pointer_child(pointer, key);
                write_json5_value(v, &child_pointer, depth + 1, preserved_numbers, out)?;
                if iter.peek().is_some() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(depth));
            out.push('}');
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                out.push_str("[]");
                return Ok(());
            }
            out.push('[');
            out.push('\n');
            for (idx, item) in arr.iter().enumerate() {
                out.push_str(&"  ".repeat(depth + 1));
                let child_pointer = pointer_index(pointer, idx);
                write_json5_value(item, &child_pointer, depth + 1, preserved_numbers, out)?;
                if idx + 1 < arr.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(depth));
            out.push(']');
        }
        Value::Number(_) => {
            if let Some(raw) = preserved_numbers.get(pointer) {
                out.push_str(raw);
            } else {
                out.push_str(&serde_json::to_string(value)?);
            }
        }
        _ => out.push_str(&serde_json::to_string(value)?),
    }

    Ok(())
}

fn pointer_child(parent: &str, key: &str) -> String {
    let escaped = key.replace('~', "~0").replace('/', "~1");
    format!("{}/{}", parent, escaped)
}

fn pointer_index(parent: &str, idx: usize) -> String {
    format!("{}/{}", parent, idx)
}

fn build_json5_numeric_preservation_map(
    existing_content: &str,
    new_content: &Map<String, Value>,
) -> HashMap<String, String> {
    let Some(existing_value) = json5::from_str::<Value>(existing_content).ok() else {
        return HashMap::new();
    };
    let new_value = Value::Object(new_content.clone());
    let raw_numbers = collect_json5_number_literals(existing_content);
    let mut preserved = HashMap::new();

    for (pointer, literal) in raw_numbers {
        let Some(existing_number) = existing_value.pointer(&pointer) else {
            continue;
        };
        let Some(new_number) = new_value.pointer(&pointer) else {
            continue;
        };
        if numeric_values_equal(existing_number, new_number) {
            preserved.insert(pointer, literal);
        }
    }

    preserved
}

fn numeric_values_equal(left: &Value, right: &Value) -> bool {
    let (Value::Number(ln), Value::Number(rn)) = (left, right) else {
        return false;
    };
    if ln == rn {
        return true;
    }

    let lf = ln
        .as_f64()
        .or_else(|| ln.as_i64().map(|v| v as f64))
        .or_else(|| ln.as_u64().map(|v| v as f64));
    let rf = rn
        .as_f64()
        .or_else(|| rn.as_i64().map(|v| v as f64))
        .or_else(|| rn.as_u64().map(|v| v as f64));

    matches!((lf, rf), (Some(a), Some(b)) if a == b)
}

fn collect_json5_number_literals(content: &str) -> HashMap<String, String> {
    let mut scanner = Json5NumberScanner::new(content);
    let mut out = HashMap::new();
    scanner.scan_root(&mut out);
    out
}

struct Json5NumberScanner<'a> {
    bytes: &'a [u8],
    idx: usize,
}

impl<'a> Json5NumberScanner<'a> {
    fn new(content: &'a str) -> Self {
        Self {
            bytes: content.as_bytes(),
            idx: 0,
        }
    }

    fn scan_root(&mut self, out: &mut HashMap<String, String>) {
        self.skip_ws_and_comments();
        self.scan_value("", out);
    }

    fn scan_value(&mut self, pointer: &str, out: &mut HashMap<String, String>) {
        self.skip_ws_and_comments();
        let Some(ch) = self.peek() else {
            return;
        };
        match ch {
            b'{' => self.scan_object(pointer, out),
            b'[' => self.scan_array(pointer, out),
            b'"' | b'\'' => {
                self.scan_string(ch);
            }
            b'-' | b'+' | b'.' | b'0'..=b'9' => {
                let literal = self.scan_token();
                if !pointer.is_empty() && !literal.is_empty() {
                    out.insert(pointer.to_string(), literal);
                }
            }
            b't' | b'f' | b'n' => {
                self.scan_identifier();
            }
            b'I' | b'N' => {
                let literal = self.scan_identifier();
                if !pointer.is_empty() && !literal.is_empty() {
                    out.insert(pointer.to_string(), literal);
                }
            }
            _ => {
                self.idx += 1;
            }
        }
    }

    fn scan_object(&mut self, pointer: &str, out: &mut HashMap<String, String>) {
        self.idx += 1; // {
        loop {
            self.skip_ws_and_comments();
            match self.peek() {
                Some(b'}') => {
                    self.idx += 1;
                    break;
                }
                None => break,
                _ => {}
            }

            let Some(key) = self.scan_object_key() else {
                break;
            };
            self.skip_ws_and_comments();
            if self.peek() == Some(b':') {
                self.idx += 1;
            } else {
                break;
            }

            let child_pointer = pointer_child(pointer, &key);
            self.scan_value(&child_pointer, out);
            self.skip_ws_and_comments();

            match self.peek() {
                Some(b',') => {
                    self.idx += 1;
                }
                Some(b'}') => {
                    self.idx += 1;
                    break;
                }
                None => break,
                _ => {}
            }
        }
    }

    fn scan_array(&mut self, pointer: &str, out: &mut HashMap<String, String>) {
        self.idx += 1; // [
        let mut index = 0usize;
        loop {
            self.skip_ws_and_comments();
            match self.peek() {
                Some(b']') => {
                    self.idx += 1;
                    break;
                }
                None => break,
                _ => {}
            }

            let child_pointer = pointer_index(pointer, index);
            self.scan_value(&child_pointer, out);
            index += 1;
            self.skip_ws_and_comments();

            match self.peek() {
                Some(b',') => {
                    self.idx += 1;
                }
                Some(b']') => {
                    self.idx += 1;
                    break;
                }
                None => break,
                _ => {}
            }
        }
    }

    fn scan_object_key(&mut self) -> Option<String> {
        self.skip_ws_and_comments();
        match self.peek()? {
            b'"' | b'\'' => self.scan_string(self.peek()?).into(),
            _ => {
                let ident = self.scan_identifier();
                if ident.is_empty() {
                    None
                } else {
                    Some(ident)
                }
            }
        }
    }

    fn scan_string(&mut self, quote: u8) -> String {
        let mut out = String::new();
        self.idx += 1; // opening quote
        while let Some(ch) = self.peek() {
            self.idx += 1;
            if ch == quote {
                break;
            }
            if ch == b'\\' {
                if let Some(next) = self.peek() {
                    self.idx += 1;
                    out.push(next as char);
                }
                continue;
            }
            out.push(ch as char);
        }
        out
    }

    fn scan_identifier(&mut self) -> String {
        let start = self.idx;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'$' {
                self.idx += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.bytes[start..self.idx]).to_string()
    }

    fn scan_token(&mut self) -> String {
        let start = self.idx;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() || matches!(ch, b',' | b'}' | b']') {
                break;
            }
            if ch == b'/' && self.peek_next().is_some_and(|n| n == b'/' || n == b'*') {
                break;
            }
            self.idx += 1;
        }
        String::from_utf8_lossy(&self.bytes[start..self.idx]).to_string()
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while let Some(ch) = self.peek() {
                if ch.is_ascii_whitespace() {
                    self.idx += 1;
                } else {
                    break;
                }
            }

            let Some(ch) = self.peek() else {
                return;
            };
            if ch != b'/' {
                return;
            }
            let Some(next) = self.peek_next() else {
                return;
            };
            if next == b'/' {
                self.idx += 2;
                while let Some(c) = self.peek() {
                    self.idx += 1;
                    if c == b'\n' {
                        break;
                    }
                }
                continue;
            }
            if next == b'*' {
                self.idx += 2;
                while self.idx + 1 < self.bytes.len() {
                    if self.bytes[self.idx] == b'*' && self.bytes[self.idx + 1] == b'/' {
                        self.idx += 2;
                        break;
                    }
                    self.idx += 1;
                }
                continue;
            }
            return;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.idx).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.bytes.get(self.idx + 1).copied()
    }
}

enum JsVariant {
    Esm,
    Cjs,
}

fn write_js_locale_with_fs<F: FileSystem>(
    path: &Path,
    content: &Map<String, Value>,
    fs: &F,
    variant: JsVariant,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let json_body = serde_json::to_string_pretty(content)?;
    let (prefix, suffix) = match variant {
        JsVariant::Esm => ("export default ", ";\n"),
        JsVariant::Cjs => ("module.exports = ", ";\n"),
    };
    let mut output = String::new();
    output.push_str(prefix);
    output.push_str(&json_body);
    output.push_str(suffix);

    fs.atomic_write(path, output.as_bytes())
        .with_context(|| format!("Failed to write locale file: {}", path.display()))
}

fn write_ts_locale_with_fs<F: FileSystem>(
    path: &Path,
    content: &Map<String, Value>,
    fs: &F,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let json_body = serde_json::to_string_pretty(content)?;
    let output = format!("export default {} as const;\n", json_body);
    fs.atomic_write(path, output.as_bytes())
        .with_context(|| format!("Failed to write locale file: {}", path.display()))
}

fn extract_json_fragment(content: &str) -> Result<String> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut start_idx = None;

    for (idx, ch) in content.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start_idx = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    bail!("Unmatched closing brace in module output");
                }
                depth -= 1;
                if depth == 0 {
                    let start = start_idx
                        .ok_or_else(|| anyhow::anyhow!("Could not locate JSON object in module"))?;
                    return Ok(content[start..=idx].to_string());
                }
            }
            _ => {}
        }
    }

    bail!("Could not locate JSON object in module output")
}

/// Write translation data atomically using the configured format.
pub fn write_locale_file(
    path: &Path,
    content: &Map<String, Value>,
    format: OutputFormat,
    style: Option<&JsonStyle>,
) -> Result<()> {
    write_locale_file_with_fs(path, content, format, style, &crate::fs::RealFileSystem)
}

/// Write translation data using the provided FileSystem (for testing)
pub fn write_locale_file_with_fs<F: FileSystem>(
    path: &Path,
    content: &Map<String, Value>,
    format: OutputFormat,
    style: Option<&JsonStyle>,
    fs: &F,
) -> Result<()> {
    match format {
        OutputFormat::Json => write_json_locale_with_fs(path, content, style, fs),
        OutputFormat::Json5 => write_json5_locale_with_fs(path, content, fs),
        OutputFormat::JsEsm => write_js_locale_with_fs(path, content, fs, JsVariant::Esm),
        OutputFormat::JsCjs => write_js_locale_with_fs(path, content, fs, JsVariant::Cjs),
        OutputFormat::Ts => write_ts_locale_with_fs(path, content, fs),
    }
}

/// Atomically read, modify, and write a locale file with exclusive file locking.
/// This prevents data corruption when multiple processes access the same file.
///
/// The lock is held for the entire read-modify-write cycle to ensure ACID-like
/// transaction guarantees.
///
/// If `dry_run` is true, the file will not be written but the result will still
/// indicate what changes would have been made.
pub(crate) fn sync_locale_file_locked(
    path: &Path,
    keys: &[ExtractedKey],
    target_namespace: &str,
    config: &Config,
    preserve_matcher: &PreserveMatcher,
    dry_run: bool,
) -> Result<SyncResult> {
    sync_locale_file_locked_with_fs(
        path,
        keys,
        target_namespace,
        config,
        preserve_matcher,
        dry_run,
        &crate::fs::RealFileSystem,
    )
}

/// Atomically read, modify, and write a locale file using the provided FileSystem.
/// This version is testable with mock file systems.
///
/// If `dry_run` is true, the file will not be written but the result will still
/// indicate what changes would have been made.
pub(crate) fn sync_locale_file_locked_with_fs<F: FileSystem>(
    path: &Path,
    keys: &[ExtractedKey],
    target_namespace: &str,
    config: &Config,
    preserve_matcher: &PreserveMatcher,
    dry_run: bool,
    fs: &F,
) -> Result<SyncResult> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Open file with exclusive lock using FileSystem abstraction
    let mut locked_file = fs.open_locked(path)?;

    // Read existing content
    let content_str = locked_file
        .content_string()
        .with_context(|| format!("Failed to read locale file: {}", path.display()))?;

    let format = config.output_format();
    let trimmed_empty = content_str.trim().is_empty();
    let style = if format == OutputFormat::Json {
        if trimmed_empty {
            // For new files, use configured indentation or default
            let indent = config
                .indentation_string()
                .unwrap_or_else(|| "  ".to_string());
            Some(JsonStyle {
                indent,
                use_crlf: false,
                trailing_newline: true,
            })
        } else {
            // For existing files, prefer configured indentation over detected
            let mut detected = detect_json_style(&content_str);
            if let Some(indent) = config.indentation_string() {
                detected.indent = indent;
            }
            Some(detected)
        }
    } else {
        None
    };

    let mut content = parse_locale_map(&content_str, format, path)?;

    // Merge new keys
    let mut sync_result = merge_keys(
        &mut content,
        keys,
        target_namespace,
        config,
        preserve_matcher,
    );
    sync_result.file_path = path.display().to_string();

    // Only write if there were changes and not in dry-run mode
    if !dry_run && (!sync_result.added_keys.is_empty() || !sync_result.removed_keys.is_empty()) {
        let sorted = sort_keys_alphabetically(&content);
        write_locale_file_with_fs(path, &sorted, format, style.as_ref(), fs)
            .with_context(|| format!("Failed to write locale file: {}", path.display()))?;
    }

    // Lock is automatically released when file is dropped
    Ok(sync_result)
}

/// Collect unique namespaces from a set of extracted keys
pub fn collect_namespaces(
    keys: &[ExtractedKey],
    default_namespace: &str,
    merge_namespaces: bool,
) -> std::collections::HashSet<String> {
    let mut namespaces = std::collections::HashSet::new();
    let effective_default = effective_namespace(default_namespace).to_string();
    namespaces.insert(effective_default.clone());

    if merge_namespaces {
        return namespaces;
    }

    for key in keys {
        if let Some(ns) = &key.namespace {
            namespaces.insert(ns.clone());
        }
    }

    namespaces
}

/// Sync extracted keys to specific namespace files only (for incremental updates)
/// This is more efficient when only a subset of namespaces have changed.
///
/// Uses file locking to prevent data corruption when multiple processes
/// (e.g., watch mode + manual extract) access the same files.
///
/// If `dry_run` is true, files will not be written but results will still
/// indicate what changes would have been made.
pub fn sync_namespaces(
    config: &Config,
    keys: &[ExtractedKey],
    output_dir: &str,
    namespaces: &std::collections::HashSet<String>,
    dry_run: bool,
) -> Result<Vec<SyncResult>> {
    let preserve_matcher = PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator)?;
    let mut results = Vec::new();
    let target_namespaces: Vec<String> = if config.merge_namespaces {
        vec![effective_namespace(&config.default_namespace).to_string()]
    } else {
        namespaces.iter().cloned().collect()
    };

    // Process only the specified namespace files
    for locale in &config.locales {
        for namespace in &target_namespaces {
            let file_path = locale_namespace_file_path(config, output_dir, locale, namespace);

            // Use locked sync for data integrity
            let sync_result = sync_locale_file_locked(
                &file_path,
                keys,
                namespace,
                config,
                &preserve_matcher,
                dry_run,
            )?;

            results.push(sync_result);
        }
    }

    Ok(results)
}

/// Sync extracted keys to a specific subset of locales.
pub fn sync_locales(
    config: &Config,
    keys: &[ExtractedKey],
    output_dir: &str,
    target_locales: &[String],
    dry_run: bool,
) -> Result<Vec<SyncResult>> {
    let preserve_matcher = PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator)?;
    let mut results = Vec::new();
    let namespaces = collect_namespaces(keys, &config.default_namespace, config.merge_namespaces);

    for locale in target_locales {
        for namespace in &namespaces {
            let file_path = locale_namespace_file_path(config, output_dir, locale, namespace);

            let sync_result = sync_locale_file_locked(
                &file_path,
                keys,
                namespace,
                config,
                &preserve_matcher,
                dry_run,
            )?;
            results.push(sync_result);
        }
    }

    Ok(results)
}

/// Sync extracted keys to all locale files.
///
/// If `dry_run` is true, files will not be written but results will still
/// indicate what changes would have been made.
pub fn sync_all_locales(
    config: &Config,
    keys: &[ExtractedKey],
    output_dir: &str,
    dry_run: bool,
) -> Result<Vec<SyncResult>> {
    sync_locales(config, keys, output_dir, &config.locales, dry_run)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_nested_key_simple() {
        let mut map = Map::new();
        let result = insert_nested_key(&mut map, &["hello"], "");

        assert!(matches!(result, InsertResult::Added));
        assert_eq!(map.get("hello"), Some(&Value::String("".to_string())));
    }

    #[test]
    fn test_insert_nested_key_deep() {
        let mut map = Map::new();
        let result = insert_nested_key(&mut map, &["button", "submit"], "");

        assert!(matches!(result, InsertResult::Added));
        let button = map
            .get("button")
            .expect("button should exist after insert_nested_key")
            .as_object()
            .expect("button should be an object after insert_nested_key");
        assert_eq!(button.get("submit"), Some(&Value::String("".to_string())));
    }

    #[test]
    fn test_insert_nested_key_existing() {
        let mut map = Map::new();
        map.insert("hello".to_string(), Value::String("world".to_string()));

        let result = insert_nested_key(&mut map, &["hello"], "");

        assert!(matches!(result, InsertResult::Existed));
        assert_eq!(map.get("hello"), Some(&Value::String("world".to_string())));
    }

    #[test]
    fn test_insert_nested_key_conflict() {
        let mut map = Map::new();
        // Add a scalar value at "button"
        map.insert("button".to_string(), Value::String("click me".to_string()));

        // Try to add a nested key "button.submit" - should conflict
        let result = insert_nested_key(&mut map, &["button", "submit"], "");

        assert!(matches!(
            result,
            InsertResult::Conflict(KeyConflict::ValueIsNotObject { .. })
        ));
        // Original value should be preserved
        assert_eq!(
            map.get("button"),
            Some(&Value::String("click me".to_string()))
        );
    }

    #[test]
    fn test_sort_keys_alphabetically() {
        let mut map = Map::new();
        map.insert("zebra".to_string(), Value::String("z".to_string()));
        map.insert("apple".to_string(), Value::String("a".to_string()));
        map.insert("mango".to_string(), Value::String("m".to_string()));

        let sorted = sort_keys_alphabetically(&map);
        let keys: Vec<_> = sorted.keys().collect();

        assert_eq!(keys, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn test_sort_nested_objects() {
        let mut inner = Map::new();
        inner.insert("z".to_string(), Value::String("1".to_string()));
        inner.insert("a".to_string(), Value::String("2".to_string()));

        let mut map = Map::new();
        map.insert("nested".to_string(), Value::Object(inner));

        let sorted = sort_keys_alphabetically(&map);
        let nested = sorted
            .get("nested")
            .expect("nested should exist after sort_keys_alphabetically")
            .as_object()
            .expect("nested should be an object in sort_keys_alphabetically");
        let keys: Vec<_> = nested.keys().collect();

        assert_eq!(keys, vec!["a", "z"]);
    }

    #[test]
    fn test_merge_keys() {
        let mut existing = Map::new();
        existing.insert(
            "existing".to_string(),
            Value::String("translated".to_string()),
        );

        let keys = vec![
            ExtractedKey {
                key: "existing".to_string(),
                namespace: None,
                default_value: None,
            },
            ExtractedKey {
                key: "new.key".to_string(),
                namespace: None,
                default_value: None,
            },
        ];

        let config = Config::default();
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();
        let result = merge_keys(&mut existing, &keys, "translation", &config, &matcher);

        assert_eq!(result.added_keys.len(), 1);
        assert_eq!(result.added_keys[0], "new.key");
        assert_eq!(result.existing_keys, 1);
        // Existing translation is preserved
        assert_eq!(
            existing.get("existing"),
            Some(&Value::String("translated".to_string()))
        );
    }

    #[test]
    fn test_merge_keys_with_default_value() {
        let mut existing = Map::new();

        let keys = vec![
            ExtractedKey {
                key: "greeting".to_string(),
                namespace: None,
                default_value: Some("Hello World!".to_string()),
            },
            ExtractedKey {
                key: "no_default".to_string(),
                namespace: None,
                default_value: None,
            },
        ];

        let config = Config::default();
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();
        let result = merge_keys(&mut existing, &keys, "translation", &config, &matcher);

        assert_eq!(result.added_keys.len(), 2);
        // Key with default_value should use that value
        assert_eq!(
            existing.get("greeting"),
            Some(&Value::String("Hello World!".to_string()))
        );
        // Key without default_value should use empty string
        assert_eq!(
            existing.get("no_default"),
            Some(&Value::String("".to_string()))
        );
    }

    #[test]
    fn test_merge_keys_flat_mode() {
        let mut existing = Map::new();

        let keys = vec![
            ExtractedKey {
                key: "button.submit".to_string(),
                namespace: None,
                default_value: Some("Submit".to_string()),
            },
            ExtractedKey {
                key: "form.validation.required".to_string(),
                namespace: None,
                default_value: None,
            },
        ];

        // Empty separator = flat key mode
        let mut config = Config::default();
        config.key_separator = String::new();
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();
        let result = merge_keys(&mut existing, &keys, "translation", &config, &matcher);

        assert_eq!(result.added_keys.len(), 2);
        // Keys should be stored as-is, not nested
        assert_eq!(
            existing.get("button.submit"),
            Some(&Value::String("Submit".to_string()))
        );
        assert_eq!(
            existing.get("form.validation.required"),
            Some(&Value::String("".to_string()))
        );
        // Should NOT have nested structure
        assert!(existing.get("button").is_none());
        assert!(existing.get("form").is_none());
    }

    #[test]
    fn test_merge_keys_with_merge_namespaces() {
        let mut existing = Map::new();
        let keys = vec![
            ExtractedKey {
                key: "hello".to_string(),
                namespace: Some("common".to_string()),
                default_value: Some("Hello".to_string()),
            },
            ExtractedKey {
                key: "title".to_string(),
                namespace: Some("home".to_string()),
                default_value: Some("Home".to_string()),
            },
        ];

        let mut config = Config::default();
        config.merge_namespaces = true;
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();
        let result = merge_keys(&mut existing, &keys, "translation", &config, &matcher);
        assert_eq!(result.added_keys.len(), 2);
        let common = existing
            .get("common")
            .and_then(|v| v.as_object())
            .expect("common namespace object should exist");
        assert_eq!(
            common.get("hello"),
            Some(&Value::String("Hello".to_string()))
        );
        let home = existing
            .get("home")
            .and_then(|v| v.as_object())
            .expect("home namespace object should exist");
        assert_eq!(home.get("title"), Some(&Value::String("Home".to_string())));
    }

    #[test]
    fn test_detect_json_style_default() {
        let style = JsonStyle::default();
        assert_eq!(style.indent, "  ");
        assert!(!style.use_crlf);
        assert!(style.trailing_newline);
    }

    #[test]
    fn test_detect_json_style_two_spaces() {
        let json = r#"{
  "key": "value"
}
"#;
        let style = detect_json_style(json);
        assert_eq!(style.indent, "  ");
        assert!(!style.use_crlf);
        assert!(style.trailing_newline);
    }

    #[test]
    fn test_detect_json_style_four_spaces() {
        let json = r#"{
    "key": "value"
}
"#;
        let style = detect_json_style(json);
        assert_eq!(style.indent, "    ");
        assert!(!style.use_crlf);
        assert!(style.trailing_newline);
    }

    #[test]
    fn test_detect_json_style_tabs() {
        let json = "{\n\t\"key\": \"value\"\n}\n";
        let style = detect_json_style(json);
        assert_eq!(style.indent, "\t");
        assert!(!style.use_crlf);
        assert!(style.trailing_newline);
    }

    #[test]
    fn test_detect_json_style_crlf() {
        let json = "{\r\n  \"key\": \"value\"\r\n}\r\n";
        let style = detect_json_style(json);
        assert_eq!(style.indent, "  ");
        assert!(style.use_crlf);
        assert!(style.trailing_newline);
    }

    #[test]
    fn test_detect_json_style_no_trailing_newline() {
        let json = r#"{
  "key": "value"
}"#;
        let style = detect_json_style(json);
        assert_eq!(style.indent, "  ");
        assert!(!style.use_crlf);
        assert!(!style.trailing_newline);
    }

    #[test]
    fn test_serialize_with_style_four_spaces() {
        let mut map = Map::new();
        map.insert("hello".to_string(), Value::String("world".to_string()));

        let style = JsonStyle {
            indent: "    ".to_string(),
            use_crlf: false,
            trailing_newline: true,
        };

        let mut output = Vec::new();
        serialize_with_style(&mut output, &Value::Object(map), &style).unwrap();
        let result = String::from_utf8(output).unwrap();

        assert!(result.contains("    \"hello\""));
    }

    #[test]
    fn test_serialize_with_style_tabs() {
        let mut map = Map::new();
        map.insert("key".to_string(), Value::String("value".to_string()));

        let style = JsonStyle {
            indent: "\t".to_string(),
            use_crlf: false,
            trailing_newline: true,
        };

        let mut output = Vec::new();
        serialize_with_style(&mut output, &Value::Object(map), &style).unwrap();
        let result = String::from_utf8(output).unwrap();

        assert!(result.contains("\t\"key\""));
    }

    #[test]
    fn test_sync_locale_file_locked_with_mock_fs() {
        use crate::fs::mock::InMemoryFileSystem;
        use std::path::Path;

        let fs = InMemoryFileSystem::new();

        // Create an empty file to start with
        fs.add_file("locales/en/translation.json", "{}");

        let keys = vec![
            ExtractedKey {
                key: "hello".to_string(),
                namespace: None,
                default_value: Some("Hello World".to_string()),
            },
            ExtractedKey {
                key: "button.submit".to_string(),
                namespace: None,
                default_value: Some("Submit".to_string()),
            },
        ];

        let config = Config::default();
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();

        let result = sync_locale_file_locked_with_fs(
            Path::new("locales/en/translation.json"),
            &keys,
            "translation",
            &config,
            &matcher,
            false, // dry_run
            &fs,
        )
        .unwrap();

        assert_eq!(result.added_keys.len(), 2);
        assert!(result.added_keys.contains(&"hello".to_string()));
        assert!(result.added_keys.contains(&"button.submit".to_string()));

        // Verify file was written
        let files = fs.get_files();
        let content = files
            .get(Path::new("locales/en/translation.json"))
            .expect("File should exist");

        // Verify JSON structure
        let parsed: Map<String, Value> = serde_json::from_str(content).unwrap();
        assert_eq!(
            parsed.get("hello"),
            Some(&Value::String("Hello World".to_string()))
        );

        // Verify nested key
        let button = parsed.get("button").unwrap().as_object().unwrap();
        assert_eq!(
            button.get("submit"),
            Some(&Value::String("Submit".to_string()))
        );
    }

    #[test]
    fn test_sync_locale_file_preserves_existing_keys() {
        use crate::fs::mock::InMemoryFileSystem;
        use std::path::Path;

        let fs = InMemoryFileSystem::new();

        // Create a file with existing translations
        fs.add_file(
            "locales/en/translation.json",
            r#"{"existing": "Already translated"}"#,
        );

        let keys = vec![
            ExtractedKey {
                key: "existing".to_string(),
                namespace: None,
                default_value: Some("New value".to_string()), // Different value
            },
            ExtractedKey {
                key: "new_key".to_string(),
                namespace: None,
                default_value: Some("New key value".to_string()),
            },
        ];

        let mut config = Config::default();
        config.remove_unused_keys = false;
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();

        let result = sync_locale_file_locked_with_fs(
            Path::new("locales/en/translation.json"),
            &keys,
            "translation",
            &config,
            &matcher,
            false, // dry_run
            &fs,
        )
        .unwrap();

        // Only new key should be added
        assert_eq!(result.added_keys.len(), 1);
        assert_eq!(result.added_keys[0], "new_key");
        assert_eq!(result.existing_keys, 1);

        // Verify existing translation was preserved
        let files = fs.get_files();
        let content = files
            .get(Path::new("locales/en/translation.json"))
            .expect("File should exist");
        let parsed: Map<String, Value> = serde_json::from_str(content).unwrap();

        // Original value should be preserved, not overwritten
        assert_eq!(
            parsed.get("existing"),
            Some(&Value::String("Already translated".to_string()))
        );
        assert_eq!(
            parsed.get("new_key"),
            Some(&Value::String("New key value".to_string()))
        );
    }
    #[test]
    fn test_remove_unused_keys_prunes_stale_entries() {
        use crate::fs::mock::InMemoryFileSystem;
        use std::path::Path;

        let fs = InMemoryFileSystem::new();
        fs.add_file("locales/en/translation.json", r#"{"stale": "keep"}"#);

        let keys: Vec<ExtractedKey> = Vec::new();
        let config = Config::default();
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();

        let result = sync_locale_file_locked_with_fs(
            Path::new("locales/en/translation.json"),
            &keys,
            "translation",
            &config,
            &matcher,
            false, // dry_run
            &fs,
        )
        .unwrap();

        assert_eq!(result.added_keys.len(), 0);
        assert_eq!(result.removed_keys, vec!["stale".to_string()]);

        let files = fs.get_files();
        let content = files
            .get(Path::new("locales/en/translation.json"))
            .expect("File should exist");
        let parsed: Map<String, Value> = serde_json::from_str(content).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_return_objects_marker_preserves_nested_keys() {
        use crate::fs::mock::InMemoryFileSystem;
        use std::path::Path;

        let fs = InMemoryFileSystem::new();
        fs.add_file(
            "locales/en/translation.json",
            r#"{"countries":{"jp":"Japan","us":"United States"},"stale":"remove"}"#,
        );

        let keys = vec![ExtractedKey {
            key: "countries.*".to_string(),
            namespace: None,
            default_value: None,
        }];
        let config = Config::default();
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();

        let result = sync_locale_file_locked_with_fs(
            Path::new("locales/en/translation.json"),
            &keys,
            "translation",
            &config,
            &matcher,
            false,
            &fs,
        )
        .unwrap();

        assert_eq!(result.removed_keys, vec!["stale".to_string()]);

        let files = fs.get_files();
        let content = files
            .get(Path::new("locales/en/translation.json"))
            .expect("File should exist");
        let parsed: Map<String, Value> = serde_json::from_str(content).unwrap();
        assert!(parsed.get("countries").is_some());
        let countries = parsed
            .get("countries")
            .and_then(|v| v.as_object())
            .expect("countries should be object");
        assert_eq!(
            countries.get("jp"),
            Some(&Value::String("Japan".to_string()))
        );
    }

    #[test]
    fn test_preserve_patterns_keep_dynamic_keys() {
        use crate::fs::mock::InMemoryFileSystem;
        use std::path::Path;

        let fs = InMemoryFileSystem::new();
        fs.add_file("locales/en/translation.json", r#"{"dynamic": "value"}"#);

        let keys: Vec<ExtractedKey> = Vec::new();
        let mut config = Config::default();
        config.preserve_patterns = vec!["translation:dynamic".to_string()];
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();

        let result = sync_locale_file_locked_with_fs(
            Path::new("locales/en/translation.json"),
            &keys,
            "translation",
            &config,
            &matcher,
            false, // dry_run
            &fs,
        )
        .unwrap();

        assert!(result.removed_keys.is_empty());

        let files = fs.get_files();
        let content = files
            .get(Path::new("locales/en/translation.json"))
            .expect("File should exist");
        let parsed: Map<String, Value> = serde_json::from_str(content).unwrap();
        assert_eq!(
            parsed.get("dynamic"),
            Some(&Value::String("value".to_string()))
        );
    }

    #[test]
    fn test_sync_locale_with_json5_format() {
        use crate::fs::mock::InMemoryFileSystem;
        use std::path::Path;

        let fs = InMemoryFileSystem::new();
        fs.add_file("locales/en/translation.json5", "{ greeting: 'Hello' }");

        let keys = vec![ExtractedKey {
            key: "farewell".to_string(),
            namespace: None,
            default_value: Some("Goodbye".to_string()),
        }];

        let mut config = Config::default();
        config.output_format = OutputFormat::Json5;
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();

        let result = sync_locale_file_locked_with_fs(
            Path::new("locales/en/translation.json5"),
            &keys,
            "translation",
            &config,
            &matcher,
            false, // dry_run
            &fs,
        )
        .unwrap();

        assert_eq!(result.added_keys, vec!["farewell".to_string()]);

        let files = fs.get_files();
        let content = files
            .get(Path::new("locales/en/translation.json5"))
            .expect("File should exist");
        assert!(content.contains("\"farewell\""));
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn test_locale_namespace_file_path_uses_merged_filename_when_enabled() {
        let mut config = Config::default();
        config.merge_namespaces = true;
        config.merged_namespace_filename = Some("all".to_string());

        let path = locale_namespace_file_path(&config, "locales", "en", "translation");
        assert_eq!(
            path.to_string_lossy().replace('\\', "/"),
            "locales/en/all.json"
        );
    }

    #[test]
    fn test_locale_namespace_file_path_detects_existing_single_merged_file() {
        let tmp = tempfile::tempdir().unwrap();
        let locale_dir = tmp.path().join("en");
        std::fs::create_dir_all(&locale_dir).unwrap();
        std::fs::write(locale_dir.join("bundle.json"), "{}").unwrap();

        let mut config = Config::default();
        config.merge_namespaces = true;
        config.output = tmp.path().to_string_lossy().to_string();

        let path = locale_namespace_file_path(&config, &config.output, "en", "translation");
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("bundle.json")
        );
    }

    #[test]
    fn test_detect_existing_merged_filename_none_for_namespaced_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let locale_dir = tmp.path().join("en");
        std::fs::create_dir_all(&locale_dir).unwrap();
        std::fs::write(locale_dir.join("common.json"), "{}").unwrap();
        std::fs::write(locale_dir.join("home.json"), "{}").unwrap();

        let detected = detect_existing_merged_filename(tmp.path().to_str().unwrap(), "en", "json");
        assert_eq!(detected, None);
    }

    #[test]
    fn test_sync_locale_with_json5_preserves_number_literals() {
        use crate::fs::mock::InMemoryFileSystem;
        use std::path::Path;

        let fs = InMemoryFileSystem::new();
        fs.add_file(
            "locales/en/translation.json5",
            "{\n  count: 1e3,\n  hex: 0x10,\n  greeting: 'Hello',\n}\n",
        );

        let keys = vec![
            ExtractedKey {
                key: "count".to_string(),
                namespace: None,
                default_value: None,
            },
            ExtractedKey {
                key: "hex".to_string(),
                namespace: None,
                default_value: None,
            },
            ExtractedKey {
                key: "greeting".to_string(),
                namespace: None,
                default_value: Some("Hello".to_string()),
            },
            ExtractedKey {
                key: "farewell".to_string(),
                namespace: None,
                default_value: Some("Goodbye".to_string()),
            },
        ];

        let mut config = Config::default();
        config.output_format = OutputFormat::Json5;
        let matcher =
            PreserveMatcher::new(&config.preserve_patterns, &config.ns_separator).unwrap();

        let _ = sync_locale_file_locked_with_fs(
            Path::new("locales/en/translation.json5"),
            &keys,
            "translation",
            &config,
            &matcher,
            false,
            &fs,
        )
        .unwrap();

        let files = fs.get_files();
        let content = files
            .get(Path::new("locales/en/translation.json5"))
            .expect("File should exist");

        assert!(
            content.contains("1e3"),
            "scientific notation should be preserved, got:\n{}",
            content
        );
        assert!(
            content.contains("0x10"),
            "hex notation should be preserved, got:\n{}",
            content
        );
        assert!(
            content.contains("\"farewell\""),
            "new key should be written"
        );
    }

    #[test]
    fn test_parse_js_module_locale() {
        let module = "export default { \"hello\": \"world\" };";
        let value = parse_locale_value_str(module, OutputFormat::JsEsm, Path::new("test.js"))
            .expect("should parse");
        assert_eq!(value["hello"], "world");
    }

    #[test]
    fn test_write_js_locale_file() {
        let tmp = tempfile::tempdir().unwrap();
        let mut map = Map::new();
        map.insert("foo".to_string(), Value::String("bar".to_string()));
        let path = tmp.path().join("translation.js");
        write_js_locale_with_fs(&path, &map, &crate::fs::RealFileSystem, JsVariant::Esm)
            .expect("write js file");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("export default"));
        assert!(content.contains("foo"));
    }

    #[test]
    fn test_write_ts_locale_file() {
        let tmp = tempfile::tempdir().unwrap();
        let mut map = Map::new();
        map.insert("foo".to_string(), Value::String("bar".to_string()));
        let path = tmp.path().join("translation.ts");
        write_ts_locale_with_fs(&path, &map, &crate::fs::RealFileSystem).expect("write ts file");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("as const"));
        assert!(content.contains("foo"));
    }
}
