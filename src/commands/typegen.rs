use anyhow::Result;
use std::path::Path;

use crate::config::Config;
use crate::typegen;

pub fn run(
    config: &Config,
    output: &str,
    default_locale: Option<String>,
    locales_dir: Option<String>,
) -> Result<()> {
    println!("=== i18next-turbo typegen ===\n");

    let locales_dir_path = locales_dir.as_ref().unwrap_or(&config.output);
    let default_locale = default_locale
        .as_ref()
        .or(config.locales.first())
        .map(|s| s.as_str())
        .unwrap_or("en");

    println!("Configuration:");
    println!("  Locales directory: {}", locales_dir_path);
    println!("  Default locale: {}", default_locale);
    println!("  Output: {}", output);
    println!();

    let locales_path = Path::new(locales_dir_path);
    let output_path = Path::new(output);

    let indentation = config.types_indentation_string();
    let input_patterns = config.types_input_patterns();
    let resources_file = config.types_resources_file();
    let enable_selector = config.types_enable_selector();
    typegen::generate_types_with_options(
        locales_path,
        output_path,
        default_locale,
        indentation.as_deref(),
        input_patterns.as_deref(),
        resources_file.as_deref().map(Path::new),
        enable_selector.as_ref(),
        config.merge_namespaces,
    )?;

    println!("TypeScript types generated successfully!");
    println!("  Output: {}", output);

    Ok(())
}
