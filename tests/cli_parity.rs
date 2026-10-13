use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

fn flatten(value: &Value, prefix: &str, entries: &mut BTreeMap<String, Value>) {
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten(value, &path, entries);
        }
    } else {
        entries.insert(prefix.to_string(), value.clone());
    }
}

fn write_json(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn cli_1_75_2_fixture_catalog() {
    // Golden output was captured from the pinned official CLI, not computed by Turbo.
    let project = tempfile::tempdir().unwrap();
    let sources: Map<String, Value> =
        serde_json::from_str(include_str!("fixtures/cli-parity/sources.json")).unwrap();
    for (relative, source) in sources {
        let path = project.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source.as_str().unwrap()).unwrap();
    }
    let seed: Value = serde_json::from_str(include_str!("fixtures/cli-parity/seed.json")).unwrap();
    for (locale, namespaces) in seed.as_object().unwrap() {
        for (namespace, value) in namespaces.as_object().unwrap() {
            write_json(
                &project
                    .path()
                    .join(format!("locales/{locale}/{namespace}.json")),
                value,
            );
        }
    }
    fs::write(
        project.path().join("i18next-turbo.json"),
        include_str!("fixtures/cli-parity/config.json"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_i18next-turbo"))
        .current_dir(project.path())
        .args(["--config", "i18next-turbo.json", "extract"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut actual = BTreeMap::new();
    for locale in ["en", "ja", "ru"] {
        for file in fs::read_dir(project.path().join("locales").join(locale)).unwrap() {
            let path = file.unwrap().path();
            let namespace = path.file_stem().unwrap().to_str().unwrap();
            let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let mut entries = BTreeMap::new();
            flatten(&value, "", &mut entries);
            for (key, value) in entries {
                actual.insert(format!("{locale}/{namespace}:{key}"), value);
            }
        }
    }
    let expected: BTreeMap<String, Value> =
        serde_json::from_str(include_str!("fixtures/cli-parity/expected.json")).unwrap();
    let missing: Vec<_> = expected
        .keys()
        .filter(|key| !actual.contains_key(*key))
        .collect();
    let extra: Vec<_> = actual
        .keys()
        .filter(|key| !expected.contains_key(*key))
        .collect();
    let changed: Vec<_> = expected
        .iter()
        .filter(|(key, value)| actual.get(*key).is_some_and(|actual| actual != *value))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty() && changed.is_empty(),
        "missing: {missing:?}\nextra: {extra:?}\nchanged: {changed:?}"
    );
}

fn small_project(source: &str) -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(project.path().join("src/App.ts"), source).unwrap();
    write_json(
        &project.path().join("i18next-turbo.json"),
        &serde_json::json!({
            "input": ["src/**/*.ts"], "ignore": ["src/ignored/**"], "output": "locales",
            "locales": ["en", "ja", "ru"], "functions": ["t"], "defaultNamespace": "common",
            "extractFromComments": false, "removeUnusedKeys": false, "defaultValue": ""
        }),
    );
    write_json(
        &project.path().join("locales/en/common.json"),
        &serde_json::json!({"keep": "TRANSLATED"}),
    );
    project
}

fn rejects_without_writes(project: &Path, strict: bool) {
    let locale = project.join("locales/en/common.json");
    let before = fs::read(&locale).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_i18next-turbo"));
    command
        .current_dir(project)
        .args(["--config", "i18next-turbo.json", "extract"]);
    if strict {
        command.arg("--fail-on-warnings");
    }
    let output = command.output().unwrap();
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(fs::read(locale).unwrap(), before);
    assert!(!project.join("locales/ja").exists());
}

#[test]
fn input_boundary_cycles_and_expansion_limits_are_rejected_before_writes() {
    let project = small_project("import {Status} from '../outside/types'; declare const state:Status; t('safe'); t(`state.${state}`);");
    fs::create_dir(project.path().join("outside")).unwrap();
    // Reading this input-excluded import would cause a UTF-8 error; resolution must not read it.
    fs::write(project.path().join("outside/types.ts"), [0xff]).unwrap();
    rejects_without_writes(project.path(), true);

    let project = small_project(
        "type State=State; declare const state:State; t('safe'); t(`state.${state}`);",
    );
    rejects_without_writes(project.path(), true);

    let union = (0..17)
        .map(|n| format!("'v{n}'"))
        .collect::<Vec<_>>()
        .join("|");
    let project = small_project(&format!(
        "declare const a:{union}; declare const b:{union}; t('safe'); t(`state.${{a}}.${{b}}`);"
    ));
    rejects_without_writes(project.path(), true);
}

#[test]
fn type_and_value_namespaces_and_literal_plural_suffixes_remain_distinct() {
    let project = small_project("type State='open'|'closed'; const State='runtime'; declare const state:State; t(`state.${state}`); t(State); t('literal_few');");
    let output = Command::new(env!("CARGO_BIN_EXE_i18next-turbo"))
        .current_dir(project.path())
        .args([
            "--config",
            "i18next-turbo.json",
            "extract",
            "--fail-on-warnings",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for locale in ["en", "ja", "ru"] {
        let value: Value = serde_json::from_slice(
            &fs::read(project.path().join(format!("locales/{locale}/common.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(value["state"]["open"], "");
        assert_eq!(value["state"]["closed"], "");
        assert_eq!(value["runtime"], "");
        assert_eq!(value["literal_few"], "");
    }
}

#[test]
fn mutable_untyped_values_and_array_interpolation_are_unknown() {
    for source in [
        "let state='open'; state='closed'; t('safe'); t(`state.${state}`);",
        "const states=['open','closed'] as const; t('safe'); t(`state.${states}`);",
    ] {
        rejects_without_writes(small_project(source).path(), true);
    }
}

#[test]
fn mutable_object_and_array_initializers_are_unknown_before_pruning() {
    for source in [
        "const labels={save:'old'}; labels.save='new'; t(labels.save);",
        "const states=['old']; states.push('new'); for(const state of states){t(`state.${state}`)}",
        "const labels={save:'old'} as any; labels.save='new'; t(labels.save);",
    ] {
        let project = small_project(source);
        let config_path = project.path().join("i18next-turbo.json");
        let mut config: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        config["removeUnusedKeys"] = Value::Bool(true);
        write_json(&config_path, &config);
        rejects_without_writes(project.path(), false);
    }
    let project = small_project("import {labels} from './labels'; t(labels.save);");
    fs::write(
        project.path().join("src/labels.ts"),
        "export const labels={save:'old'}; labels.save='new';",
    )
    .unwrap();
    rejects_without_writes(project.path(), true);
}

#[test]
fn primary_language_outside_locales_keeps_its_plural_forms() {
    let project = small_project("t('items',{count:1}); t('position',{count:1,ordinal:true});");
    let config_path = project.path().join("i18next-turbo.json");
    let mut config: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    config["locales"] = serde_json::json!(["ja"]);
    config["primaryLanguage"] = Value::String("en".into());
    write_json(&config_path, &config);
    let output = Command::new(env!("CARGO_BIN_EXE_i18next-turbo"))
        .current_dir(project.path())
        .args([
            "--config",
            "i18next-turbo.json",
            "extract",
            "--sync-primary",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value =
        serde_json::from_slice(&fs::read(project.path().join("locales/en/common.json")).unwrap())
            .unwrap();
    for key in [
        "items_one",
        "items_other",
        "position_ordinal_one",
        "position_ordinal_two",
        "position_ordinal_few",
        "position_ordinal_other",
    ] {
        assert_eq!(value[key], "", "missing {key}");
    }
    assert!(!project.path().join("locales/ja").exists());
}

#[test]
fn namespace_traversal_is_rejected_before_any_catalog_write() {
    let project = small_project("const key='../../outside:danger'; t('safe'); t(key);");
    rejects_without_writes(project.path(), false);
    assert!(!project.path().join("outside.json").exists());
}

#[test]
fn aliases_do_not_escape_blocks_or_shadow_function_parameters() {
    let source = r#"
      function Parent(){const {t:translate}=useTranslation('account',{keyPrefix:'profile'});
        function other(translate){translate('not_a_key')}
        const arrow=(translate)=>translate('not_a_key');
        translate('name');
      }
      translate('outside');
    "#;
    let keys =
        i18next_turbo::extractor::extract_from_source(source, "app.ts", &["t".into()]).unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].key, "profile.name");
    assert_eq!(keys[0].namespace.as_deref(), Some("account"));
}
