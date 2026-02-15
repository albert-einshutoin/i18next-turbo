#!/usr/bin/env node
/**
 * Measure i18next-turbo Node wrapper overhead with and without plugins.
 *
 * Usage:
 *   node scripts/run-plugin-benchmark.mjs [runs] [options]
 *
 * Options:
 *   --json <path>           write machine-readable report
 *   --max-overhead <ratio>  fail when overhead ratio is above threshold
 *                           (e.g. 0.20 = 20%)
 */

import fs from 'fs';
import path from 'path';
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const fixtureDir = path.join(root, 'benchmarks', 'fixtures', 'large');
const wrapperPath = path.join(root, 'bin', 'cli.js');

function parseArgs(argv) {
  const out = {
    runs: 3,
    jsonPath: null,
    maxOverhead: null,
  };

  let i = 0;
  if (argv[i] && !argv[i].startsWith('-')) {
    const parsed = parseInt(argv[i], 10);
    if (Number.isFinite(parsed) && parsed > 0) {
      out.runs = parsed;
    }
    i += 1;
  }

  while (i < argv.length) {
    const arg = argv[i];
    if (arg === '--json') {
      out.jsonPath = argv[i + 1] || null;
      i += 2;
      continue;
    }
    if (arg === '--max-overhead') {
      const parsed = Number(argv[i + 1]);
      out.maxOverhead = Number.isFinite(parsed) ? parsed : null;
      i += 2;
      continue;
    }
    i += 1;
  }
  return out;
}

const args = parseArgs(process.argv.slice(2));

function runOnce(configFile) {
  return new Promise((resolve, reject) => {
    const start = performance.now();
    const child = spawn('node', [wrapperPath, '--config', configFile, 'extract'], {
      cwd: fixtureDir,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env },
    });
    let stderr = '';
    child.stderr?.on('data', (d) => { stderr += d; });
    child.on('close', (code) => {
      const elapsed = performance.now() - start;
      if (code !== 0) {
        reject(new Error(`${configFile} exited ${code}: ${stderr.slice(0, 600)}`));
      } else {
        resolve(elapsed);
      }
    });
    child.on('error', reject);
  });
}

async function measure(configFile, warmup = 1, runs = 3) {
  for (let i = 0; i < warmup; i++) {
    await runOnce(configFile);
  }
  const samples = [];
  for (let i = 0; i < runs; i++) {
    samples.push(await runOnce(configFile));
  }
  return samples;
}

function stats(samples) {
  const sum = samples.reduce((a, b) => a + b, 0);
  return {
    avg: sum / samples.length,
    min: Math.min(...samples),
    max: Math.max(...samples),
  };
}

function fmt(ms) {
  return `${ms.toFixed(2)} ms`;
}

async function main() {
  console.log('Benchmark: i18next-turbo node wrapper (plugin off vs plugin on)');
  console.log(`Fixture: ${path.relative(root, fixtureDir)}\n`);

  const noPluginSamples = await measure('i18next.config.cjs', 1, args.runs);
  const withPluginSamples = await measure('i18next.plugin.config.cjs', 1, args.runs);

  const noPlugin = stats(noPluginSamples);
  const withPlugin = stats(withPluginSamples);
  const overheadRatio = (withPlugin.avg - noPlugin.avg) / noPlugin.avg;

  console.log('--- Result ---');
  console.log(`without plugin  avg: ${fmt(noPlugin.avg)}  min: ${fmt(noPlugin.min)}  max: ${fmt(noPlugin.max)}  (${args.runs} runs)`);
  console.log(`with plugin     avg: ${fmt(withPlugin.avg)}  min: ${fmt(withPlugin.min)}  max: ${fmt(withPlugin.max)}  (${args.runs} runs)`);
  console.log(`overhead        ${(overheadRatio * 100).toFixed(2)}%`);

  const report = {
    fixture: 'benchmarks/fixtures/large',
    runs: args.runs,
    withoutPlugin: noPlugin,
    withPlugin,
    overheadRatio,
    timestamp: new Date().toISOString(),
  };

  if (args.jsonPath) {
    const outPath = path.resolve(root, args.jsonPath);
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(`Saved benchmark report: ${outPath}`);
  }

  if (args.maxOverhead != null && overheadRatio > args.maxOverhead) {
    console.error(
      `Benchmark assertion failed: overhead ${(overheadRatio * 100).toFixed(2)}% exceeds ${(args.maxOverhead * 100).toFixed(2)}%`
    );
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
