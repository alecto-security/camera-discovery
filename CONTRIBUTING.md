# Contributing

Issues, bug reports and pull requests are welcome.

This is a small, single-purpose tool. The most useful contributions are
device quirks that can be reproduced without a live camera: a redacted
WS-Discovery or ONVIF response, a failing helper, or a test vector. Never
attach real credentials, customer addresses or case data to an issue or PR.

## Authorization

Only scan or resolve devices on networks you own or have written permission
to assess. A pull request that makes the tool easier to use against
third-party networks without authorization will not be merged.

## Setup

Rust 1.89 or newer is the declared minimum; CI also tests stable. No other
dependency is required.

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo package
```

No live camera, multicast route or external service is required for the test
suite; CI has none either.

## Changes

1. **Behavior changes start with the contract.** Update
   [docs/SPEC.md](docs/SPEC.md) in the same PR as the code and tests. If the
   text and the implementation disagree, that is a bug.
2. **Keep the surface narrow.** Discovery, reporting and read-only source
   resolution. No credential testing, no mutation requests, no vendor NVR
   APIs, no persistence, no GUI.
3. **Add a test for behavior.** Parsing, output shape, exit codes and safety
   properties need a test that fails before the fix. Prefer synthetic
   fixtures over captures.
4. **Stay read-only.** Any request that changes camera or recorder state is
   out of scope.
5. **Do not grow dependencies casually.** Network-capable crates are expected
   here, but every new crate needs a reason in the PR. The crate must build
   standalone; no path dependencies on other Alecto repositories.

## Versioning

`0.x` is allowed to move, but the documented output contract is still a
contract. Additive JSON fields are a minor release. Removing a field,
changing an exit code, or changing the meaning of `xaddrs`, `scopes` or
`types` is a major release.

## Security reports

Do not open a public issue for a vulnerability. Follow
[SECURITY.md](SECURITY.md).
