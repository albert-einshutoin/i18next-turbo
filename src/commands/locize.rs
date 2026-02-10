use crate::config::{Config, LocizeConfig, OutputFormat};
use crate::logging;
use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::LAST_MODIFIED;
use serde_json::Value;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

pub fn upload(
    config: &Config,
    locale: Option<String>,
    namespace: Option<String>,
    dry_run: bool,
) -> Result<()> {
    check_locize_cli_dependency();
    let locize = require_locize_config(config)?;
    ensure_supported_format(config)?;
    let mut locales = resolve_locales(config, locale)?;
    if locize.source_language_only.unwrap_or(false) {
        locales = vec![resolve_source_locale(config, locize)];
    }
    let extension = config.output_format.extension().to_string();
    let namespaces = resolve_namespaces(config, locize, namespace.as_deref(), &extension)?;
    let api_key = resolve_api_key(locize)?;
    let dry_run = dry_run || locize.dry_run.unwrap_or(false);
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
            let payload = if !locize.update_values.unwrap_or(true) {
                let existing =
                    fetch_remote_payload(&client, locize, &api_key, &version, &locale, ns)
                        .unwrap_or(Value::Object(Default::default()));
                filter_new_keys(&payload, &existing)
            } else {
                payload
            };
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

pub fn sync(
    config: &Config,
    locale: Option<String>,
    namespace: Option<String>,
    dry_run: bool,
) -> Result<()> {
    check_locize_cli_dependency();
    upload(config, locale.clone(), namespace.clone(), dry_run)?;
    download(config, locale, namespace, dry_run)
}

pub fn migrate(
    config: &Config,
    locale: Option<String>,
    namespace: Option<String>,
    dry_run: bool,
) -> Result<()> {
    println!("Locize migration: uploading local resources and downloading normalized resources...");
    sync(config, locale, namespace, dry_run)
}

pub fn download(
    config: &Config,
    locale: Option<String>,
    namespace: Option<String>,
    dry_run: bool,
) -> Result<()> {
    check_locize_cli_dependency();
    let locize = require_locize_config(config)?;
    ensure_supported_format(config)?;
    let mut locales = resolve_locales(config, locale)?;
    if locize.source_language_only.unwrap_or(false) {
        locales = vec![resolve_source_locale(config, locize)];
    }
    let extension = config.output_format.extension().to_string();
    let namespaces = resolve_namespaces(config, locize, namespace.as_deref(), &extension)?;
    let api_key = resolve_api_key(locize)?;
    let dry_run = dry_run || locize.dry_run.unwrap_or(false);
    let version = locize
        .version
        .clone()
        .unwrap_or_else(|| "latest".to_string());
    let client = Client::new();

    for locale in locales {
        for ns in &namespaces {
            let host = download_base_host(locize);
            let url = format!(
                "https://{}/{}/{}/{}/{}",
                host, locize.project_id, version, locale, ns
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
            let remote_last_modified = response
                .headers()
                .get(LAST_MODIFIED)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| httpdate::parse_http_date(s).ok());
            let response = ensure_success(response, &url)?;
            let payload: Value = response
                .json()
                .with_context(|| format!("Failed to parse Locize response: {}", url))?;

            let file_path = locale_namespace_path(config, &locale, ns, &extension);
            if locize.compare_modification_time.unwrap_or(false)
                && should_skip_download_due_to_mtime(&file_path, remote_last_modified)?
            {
                println!("↷ Skipped {} / {} (local file is newer)", locale, ns);
                continue;
            }
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

#[allow(clippy::too_many_arguments)]
pub fn setup(
    config: &Config,
    source_config_path: Option<&Path>,
    output: Option<PathBuf>,
    project_id: Option<String>,
    api_key: Option<String>,
    version: Option<String>,
    source_language: Option<String>,
    namespaces: Option<String>,
    yes: bool,
) -> Result<()> {
    let save_path = resolve_setup_output_path(source_config_path, output);
    let mut root = load_or_default_config_json(&save_path)?;

    let project_id = resolve_required_input(
        "Locize projectId",
        project_id,
        None,
        yes,
        "locize.projectId は必須です。--project-id で指定するか、対話入力を行ってください。",
    )?;
    let api_key = resolve_optional_input("Locize apiKey", api_key, None, yes)?;
    let version =
        resolve_optional_input("Locize version", version, Some("latest".to_string()), yes)?;
    let source_language = resolve_optional_input(
        "Locize sourceLanguage",
        source_language,
        Some(config.primary_language().to_string()),
        yes,
    )?;
    let namespaces_raw =
        resolve_optional_input("Locize namespaces (comma separated)", namespaces, None, yes)?;
    let parsed_namespaces = namespaces_raw
        .as_deref()
        .map(parse_csv_list)
        .filter(|v| !v.is_empty());

    let mut locize = serde_json::Map::new();
    locize.insert("projectId".to_string(), Value::String(project_id));
    if let Some(v) = api_key.filter(|v| !v.trim().is_empty()) {
        locize.insert("apiKey".to_string(), Value::String(v));
    }
    if let Some(v) = version.filter(|v| !v.trim().is_empty()) {
        locize.insert("version".to_string(), Value::String(v));
    }
    if let Some(v) = source_language.filter(|v| !v.trim().is_empty()) {
        locize.insert("sourceLanguage".to_string(), Value::String(v));
    }
    if let Some(list) = parsed_namespaces {
        locize.insert(
            "namespaces".to_string(),
            Value::Array(list.into_iter().map(Value::String).collect()),
        );
    }

    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("設定ファイルのルートは JSON オブジェクトである必要があります"))?;
    root_obj.insert("locize".to_string(), Value::Object(locize));

    if let Some(parent) = save_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    let serialized = serde_json::to_string_pretty(&root)?;
    fs::write(&save_path, format!("{}\n", serialized))
        .with_context(|| format!("設定ファイルの保存に失敗しました: {}", save_path.display()))?;

    println!("✓ Locize 設定を保存しました: {}", save_path.display());
    Ok(())
}

fn require_locize_config(config: &Config) -> Result<&LocizeConfig> {
    config
        .locize
        .as_ref()
        .ok_or_else(|| anyhow!("Locize 設定が見つかりません。config.locize を設定してください。"))
}

fn resolve_setup_output_path(
    source_config_path: Option<&Path>,
    output: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = output {
        return path;
    }
    if let Some(path) = source_config_path {
        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if is_json {
            return path.to_path_buf();
        }
    }
    PathBuf::from("i18next-turbo.json")
}

fn load_or_default_config_json(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("設定ファイルの読み込みに失敗しました: {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let parsed: Value = serde_json::from_str(&content)
        .with_context(|| format!("JSON 設定の解析に失敗しました: {}", path.display()))?;
    Ok(parsed)
}

fn resolve_required_input(
    label: &str,
    provided: Option<String>,
    default_value: Option<String>,
    yes: bool,
    missing_err: &str,
) -> Result<String> {
    let value = resolve_optional_input(label, provided, default_value, yes)?;
    let value = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow!(missing_err.to_string()))?;
    Ok(value)
}

fn resolve_optional_input(
    label: &str,
    provided: Option<String>,
    default_value: Option<String>,
    yes: bool,
) -> Result<Option<String>> {
    if let Some(v) = provided {
        return Ok(Some(v));
    }
    if yes {
        return Ok(default_value);
    }
    prompt_input(label, default_value)
}

fn prompt_input(label: &str, default_value: Option<String>) -> Result<Option<String>> {
    let mut stdout = io::stdout();
    if let Some(default_value) = &default_value {
        print!("{} [{}]: ", label, default_value);
    } else {
        print!("{}: ", label);
    }
    stdout
        .flush()
        .context("標準出力への書き込みに失敗しました")?;
    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .context("標準入力の読み取りに失敗しました")?;
    let entered = buf.trim().to_string();
    if entered.is_empty() {
        Ok(default_value)
    } else {
        Ok(Some(entered))
    }
}

fn parse_csv_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
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

fn resolve_source_locale(config: &Config, locize: &LocizeConfig) -> String {
    locize
        .source_language
        .clone()
        .unwrap_or_else(|| config.primary_language().to_string())
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
            if path.is_file()
                && path
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

        if !namespaces.is_empty() {
            namespaces.sort();
            namespaces.dedup();
            return Ok(namespaces);
        }
    }

    Ok(vec![config.effective_default_namespace().to_string()])
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

fn download_base_host(locize: &LocizeConfig) -> &'static str {
    match locize.cdn_type.as_deref() {
        Some("pro") => "api.locize.pro",
        _ => "api.locize.app",
    }
}

fn fetch_remote_payload(
    client: &Client,
    locize: &LocizeConfig,
    api_key: &str,
    version: &str,
    locale: &str,
    namespace: &str,
) -> Result<Value> {
    let host = download_base_host(locize);
    let url = format!(
        "https://{}/{}/{}/{}/{}",
        host, locize.project_id, version, locale, namespace
    );
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .with_context(|| format!("Locize fetch request failed: {}", url))?;
    if !response.status().is_success() {
        return Ok(Value::Object(Default::default()));
    }
    response
        .json()
        .with_context(|| format!("Failed to parse Locize response: {}", url))
}

fn filter_new_keys(local: &Value, remote: &Value) -> Value {
    match (local, remote) {
        (Value::Object(local_obj), Value::Object(remote_obj)) => {
            let mut out = serde_json::Map::new();
            for (k, local_v) in local_obj {
                match remote_obj.get(k) {
                    None => {
                        out.insert(k.clone(), local_v.clone());
                    }
                    Some(remote_v) => {
                        let diff = filter_new_keys(local_v, remote_v);
                        if !diff.is_null() {
                            out.insert(k.clone(), diff);
                        }
                    }
                }
            }
            Value::Object(out)
        }
        // If remote has any value at this key, keep existing (no update)
        (_, Value::Null) => local.clone(),
        (_, _) => Value::Null,
    }
}

fn should_skip_download_due_to_mtime(
    path: &Path,
    remote_mtime: Option<SystemTime>,
) -> Result<bool> {
    let Some(remote_time) = remote_mtime else {
        return Ok(false);
    };
    if !path.exists() {
        return Ok(false);
    }
    let local_modified = fs::metadata(path)
        .with_context(|| format!("Failed to read metadata: {}", path.display()))?
        .modified()
        .with_context(|| format!("Failed to read modified time: {}", path.display()))?;
    Ok(local_modified > remote_time)
}

fn check_locize_cli_dependency() {
    if Command::new("locize").arg("--version").output().is_err() {
        logging::warn(
            "locize-cli が見つかりません。API同期は継続しますが、互換性のため `npm i -g locize-cli` を推奨します。",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
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
        assert_eq!(
            namespaces,
            vec![config.effective_default_namespace().to_string()]
        );
    }

    #[test]
    fn filter_new_keys_keeps_only_missing_remote_keys() {
        let local: Value = serde_json::json!({
            "a": "1",
            "b": { "x": "2", "y": "3" },
            "c": "4"
        });
        let remote: Value = serde_json::json!({
            "a": "old",
            "b": { "x": "old" }
        });
        let diff = filter_new_keys(&local, &remote);
        assert_eq!(diff, serde_json::json!({ "b": { "y": "3" }, "c": "4" }));
    }

    #[test]
    fn should_skip_download_when_local_newer() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("a.json");
        fs::write(&file, "{}").unwrap();
        let remote = Some(SystemTime::now() - Duration::from_secs(3600));
        assert!(should_skip_download_due_to_mtime(&file, remote).unwrap());
    }

    #[test]
    fn ensure_supported_format_rejects_js_output() {
        let mut config = Config::default();
        config.output_format = OutputFormat::JsEsm;
        let err = ensure_supported_format(&config).unwrap_err();
        assert!(err.to_string().contains("JSON/JSON5"));
    }

    #[test]
    fn resolve_api_key_prefers_config_value() {
        let locize = LocizeConfig {
            project_id: "pid".to_string(),
            api_key: Some("from-config".to_string()),
            version: None,
            source_language: None,
            namespaces: None,
            update_values: None,
            source_language_only: None,
            compare_modification_time: None,
            cdn_type: None,
            dry_run: None,
        };
        let resolved = resolve_api_key(&locize).unwrap();
        assert_eq!(resolved, "from-config");
    }

    #[test]
    fn resolve_namespaces_uses_override_first() {
        let mut config = Config::default();
        config.output = ".".to_string();
        let locize = LocizeConfig {
            project_id: "pid".to_string(),
            api_key: Some("k".to_string()),
            version: None,
            source_language: None,
            namespaces: Some(vec!["ignored".to_string()]),
            update_values: None,
            source_language_only: None,
            compare_modification_time: None,
            cdn_type: None,
            dry_run: None,
        };
        let resolved = resolve_namespaces(&config, &locize, Some("forced"), "json").unwrap();
        assert_eq!(resolved, vec!["forced".to_string()]);
    }

    #[test]
    fn resolve_namespaces_uses_locize_namespaces_when_present() {
        let mut config = Config::default();
        config.output = ".".to_string();
        let locize = LocizeConfig {
            project_id: "pid".to_string(),
            api_key: Some("k".to_string()),
            version: None,
            source_language: None,
            namespaces: Some(vec!["a".to_string(), "b".to_string()]),
            update_values: None,
            source_language_only: None,
            compare_modification_time: None,
            cdn_type: None,
            dry_run: None,
        };
        let resolved = resolve_namespaces(&config, &locize, None, "json").unwrap();
        assert_eq!(resolved, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn download_base_host_uses_pro_when_configured() {
        let locize = LocizeConfig {
            project_id: "pid".to_string(),
            api_key: Some("k".to_string()),
            version: None,
            source_language: None,
            namespaces: None,
            update_values: None,
            source_language_only: None,
            compare_modification_time: None,
            cdn_type: Some("pro".to_string()),
            dry_run: None,
        };
        assert_eq!(download_base_host(&locize), "api.locize.pro");
    }

    #[test]
    fn locale_namespace_path_builds_expected_path() {
        let mut config = Config::default();
        config.output = "locales".to_string();
        let p = locale_namespace_path(&config, "en", "common", "json");
        assert!(p.ends_with(Path::new("locales/en/common.json")));
    }

    #[test]
    fn read_local_payload_supports_json_and_json5() {
        let tmp = tempdir().unwrap();
        let json_path = tmp.path().join("a.json");
        fs::write(&json_path, r#"{"hello":"world"}"#).unwrap();
        let mut config = Config::default();
        config.output_format = OutputFormat::Json;
        let json_v = read_local_payload(&config, &json_path).unwrap();
        assert_eq!(json_v["hello"], "world");

        let json5_path = tmp.path().join("b.json5");
        fs::write(&json5_path, "{ hello: 'world' }").unwrap();
        config.output_format = OutputFormat::Json5;
        let json5_v = read_local_payload(&config, &json5_path).unwrap();
        assert_eq!(json5_v["hello"], "world");
    }

    #[test]
    fn resolve_setup_output_path_prefers_json_source_when_available() {
        let p = resolve_setup_output_path(Some(Path::new("/tmp/project/i18next-turbo.json")), None);
        assert_eq!(p, PathBuf::from("/tmp/project/i18next-turbo.json"));
    }

    #[test]
    fn parse_csv_list_splits_and_trims_entries() {
        let items = parse_csv_list("common, home, ,auth");
        assert_eq!(
            items,
            vec!["common".to_string(), "home".to_string(), "auth".to_string()]
        );
    }
}
