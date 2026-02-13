#![allow(clippy::too_many_arguments)]

use crate::config::EnableSelector;
use anyhow::{Context, Result};
use glob::glob;
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Generate TypeScript type definitions from translation JSON files
pub fn generate_types(locales_dir: &Path, output_path: &Path, default_locale: &str) -> Result<()> {
    generate_types_with_options(
        locales_dir,
        output_path,
        default_locale,
        None,
        None,
        None,
        None,
        false,
    )
}

/// Generate TypeScript type definitions with custom indentation
pub fn generate_types_with_options(
    locales_dir: &Path,
    output_path: &Path,
    default_locale: &str,
    indentation: Option<&str>,
    input_patterns: Option<&[String]>,
    resources_file: Option<&Path>,
    enable_selector: Option<&EnableSelector>,
    merge_namespaces: bool,
) -> Result<()> {
    let resources = load_resources(
        locales_dir,
        default_locale,
        input_patterns,
        merge_namespaces,
    )?;

    if resources.is_empty() {
        return Ok(());
    }

    write_types_file(
        output_path,
        &resources,
        indentation.unwrap_or("  "),
        true,
        enable_selector,
    )?;
    if let Some(resources_path) = resources_file {
        write_types_file(
            resources_path,
            &resources,
            indentation.unwrap_or("  "),
            false,
            enable_selector,
        )?;
    }

    Ok(())
}

fn load_resources(
    locales_dir: &Path,
    default_locale: &str,
    input_patterns: Option<&[String]>,
    merge_namespaces: bool,
) -> Result<Map<String, Value>> {
    let mut resources: Map<String, Value> = Map::new();
    let locale_dir = locales_dir.join(default_locale);
    if !locale_dir.exists() {
        return Ok(resources);
    }

    let files = resolve_typegen_files(&locale_dir, input_patterns)?;
    for path in files {
        let namespace = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("translation");
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read: {}", path.display()))?;
        let json: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse: {}", path.display()))?;
        if merge_namespaces {
            if let Value::Object(obj) = json {
                for (ns, value) in obj {
                    resources.insert(ns, value);
                }
            } else {
                resources.insert(namespace.to_string(), json);
            }
        } else {
            resources.insert(namespace.to_string(), json);
        }
    }

    Ok(resources)
}

fn resolve_typegen_files(
    locale_dir: &Path,
    input_patterns: Option<&[String]>,
) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = Vec::new();

    if let Some(patterns) = input_patterns {
        for pattern in patterns {
            let candidate = Path::new(pattern);
            let glob_pattern = if candidate.is_absolute() || pattern.contains('/') {
                pattern.to_string()
            } else {
                locale_dir.join(pattern).to_string_lossy().to_string()
            };

            let matches =
                glob(&glob_pattern).with_context(|| format!("Invalid typegen input pattern: {}", pattern))?;
            for path in matches.flatten() {
                if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
                    files.push(path);
                }
            }
        }
    } else {
        for entry in std::fs::read_dir(locale_dir)
            .with_context(|| format!("Failed to read locale directory: {}", locale_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
                files.push(path);
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn write_types_file(
    output_path: &Path,
    resources: &Map<String, Value>,
    indentation: &str,
    include_default_export: bool,
    enable_selector: Option<&EnableSelector>,
) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp_path = output_path.with_extension("d.ts.tmp");
    {
        let file = File::create(&temp_path)
            .with_context(|| format!("Failed to create temp file: {}", temp_path.display()))?;
        let mut writer = BufWriter::new(file);
        write_ts_content(
            &mut writer,
            resources,
            indentation,
            include_default_export,
            enable_selector,
        )?;
        writer.flush()?;
    }
    std::fs::rename(&temp_path, output_path)
        .with_context(|| format!("Failed to rename temp file to: {}", output_path.display()))?;
    Ok(())
}

/// Stream TypeScript content directly to a writer (memory-efficient)
fn write_ts_content<W: Write>(
    writer: &mut W,
    resources: &Map<String, Value>,
    indentation: &str,
    include_default_export: bool,
    enable_selector: Option<&EnableSelector>,
) -> Result<()> {
    // Header comment
    writeln!(writer, "// This file is auto-generated by i18next-turbo")?;
    writeln!(writer, "// Do not edit manually\n")?;

    // Generate interface for each namespace
    for (namespace, value) in resources {
        let interface_name = to_pascal_case(namespace);
        writeln!(writer, "interface {} {{", interface_name)?;
        write_interface_body(writer, value, 1, indentation)?;
        writeln!(writer, "}}\n")?;
    }

    // Generate the Resources interface
    writeln!(writer, "interface Resources {{")?;
    for namespace in resources.keys() {
        let interface_name = to_pascal_case(namespace);
        writeln!(
            writer,
            "{}\"{}\": {};",
            indentation, namespace, interface_name
        )?;
    }
    writeln!(writer, "}}\n")?;

    // Export declaration
    writeln!(writer, "export {{ Resources }};")?;
    if include_default_export {
        writeln!(writer, "export default Resources;")?;
    }

    if let Some(selector) = enable_selector {
        if selector.enabled() {
            writeln!(writer)?;
            writeln!(writer, "export type SelectorRoot = Resources;")?;
            writeln!(
                writer,
                "export type SelectorFn<T = unknown> = ($: SelectorRoot) => T;"
            )?;
            writeln!(
                writer,
                "export declare function keyFromSelector<T>(selector: SelectorFn<T>): string;"
            )?;

            if selector.optimize() {
                let mut keys = Vec::new();
                for (namespace, value) in resources {
                    collect_selector_keys(namespace, value, "", &mut keys);
                }
                keys.sort();
                keys.dedup();

                if keys.is_empty() {
                    writeln!(writer, "export type SelectorKey = never;")?;
                } else {
                    writeln!(writer, "export type SelectorKey =")?;
                    for key in keys {
                        writeln!(writer, "{}| \"{}\"", indentation, key)?;
                    }
                    writeln!(writer, ";")?;
                }
            }
        }
    }

    Ok(())
}

fn collect_selector_keys(namespace: &str, value: &Value, prefix: &str, out: &mut Vec<String>) {
    if let Value::Object(obj) = value {
        for (k, v) in obj {
            let next = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{}.{}", prefix, k)
            };
            if v.is_object() {
                collect_selector_keys(namespace, v, &next, out);
            } else {
                out.push(format!("{}.{}", namespace, next));
            }
        }
    }
}

/// Stream interface body recursively to a writer
fn write_interface_body<W: Write>(
    writer: &mut W,
    value: &Value,
    depth: usize,
    indentation: &str,
) -> Result<()> {
    let indent = indentation.repeat(depth);

    if let Value::Object(obj) = value {
        for (key, val) in obj {
            let key_safe = if key.contains('.') || key.contains('-') {
                format!("\"{}\"", key)
            } else {
                key.clone()
            };

            match val {
                Value::Object(_) => {
                    writeln!(writer, "{}{}: {{", indent, key_safe)?;
                    write_interface_body(writer, val, depth + 1, indentation)?;
                    writeln!(writer, "{}}};", indent)?;
                }
                Value::String(_) => {
                    writeln!(writer, "{}{}: string;", indent, key_safe)?;
                }
                _ => {
                    writeln!(writer, "{}{}: unknown;", indent, key_safe)?;
                }
            }
        }
    }

    Ok(())
}

/// Generate TypeScript interface content from resources (for testing)
#[cfg(test)]
fn generate_ts_content(resources: &Map<String, Value>) -> String {
    let mut output = Vec::new();
    write_ts_content(&mut output, resources, "  ", true, None).expect("Failed to write to buffer");
    String::from_utf8(output).expect("Invalid UTF-8")
}

/// Convert namespace to PascalCase for interface name
fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("translation"), "Translation");
        assert_eq!(to_pascal_case("common-errors"), "CommonErrors");
        assert_eq!(to_pascal_case("my_namespace"), "MyNamespace");
    }

    #[test]
    fn test_generate_ts_content() {
        let mut resources = Map::new();

        let mut translation = Map::new();
        translation.insert("hello".to_string(), Value::String("Hello".to_string()));

        let mut button = Map::new();
        button.insert("submit".to_string(), Value::String("Submit".to_string()));
        button.insert("cancel".to_string(), Value::String("Cancel".to_string()));
        translation.insert("button".to_string(), Value::Object(button));

        resources.insert("translation".to_string(), Value::Object(translation));

        let ts = generate_ts_content(&resources);

        assert!(ts.contains("interface Translation {"));
        assert!(ts.contains("hello: string;"));
        assert!(ts.contains("button: {"));
        assert!(ts.contains("submit: string;"));
        assert!(ts.contains("interface Resources {"));
        assert!(ts.contains("\"translation\": Translation;"));
    }

    #[test]
    fn test_generate_ts_content_with_custom_indent() {
        let mut resources = Map::new();
        resources.insert(
            "translation".to_string(),
            Value::Object(
                [("hello".to_string(), Value::String("Hello".to_string()))]
                    .into_iter()
                    .collect(),
            ),
        );

        let mut output = Vec::new();
        write_ts_content(&mut output, &resources, "\t", true, None).unwrap();
        let ts = String::from_utf8(output).unwrap();
        assert!(ts.contains("\t\"translation\": Translation;"));
        assert!(ts.contains("\thello: string;"));
    }

    #[test]
    fn test_generate_types_with_input_patterns_and_resources_file() {
        let tmp = tempdir().unwrap();
        let locales_dir = tmp.path().join("locales");
        let en_dir = locales_dir.join("en");
        fs::create_dir_all(&en_dir).unwrap();
        fs::write(
            en_dir.join("common.json"),
            r#"{ "hello": "Hello", "nested": { "x": "y" } }"#,
        )
        .unwrap();
        fs::write(en_dir.join("ignore.json"), r#"{ "skip": "me" }"#).unwrap();

        let output = tmp.path().join("types").join("i18next.d.ts");
        let resources_file = tmp.path().join("types").join("resources.d.ts");
        let patterns = vec!["common.json".to_string()];

        generate_types_with_options(
            &locales_dir,
            &output,
            "en",
            Some("  "),
            Some(&patterns),
            Some(resources_file.as_path()),
            None,
            false,
        )
        .unwrap();

        let main_content = fs::read_to_string(output).unwrap();
        let resources_content = fs::read_to_string(resources_file).unwrap();
        assert!(main_content.contains("interface Common"));
        assert!(!main_content.contains("interface Ignore"));
        assert!(main_content.contains("export default Resources;"));
        assert!(resources_content.contains("export { Resources };"));
        assert!(!resources_content.contains("export default Resources;"));
    }

    #[test]
    fn test_generate_ts_content_with_enable_selector_optimize() {
        let mut resources = Map::new();
        resources.insert(
            "common".to_string(),
            Value::Object(
                [("hello".to_string(), Value::String("Hello".to_string()))]
                    .into_iter()
                    .collect(),
            ),
        );

        let mut output = Vec::new();
        write_ts_content(
            &mut output,
            &resources,
            "  ",
            true,
            Some(&EnableSelector::Mode("optimize".to_string())),
        )
        .unwrap();
        let ts = String::from_utf8(output).unwrap();
        assert!(ts.contains("export type SelectorRoot = Resources;"));
        assert!(ts.contains("export type SelectorKey ="));
        assert!(ts.contains("\"common.hello\""));
    }
}
