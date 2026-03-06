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

To build the native Node addon used by `lib/index.js`:

```bash
cargo build --release --features napi
```

## CI Recommendations

- Cache npm dependencies and Rust target artifacts separately.
- Keep `package.json` and `package-lock.json` aligned.
- Run extraction/lint/check in CI with explicit config path when needed (`--config`).
- Validate both `cargo test` and `cargo test --features napi`.
