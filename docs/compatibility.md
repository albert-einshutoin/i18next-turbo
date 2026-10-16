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
- The Node wrapper rejects `watch` with a non-empty resolved plugin list before loading plugins or starting Rust, with exit code 1 and no translation-file changes. Use plugin-free `watch` or single-run `extract` with plugins; see [plugin runtime](plugin-runtime.md#watch-support).
- Plugin hooks are project-specific extension points, not a compatibility promise with other plugin APIs

### Static extraction checked against i18next-cli 1.75.2

The checked-in `tests/fixtures/cli-parity` catalog covers scoped `useTranslation` aliases, `getFixedT`, finite dynamic keys, locale-specific cardinal/ordinal forms, and call/Trans defaults. It verifies logical locale/namespace/key/value equality; it does not promise identical JSON formatting or every CLI option.

Dynamic keys support string literal unions (including relative named imports), constant strings, `as const` maps and array loops, and finite template combinations. Imported declarations must be in the configured, ignore-filtered input set. No source modules are executed. Objects and arrays stored in bindings require a const assertion; ordinary `const` bindings can still contain mutable properties. Type assertions do not establish general runtime immutability.

Resolution is limited to depth 8, 256 values and 1 MiB of expanded strings, with a shared node budget. Unsupported expressions, cycles, excluded imports and limit violations produce diagnostics. `--fail-on-warnings` rejects incomplete extraction before writes; pruning also rejects incomplete extraction. Without pruning or strict mode, resolved keys can still be synchronized while unresolved keys remain diagnosed.

Watch mode retains the configured input index and reparses changed files plus their transitive relative-import consumers. Byte-identical delayed notifications are skipped, including those following a rename. Unresolved values still prevent pruning; lost watch events stop the watcher rather than trusting an incomplete cache. Configured function names remain syntactic matches, as in the reference CLI; automatically discovered aliases are scoped to their blocks and functions.
