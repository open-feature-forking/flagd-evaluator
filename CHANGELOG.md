# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0](https://github.com/open-feature-forking/flagd-evaluator/compare/v0.1.0...v0.2.0) (2026-02-21)


### Features

* **dotnet:** add .NET WASM evaluator package ([#107](https://github.com/open-feature-forking/flagd-evaluator/issues/107)) ([cb73a19](https://github.com/open-feature-forking/flagd-evaluator/commit/cb73a19cf4c603cd47abaf8fed43ca5e99ef3a33))
* **evaluation:** pre-evaluate static flags for host-side caching ([#68](https://github.com/open-feature-forking/flagd-evaluator/issues/68)) ([0867574](https://github.com/open-feature-forking/flagd-evaluator/commit/0867574ca632d2b043d1542fdff5d6628b3de569))
* **go:** add Go package with wazero WASM runtime, instance pool, and optimized parsing ([#71](https://github.com/open-feature-forking/flagd-evaluator/issues/71)) ([75a94f2](https://github.com/open-feature-forking/flagd-evaluator/commit/75a94f2f4abe8e1a41c0dcd8222ec2d5622cfbb6))
* **js:** add JavaScript/TypeScript WASM evaluator package ([#110](https://github.com/open-feature-forking/flagd-evaluator/issues/110)) ([42bce60](https://github.com/open-feature-forking/flagd-evaluator/commit/42bce60ea1945e33e054f987ba6459a25b69d5e9))
* python native bindings ([#49](https://github.com/open-feature-forking/flagd-evaluator/issues/49)) ([8d2baa3](https://github.com/open-feature-forking/flagd-evaluator/commit/8d2baa38aba7a3487f5d36518d314b58b7cb4cb0))
* **python:** add WASM evaluator and 3-way comparison benchmarks ([#73](https://github.com/open-feature-forking/flagd-evaluator/issues/73)) ([3ebed59](https://github.com/open-feature-forking/flagd-evaluator/commit/3ebed597b09afd6fb8175e78c45f1b830ed3f5e6))
* rust based improvements ([#52](https://github.com/open-feature-forking/flagd-evaluator/issues/52)) ([948adcd](https://github.com/open-feature-forking/flagd-evaluator/commit/948adcd6300dceb971dc62643ea670ccc69eacfd))


### Bug Fixes

* improve python ([#51](https://github.com/open-feature-forking/flagd-evaluator/issues/51)) ([004db1e](https://github.com/open-feature-forking/flagd-evaluator/commit/004db1e0d672583da719ffaeed9d556e21ad4605))


### Performance Improvements

* add C7-C10 high-concurrency benchmarks (16 threads) ([#99](https://github.com/open-feature-forking/flagd-evaluator/issues/99)) ([9e85110](https://github.com/open-feature-forking/flagd-evaluator/commit/9e85110496d1c45df26c653c09e95ab416e29099))
* cross-language concurrent comparison benchmarks ([#102](https://github.com/open-feature-forking/flagd-evaluator/issues/102)) ([a4ea118](https://github.com/open-feature-forking/flagd-evaluator/commit/a4ea118c1b2d5422951b975f7348df96af5ef595))
* **evaluation:** add context key filtering and index-based WASM evaluation ([#70](https://github.com/open-feature-forking/flagd-evaluator/issues/70)) ([50d7cc0](https://github.com/open-feature-forking/flagd-evaluator/commit/50d7cc059e1d8d0149c9fc7439e8d4158dd100e4))
* **java:** add concurrency benchmarks C1-C6 ([#94](https://github.com/open-feature-forking/flagd-evaluator/issues/94)) ([23b47f4](https://github.com/open-feature-forking/flagd-evaluator/commit/23b47f430f51342731dec145d0194b4c2c32e6fa))
* **java:** add custom operator benchmarks O1-O6 ([#91](https://github.com/open-feature-forking/flagd-evaluator/issues/91)) ([eb820f3](https://github.com/open-feature-forking/flagd-evaluator/commit/eb820f32313f399fca3bff3bcccf8f1bc6e6ea60))
* **java:** add evaluation benchmarks E3, E6, E7, E10, E11 ([#92](https://github.com/open-feature-forking/flagd-evaluator/issues/92)) ([033f287](https://github.com/open-feature-forking/flagd-evaluator/commit/033f2878060422a026a4575277108e9993998fc3))
* **java:** replace synchronized single instance with WASM instance pool ([#104](https://github.com/open-feature-forking/flagd-evaluator/issues/104)) ([84fff31](https://github.com/open-feature-forking/flagd-evaluator/commit/84fff31ca3f6a570f6fdf05e1bd8a88f07d00c68))
* **python:** add host-side optimizations for pre-evaluation, context filtering, and index-based eval ([#72](https://github.com/open-feature-forking/flagd-evaluator/issues/72)) ([2b7a0b9](https://github.com/open-feature-forking/flagd-evaluator/commit/2b7a0b9b30b297aefc8cdbd0fc4f96a2b133f56c))
* **python:** add O2 and O4 operator benchmarks ([#98](https://github.com/open-feature-forking/flagd-evaluator/issues/98)) ([a78af74](https://github.com/open-feature-forking/flagd-evaluator/commit/a78af7478fc36fda712760dd100ad411d149695f))
* **rust:** add E4 and E6 evaluation benchmarks ([#97](https://github.com/open-feature-forking/flagd-evaluator/issues/97)) ([7638db9](https://github.com/open-feature-forking/flagd-evaluator/commit/7638db9acac4480b1c494ea90a52ebac37f67f39))
* **rust:** add scale benchmarks S6-S8, S10-S11 for large flag stores ([#93](https://github.com/open-feature-forking/flagd-evaluator/issues/93)) ([044f4c3](https://github.com/open-feature-forking/flagd-evaluator/commit/044f4c3f5be981e0e9342d90b345d11fde7c15a6))


### Documentation

* add standardized benchmark matrix for cross-language comparison ([#67](https://github.com/open-feature-forking/flagd-evaluator/issues/67)) ([aa3308c](https://github.com/open-feature-forking/flagd-evaluator/commit/aa3308c68b68416885d6e94b09222de31b059133))
* document WASM context serialization optimizations and updated benchmarks ([#69](https://github.com/open-feature-forking/flagd-evaluator/issues/69)) ([53f2cb7](https://github.com/open-feature-forking/flagd-evaluator/commit/53f2cb724bbc531eccf33245b691625f79e11334))
* restructure CLAUDE.md, add ARCHITECTURE.md, extend BENCHMARKS.md ([09b5bd4](https://github.com/open-feature-forking/flagd-evaluator/commit/09b5bd489881f95c710f97d64dbdd85478bfd154))
* rewrite README.md as clean entry point ([#109](https://github.com/open-feature-forking/flagd-evaluator/issues/109)) ([46e25ef](https://github.com/open-feature-forking/flagd-evaluator/commit/46e25efdcfe1aeb30654093f24d6b372bc3220eb))
* update README to reflect instance-based API refactoring ([#53](https://github.com/open-feature-forking/flagd-evaluator/issues/53)) ([f0fe238](https://github.com/open-feature-forking/flagd-evaluator/commit/f0fe238f5c28bf9191c55aa318bae4a4f5f1aa96))


### Code Refactoring

* improve architecture and add edge case tests ([#54](https://github.com/open-feature-forking/flagd-evaluator/issues/54)) ([c192a4f](https://github.com/open-feature-forking/flagd-evaluator/commit/c192a4f402079463ecfee6bd60c64d310570260d))
* **java:** dynamically match wasm-bindgen host functions by prefix ([#96](https://github.com/open-feature-forking/flagd-evaluator/issues/96)) ([6c531e5](https://github.com/open-feature-forking/flagd-evaluator/commit/6c531e52c6fcbf9e23a240817c35275b80ee1695))

## [Unreleased]

### Features

- Initial release of flagd-evaluator
- WebAssembly module for JSON Logic evaluation
- Support for fractional operator
- CLI tool for testing and development

[Unreleased]: https://github.com/open-feature-forking/flagd-evaluator/compare/v0.1.0...HEAD
