# Changelog

All notable changes to `camera-discovery` are recorded here. Versions follow
SemVer; the output contract is described in [docs/SPEC.md](docs/SPEC.md).

## [Unreleased]

### Added

- Release workflow: a `vX.Y.Z` tag (or `workflow_dispatch` against an
  existing tag, including `v0.1.0`, which was cut before this workflow
  existed) builds `camera-discovery` for Linux x86_64 and aarch64 (static
  musl), macOS x86_64 and aarch64, and Windows x86_64, gates on clippy and
  the test suite, smoke-tests the shipped Linux binary's CLI contract, and
  publishes the archives with a `sha256sums.txt` as release assets. Running
  the scanner needs no Rust toolchain.
- README quickstart for the prebuilt binaries.

## [0.1.0] - 2026-10-19

Born public, born tagged.

### Added

- `camera-discovery` command: one WS-Discovery Probe to
  `239.255.255.250:3702`, a bounded response window, optional IPv4 CIDR
  filtering, and text or JSON output.
- `alecto-discovery` library: discovery, ProbeMatch parsing, report rendering,
  URL/time helpers, and read-only ONVIF source resolution (`resolve`).
- XML escaping for the SOAP username and media profile token.
- Validation of `--network` before any packet is sent.
- SOAP client hardening: redirects and environment proxies disabled, response
  bodies capped at 4 MiB, and non-timeout socket errors reported as errors.
- Unit, integration and CLI tests for parsing, output shape, helper behavior,
  invalid input, response caps and exit codes.
- CI: format, clippy `-D warnings`, tests, a standalone `cargo package` check,
  and an MSRV check on Rust 1.89.

### Known limits

- IPv4 and WS-Discovery multicast only; one probe with no retries.
- Discovery responses are unauthenticated and unverified.
- The XML reader is a minimal tag extractor, not a conforming parser.
- ONVIF resolution uses plain HTTP with password digest.
