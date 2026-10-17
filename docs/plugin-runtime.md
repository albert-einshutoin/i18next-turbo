# Plugin Runtime Design (Issue #59)

## Goals

- Clarify how JS plugins run when `i18next-turbo` is distributed as a Rust binary + Node wrapper.
- Define safe failure behavior for plugin hook errors.
- Provide measurable performance baseline for "plugin off" vs "plugin on".

## Runtime Model

- Core extraction/sync logic runs in Rust binary.
- Dynamic JS plugins run in the Node wrapper (`bin/cli.js`).
- Execution model is single Node process + child Rust process.
  - Node preprocesses (`onLoad`) and postprocesses (`onVisitNode`/`afterSync`) around Rust execution.
  - Rust remains plugin-agnostic and receives config JSON via stdin.

This model was selected to keep Rust core deterministic and avoid embedding a JS runtime.

## Watch Support

- Use the Node wrapper's `watch` without plugins (`plugins` omitted or an empty array).
- A non-empty resolved plugin list (top-level `plugins` or `extract.plugins`) is rejected
  with `watch does not support plugins` and exit code 1, before loading plugin modules,
  calling factories/hooks, or starting Rust. Translation files are not changed.
  `pluginErrorMode: continue` does not bypass this restriction.
- The restriction applies to JS/TS configs, explicit JSON configs, and discovered configs.
  Inline watch configs (`--config-json JSON`, `--config-json=JSON`, or `--config-stdin`)
  pass through the same validation, including when no config file exists. Stdin takes
  priority over inline JSON, which takes priority over file configs. The wrapper reads
  watch config stdin once and forwards the validated config to Rust; other commands
  retain stdin passthrough, and help/version do not consume it.
  Evaluating the config itself is required to inspect plugins; this does not prevent
  arbitrary JavaScript executed by the config.
- Attached forms such as `-cFILE`, `-c=FILE`, and `-vcFILE` use the same config resolution.
  Empty or non-object explicit configs fail with exit code 1 before processing;
  they do not fall back to the default config.
- Every explicit watch config is resolved through one Node entry point. JS/TS uses the
  existing loaders; all other paths, including `.conf` and extensionless files, are read
  as JSON regardless of extension. Read, JSON parse, and object-validation failures
  reject processing without falling back to Rust or default config.
- Run `extract` when plugins are needed. Single-run source transforms, AST events, and
  completion hooks remain supported.
- Plugin-free incremental watch, translation preservation, and prune refusal for unresolved
  values remain supported. The Rust binary does not evaluate JS plugins and is not an
  alternative for plugin-assisted watch.

## Hook Boundaries

- `setup(context)`:
  Called before preprocess and Rust execution.
- `onLoad({ filePath, relativePath, source, ... })`:
  Source-level preprocessing before Rust parse/extract.
- `onVisitNode(node)`:
  Receives AST-event bridge output and translation tree traversal events.
- `onEnd(context)`:
  Called after Rust execution and optional runtime transforms.
- `afterSync(context)`:
  Final post-sync hook.

## Failure Policy

`pluginErrorMode` controls behavior when plugin load/hook fails.

- `continue` (default):
  Log warning and continue pipeline.
- `fail-fast`:
  Throw immediately and exit with non-zero status.

Supported positions:

- top-level: `pluginErrorMode`
- extract scope: `extract.pluginErrorMode`

`extract.pluginErrorMode` takes precedence when both are defined.

## Compatibility (No-Node Environment)

- JS/TS config loading and plugin runtime require Node wrapper.
- Pure Rust binary execution does not evaluate JS plugins.
- Recommended for CI and production:
  - Use Node wrapper when plugin hooks are required.
  - Use static JSON config and Rust-only path when deterministic minimal runtime is preferred.

## Benchmark Plan

- Baseline:
  `npm run benchmark` (turbo vs i18next-cli)
- Plugin overhead:
  `npm run benchmark:plugin` (same fixture, plugin off vs plugin on)

Telemetry fields:

- average/min/max process time
- overhead ratio (`withPlugin.avg - withoutPlugin.avg`) / `withoutPlugin.avg`

Optional threshold gate:

- `node scripts/run-plugin-benchmark.mjs 5 --max-overhead 0.20`

## Security & Isolation Notes

- Plugins execute with Node process privileges (same trust boundary as build scripts).
- No additional sandbox is applied by default.
- Treat third-party plugins as trusted code.
