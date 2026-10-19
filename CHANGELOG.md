# Changelog

All notable changes to `camera-discovery` are recorded here. Versions follow
SemVer; the output contract is described in [docs/SPEC.md](docs/SPEC.md).

## [Unreleased]

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
- Unit, integration and CLI tests for parsing, output shape, helper behavior,
  invalid input and exit codes.
- CI: format, clippy `-D warnings`, tests, and a standalone `cargo package`
  check.

### Known limits

- IPv4 and WS-Discovery multicast only; one probe with no retries.
- Discovery responses are unauthenticated and unverified.
- The XML reader is a minimal tag extractor, not a conforming parser.
- ONVIF resolution uses plain HTTP with password digest.
