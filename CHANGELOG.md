# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

---

## [0.5.3] - 2026-02-15

### Added
- CI benchmark regression gate (`benchmark-regression`) with benchmark report artifact upload.
- README trust badges (npm version/downloads, Node.js, CI, codecov, MIT, Rust) in both English and Japanese docs.

### Changed
- Reduced SWC parser dependency surface to minimum required defaults for smaller/faster builds.
- Benchmarks are now executed non-interactively in CI (`npx --yes i18next-cli@latest`).

### Fixed
- Prevented benchmark hangs caused by subprocess stdout backpressure.
- Added workflow-level and per-run timeout guards for benchmark execution stability.

## [0.1.0] - 2025-01-XX

### Added
- Initial release
- Core extraction functionality
- Watch mode
- JSON synchronization

[Unreleased]: https://github.com/albert-einshutoin/i18next-turbo/compare/v0.5.3...HEAD
[0.5.3]: https://github.com/albert-einshutoin/i18next-turbo/compare/v0.1.0...v0.5.3
[0.1.0]: https://github.com/albert-einshutoin/i18next-turbo/releases/tag/v0.1.0
