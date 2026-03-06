# Compatibility and Format Guarantees

## Supported Locale Formats

`i18next-turbo` supports these locale output formats:

- `json`
- `json5`
- `js` / `js-esm`
- `js-cjs`
- `ts`

The following commands share the same locale file discovery and parsing rules:

- `extract`
- `status`
- `check`
- `sync`
- `rename-key`

## Node API Guarantees

- `lib/index.js` prefers a native addon when one is present.
- If the native addon is unavailable, it uses the CLI wrapper as a fallback.
- `extract`, `lint`, and `check` return structured results with the same contract on both paths.

## Compatibility Scope

The project is designed for i18next-style extraction workflows, not for full behavioral emulation of every third-party extractor.

Guaranteed:

- TS/JS source parsing via Rust + SWC
- i18next-style key extraction and locale synchronization
- Shared handling of supported locale formats across the commands listed above

Not guaranteed:

- Full drop-in compatibility with every `i18next-parser` config edge case
- Exact output parity with other extractors
- `locize` support for JS/TS module locale formats

## Command-Specific Limits

- `locize` supports `json` and `json5` only
- `watch` is optimized for supported project inputs, but it is not presented as a compatibility layer for other toolchains
- Plugin hooks are project-specific extension points, not a compatibility promise with other plugin APIs
