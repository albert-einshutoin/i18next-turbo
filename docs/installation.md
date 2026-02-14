# Installation and Binary Resolution

## Recommended

```bash
npm install --save-dev i18next-turbo
```

`i18next-turbo` uses platform packages via `optionalDependencies`.
At runtime, `bin/cli.js` resolves the binary in this order:

1. `CARGO_BIN_EXE_i18next_turbo` (Cargo test/dev environments)
2. `I18NEXT_TURBO_BINARY` (manual override)
3. Local builds (`target/debug`, `target/release`)
4. Installed platform package

## Platform Packages

- `i18next-turbo-darwin-arm64`
- `i18next-turbo-darwin-x64`
- `i18next-turbo-linux-x64-gnu`
- `i18next-turbo-linux-x64-musl`
- `i18next-turbo-win32-x64-msvc`

Linux x64 resolves `-musl` or `-gnu` depending on detected runtime.

## Fallback for Development

If platform packages are unavailable, local build still works:

```bash
cargo build --release
```

Then run:

```bash
i18next-turbo extract
```

## CI Recommendations

- Cache npm dependencies and Rust target artifacts separately.
- Keep `package.json` and `package-lock.json` aligned.
- Run extraction/lint/check in CI with explicit config path when needed (`--config`).

## Known Registry Limitation (as of February 14, 2026)

At the time of writing, `i18next-turbo-win32-x64-msvc` may fail to publish due to npm spam-detection moderation (`403`).

If this occurs:

1. Open a support request at [npm support](https://www.npmjs.com/support).
2. Ask for package-name moderation review for `i18next-turbo-win32-x64-msvc`.
3. Re-run release publish after approval.

Until then, Windows users should use source build fallback (`cargo build --release`).
