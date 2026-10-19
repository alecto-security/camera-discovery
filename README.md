# camera-discovery

ONVIF WS-Discovery scanner for cameras and NVRs. Ask it what is on a local
network, and it answers with the devices that responded and the services they
advertise. Discovery is read-only and needs no credentials.

Built for integrators and anyone who inherits a mixed camera fleet: bring the
infrastructure you already own, find out what it speaks, and skip the
spreadsheet archaeology.

<p align="center">
  <a href="https://github.com/alecto-security/camera-discovery/actions/workflows/ci.yml"><img src="https://github.com/alecto-security/camera-discovery/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust 1.75+">
</p>

## Quickstart

```bash
cargo install --path .
camera-discovery --timeout 5
camera-discovery --network 192.168.1.0/24 --timeout 10
camera-discovery --json
```

The probe goes to the WS-Discovery multicast group
`239.255.255.250:3702`. Devices must be on the same layer-2 network as the
machine running the scan. `--network` filters responses by the IPv4 address in
each advertised XAddr; it does not change the multicast scope.

Text output:

```text
http://192.168.1.64/onvif/device_service
  onvif://www.onvif.org/name/West%20Entrance onvif://www.onvif.org/type/video_encoder
http://192.168.1.72/onvif/device_service
  onvif://www.onvif.org/name/Loading%20Bay
```

`--json` prints an array in response order:

```json
[
  {
    "xaddrs": [
      "http://192.168.1.64/onvif/device_service"
    ],
    "scopes": "onvif://www.onvif.org/name/West%20Entrance onvif://www.onvif.org/type/video_encoder",
    "types": "dn:NetworkVideoTransmitter"
  }
]
```

An empty result is a successful scan: text prints `No ONVIF devices found.`
and JSON prints `[]`. Exit codes are `0` for a completed scan (including an
empty one), `1` for a runtime or configuration error, and `2` for command-line
usage errors.

## What it does and does not do

**Does:** send one WS-Discovery Probe for `dn:NetworkVideoTransmitter`, collect
ProbeMatch responses for the requested timeout, and report XAddrs, scopes and
types exactly as sent.

**Does not:** log in during discovery, change camera settings, pull video,
scan ports, test credentials, or claim that a response is trustworthy. Any
host on the local network can answer, and a responder can put anything in its
scopes or XAddrs.

The library also exposes `resolve`, a read-only ONVIF source resolver for
callers that supply credentials. It performs `GetDeviceInformation`,
`GetProfiles` and `GetStreamUri` (plus `GetReplayUri` when the device
advertises replay). The returned authenticated RTSP URI is a secret; print
`stream_uri_redacted` instead.

## Library

```rust
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), alecto_discovery::DiscoveryError> {
    let devices = alecto_discovery::discover(
        Some("192.168.1.0/24"),
        Duration::from_secs(5),
    )
    .await?;
    println!("{}", alecto_discovery::render_text(&devices));
    Ok(())
}
```

The crate is `alecto-discovery`; the command is `camera-discovery`. The plain
`camera-discovery` crate name is already used by an unrelated project on
crates.io, so the import name carries the vendor prefix.

## Known limits

- IPv4 only. No IPv6 multicast, no unicast sweep, no ONVIF port scan.
- One probe, one timeout. Packet loss and slow responders are not retried.
- Responses are unauthenticated and unverified. Treat the output as leads,
  not as an inventory of record.
- The XML reader is deliberately small and not namespace-aware; malformed or
  hostile documents can be ignored or misreported.
- Duplicate responses are reported as separate entries.
- `--network` is a post-filter over advertised XAddrs; a device with no XAddr
  is dropped when the filter is set.
- ONVIF resolution uses plain HTTP and ONVIF password digest. The password is
  not sent in clear text, but the conversation is not encrypted. Use it only
  on networks you trust.
- No vendor-specific NVR APIs, no stream playback check, no retention or
  replay verification.
- No rate limiting or result cap. A hostile local host can flood the scan.

The full behavior and output contract is in [docs/SPEC.md](docs/SPEC.md).

## Security

Please report vulnerabilities through GitHub's private security advisories on
this repository, not in a public issue. See [SECURITY.md](SECURITY.md).

Scan only networks you own or are authorized to assess.

## License

Apache-2.0. See [LICENSE](LICENSE).
