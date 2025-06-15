# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.2] - 2025-06-15

### Changed

* Send sub-text even if it is empty.
* Avoid crashing when WebSocket client closes abruptly.

## [0.4.1] - 2025-02-10

### Changed

- Unify run WebSocket server scripts with platform auto detection.

## [0.4.0] - 2025-02-10

### Changed

- Get mpv socket location from the mpv config file instead of hardcoding it.
- Replace unmaintained parity-tokio-ipc dependency with mio.

## [0.3.0] - 2025-02-09

### Added

- `aarch64-apple-darwih` pre-compiled binary for Mac users with the newer Apple
  silicon chip. Users with Intel-based Macs should continue using
  `x86_64-apple-darwin`.
- Optional `-a` flag to change the WebSocket server bind address.
- winapi dependency to fix unresolved winerror import error caused by
  parity-tokio-ipc for Windows.

### Changed

- Upgrade to stable Rust 1.84.0.
- Upgrade clap dependency to 4.5.28.
- Upgrade serde dependency to 1.0.217.
- Upgrade serde_json dependency to 1.0.138.
- Upgrade tokio dependency to 1.43.0.

### Security

- [RUSTSEC-2020-0016](https://rustsec.org/advisories/RUSTSEC-2020-0016) Replace
ws crate with tungstenite and mio.

## [0.2.0] - 2024-07-16

### Added

- Ability to toggle WebSocket.

## [0.1.1] - 2024-07-11

### Changed

- Upgrade clap dependency to 4.5.9.
- Add release profile which reduces binary size up to 10 times.

## [0.1.0] - 2023-03-12

### Added

- mpv WebSocket.
