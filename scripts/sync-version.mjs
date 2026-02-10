#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const cargoTomlPath = path.join(root, 'Cargo.toml');
const packageJsonPath = path.join(root, 'package.json');

const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
if (!versionMatch) {
  console.error('Failed to detect version from Cargo.toml');
  process.exit(1);
}

const version = versionMatch[1];
const pkg = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));

pkg.version = version;
if (pkg.optionalDependencies && typeof pkg.optionalDependencies === 'object') {
  for (const key of Object.keys(pkg.optionalDependencies)) {
    if (key.startsWith('i18next-turbo-')) {
      pkg.optionalDependencies[key] = version;
    }
  }
}

fs.writeFileSync(packageJsonPath, JSON.stringify(pkg, null, 2) + '\n');
console.log(`Synced package.json version to ${version}`);
