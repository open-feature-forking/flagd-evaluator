# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0](https://github.com/open-feature-forking/flagd-evaluator/compare/v0.1.0...v0.2.0) (2025-12-09)


### Features

* Add permissions for pull request title validation ([d9a22eb](https://github.com/open-feature-forking/flagd-evaluator/commit/d9a22eb111efe3e05a5b72fbd8ec42fc8a8e0146))
* add PR title validation for conventional commits ([#30](https://github.com/open-feature-forking/flagd-evaluator/issues/30)) ([e496c9c](https://github.com/open-feature-forking/flagd-evaluator/commit/e496c9c04c9dbbb97d83a6032e4d58728b95b9d2))
* **docs:** add Mermaid diagrams for state update, evaluation, and memory flows ([#41](https://github.com/open-feature-forking/flagd-evaluator/issues/41)) ([f2bb69d](https://github.com/open-feature-forking/flagd-evaluator/commit/f2bb69d929ed5a91b23fa0e18fbe2d33870e99df))
* **storage:** detect and report changed flags in update_state ([#38](https://github.com/open-feature-forking/flagd-evaluator/issues/38)) ([8d87e01](https://github.com/open-feature-forking/flagd-evaluator/commit/8d87e018100f0a83b21bbf9a69d540a4f1d88b65))
* support metadata merging in flag evaluation responses ([#40](https://github.com/open-feature-forking/flagd-evaluator/issues/40)) ([decafdb](https://github.com/open-feature-forking/flagd-evaluator/commit/decafdb81a6a5ae88d22d059acaa43e9828045b3))


### Bug Fixes

* Fix WASM module to load in Chicory by removing wasm-bindgen imports ([#10](https://github.com/open-feature-forking/flagd-evaluator/issues/10)) ([87f8e2c](https://github.com/open-feature-forking/flagd-evaluator/commit/87f8e2cda0e41de86b10590458c1b3b613f9622a))


### Continuous Integration

* Setup Release Please for automated releases and changelog generation ([#7](https://github.com/open-feature-forking/flagd-evaluator/issues/7)) ([375433c](https://github.com/open-feature-forking/flagd-evaluator/commit/375433c9ff1259031e2893ad2dc187a3d719eb56))

## [Unreleased]

### Features

- Initial release of flagd-evaluator
- WebAssembly module for JSON Logic evaluation
- Support for fractional operator
- CLI tool for testing and development

[Unreleased]: https://github.com/open-feature-forking/flagd-evaluator/compare/v0.1.0...HEAD
