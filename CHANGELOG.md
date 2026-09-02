# Changelog

# [2.0.0](https://github.com/bolorundurovj/screenr/compare/v1.2.1...v2.0.0) (2026-09-02)

* feat!: ScreenR v2, five screens and a native capture pipeline (#59) ([b038e12](https://github.com/bolorundurovj/screenr/commit/b038e125069abdccaeaaf838d543608368a7f89e)), closes [#59](https://github.com/bolorundurovj/screenr/issues/59)

### BREAKING CHANGES

* start_recording takes sourceIds instead of sourceId and
  path, and settings move to a settings.json in the OS config dir.

  * test: add contract, unit and integration coverage

  * perf: optimise dev builds for per-pixel work

  Compositing cost 433ms a frame unoptimised, capping tauri dev recordings
  near 1.5fps. Optimising dependencies alone is not enough: the generic
  half of image monomorphises into this crate.

  * ci: add CI workflows for frontend and Rust, update release dependencies

  * refactor: improve sorting of takes by modified time using Reverse

## [1.2.1](https://github.com/bolorundurovj/screenr/compare/v1.2.0...v1.2.1) (2026-08-22)

# [1.2.0](https://github.com/bolorundurovj/screenr/compare/v1.1.3...v1.2.0) (2026-08-05)

### Bug Fixes

* add libgbm-dev and rollback release to trigger clean pipeline ([1316e41](https://github.com/bolorundurovj/screenr/commit/1316e4124cac213a974fbfd4dffdd3907aeb3dbf))
* add missing linux dependencies for xcap ([1af69e9](https://github.com/bolorundurovj/screenr/commit/1af69e911ef11fb3b048447c361f6f43e8caff3d))
* rollback versions and restore erased Cargo.toml dependencies ([c6bff67](https://github.com/bolorundurovj/screenr/commit/c6bff677190be42c81ae42cf2be978b2e1761335))
* rollback versions to trigger clean release ([1152929](https://github.com/bolorundurovj/screenr/commit/11529294139b4ce698c42dcf666738e8d5a949af))

### Features

* migrate capture engine to native Rust (xcap + ffmpeg) ([a50940c](https://github.com/bolorundurovj/screenr/commit/a50940cb8a438ac4ab688dea1a2e615230542bf4)), closes [high-performance](https://github.com/hi/issues/performance)

## [1.1.3](https://github.com/bolorundurovj/screenr/compare/v1.1.2...v1.1.3) (2026-08-05)

## [1.1.2](https://github.com/bolorundurovj/screenr/compare/v1.1.1...v1.1.2) (2026-08-05)

## [1.1.1](https://github.com/bolorundurovj/screenr/compare/v1.1.0...v1.1.1) (2026-08-05)

# [1.1.0](https://github.com/bolorundurovj/screenr/compare/v1.0.3...v1.1.0) (2026-08-05)

### Bug Fixes

* use IPC instead of deprecated remote module for video source selection ([47d2acb](https://github.com/bolorundurovj/screenr/commit/47d2acbb24262a0b718b104102a9d9fe73bc3b83))

### Features

* Move away from deprecated methods ([#53](https://github.com/bolorundurovj/screenr/issues/53)) ([292c33a](https://github.com/bolorundurovj/screenr/commit/292c33ad95b940894142b04290f5d85d38413bc9)), closes [#50](https://github.com/bolorundurovj/screenr/issues/50) [#51](https://github.com/bolorundurovj/screenr/issues/51)

## [1.0.3](https://github.com/bolorundurovj/screenr/compare/v1.0.2...v1.0.3) (2023-12-29)
