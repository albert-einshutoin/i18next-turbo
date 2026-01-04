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

    typegen::generate_types(locales_path, output_path, default_locale)?;

    println!("TypeScript types generated successfully!");
    println!("  Output: {}", output);

    Ok(())
}
