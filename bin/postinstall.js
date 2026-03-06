#!/usr/bin/env node

/**
 * Post-install script for i18next-turbo
 *
 * This script is run after npm install. It checks whether a local development
 * build exists and prints actionable fallback guidance when it does not.
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
console.log('⚠ i18next-turbo local release binary not found');
console.log('');
console.log('The package normally resolves a platform binary via optionalDependencies.');
console.log('If you are developing from source, build the CLI binary with:');
console.log('  cargo build --release');
console.log('');
console.log('If you need the native Node addon as well, build:');
console.log('  cargo build --release --features napi');
