/**
 * i18next-turbo Node.js API
 * 
 * This module provides a programmatic API for using i18next-turbo from Node.js.
 * It loads the native NAPI addon and exports the extract and watch functions.
 */

const path = require('path');
const fs = require('fs');

// Try to load the native addon
let nativeAddon;
try {
  // First, try to load from the standard napi-rs output location
  // napi-rs typically outputs to: target/<profile>/i18next-turbo.<platform>.node
  const os = require('os');
  const platform = os.platform();
  const arch = os.arch();
  
  // Map platform/arch to napi-rs naming convention
  const platformMap = {
    'darwin': 'darwin',
    'linux': 'linux',
    'win32': 'win32'
  };
  const archMap = {
    'x64': 'x64',
    'arm64': 'arm64',
    'ia32': 'ia32'
  };
  
  const rustPlatform = platformMap[platform];
  const rustArch = archMap[arch];
  
  if (rustPlatform && rustArch) {
    // Try development build first
    const devPath = path.join(__dirname, '..', 'target', 'debug', `i18next-turbo.${rustPlatform}-${rustArch}.node`);
    // Try release build
    const releasePath = path.join(__dirname, '..', 'target', 'release', `i18next-turbo.${rustPlatform}-${rustArch}.node`);
    // Try npm package location (for installed packages)
    const npmPath = path.join(__dirname, `i18next-turbo.${rustPlatform}-${rustArch}.node`);
    
    let addonPath;
    if (fs.existsSync(devPath)) {
      addonPath = devPath;
    } else if (fs.existsSync(releasePath)) {
      addonPath = releasePath;
    } else if (fs.existsSync(npmPath)) {
      addonPath = npmPath;
    }
    
    if (addonPath) {
      nativeAddon = require(addonPath);
    }
  }
} catch (error) {
  // Native addon not found or failed to load
  console.warn('Warning: Native addon not found. NAPI functions will not be available.');
  console.warn('Please build the project with: cargo build --release');
}

/**
 * Extract translation keys from source files
 * 
 * @param {object} config - Configuration object
 * @param {object} [options] - Optional extraction options
 * @param {string} [options.output] - Output directory (overrides config)
 * @param {boolean} [options.fail_on_warnings] - Fail on warnings
 * @param {boolean} [options.generate_types] - Generate TypeScript types
 * @param {string} [options.types_output] - TypeScript output path
 * @returns {Promise<object>} Extraction results
 */
async function extract(config, options = {}) {
  if (!nativeAddon) {
    throw new Error('Native addon not available. Please build the project with: cargo build --release');
  }
  
  // Call native function
  const resultJson = nativeAddon.extract(config, options);
  
  // Parse and return result
  return JSON.parse(resultJson);
}

/**
 * Lint source files for hardcoded strings
 *
 * @param {object} config - Configuration object
 * @param {object} [options] - Optional lint options
 * @param {boolean} [options.fail_on_error] - Fail on lint errors
 * @returns {Promise<object>} Lint results
 */
async function lint(config, options = {}) {
  if (!nativeAddon) {
    throw new Error('Native addon not available. Please build the project with: cargo build --release');
  }

  const resultJson = nativeAddon.lint(config, options);
  return JSON.parse(resultJson);
}

/**
 * Check for dead (unused) translation keys
 *
 * @param {object} config - Configuration object
 * @param {object} [options] - Optional check options
 * @param {boolean} [options.remove] - Remove dead keys
 * @param {boolean} [options.dry_run] - Preview changes without applying
 * @param {string} [options.locale] - Locale to check
 * @returns {Promise<object>} Check results
 */
async function check(config, options = {}) {
  if (!nativeAddon) {
    throw new Error('Native addon not available. Please build the project with: cargo build --release');
  }

  const resultJson = nativeAddon.check(config, options);
  return JSON.parse(resultJson);
}

/**
 * Watch for file changes and extract keys automatically
 * 
 * @param {object} config - Configuration object
 * @param {object} [options] - Optional watch options
 * @param {string} [options.output] - Output directory (overrides config)
 * @returns {Promise<void>}
 * 
 * @note This function runs indefinitely until interrupted.
 * In a Node.js context, consider running this in a separate thread or worker.
 */
async function watch(config, options = {}) {
  if (!nativeAddon) {
    throw new Error('Native addon not available. Please build the project with: cargo build --release');
  }
  
  // Call native function (this blocks)
  nativeAddon.watch(config, options);
}

module.exports = {
  extract,
  lint,
  check,
  watch
};
