# Parser Migration Plan (Regex -> Scanner/AST)

Issue: `#57`

## Goal

Move extraction logic from regex-dependent matching toward scanner/AST-based parsing for better correctness and maintainability.

## Current Status

- Completed in this step:
  - Comment extraction path no longer depends on regex patterns for:
    - `t('key')`
    - `t('key', 'default')`
    - `t('key', { ...options })`
  - Option parsing (`defaultValue`, `ns`, `context`, `count`, `ordinal`) uses scanner helpers.
- Remaining regex usage:
  - framework block extraction for `.vue/.svelte` script/template/style segmentation
  - other non-comment legacy helpers

## Migration Strategy

1. Replace high-frequency extraction paths first (comment parser done).
2. Keep behavior parity with integration/unit tests.
3. Remove remaining regex paths incrementally with benchmark checks (`npm run benchmark`).

## Notes

- This is a staged migration to keep risk low while reducing regex dependency from major extraction paths.
