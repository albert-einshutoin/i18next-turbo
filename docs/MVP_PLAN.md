# MVP Plan (Issue Priority Driven)

This plan defines implementation order based on current open issues and `priority:*` labels.

## Priority Policy

- `priority:P0`: Release quality blocker or architecture baseline.
- `priority:P1`: High-value improvements after baseline stabilization.
- `priority:P2`: Backlog/duplicate/superseded items.

## Current Open Issues by Priority

### P0 (Execute First)

1. [#58](https://github.com/albert-einshutoin/i18next-turbo/issues/58) Architecture: separate network dependencies (Locize, etc.) from core.
2. [#60](https://github.com/albert-einshutoin/i18next-turbo/issues/60) Make reqwest/Locize optional feature, default to minimal core.
3. [#35](https://github.com/albert-einshutoin/i18next-turbo/issues/35) Replace `anyhow` in library path with strict error enum (`thiserror`).
4. [#61](https://github.com/albert-einshutoin/i18next-turbo/issues/61) Expand destructive/edge-case testing (broken Unicode, huge files, permission/resource failures).

### P1 (Execute Next)

1. [#57](https://github.com/albert-einshutoin/i18next-turbo/issues/57) Migrate from regex-driven extraction to scanner/AST-first architecture.
2. [#59](https://github.com/albert-einshutoin/i18next-turbo/issues/59) Define plugin runtime model and quantify overhead.
3. [#42](https://github.com/albert-einshutoin/i18next-turbo/issues/42) SWC dependency review for binary size optimization.
4. [#40](https://github.com/albert-einshutoin/i18next-turbo/issues/40) Preserve JSON formatting style to reduce diff noise.

### P2 (Backlog / Consolidation)

1. [#37](https://github.com/albert-einshutoin/i18next-turbo/issues/37) Build size and compile time reduction (overlaps with #42/#60).
2. [#31](https://github.com/albert-einshutoin/i18next-turbo/issues/31) Build size and compile time reduction (older duplicate line).

## Execution Waves

### Wave 1: Architecture Baseline
- Execute #58 and #60 together.
- Outcome: a core-only build path with optional network integration.

### Wave 2: Reliability and Error Contracts
- Execute #35 and #61.
- Outcome: typed errors + robust failure behavior under adverse conditions.

### Wave 3: Extraction and Extension Model
- Execute #57 and #59.
- Outcome: deterministic extraction engine and documented plugin runtime boundary.

### Wave 4: Size/UX Polish
- Execute #42 and #40.
- Outcome: smaller binaries and low-noise JSON diffs.

## Notes

- Priority labels are maintained on GitHub issues (`priority:P0`, `priority:P1`, `priority:P2`).
- Re-prioritization should happen only after wave completion or major production incidents.
