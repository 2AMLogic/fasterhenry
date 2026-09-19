# Changelog

All notable changes to the `fasterhenry` library and the `fasterhenry-cli`
binary crate are recorded here. Both crates share one version number and this
one file (each crate ships a copy in its published package).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While the version is `0.0.x`, the API and the command-line interface are
unstable and may change in any release.

## [Unreleased]

### Added

- Cargo workspace with the `fasterhenry` library crate and the
  `fasterhenry-cli` crate, which installs the `fasterhenry` binary.
- Packaging metadata for both crates (`readme`, `documentation`, `keywords`,
  `categories`), per-crate READMEs, and an `include` allowlist so a published
  package carries only sources, the manifest, README, LICENSE, and this
  changelog.
- CI packaging job: `cargo publish --dry-run` for the library and a
  package-contents check for both crates.

### Changed

- `fasterhenry-cli` now takes the library from `[workspace.dependencies]` with
  both a `path` and a `version`, so the CLI crate is publishable.

[Unreleased]: https://github.com/2AMLogic/fasterhenry/commits/main
