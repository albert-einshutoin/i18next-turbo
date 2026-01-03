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

// Load configuration file if it exists
let configJson = null;
const args = process.argv.slice(2);

// Check if --config is already specified
const configIndex = args.findIndex(arg => arg === '--config' || arg === '-c');
if (configIndex === -1) {
  // Try to find and load config file
  const configPath = findConfigFile();
  if (configPath) {
    try {
      const config = loadConfigFile(configPath);
      if (config) {
        configJson = JSON.stringify(config);
      }
    } catch (error) {
      console.warn(`Warning: Failed to load config file ${configPath}: ${error.message}`);
      // Continue without config - Rust binary will use defaults
    }
  }
}

// Build arguments for Rust binary
const rustArgs = [];
if (configJson) {
  rustArgs.push('--config-json', configJson);
}
// Add all other arguments (including --config if specified)
rustArgs.push(...args);

// Spawn the Rust binary
const child = spawn(binaryPath, rustArgs, {
  stdio: 'inherit',
  cwd: process.cwd()
});

child.on('error', (error) => {
  console.error(`Error: Failed to start i18next-turbo: ${error.message}`);
  process.exit(1);
});

child.on('exit', (code) => {
  process.exit(code || 0);
});

/**
 * Find configuration file in current directory
 * Priority: i18next-turbo.json > i18next-parser.config.js > i18next.config.ts > i18next.config.js
 */
function findConfigFile() {
  const cwd = process.cwd();
  const configFiles = [
    'i18next-turbo.json',
    'i18next-parser.config.js',
    'i18next.config.ts',
    'i18next.config.js'
  ];

  for (const file of configFiles) {
    const filePath = path.join(cwd, file);
    if (fs.existsSync(filePath)) {
      return filePath;
    }
  }

  return null;
}

/**
 * Load configuration file (supports JS/TS files)
 */
function loadConfigFile(configPath) {
  const ext = path.extname(configPath);
  
  if (ext === '.json') {
    // JSON file - read and parse directly
    const content = fs.readFileSync(configPath, 'utf-8');
    return JSON.parse(content);
  } else if (ext === '.js' || ext === '.ts') {
    // JS/TS file - use require() or jiti
    try {
      // Try to use jiti for TypeScript support (if available)
      let jiti;
      try {
        jiti = require('jiti')(process.cwd(), {
          esmResolve: true,
          interopDefault: true
        });
      } catch (e) {
        // jiti not available, fall back to require() for .js files
        if (ext === '.ts') {
          throw new Error('TypeScript config files require "jiti" package. Install it with: npm install --save-dev jiti');
        }
        // For .js files, use require()
        delete require.cache[require.resolve(configPath)];
        const config = require(configPath);
        // Handle both default export and module.exports
        return config.default || config;
      }
      
      // Use jiti to load the config file
      const config = jiti(configPath);
      // Handle both default export and module.exports
      return config.default || config;
    } catch (error) {
      throw new Error(`Failed to load config file: ${error.message}`);
    }
  }
  
  return null;
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
