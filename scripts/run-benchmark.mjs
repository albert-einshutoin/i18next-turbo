#!/usr/bin/env node
/**
 * Runs extract benchmark for i18next-turbo and i18next-cli on the same fixture,
 * then prints a comparison table.
 *
 * Prerequisites:
 *   - i18next-turbo: cargo build --release, or bin in PATH
 *   - i18next-cli: npm install in repo root or npx
 *
 * Usage: node scripts/run-benchmark.mjs [runs] [options]
 *   runs               number of warmup + timed runs per CLI (default 3)
 *   --json <path>      write machine-readable benchmark report
 *   --min-speedup <n>  fail if turbo is less than n times faster than cli
 *
 * Example:
 *   node scripts/run-benchmark.mjs 5 --json benchmarks/latest.json --min-speedup 10
 */

import { spawn } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const fixtureDir = path.join(root, 'benchmarks', 'fixtures', 'large');

function parseArgs(argv) {
  const out = {
    runs: 3,
    jsonPath: null,
    minSpeedup: null,
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
    if (arg === '--min-speedup') {
      const parsed = Number(argv[i + 1]);
      out.minSpeedup = Number.isFinite(parsed) ? parsed : null;
      i += 2;
      continue;
    }
    i += 1;
  }
  return out;
}

const parsedArgs = parseArgs(process.argv.slice(2));
const runs = parsedArgs.runs;

function runExtract(name, command, args, cwd) {
  return new Promise((resolve, reject) => {
    const start = performance.now();
    const child = spawn(command, args, {
      cwd,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stderr = '';
    child.stderr?.on('data', (d) => { stderr += d; });
    child.on('close', (code) => {
      const elapsed = performance.now() - start;
      if (code !== 0) {
        reject(new Error(`${name} exited ${code}: ${stderr.slice(0, 500)}`));
      } else {
        resolve(elapsed);
      }
    });
    child.on('error', reject);
  });
}

async function measure(name, command, args, cwd, warmup = 1, count = runs) {
  for (let i = 0; i < warmup; i++) {
    await runExtract(name, command, args, cwd);
  }
  const times = [];
  for (let i = 0; i < count; i++) {
    times.push(await runExtract(name, command, args, cwd));
  }
  return times;
}

function formatMs(ms) {
  return `${ms.toFixed(2)} ms`;
}

function stats(times) {
  const sum = times.reduce((a, b) => a + b, 0);
  const avg = sum / times.length;
  const min = Math.min(...times);
  const max = Math.max(...times);
  return { avg, min, max };
}

async function main() {
  console.log('Benchmark: extract on benchmarks/fixtures/large (same fixture for both)\n');

  const turboBin = process.platform === 'win32' ? 'i18next-turbo.exe' : 'i18next-turbo';
  const turboPath = path.join(root, 'target', 'release', turboBin);
  const turboCmd = fs.existsSync(turboPath) ? turboPath : turboBin;
  const turboArgs = ['--config', 'i18next-turbo.json', 'extract'];

  const cliCmd = 'npx';
  const cliArgs = ['i18next-cli', 'extract', '--config', 'i18next.config.cjs'];

  const results = { turbo: null, cli: null };

  try {
    console.log('i18next-turbo ...');
    results.turbo = await measure('i18next-turbo', turboCmd, turboArgs, fixtureDir);
  } catch (e) {
    console.warn('i18next-turbo failed:', e.message);
  }

  try {
    console.log('i18next-cli ...');
    results.cli = await measure('i18next-cli', cliCmd, cliArgs, fixtureDir);
  } catch (e) {
    console.warn('i18next-cli failed:', e.message);
  }

  console.log('');
  console.log('--- Result ---');
  console.log('(Time = process start → exit, including startup and I/O)\n');

  if (results.turbo) {
    const s = stats(results.turbo);
    console.log(`i18next-turbo  avg: ${formatMs(s.avg)}  min: ${formatMs(s.min)}  max: ${formatMs(s.max)}  (${runs} runs)`);
  }
  if (results.cli) {
    const s = stats(results.cli);
    console.log(`i18next-cli    avg: ${formatMs(s.avg)}  min: ${formatMs(s.min)}  max: ${formatMs(s.max)}  (${runs} runs)`);
  }

  let speedup = null;
  if (results.turbo && results.cli) {
    const turboAvg = stats(results.turbo).avg;
    const cliAvg = stats(results.cli).avg;
    speedup = cliAvg / turboAvg;
    const ratio = speedup.toFixed(2);
    console.log('');
    console.log(`i18next-turbo is ~${ratio}x faster (by average time).`);
  }

  const report = {
    fixture: 'benchmarks/fixtures/large',
    runs,
    turbo: results.turbo ? stats(results.turbo) : null,
    cli: results.cli ? stats(results.cli) : null,
    speedup,
    timestamp: new Date().toISOString(),
  };

  if (parsedArgs.jsonPath) {
    const outPath = path.resolve(root, parsedArgs.jsonPath);
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(`Saved benchmark report: ${outPath}`);
  }

  if (parsedArgs.minSpeedup != null) {
    if (speedup == null) {
      console.error(
        `Benchmark assertion failed: speedup is unavailable (turbo or cli result missing).`
      );
      process.exit(1);
    }
    if (speedup < parsedArgs.minSpeedup) {
      console.error(
        `Benchmark assertion failed: required >= ${parsedArgs.minSpeedup}x, actual ${speedup.toFixed(2)}x`
      );
      process.exit(1);
    }
    console.log(
      `Benchmark assertion passed: ${speedup.toFixed(2)}x >= ${parsedArgs.minSpeedup}x`
    );
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
