use i18next_turbo::config::Config;
use i18next_turbo::extractor::{extract_from_source_with_options, ExtractedKey};
use std::path::Path;

fn has_key(keys: &[ExtractedKey], key: &str, ns: Option<&str>) -> bool {
    keys.iter()
        .any(|k| k.key == key && k.namespace.as_deref() == ns)
}

#[test]
fn pattern_t_function_call() {
    let functions = vec!["t".to_string()];
    let cfg = Config::default();
    let keys = extract_from_source_with_options(
        "t('home.title')",
        Path::new("a.ts"),
        &functions,
        true,
        &cfg.plural_config(),
    )
    .unwrap();
    assert!(has_key(&keys, "home.title", None));
}

#[test]
fn pattern_i18n_t_member_call() {
    let functions = vec!["i18n.t".to_string()];
    let cfg = Config::default();
    let keys = extract_from_source_with_options(
        "i18n.t('common:save')",
        Path::new("a.ts"),
        &functions,
        true,
        &cfg.plural_config(),
    )
    .unwrap();
    assert!(has_key(&keys, "save", Some("common")));
}

#[test]
fn pattern_trans_component() {
    let functions = vec!["t".to_string()];
    let cfg = Config::default();
    let keys = extract_from_source_with_options(
        "const v = <Trans i18nKey=\"checkout.total\">x</Trans>;",
        Path::new("a.tsx"),
        &functions,
        true,
        &cfg.plural_config(),
    )
    .unwrap();
    assert!(has_key(&keys, "checkout.total", None));
}

#[test]
fn pattern_comment_extraction() {
    let functions = vec!["t".to_string()];
    let cfg = Config::default();
    let keys = extract_from_source_with_options(
        "// t('comment.key')\nconst x = 1;",
        Path::new("a.ts"),
        &functions,
        true,
        &cfg.plural_config(),
    )
    .unwrap();
    assert!(has_key(&keys, "comment.key", None));
}

#[test]
fn pattern_template_literal_static() {
    let functions = vec!["t".to_string()];
    let cfg = Config::default();
    let keys = extract_from_source_with_options(
        "t(`static.key`)",
        Path::new("a.ts"),
        &functions,
        true,
        &cfg.plural_config(),
    )
    .unwrap();
    assert!(has_key(&keys, "static.key", None));
}

#[test]
fn pattern_use_translation_scope_key_prefix() {
    let functions = vec!["t".to_string()];
    let cfg = Config::default();
    let source = "const { t } = useTranslation('common', { keyPrefix: 'user' }); t('name');";
    let keys = extract_from_source_with_options(
        source,
        Path::new("a.ts"),
        &functions,
        true,
        &cfg.plural_config(),
    )
    .unwrap();
    assert!(has_key(&keys, "user.name", Some("common")));
}
