use anyhow::{bail, Result};
use std::path::Path;

pub fn run(
    force: bool,
    input: &str,
    output: &str,
    locales: &str,
    namespace: &str,
    functions: &str,
) -> Result<()> {
    println!("=== i18next-turbo init ===\n");

    let config_path = Path::new("i18next-turbo.json");

    // Check if config already exists
    if config_path.exists() && !force {
        bail!(
            "Configuration file already exists: {}. Use --force to overwrite.",
            config_path.display()
        );
    }

    // Parse comma-separated values
    let input_patterns: Vec<String> = input.split(',').map(|s| s.trim().to_string()).collect();
    let locales_vec: Vec<String> = locales.split(',').map(|s| s.trim().to_string()).collect();
    let functions_vec: Vec<String> = functions.split(',').map(|s| s.trim().to_string()).collect();

    // Create config JSON
    let config = serde_json::json!({
        "input": input_patterns,
        "output": output,
        "locales": locales_vec,
        "defaultNamespace": namespace,
        "functions": functions_vec,
        "keySeparator": ".",
        "nsSeparator": ":"
    });

    // Write config file
    let config_str = serde_json::to_string_pretty(&config)?;
    std::fs::write(config_path, format!("{}\n", config_str))?;

    println!("Created configuration file: {}\n", config_path.display());
    println!("Configuration:");
    println!("  Input patterns: {:?}", input_patterns);
    println!("  Output: {}", output);
    println!("  Locales: {:?}", locales_vec);
    println!("  Default namespace: {}", namespace);
    println!("  Functions: {:?}", functions_vec);

    println!("\nNext steps:");
    println!("  1. Run 'i18next-turbo extract' to extract translation keys");
    println!("  2. Run 'i18next-turbo watch' for continuous extraction");
    println!("  3. Run 'i18next-turbo typegen' to generate TypeScript types");

    // Create output directories for each locale
    println!("\nCreating locale directories...");
    for locale in &locales_vec {
        let locale_dir = Path::new(output).join(locale);
        if !locale_dir.exists() {
            std::fs::create_dir_all(&locale_dir)?;
            println!("  Created: {}", locale_dir.display());
        }
    }

    println!("\nDone!");
    Ok(())
}
