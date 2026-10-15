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

#[cfg(unix)]
#[test]
fn node_wrapper_reports_signal_termination_as_failure() {
    use std::os::unix::fs::PermissionsExt;

    if !should_run_node_wrapper_tests() {
        return;
    }
    let tmp = tempdir().unwrap();
    let binary = tmp.path().join("terminated-cli");
    fs::write(&binary, "#!/bin/sh\nkill -TERM $$\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new("node")
        .arg(wrapper_bin())
        .arg("--version")
        .env("I18NEXT_TURBO_BINARY", &binary)
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "signal termination must fail");
    assert!(String::from_utf8_lossy(&output.stderr).contains("SIGTERM"));
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

#[cfg(unix)]
#[test]
fn node_wrapper_drains_captured_child_output() {
    use std::os::unix::fs::PermissionsExt;

    if !should_run_node_wrapper_tests() {
        return;
    }
    let tmp = tempdir().unwrap();
    let binary = tmp.path().join("verbose-cli");
    fs::write(
        &binary,
        "#!/usr/bin/env node\nprocess.stdout.write('x'.repeat(262144));\nprocess.stderr.write('y'.repeat(262144));\n",
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new("node")
        .arg(wrapper_bin())
        .arg("--version")
        .env("I18NEXT_TURBO_BINARY", &binary)
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, vec![b'x'; 262144]);
    assert_eq!(output.stderr, vec![b'y'; 262144]);
}

#[cfg(unix)]
fn assert_exit_with_active_plugin(exit_code: i32, plugin_error: bool) {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::thread;
    use std::time::{Duration, Instant};

    if !should_run_node_wrapper_tests() {
        return;
    }
    let tmp = tempdir().unwrap();
    let binary = tmp.path().join("child.cjs");
    fs::write(
        &binary,
        format!("#!/usr/bin/env node\nprocess.stdout.write('x'.repeat(262144));\nprocess.stderr.write('y'.repeat(262144));\nprocess.exitCode = {exit_code};\n"),
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        tmp.path().join("plugin.cjs"),
        format!(r#"module.exports = {{
  setup() {{ setInterval(() => {{}}, 50); }},
  afterSync() {{
    require('fs').writeFileSync('.after-sync', 'done');
    if ({plugin_error}) throw new Error('DIAGNOSTIC-BEGIN:' + 'z'.repeat(262144) + ':DIAGNOSTIC-END');
  }}
}};
"#),
    )
    .unwrap();
    fs::write(
        tmp.path().join("i18next.config.cjs"),
        "module.exports = {locales:['en'],pluginErrorMode:'fail-fast',plugins:[{resolve:'./plugin.cjs'}],extract:{input:'src/**/*.ts',output:'locales/{{language}}/{{namespace}}.json'}};",
    )
    .unwrap();
    let mut child = Command::new("node")
        .arg(wrapper_bin())
        .args(["--config", "i18next.config.cjs", "extract"])
        .env("I18NEXT_TURBO_BINARY", &binary)
        .current_dir(tmp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    // Read both pipes while waiting: large output must not turn the timeout into pipe deadlock.
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break (status, false);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            break (child.wait().unwrap(), true);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = stdout_reader.join().unwrap();
    let stderr = stderr_reader.join().unwrap();
    assert!(
        !timed_out,
        "CLI must exit even when a plugin leaves an interval active"
    );
    assert_eq!(
        status.code(),
        Some(if plugin_error { 1 } else { exit_code })
    );
    assert_eq!(stdout.len(), 262144);
    assert!(stdout.iter().all(|byte| *byte == b'x'));
    assert!(stderr.starts_with(&vec![b'y'; 262144]));
    assert_eq!(tmp.path().join(".after-sync").exists(), exit_code == 0);
    if plugin_error {
        let diagnostic = format!("DIAGNOSTIC-BEGIN:{}:DIAGNOSTIC-END\n", "z".repeat(262144));
        assert!(stderr.ends_with(diagnostic.as_bytes()));
        assert!(String::from_utf8_lossy(&stderr).contains("plugin hook \"afterSync\" failed"));
    } else {
        assert_eq!(stderr.len(), 262144);
    }
}

#[cfg(unix)]
#[test]
fn node_wrapper_exits_with_active_plugin_after_success() {
    assert_exit_with_active_plugin(0, false);
}

#[cfg(unix)]
#[test]
fn node_wrapper_exits_with_active_plugin_after_child_failure() {
    assert_exit_with_active_plugin(7, false);
}

#[cfg(unix)]
#[test]
fn node_wrapper_exits_with_active_plugin_after_hook_failure() {
    assert_exit_with_active_plugin(0, true);
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

#[cfg(unix)]
#[test]
fn node_wrapper_rejects_plugin_watch_before_plugins_or_translation_writes() {
    use std::os::unix::fs::PermissionsExt;

    if !should_run_node_wrapper_tests() {
        return;
    }

    for (filename, args, nested, error_mode) in [
        (
            "i18next.config.cjs",
            vec!["--config", "i18next.config.cjs", "watch"],
            false,
            "continue",
        ),
        (
            "i18next.config.cjs",
            vec!["watch", "--config=i18next.config.cjs"],
            true,
            "fail-fast",
        ),
        (
            "i18next-turbo.json",
            vec!["-c", "i18next-turbo.json", "watch"],
            false,
            "continue",
        ),
        (
            "i18next-turbo.json",
            vec!["watch", "--config=i18next-turbo.json"],
            true,
            "fail-fast",
        ),
        ("i18next.config.cjs", vec!["watch"], true, "continue"),
        ("i18next-turbo.json", vec!["watch"], false, "fail-fast"),
        (
            "i18next.config.cjs",
            vec!["--log-level", "warn", "watch", "-c", "i18next.config.cjs"],
            false,
            "continue",
        ),
    ] {
        let tmp = tempdir().unwrap();
        let project = tmp.path();
        fs::create_dir_all(project.join("src")).unwrap();
        fs::write(project.join("src/app.ts"), "t('new.key');").unwrap();
        let translations = [
            (
                "en",
                "{\r\n\t\"retained\": \"Keep\",\r\n\t\"unused\": \"Keep too\"\r\n}\r\n",
            ),
            ("ja", "{\"retained\":\"保持\",\"unused\":\"削除しない\"}\n"),
        ];
        for (locale, content) in translations {
            let dir = project.join("locales").join(locale);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("translation.json"), content).unwrap();
        }
        fs::write(
            project.join("plugin.cjs"),
            "const fs = require('fs');\nfs.writeFileSync('.plugin-loaded', 'loaded');\nmodule.exports = () => {\nfs.writeFileSync('.plugin-factory', 'called');\nreturn {setup() {fs.writeFileSync('.plugin-setup', 'called');}};\n};\n",
        )
        .unwrap();
        let binary = project.join("child.cjs");
        fs::write(
            &binary,
            "#!/usr/bin/env node\nconst fs = require('fs');\nfs.writeFileSync('.rust-started', 'started');\nfs.writeFileSync('locales/en/translation.json', '{}');\n",
        )
        .unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let mut config = serde_json::json!({
            "input": ["src/**/*.ts"], "output": "locales", "locales": ["en", "ja"],
            "functions": ["t"], "removeUnusedKeys": true, "pluginErrorMode": error_mode
        });
        let plugins = serde_json::json!([{ "resolve": "./plugin.cjs" }]);
        if nested {
            config["extract"] = serde_json::json!({ "plugins": plugins });
        } else {
            config["plugins"] = plugins;
        }
        let config = if filename.ends_with(".json") {
            config.to_string()
        } else {
            format!("module.exports = {config};\n")
        };
        fs::write(project.join(filename), config).unwrap();

        let output = Command::new("node")
            .arg(wrapper_bin())
            .args(&args)
            .env("I18NEXT_TURBO_BINARY", &binary)
            .current_dir(project)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(
            stderr.contains("watch does not support plugins"),
            "{args:?}: {stderr}"
        );
        for marker in [
            ".plugin-loaded",
            ".plugin-factory",
            ".plugin-setup",
            ".rust-started",
        ] {
            assert!(!project.join(marker).exists(), "{args:?}: {marker}");
        }
        for (locale, content) in translations {
            assert_eq!(
                fs::read(
                    project
                        .join("locales")
                        .join(locale)
                        .join("translation.json")
                )
                .unwrap(),
                content.as_bytes(),
                "{args:?}: {locale} must remain byte-identical"
            );
        }
        assert_eq!(fs::read_dir(project.join("locales")).unwrap().count(), 2);
    }
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
  if (result.message !== null) {{
    throw new Error('extract message must be null when absent');
  }}
  if (!fs.existsSync(path.join(process.cwd(), 'locales', 'en', 'translation.json'))) {{
    throw new Error('translation.json was not created');
  }}
  const localeFile = path.join(process.cwd(), 'locales', 'en', 'translation.json');
  const catalog = JSON.parse(fs.readFileSync(localeFile, 'utf8'));
  catalog.obsolete = 'KEEP';
  fs.writeFileSync(localeFile, JSON.stringify(catalog));
  const beforeCheck = fs.readFileSync(localeFile, 'utf8');
  const checked = nativeAddon.check({{
    input: ['src/**/*.ts'], output: 'locales', locales: ['en'], functions: ['t']
  }}, {{ remove: true, dry_run: true }});
  if (checked.dead_keys.length !== 1 || checked.dead_keys[0].key_path !== 'obsolete'
      || typeof checked.dead_keys[0].file_path !== 'string'
      || checked.removed_count !== 0 || fs.readFileSync(localeFile, 'utf8') !== beforeCheck) {{
    throw new Error(`native check contract/dry_run failed: ${{JSON.stringify(checked)}}`);
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
      ignoredAttributes: ['placeholder'], acceptedTags: ['input']
    }}
  }});
  if (!result || result.files_checked !== 1 || !Array.isArray(result.issues) || result.issues.length !== 0) {{
    throw new Error(`native lint ignored config: ${{JSON.stringify(result)}}`);
  }}
  const unignored = nativeAddon.lint({{
    input: ['src/**/*.tsx'], output: 'locales', locales: ['en'], functions: ['t'],
    lint: {{ ignoredAttributes: [], acceptedTags: ['input'] }}
  }});
  if (unignored.issues.length !== 1 || typeof unignored.issues[0].file_path !== 'string') {{
    throw new Error(`native lint issue contract failed: ${{JSON.stringify(unignored)}}`);
  }}
  let rejected = false;
  try {{
    nativeAddon.lint({{
      input: ['src/**/*.tsx'], output: 'locales', locales: ['en'], functions: ['t'],
      lint: {{ ignoredAttributes: [], acceptedTags: ['input'] }}
    }}, {{ fail_on_error: true }});
  }} catch (error) {{ rejected = /Lint failed/.test(String(error.message)); }}
  if (!rejected) {{ throw new Error('native lint ignored fail_on_error'); }}
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
