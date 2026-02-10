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
