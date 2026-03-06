use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::tempdir;

fn has_node() -> bool {
    Command::new("node").arg("--version").output().is_ok()
}

fn has_node_wrapper_deps() -> bool {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    Command::new("node")
        .arg("-e")
        .arg("require('cosmiconfig'); require('jiti');")
        .current_dir(repo_root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn should_run_node_wrapper_tests() -> bool {
    has_node() && has_node_wrapper_deps()
}

fn wrapper_bin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bin/cli.js")
}

fn run_node_cli<P: AsRef<Path>>(cwd: P, args: &[&str]) -> Output {
    Command::new("node")
        .arg(wrapper_bin())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("failed to run node wrapper")
}

fn run_node_script<P: AsRef<Path>>(cwd: P, script: &str) -> Output {
    Command::new("node")
        .arg("-e")
        .arg(script)
        .current_dir(cwd)
        .output()
        .expect("failed to run node script")
}

#[cfg(feature = "napi")]
fn native_addon_require_script_body() -> String {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).display().to_string();
    format!(
        r#"
const path = require('path');
const repoRoot = {repo_root:?};
const platformMap = {{ darwin: 'darwin', linux: 'linux', win32: 'win32' }};
const archMap = {{ x64: 'x64', arm64: 'arm64', ia32: 'ia32' }};
const rustPlatform = platformMap[process.platform];
const rustArch = archMap[process.arch];
if (!rustPlatform || !rustArch) {{
  throw new Error(`unsupported test platform: ${{process.platform}}/${{process.arch}}`);
}}
const candidates = [
  path.join(repoRoot, 'target', 'debug', `i18next-turbo.${{rustPlatform}}-${{rustArch}}.node`),
  path.join(repoRoot, 'target', 'release', `i18next-turbo.${{rustPlatform}}-${{rustArch}}.node`)
];
let nativeAddon = null;
for (const candidate of candidates) {{
  try {{
    nativeAddon = require(candidate);
    break;
  }} catch (_error) {{}}
}}
if (!nativeAddon) {{
  throw new Error('native addon could not be loaded from target/debug or target/release');
}}
"#
    )
}

fn read_json(path: &Path) -> Value {
    let content = fs::read_to_string(path).expect("missing json file");
    serde_json::from_str(&content).expect("invalid json")
}

#[test]
fn node_wrapper_supports_default_value_and_sort_function_forms() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.ts"),
        "t('z.key');\nt('a.key');\nt('m.key');\n",
    )
    .unwrap();

    let config_js = r#"
module.exports = {
  locales: ['en'],
  extract: {
    input: 'src/**/*.ts',
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t'],
    defaultValue: (key, namespace, language, value) => `${language}:${namespace}:${key}`,
    sort: (a, b) => a.key.localeCompare(b.key)
  }
};
"#;
    fs::write(project.join("i18next.config.js"), config_js).unwrap();

    let output = run_node_cli(project, &["--config", "i18next.config.js", "extract"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let locale_file = project.join("locales/en/translation.json");
    let json = read_json(&locale_file);
    assert_eq!(json["a"]["key"], "en:translation:a.key");
    assert_eq!(json["m"]["key"], "en:translation:m.key");
    assert_eq!(json["z"]["key"], "en:translation:z.key");

    let content = fs::read_to_string(locale_file).unwrap();
    let a_idx = content.find("\"a\"").unwrap();
    let m_idx = content.find("\"m\"").unwrap();
    let z_idx = content.find("\"z\"").unwrap();
    assert!(a_idx < m_idx && m_idx < z_idx, "keys should be sorted");
}

#[test]
fn node_wrapper_plugin_onload_transforms_source_and_aftersync_runs() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.ts"),
        "const v = __('plugin.key');\nconsole.log(v);\n",
    )
    .unwrap();

    let plugin_js = r#"
const fs = require('fs');
const path = require('path');
module.exports = {
  onLoad({ source }) {
    return source.replace(/__\('([^']+)'\)/g, "t('$1')");
  },
  afterSync() {
    fs.writeFileSync(path.resolve(process.cwd(), '.after-sync'), 'ok');
  }
};
"#;
    fs::write(project.join("test-plugin.js"), plugin_js).unwrap();

    let config_js = r#"
module.exports = {
  locales: ['en'],
  plugins: [{ resolve: './test-plugin.js' }],
  extract: {
    input: 'src/**/*.ts',
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t']
  }
};
"#;
    fs::write(project.join("i18next.config.js"), config_js).unwrap();

    let output = run_node_cli(project, &["--config", "i18next.config.js", "extract"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let locale_file = project.join("locales/en/translation.json");
    let json = read_json(&locale_file);
    assert_eq!(json["plugin"]["key"], "");
    assert!(project.join(".after-sync").exists());
}

#[test]
fn node_wrapper_plugin_onvisitnode_receives_rust_ast_events() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.tsx"),
        r#"
const title = t('event.key');
const node = <Trans i18nKey="trans.event">x</Trans>;
"#,
    )
    .unwrap();

    let plugin_js = r#"
const fs = require('fs');
const path = require('path');
module.exports = {
  onVisitNode(node) {
    const out = path.resolve(process.cwd(), '.visit-events.jsonl');
    fs.appendFileSync(out, JSON.stringify(node) + '\n');
  }
};
"#;
    fs::write(project.join("visit-plugin.js"), plugin_js).unwrap();

    let config_js = r#"
module.exports = {
  locales: ['en'],
  plugins: [{ resolve: './visit-plugin.js' }],
  extract: {
    input: ['src/**/*.{ts,tsx}'],
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t']
  }
};
"#;
    fs::write(project.join("i18next.config.js"), config_js).unwrap();

    let output = run_node_cli(project, &["--config", "i18next.config.js", "extract"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let events_path = project.join(".visit-events.jsonl");
    let lines = fs::read_to_string(events_path).unwrap();
    assert!(lines.contains("\"eventType\":\"AstNodeVisit\""));
    assert!(lines.contains("\"type\":\"CallExpression\""));
    assert!(lines.contains("\"type\":\"TranslationKey\""));
    assert!(lines.contains("\"key\":\"event.key\""));
    assert!(lines.contains("\"type\":\"JSXElement\""));
}

#[test]
fn node_wrapper_merge_namespaces_without_namespace_placeholder_writes_single_file() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.ts"),
        "t('common:hello');\nt('home:title');\n",
    )
    .unwrap();

    let config_js = r#"
module.exports = {
  locales: ['en'],
  extract: {
    input: 'src/**/*.ts',
    output: 'locales/{{language}}/all.json',
    functions: ['t'],
    mergeNamespaces: true
  }
};
"#;
    fs::write(project.join("i18next.config.js"), config_js).unwrap();

    let output = run_node_cli(project, &["--config", "i18next.config.js", "extract"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged_file = project.join("locales/en/all.json");
    assert!(merged_file.exists(), "merged file should exist at all.json");
    let json = read_json(&merged_file);
    assert_eq!(json["common"]["hello"], "");
    assert_eq!(json["home"]["title"], "");
    assert!(
        !project.join("locales/en/translation.json").exists(),
        "translation.json should not be created for this output template"
    );
}

#[test]
fn node_api_fallback_returns_structured_results() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('api.hello');").unwrap();

    let script = format!(
        r#"
const path = require('path');
const fs = require('fs');
const os = require('os');
const Module = require('module');
const originalLoad = Module._load;
Module._load = function(request, parent, isMain) {{
  if (String(request).endsWith('.node') && String(request).includes('i18next-turbo.')) {{
    throw new Error('forced native load failure for fallback test');
  }}
  return originalLoad.apply(this, arguments);
}};
const api = require('{}');
(async () => {{
  const project = process.cwd();
  const extractResult = await api.extract({{
    input: ['src/**/*.ts'],
    output: 'locales',
    locales: ['en'],
    functions: ['t']
  }});
  if (!extractResult || extractResult.unique_keys !== 1) {{
    throw new Error(`unexpected extract result: ${{JSON.stringify(extractResult)}}`);
  }}
  const lintResult = await api.lint({{
    input: ['src/**/*.ts'],
    output: 'locales',
    locales: ['en'],
    functions: ['t']
  }});
  if (!lintResult || !Array.isArray(lintResult.issues)) {{
    throw new Error(`unexpected lint result: ${{JSON.stringify(lintResult)}}`);
  }}
  const checkResult = await api.check({{
    input: ['src/**/*.ts'],
    output: 'locales',
    locales: ['en'],
    functions: ['t']
  }}, {{ dry_run: true }});
  if (!checkResult || !Array.isArray(checkResult.dead_keys)) {{
    throw new Error(`unexpected check result: ${{JSON.stringify(checkResult)}}`);
  }}
}})().catch((err) => {{
  console.error(err);
  process.exit(1);
}});
"#,
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("lib")
            .join("index.js")
            .display()
    );

    let output = run_node_script(project, &script);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "napi")]
#[test]
fn native_node_api_extract_matches_documented_result_shape() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('api.native');").unwrap();

    let script = format!(
        r#"
{}
(async () => {{
  const fs = require('fs');
  const path = require('path');
  process.chdir({project:?});
  const result = await nativeAddon.extract({{
    input: ['src/**/*.ts'],
    output: 'locales',
    locales: ['en'],
    functions: ['t']
  }});
  const expectedKeys = [
    'conflicts',
    'extraction_warnings',
    'files_processed',
    'keys_added',
    'keys_removed',
    'message',
    'success',
    'unique_keys',
    'updated_files',
    'warnings'
  ];
  const resultKeys = Object.keys(result).sort();
  if (JSON.stringify(resultKeys) !== JSON.stringify(expectedKeys)) {{
    throw new Error(`unexpected native extract keys: ${{JSON.stringify(resultKeys)}}`);
  }}
  if (result.unique_keys !== 1 || result.keys_added !== 1 || result.conflicts !== 0) {{
    throw new Error(`unexpected native extract result: ${{JSON.stringify(result)}}`);
  }}
  if (!fs.existsSync(path.join(process.cwd(), 'locales', 'en', 'translation.json'))) {{
    throw new Error('translation.json was not created');
  }}
}})().catch((err) => {{
  console.error(err);
  process.exit(1);
}});
"#,
        native_addon_require_script_body(),
        project = project.display().to_string()
    );

    let output = run_node_script(project, &script);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "napi")]
#[test]
fn native_node_api_extract_respects_fail_on_warnings_for_conflicts() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("locales/en")).unwrap();
    fs::write(project.join("src/app.ts"), "t('hello.world');").unwrap();
    fs::write(
        project.join("locales/en/translation.json"),
        "{\n  \"hello\": \"scalar\"\n}\n",
    )
    .unwrap();

    let script = format!(
        r#"
{}
(async () => {{
  process.chdir({project:?});
  let rejected = false;
  try {{
    await nativeAddon.extract({{
      input: ['src/**/*.ts'],
      output: 'locales',
      locales: ['en'],
      functions: ['t']
    }}, {{
      fail_on_warnings: true
    }});
  }} catch (error) {{
    rejected = /warning\(s\) encountered/.test(String(error && error.message));
  }}
  if (!rejected) {{
    throw new Error('native extract did not reject on conflict warning');
  }}
}})().catch((err) => {{
  console.error(err);
  process.exit(1);
}});
"#,
        native_addon_require_script_body(),
        project = project.display().to_string()
    );

    let output = run_node_script(project, &script);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "napi")]
#[test]
fn native_node_api_lint_honors_lint_configuration() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("src/app.tsx"),
        "<input placeholder=\"Name\" />\n",
    )
    .unwrap();

    let script = format!(
        r#"
{}
(async () => {{
  process.chdir({project:?});
  const result = await nativeAddon.lint({{
    input: ['src/**/*.tsx'],
    output: 'locales',
    locales: ['en'],
    functions: ['t'],
    lint: {{
      ignoredAttributes: ['placeholder']
    }}
  }});
  if (!result || !Array.isArray(result.issues) || result.issues.length !== 0) {{
    throw new Error(`native lint ignored config: ${{JSON.stringify(result)}}`);
  }}
}})().catch((err) => {{
  console.error(err);
  process.exit(1);
}});
"#,
        native_addon_require_script_body(),
        project = project.display().to_string()
    );

    let output = run_node_script(project, &script);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn node_wrapper_plugin_errors_continue_by_default() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('continue.mode');\n").unwrap();

    let plugin_js = r#"
module.exports = {
  afterSync() {
    throw new Error('afterSync boom');
  }
};
"#;
    fs::write(project.join("error-plugin.js"), plugin_js).unwrap();

    let config_js = r#"
module.exports = {
  locales: ['en'],
  plugins: [{ resolve: './error-plugin.js' }],
  extract: {
    input: 'src/**/*.ts',
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t']
  }
};
"#;
    fs::write(project.join("i18next.config.js"), config_js).unwrap();

    let output = run_node_cli(project, &["--config", "i18next.config.js", "extract"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let locale_file = project.join("locales/en/translation.json");
    let json = read_json(&locale_file);
    assert_eq!(json["continue"]["mode"], "");
}

#[test]
fn node_wrapper_plugin_errors_fail_fast_when_configured() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/app.ts"), "t('fail.fast');\n").unwrap();

    let plugin_js = r#"
module.exports = {
  afterSync() {
    throw new Error('afterSync boom');
  }
};
"#;
    fs::write(project.join("error-plugin.js"), plugin_js).unwrap();

    let config_js = r#"
module.exports = {
  locales: ['en'],
  pluginErrorMode: 'fail-fast',
  plugins: [{ resolve: './error-plugin.js' }],
  extract: {
    input: 'src/**/*.ts',
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t']
  }
};
"#;
    fs::write(project.join("i18next.config.js"), config_js).unwrap();

    let output = run_node_cli(project, &["--config", "i18next.config.js", "extract"]);
    assert!(
        !output.status.success(),
        "expected fail-fast mode to return non-zero; stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("plugin hook \"afterSync\" failed"));
}

#[test]
fn node_wrapper_runtime_transform_preserves_json_style() {
    if !should_run_node_wrapper_tests() {
        return;
    }

    let tmp = tempdir().unwrap();
    let project = tmp.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("locales/en")).unwrap();
    fs::write(project.join("src/app.ts"), "t('z.key');\nt('a.key');\n").unwrap();
    // tabs + CRLF + no trailing newline
    fs::write(
        project.join("locales/en/translation.json"),
        "{\r\n\t\"z\": {\r\n\t\t\"key\": \"\"\r\n\t},\r\n\t\"a\": {\r\n\t\t\"key\": \"\"\r\n\t}\r\n}",
    )
    .unwrap();

    let config_js = r#"
module.exports = {
  locales: ['en'],
  extract: {
    input: 'src/**/*.ts',
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t'],
    defaultValue: (key) => key,
    sort: (a, b) => a.key.localeCompare(b.key)
  }
};
"#;
    fs::write(project.join("i18next.config.js"), config_js).unwrap();

    let output = run_node_cli(project, &["--config", "i18next.config.js", "extract"]);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(project.join("locales/en/translation.json")).unwrap();
    assert!(content.contains("\r\n\t\"a\""));
    assert!(!content.ends_with('\n'));
}
