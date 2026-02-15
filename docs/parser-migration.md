# Parser Migration Plan (Regex -> Scanner/AST)

Issue: `#57`

## Goal

Move extraction logic from regex-dependent matching toward scanner/AST-based parsing for better correctness and maintainability.

## Current Status

- Completed:
  - Comment extraction path no longer depends on regex patterns for:
    - `t('key')`
    - `t('key', 'default')`
    - `t('key', { ...options })`
  - Option parsing (`defaultValue`, `ns`, `context`, `count`, `ordinal`) uses scanner helpers.
  - framework block extraction for `.vue/.svelte` script/template/style is scanner-based
  - `regex` crate dependency removed from extractor path and `Cargo.toml`

## Outcome

- Extraction behavior parity is maintained by unit/integration tests.
- Benchmark sanity check (`npm run benchmark`) remains part of regression validation.
