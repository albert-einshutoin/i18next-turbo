# Performance Testing

## Quick run

```bash
npm run benchmark
# or
node scripts/run-benchmark.mjs 5
```

## CI-style assertion

```bash
npm run benchmark:ci
```

This command:
- runs 5 timed benchmark runs
- saves JSON report to `benchmarks/latest.json`
- fails if turbo speedup is below `10x`

## Custom options

```bash
node scripts/run-benchmark.mjs 5 \
  --json benchmarks/latest.json \
  --min-speedup 20
```

## Current sample results (provided)

- Run A:
  - turbo avg: `8.62 ms`
  - cli avg: `419.20 ms`
  - speedup: `~48.60x`
- Run B:
  - turbo avg: `14.21 ms`
  - cli avg: `1034.04 ms`
  - speedup: `~72.77x`

## Notes

- Time includes process startup and file I/O.
- For stable comparisons, run on the same machine with low background load.
