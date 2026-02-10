#!/usr/bin/env node
/**
 * Generates benchmark fixtures: one source file with N t('translate_targetK') calls
 * and a matching locales/en/translation.json with translated_targetK values.
 *
 * Usage: node scripts/generate-bench-fixtures.mjs [count]
 *   count  default 5000
 *
 * Output:
 *   benchmarks/fixtures/large/src/app.ts
 *   benchmarks/fixtures/large/locales/en/translation.json
 *   benchmarks/fixtures/large/i18next-turbo.json
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const count = Math.max(1, parseInt(process.argv[2] || '5000', 10));

const benchDir = path.join(root, 'benchmarks', 'fixtures', 'large');
const srcDir = path.join(benchDir, 'src');
const localesDir = path.join(benchDir, 'locales', 'en');

for (const dir of [srcDir, path.join(localesDir)]) {
  fs.mkdirSync(dir, { recursive: true });
}

// Single source file with t('translate_target1'), t('translate_target2'), ...
const lines = ["import { t } from 'i18next';", ''];
for (let i = 1; i <= count; i++) {
  lines.push(`t('translate_target${i}');`);
}
const srcPath = path.join(srcDir, 'app.ts');
fs.writeFileSync(srcPath, lines.join('\n') + '\n', 'utf8');

// JSON: { "translate_target1": "translated_target1", ... }
const obj = {};
for (let i = 1; i <= count; i++) {
  obj[`translate_target${i}`] = `translated_target${i}`;
}
const jsonPath = path.join(localesDir, 'translation.json');
fs.writeFileSync(jsonPath, JSON.stringify(obj, null, 0) + '\n', 'utf8');

// Config for i18next-turbo extract
const turboConfig = {
  input: ['src/**/*.ts', 'src/**/*.tsx'],
  output: 'locales',
  locales: ['en'],
  functions: ['t'],
  extractFromComments: false,
};
const turboConfigPath = path.join(benchDir, 'i18next-turbo.json');
fs.writeFileSync(turboConfigPath, JSON.stringify(turboConfig, null, 2) + '\n', 'utf8');

// Config for i18next-cli (extract.input, extract.output, locales, extract.functions)
const cliConfigCjs = `module.exports = {
  locales: ['en'],
  extract: {
    input: ['src/**/*.ts', 'src/**/*.tsx'],
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t'],
  },
};
`;
const cliConfigPath = path.join(benchDir, 'i18next.config.cjs');
fs.writeFileSync(cliConfigPath, cliConfigCjs, 'utf8');

console.log(`Generated benchmark fixtures (N=${count}):`);
console.log('  ', srcPath);
console.log('  ', jsonPath);
console.log('  ', turboConfigPath);
console.log('  ', cliConfigPath);
