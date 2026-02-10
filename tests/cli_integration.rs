use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};
use tempfile::tempdir;

fn cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_i18next-turbo")
}

fn run_cli<P: AsRef<Path>>(cwd: P, args: &[&str]) -> Output {
    Command::new(cli_bin())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("failed to run i18next-turbo")
}

fn write_config(root: &Path) -> PathBuf {
    write_config_with_locales(root, &["en"])
}

fn write_config_with_locales(root: &Path, locales: &[&str]) -> PathBuf {
    write_config_with_options(root, locales, None)
}

fn write_config_with_options(
    root: &Path,
    locales: &[&str],
    output_format: Option<&str>,
) -> PathBuf {
    let mut config = json!({
        "input": ["src/**/*.ts", "src/**/*.tsx"],
        "output": "locales",
        "locales": locales,
        "functions": ["t"],
        "extractFromComments": false
    });

    if let Some(fmt) = output_format {
        config["outputFormat"] = Value::String(fmt.to_string());
    }

    let config_path = root.join("i18next-turbo.json");
    fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    config_path
}

fn read_json(path: &Path) -> Value {
    let content = fs::read_to_string(path).expect("missing json file");
    serde_json::from_str(&content).expect("invalid json")
}

fn write_locale_json(path: &Path, value: Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let content = serde_json::to_string_pretty(&value).unwrap();
    fs::write(path, format!("{}\n", content)).unwrap();
}

#[test]
fn extract_creates_locale_file() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.tsx"),
        r#"import { t } from 'i18next';
        const value = t('hello.world');
        console.log(value);
        "#,
    )
    .unwrap();

    let config_path = write_config(project);
    let output = run_cli(
        project,
        &["--config", config_path.to_str().unwrap(), "extract"],
    );
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let locale_json = project.join("locales/en/translation.json");
    let json = read_json(&locale_json);
    assert_eq!(json["hello"]["world"], "");
}

#[test]
fn extract_generates_types_file() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src"))
        .and_then(|_| fs::create_dir_all(project.join("src/@types")))
        .unwrap();
    fs::write(project.join("src/index.ts"), "t('greeting');").unwrap();
    let config_path = write_config(project);

    let types_path = project.join("src/@types/i18next.d.ts");
    let output = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "extract",
            "--generate-types",
            "--types-output",
            types_path.to_str().unwrap(),
        ],
    );
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(types_path.exists(), "types file was not generated");
    let content = fs::read_to_string(types_path).unwrap();
    assert!(content.contains("greeting"));
}

#[test]
fn extract_fail_on_warnings_returns_error() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.ts"),
        r#"const name = 'test';
        t(`dynamic-${name}`);
        "#,
    )
    .unwrap();
    let config_path = write_config(project);

    let output = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "extract",
            "--fail-on-warnings",
        ],
    );
    assert!(
        !output.status.success(),
        "command should fail; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("warning"),
        "expected warning in stderr"
    );
}

#[test]
fn extract_writes_json5_when_configured() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('hello.fromjson5');").unwrap();
    let config_path = write_config_with_options(project, &["en"], Some("json5"));

    let output = run_cli(
        project,
        &["--config", config_path.to_str().unwrap(), "extract"],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json5_path = project.join("locales/en/translation.json5");
    assert!(json5_path.exists(), "json5 file was not created");
    assert!(!project.join("locales/en/translation.json").exists());

    let content = fs::read_to_string(&json5_path).unwrap();
    let parsed: Value = json5::from_str(&content).expect("valid json5");
    assert_eq!(parsed["hello"]["fromjson5"], "");
}

#[test]
fn extract_writes_js_module_when_requested() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('hello.js');").unwrap();
    let config_path = write_config_with_options(project, &["en"], Some("js"));

    let output = run_cli(
        project,
        &["--config", config_path.to_str().unwrap(), "extract"],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let module_path = project.join("locales/en/translation.js");
    let contents = fs::read_to_string(&module_path).unwrap();
    assert!(contents.starts_with("export default"));
    assert!(contents.contains("hello"));
    assert!(module_path.exists());
}

#[test]
fn extract_writes_ts_module_when_requested() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('hello.ts');").unwrap();
    let config_path = write_config_with_options(project, &["en"], Some("ts"));

    let output = run_cli(
        project,
        &["--config", config_path.to_str().unwrap(), "extract"],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let module_path = project.join("locales/en/translation.ts");
    let contents = fs::read_to_string(&module_path).unwrap();
    assert!(contents.contains("as const"));
    assert!(contents.contains("hello"));
}

#[test]
fn sync_adds_missing_keys_to_secondary_locale() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "console.log('sync');").unwrap();
    let config_path = write_config_with_locales(project, &["en", "ja"]);

    write_locale_json(
        &project.join("locales/en/translation.json"),
        json!({"hello": "Hello"}),
    );
    write_locale_json(&project.join("locales/ja/translation.json"), json!({}));

    let output = run_cli(
        project,
        &["--config", config_path.to_str().unwrap(), "sync"],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let ja = read_json(&project.join("locales/ja/translation.json"));
    assert_eq!(ja["hello"], "");
}

#[test]
fn sync_remove_unused_respects_dry_run() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "console.log('sync');").unwrap();
    let config_path = write_config_with_locales(project, &["en", "ja"]);

    write_locale_json(
        &project.join("locales/en/translation.json"),
        json!({"keep": "Hello"}),
    );
    write_locale_json(
        &project.join("locales/ja/translation.json"),
        json!({"keep": "", "extra": ""}),
    );

    let dry = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "sync",
            "--remove-unused",
            "--dry-run",
        ],
    );
    assert!(
        dry.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&dry.stdout),
        String::from_utf8_lossy(&dry.stderr)
    );
    let before = read_json(&project.join("locales/ja/translation.json"));
    assert!(before.get("extra").is_some());

    let output = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "sync",
            "--remove-unused",
        ],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let after = read_json(&project.join("locales/ja/translation.json"));
    assert!(after.get("extra").is_none());
}

#[test]
fn status_fail_on_incomplete_returns_error() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "console.log('status');").unwrap();
    let config_path = write_config_with_locales(project, &["en", "ja"]);

    write_locale_json(
        &project.join("locales/en/translation.json"),
        json!({"hello": "Hello"}),
    );
    write_locale_json(
        &project.join("locales/ja/translation.json"),
        json!({"hello": ""}),
    );

    let ok = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "status",
            "--locale",
            "en",
        ],
    );
    assert!(ok.status.success());

    let fail = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "status",
            "--locale",
            "ja",
            "--fail-on-incomplete",
        ],
    );
    assert!(
        !fail.status.success(),
        "expected status failure; stdout: {} stderr: {}",
        String::from_utf8_lossy(&fail.stdout),
        String::from_utf8_lossy(&fail.stderr)
    );
}

#[test]
fn typegen_command_generates_file() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src"))
        .and_then(|_| fs::create_dir_all(project.join("src/@types")))
        .unwrap();
    let config_path = write_config(project);

    write_locale_json(
        &project.join("locales/en/translation.json"),
        json!({"bye": "Bye"}),
    );

    let types_out = project.join("types.d.ts");
    let output = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "typegen",
            "--output",
            types_out.to_str().unwrap(),
            "--default-locale",
            "en",
            "--locales-dir",
            "locales",
        ],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let content = fs::read_to_string(&types_out).unwrap();
    assert!(content.contains("bye"));
}

#[test]
fn check_dry_run_reports_dead_keys() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('alive.key');").unwrap();
    let config_path = write_config(project);

    write_locale_json(
        &project.join("locales/en/translation.json"),
        json!({
            "alive": { "key": "" },
            "dead": { "key": "" }
        }),
    );

    let output = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "check",
            "--dry-run",
        ],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would remove 1 key(s)"));

    let locale = read_json(&project.join("locales/en/translation.json"));
    assert!(locale["dead"]["key"].is_string());
}

#[test]
fn lint_fail_on_error_returns_non_zero() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.tsx"),
        "export const App = () => <div>Hello hardcoded</div>;",
    )
    .unwrap();
    let config_path = write_config(project);

    let output = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "lint",
            "--fail-on-error",
        ],
    );
    assert!(
        !output.status.success(),
        "expected lint failure; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lint issue"));
}

#[test]
fn lint_passes_when_only_translated_text_exists() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.tsx"),
        "import { t } from 'i18next'; export const App = () => <div>{t('ui.title')}</div>;",
    )
    .unwrap();
    let config_path = write_config(project);

    let output = run_cli(
        project,
        &["--config", config_path.to_str().unwrap(), "lint"],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No hardcoded strings found"));
}

#[test]
fn migrate_dry_run_prints_preview_for_existing_turbo_config() {
    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    let config_path = write_config(project);

    let output = run_cli(
        project,
        &[
            "--config",
            config_path.to_str().unwrap(),
            "migrate",
            "--dry-run",
            "--yes",
        ],
    );
    assert!(
        output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("変換後プレビュー"));
    assert!(stdout.contains("dry-run"));
}
