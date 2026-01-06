use crate::config::{Config, LocizeConfig, OutputFormat};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::{Client, Response};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn upload(
    config: &Config,
    locale: Option<String>,
    namespace: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let locize = require_locize_config(config)?;
    ensure_supported_format(config)?;
    let locales = resolve_locales(config, locale)?;
    let extension = config.output_format.extension().to_string();
    let namespaces = resolve_namespaces(config, locize, namespace.as_deref(), &extension)?;
    let api_key = resolve_api_key(locize)?;
    let version = locize
        .version
        .clone()
        .unwrap_or_else(|| "latest".to_string());
    let client = Client::new();

    for locale in locales {
        for ns in &namespaces {
            let file_path = locale_namespace_path(config, &locale, ns, &extension);
            if !file_path.exists() {
                println!(
                    "⚠︎ {} をスキップ (ファイルが見つかりません)",
                    file_path.display()
                );
                continue;
            }

            let payload = read_local_payload(config, &file_path)?;
            if dry_run {
                let key_count = payload.as_object().map(|o| o.len()).unwrap_or_default();
                println!("[dry-run] Upload {} / {} ({} keys)", locale, ns, key_count);
                continue;
            }

            let url = format!(
                "https://api.locize.io/{}/{}/{}/{}",
                locize.project_id, version, locale, ns
            );
            let response = client
                .put(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&payload)
                .send()
                .with_context(|| format!("Locize upload request failed: {}", url))?;
            ensure_success(response, &url)?;
            println!("✓ Uploaded {} / {}", locale, ns);
        }
    }

    Ok(())
}

pub fn download(
    config: &Config,
    locale: Option<String>,
    namespace: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let locize = require_locize_config(config)?;
    ensure_supported_format(config)?;
    let locales = resolve_locales(config, locale)?;
    let extension = config.output_format.extension().to_string();
    let namespaces = resolve_namespaces(config, locize, namespace.as_deref(), &extension)?;
    let api_key = resolve_api_key(locize)?;
    let version = locize
        .version
        .clone()
        .unwrap_or_else(|| "latest".to_string());
    let client = Client::new();

    for locale in locales {
        for ns in &namespaces {
            let url = format!(
                "https://api.locize.app/{}/{}/{}/{}",
                locize.project_id, version, locale, ns
            );
            if dry_run {
                println!("[dry-run] Download {} / {} ({})", locale, ns, url);
                continue;
            }

            let mut request = client.get(&url);
            request = request.header("Authorization", format!("Bearer {}", api_key));
            let response = request
                .send()
                .with_context(|| format!("Locize download request failed: {}", url))?;
            let response = ensure_success(response, &url)?;
            let payload: Value = response
                .json()
                .with_context(|| format!("Failed to parse Locize response: {}", url))?;

            let file_path = locale_namespace_path(config, &locale, ns, &extension);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
            let formatted = serde_json::to_string_pretty(&payload)?;
            fs::write(&file_path, format!("{}\n", formatted))
                .with_context(|| format!("Failed to write {}", file_path.display()))?;
            println!("✓ Downloaded {} / {}", locale, ns);
        }
    }

    Ok(())
}

fn require_locize_config<'a>(config: &'a Config) -> Result<&'a LocizeConfig> {
    config
        .locize
        .as_ref()
        .ok_or_else(|| anyhow!("Locize 設定が見つかりません。config.locize を設定してください。"))
}

fn ensure_supported_format(config: &Config) -> Result<()> {
    match config.output_format {
        OutputFormat::Json | OutputFormat::Json5 => Ok(()),
        other => bail!(
            "Locize コマンドは JSON/JSON5 出力のみサポートしています (現在: {:?})",
            other
        ),
    }
}

fn resolve_api_key(locize: &LocizeConfig) -> Result<String> {
    if let Some(key) = &locize.api_key {
        if !key.trim().is_empty() {
            return Ok(key.clone());
        }
    }

    if let Ok(env_key) = env::var("LOCIZE_API_KEY") {
        if !env_key.trim().is_empty() {
            return Ok(env_key);
        }
    }

    bail!("Locize API キーが設定されていません。config.locize.apiKey か LOCIZE_API_KEY を指定してください。");
}

fn resolve_locales(config: &Config, override_locale: Option<String>) -> Result<Vec<String>> {
    if let Some(locale) = override_locale {
        return Ok(vec![locale]);
    }
    Ok(config.locales.clone())
}

fn resolve_namespaces(
    config: &Config,
    locize: &LocizeConfig,
    override_namespace: Option<&str>,
    extension: &str,
) -> Result<Vec<String>> {
    if let Some(ns) = override_namespace {
        return Ok(vec![ns.to_string()]);
    }

    if let Some(namespaces) = &locize.namespaces {
        if !namespaces.is_empty() {
            return Ok(namespaces.clone());
        }
    }

    detect_namespaces_from_files(config, extension)
}

fn detect_namespaces_from_files(config: &Config, extension: &str) -> Result<Vec<String>> {
    let output_dir = Path::new(&config.output);
    for locale in &config.locales {
        let locale_dir = output_dir.join(locale);
        if !locale_dir.exists() {
            continue;
        }

        let mut namespaces = Vec::new();
        for entry in fs::read_dir(&locale_dir)
            .with_context(|| format!("Failed to read directory: {}", locale_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case(extension))
                    .unwrap_or(false)
                {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        namespaces.push(stem.to_string());
                    }
                }
            }
        }

        if !namespaces.is_empty() {
            namespaces.sort();
            namespaces.dedup();
            return Ok(namespaces);
        }
    }

    Ok(vec![config.default_namespace.clone()])
}

fn locale_namespace_path(
    config: &Config,
    locale: &str,
    namespace: &str,
    extension: &str,
) -> PathBuf {
    Path::new(&config.output)
        .join(locale)
        .join(format!("{}.{}", namespace, extension))
}

fn read_local_payload(config: &Config, path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read translation file: {}", path.display()))?;
    match config.output_format {
        OutputFormat::Json => Ok(serde_json::from_str(&content)?),
        OutputFormat::Json5 => Ok(json5::from_str(&content)?),
        _ => bail!("Unsupported output format for Locize upload"),
    }
}

fn ensure_success(response: Response, url: &str) -> Result<Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        bail!("Locize API error ({} {}): {}", url, status, body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detect_namespaces_from_files_prefers_existing_locale() {
        let tmp = tempdir().unwrap();
        let base = tmp.path().join("locales");
        fs::create_dir_all(base.join("en")).unwrap();
        fs::write(base.join("en").join("common.json"), "{}").unwrap();
        fs::write(base.join("en").join("home.json"), "{}").unwrap();

        let mut config = Config::default();
        config.output = base.to_string_lossy().to_string();
        config.locales = vec!["en".into()];

        let namespaces = detect_namespaces_from_files(&config, "json").unwrap();
        assert_eq!(namespaces, vec!["common".to_string(), "home".to_string()]);
    }

    #[test]
    fn detect_namespaces_falls_back_to_default() {
        let tmp = tempdir().unwrap();
        let base = tmp.path().join("locales");
        fs::create_dir_all(&base).unwrap();

        let mut config = Config::default();
        config.output = base.to_string_lossy().to_string();
        let namespaces = detect_namespaces_from_files(&config, "json").unwrap();
        assert_eq!(namespaces, vec![config.default_namespace]);
    }
}
