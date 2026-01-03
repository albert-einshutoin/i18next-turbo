#!/usr/bin/env node

/**
 * Post-install script for i18next-turbo
 * 
 * This script is run after npm install. It checks if the Rust binary exists,
 * and if not, provides instructions for building it.
 * 
 * In the future, this will download pre-built binaries from GitHub Releases
 * or optionalDependencies.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const platform = os.platform();
const binaryName = platform === 'win32' ? 'i18next-turbo.exe' : 'i18next-turbo';
const binaryPath = path.join(__dirname, '..', 'target', 'release', binaryName);

// Check if binary exists
if (fs.existsSync(binaryPath)) {
  console.log('✓ i18next-turbo binary found');
  process.exit(0);
}

// Binary not found - provide instructions
console.log('⚠ i18next-turbo binary not found');
console.log('');
console.log('To build the binary, run:');
console.log('  cargo build --release');
console.log('');
console.log('Or install from npm package which includes pre-built binaries.');
console.log('(Pre-built binaries will be available in future releases)');

