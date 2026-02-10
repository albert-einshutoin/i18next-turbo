# Troubleshooting

## No keys extracted

- Verify `input` globs match real files.
- Check `ignore` patterns are not too broad.
- Run `i18next-turbo status` to inspect detected sources.

## Keys are reported as dead unexpectedly

- Confirm namespace usage is consistent (`ns:key` vs default namespace).
- If using merged output, ensure `mergeNamespaces` is enabled.
- If needed, add `preservePatterns`.

## Node wrapper cannot load config

- Install config loader dependency for TS: `jiti`.
- Pass explicit config: `--config i18next.config.ts`.

## Plugin hook errors

- Hook errors are logged as warnings.
- Start with `setup` / `onLoad` only, then add `onVisitNode`.

## JSON5 formatting changed

- Comments and trailing commas are preserved.
- Numeric literal forms are preserved when value-equivalent (e.g. `1e3`, `0x10`).
