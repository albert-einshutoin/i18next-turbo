#!/usr/bin/env node

/**
 * i18next-turbo CLI wrapper
 * 
 * This script detects the user's OS and architecture, then calls the appropriate
 * Rust binary. It also handles loading JS/TS configuration files and converting
 * them to JSON for the Rust binary.
 */

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');
const { cosmiconfig, defaultLoaders } = require('cosmiconfig');
const { pathToFileURL } = require('url');

const JS_TS_CONFIG_EXTENSIONS = new Set(['.js', '.cjs', '.mjs', '.ts']);

// Detect platform and architecture
const platform = os.platform();
const arch = os.arch();

// Map Node.js platform names to Rust target triplets
const platformMap = {
  'darwin': 'darwin',
  'linux': 'linux',
  'win32': 'win32'
};

// Map Node.js architecture names to Rust architecture names
const archMap = {
  'x64': 'x64',
  'arm64': 'arm64',
  'ia32': 'ia32'
};

// Get the Rust target name
const rustPlatform = platformMap[platform];
const rustArch = archMap[arch];

if (!rustPlatform || !rustArch) {
  console.error(`Error: Unsupported platform/architecture: ${platform}/${arch}`);
  console.error('Supported platforms: darwin, linux, win32');
  console.error('Supported architectures: x64, arm64, ia32');
  process.exit(1);
}

// Determine binary name and path
const binaryName = platform === 'win32' ? 'i18next-turbo.exe' : 'i18next-turbo';
const binaryPath = resolveBinaryPath(platform, arch, binaryName);

// Check if binary exists
if (!binaryPath || !fs.existsSync(binaryPath)) {
  console.error('Error: Suitable binary not found for this platform.');
  if (binaryPath) {
    console.error(`Checked: ${binaryPath}`);
  }
  console.error('Please install the optional binary package for your platform,');
  console.error('or build the project first: cargo build --release');
  process.exit(1);
}

async function main() {
  const rawArgs = process.argv.slice(2);
  const { args, inlineConfigJson, configProvided, configPathHint: inlineConfigPath } = await resolveConfigArgs(rawArgs);

  let configJson = inlineConfigJson;
  let configPathHint = inlineConfigPath;

  if (!configJson && !configProvided) {
    try {
      const result = await loadConfigFromDisk();
      if (result) {
        const normalized = normalizeConfig(result.config);
        if (normalized) {
          configJson = JSON.stringify(normalized);
          configPathHint = result.filepath;
        }
      }
    } catch (error) {
      console.warn(`Warning: Failed to load config file: ${error.message}`);
    }
  }

  const rustArgs = [];
  if (configJson) {
    rustArgs.push('--config-stdin');
    if (configPathHint) {
      rustArgs.push('--config-path-hint', configPathHint);
    }
  }
  rustArgs.push(...args);

  // Spawn the Rust binary
  const child = spawn(binaryPath, rustArgs, {
    stdio: configJson ? ['pipe', 'inherit', 'inherit'] : 'inherit',
    cwd: process.cwd()
  });

  // If we have config, write it to stdin
  if (configJson) {
    child.stdin.write(configJson);
    child.stdin.end();
  }

  child.on('error', (error) => {
    console.error(`Error: Failed to start i18next-turbo: ${error.message}`);
    process.exit(1);
  });

  child.on('exit', (code) => {
    process.exit(code || 0);
  });
}

main().catch((error) => {
  console.error(`Error: ${error.message}`);
  process.exit(1);
});

async function resolveConfigArgs(argv) {
  const processedArgs = [];
  let inlineConfigJson = null;
  let configProvided = false;
  let configPathHint = null;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];

    if (arg === '--config' || arg === '-c') {
      configProvided = true;
      const value = argv[i + 1];
      if (!value) {
        processedArgs.push(arg);
        continue;
      }

      if (shouldLoadWithNode(value)) {
        const result = await loadConfigFromFile(value);
        if (result) {
          const normalized = normalizeConfig(result.config);
          if (normalized) {
            inlineConfigJson = JSON.stringify(normalized);
            configPathHint = result.filepath;
          }
        }
        i += 1;
        continue;
      }

      processedArgs.push(arg, value);
      i += 1;
      continue;
    }

    if (arg.startsWith('--config=')) {
      configProvided = true;
      const value = arg.split('=')[1];
      if (shouldLoadWithNode(value)) {
        const result = await loadConfigFromFile(value);
        if (result) {
          const normalized = normalizeConfig(result.config);
          if (normalized) {
            inlineConfigJson = JSON.stringify(normalized);
            configPathHint = result.filepath;
          }
        }
        continue;
      }
    }

    processedArgs.push(arg);
  }

  return { args: processedArgs, inlineConfigJson, configProvided, configPathHint };
}

/**
 * Find and load configuration file in current directory
 * Priority: i18next-turbo.json > i18next-parser.config.(js|json) > i18next.config.(ts|js)
 */
async function loadConfigFromDisk() {
  const explorer = createExplorer();
  const result = await explorer.search();
  return formatExplorerResult(result);
}

async function loadConfigFromFile(filepath) {
  const explorer = createExplorer();
  const absolute = path.resolve(process.cwd(), filepath);
  const result = await explorer.load(absolute);
  return formatExplorerResult(result);
}

function createExplorer() {
  return cosmiconfig('i18next-turbo', {
    searchPlaces: [
      'i18next-turbo.json',
      'i18next-parser.config.json',
      'i18next-parser.config.js',
      'i18next-parser.config.cjs',
      'i18next-parser.config.mjs',
      'i18next-parser.config.ts',
      'i18next.config.ts',
      'i18next.config.js',
      'i18next.config.cjs',
      'i18next.config.mjs'
    ],
    loaders: {
      '.js': defaultLoaders['.js'],
      '.json': defaultLoaders['.json'],
      '.cjs': loadCommonJsConfig,
      '.mjs': loadEsmConfig,
      '.ts': loadTypeScriptConfig
    }
  });
}

function formatExplorerResult(result) {
  if (!result) {
    return null;
  }
  const filepath = result.filepath ? path.resolve(result.filepath) : null;
  const config = result.config && result.config.default ? result.config.default : result.config;
  return { config, filepath };
}

function shouldLoadWithNode(filepath) {
  const ext = path.extname(filepath).toLowerCase();
  return JS_TS_CONFIG_EXTENSIONS.has(ext);
}

function loadTypeScriptConfig(filepath) {
  let jiti;
  try {
    jiti = require('jiti')(process.cwd(), {
      esmResolve: true,
      interopDefault: true
    });
  } catch (error) {
    throw new Error(
      'TypeScript config files require "jiti" package. Install it with: npm install --save-dev jiti'
    );
  }

  const config = jiti(filepath);
  return config && config.default ? config.default : config;
}

function loadCommonJsConfig(filepath) {
  delete require.cache[require.resolve(filepath)];
  const config = require(filepath);
  return config && config.default ? config.default : config;
}

async function loadEsmConfig(filepath) {
  const configUrl = pathToFileURL(filepath).href;
  const configModule = await import(`${configUrl}?t=${Date.now()}`);
  return configModule && configModule.default ? configModule.default : configModule;
}

function normalizeConfig(rawConfig) {
  if (!rawConfig || typeof rawConfig !== 'object') {
    return null;
  }

  const mapped = mapCliExtractConfig(rawConfig);
  const normalized = {
    ...mapped,
    ...rawConfig
  };

  delete normalized.extract;
  return normalized;
}

function mapCliExtractConfig(rawConfig) {
  const extract = rawConfig.extract;
  if (!extract || typeof extract !== 'object') {
    return {};
  }

  const mapped = {};

  if (Array.isArray(rawConfig.locales)) {
    mapped.locales = rawConfig.locales;
  }

  if (typeof extract.input === 'string') {
    mapped.input = [extract.input];
  } else if (Array.isArray(extract.input)) {
    mapped.input = extract.input;
  }

  if (typeof extract.output === 'string') {
    const outputDir = coerceOutputDir(extract.output);
    if (outputDir) {
      mapped.output = outputDir;
    }
  } else if (typeof extract.output === 'function') {
    console.warn('Warning: extract.output function is not supported by i18next-turbo.');
  }

  if (Array.isArray(extract.functions)) {
    mapped.functions = extract.functions;
  }

  if (Array.isArray(extract.useTranslationNames)) {
    mapped.useTranslationNames = extract.useTranslationNames;
  }

  if (typeof extract.defaultNS === 'string') {
    mapped.defaultNamespace = extract.defaultNS;
  } else if (extract.defaultNS === false) {
    console.warn('Warning: extract.defaultNS=false is not supported by i18next-turbo.');
  }

  if (typeof extract.keySeparator === 'string') {
    mapped.keySeparator = extract.keySeparator;
  } else if (extract.keySeparator === false || extract.keySeparator === null) {
    mapped.keySeparator = '';
  }

  if (typeof extract.nsSeparator === 'string') {
    mapped.nsSeparator = extract.nsSeparator;
  } else if (extract.nsSeparator === false || extract.nsSeparator === null) {
    mapped.nsSeparator = '';
  }

  if (typeof extract.contextSeparator === 'string') {
    mapped.contextSeparator = extract.contextSeparator;
  }

  if (typeof extract.pluralSeparator === 'string') {
    mapped.pluralSeparator = extract.pluralSeparator;
  }

  if (Array.isArray(extract.transComponents)) {
    mapped.transComponents = extract.transComponents;
  }

  if (Array.isArray(extract.ignore)) {
    mapped.ignore = extract.ignore;
  }

  if (Array.isArray(extract.preservePatterns)) {
    mapped.preservePatterns = extract.preservePatterns;
  }

  if (typeof extract.removeUnusedKeys === 'boolean') {
    mapped.removeUnusedKeys = extract.removeUnusedKeys;
  }

  if (typeof extract.defaultValue === 'string') {
    mapped.defaultValue = extract.defaultValue;
  }

  if (typeof extract.generateBasePluralForms === 'boolean') {
    mapped.generateBasePluralForms = extract.generateBasePluralForms;
  }

  if (typeof extract.disablePlurals === 'boolean') {
    mapped.disablePlurals = extract.disablePlurals;
  }

  if (typeof extract.nestingPrefix === 'string') {
    mapped.nestingPrefix = extract.nestingPrefix;
  }

  if (typeof extract.nestingSuffix === 'string') {
    mapped.nestingSuffix = extract.nestingSuffix;
  }

  if (typeof extract.nestingOptionsSeparator === 'string') {
    mapped.nestingOptionsSeparator = extract.nestingOptionsSeparator;
  }

  return mapped;
}

function coerceOutputDir(output) {
  if (!output) {
    return null;
  }

  const templateIndex = output.indexOf('{{');
  if (templateIndex !== -1) {
    const base = output.slice(0, templateIndex).replace(/[\\/]+$/, '');
    if (base) {
      return base;
    }
    console.warn('Warning: extract.output template could not be mapped to an output directory.');
    return null;
  }

  const ext = path.extname(output);
  if (ext) {
    return path.dirname(output);
  }

  return output;
}

/**
 * Resolve the prebuilt binary path from optionalDependencies.
 * Falls back to local target/release for dev environments.
 */
function resolveBinaryPath(platformName, archName, binName) {
  const pkgName = getBinaryPackageName(platformName, archName);
  if (pkgName) {
    try {
      const pkgJsonPath = require.resolve(`${pkgName}/package.json`);
      return path.join(path.dirname(pkgJsonPath), binName);
    } catch (error) {
      // Optional package not installed; fall back to local build.
    }
  }

  return path.join(__dirname, '..', 'target', 'release', binName);
}

function getBinaryPackageName(platformName, archName) {
  if (platformName === 'darwin' && archName === 'x64') return 'i18next-turbo-darwin-x64';
  if (platformName === 'darwin' && archName === 'arm64') return 'i18next-turbo-darwin-arm64';
  if (platformName === 'linux' && archName === 'x64') return 'i18next-turbo-linux-x64';
  if (platformName === 'linux' && archName === 'arm64') return 'i18next-turbo-linux-arm64';
  if (platformName === 'win32' && archName === 'x64') return 'i18next-turbo-win32-x64';
  if (platformName === 'win32' && archName === 'ia32') return 'i18next-turbo-win32-ia32';
  return null;
}
