# camera-discovery Specification

**Version:** 0.1.0 · 2026-10-19
**Status:** normative for this repository. It describes the behavior of the
`camera-discovery` command and the `alecto-discovery` library at version
`0.1.0`.

The key words MUST, MUST NOT, SHOULD, and MAY are interpreted as described in
RFC 2119.

## 1. Scope

`camera-discovery` is a read-only ONVIF WS-Discovery scanner. It sends one
Probe, collects ProbeMatch responses for a bounded time, and reports the
XAddrs, scopes and types the devices advertised.

The library additionally resolves an ONVIF camera to a stream URI using
caller-supplied credentials. It never writes camera or recorder state, never
stores credentials, and does not retrieve media.

Out of scope for 0.1.0: IPv6, port scanning, unicast probing, NVR vendor
APIs, credential testing, stream validation, persistence, and a GUI.

## 2. Discovery protocol

1. The scanner MUST send one UDP Probe from an ephemeral IPv4 socket to
   `239.255.255.250:3702`.
2. The Probe MUST target `dn:NetworkVideoTransmitter`.
3. The Probe MUST contain a fresh UUID message id.
4. The scanner MUST listen for ProbeMatch datagrams until the requested
   timeout expires. The timeout starts after the Probe is sent.
5. A datagram is parsed only when it contains `ProbeMatch` as a
   case-insensitive substring. The first matching `XAddrs`, `Scopes` and
   `Types` elements are used.
6. `XAddrs` text is split on ASCII whitespace. Empty XAddrs are allowed.
7. The scanner MUST send no credentials during discovery.
8. If `--network` is supplied, it MUST parse as an IPv4 CIDR before any packet
   is sent. An invalid value is a configuration error.
9. With `--network`, a response is kept only when at least one XAddr host
   parses as an IPv4 address inside the CIDR. The filter is applied after the
   response arrives. A device with no such XAddr is dropped.
10. Responses are appended in arrival order. The scanner does not deduplicate,
    rank, authenticate or verify them.
11. A receive timeout ends the scan successfully. Other receive errors also end
    the scan; results collected so far are returned.

The scanner does not use WS-Discovery Hello, Bye, Resolve or metadata
exchange, and does not retry a lost Probe.

## 3. Command-line contract

```text
camera-discovery [--network CIDR] [--timeout SECONDS] [--json]
```

| Option | Default | Meaning |
|--------|---------|---------|
| `--network CIDR` | none | Post-filter responses by IPv4 XAddr host |
| `--timeout SECONDS` | `5` | Total response window after the Probe |
| `--json` | off | Print the JSON representation instead of text |
| `--help` | | Print usage |
| `--version` | | Print the package version |

Only the report is written to stdout. Errors are written to stderr.

Exit codes:

| Code | Meaning |
|------|---------|
| `0` | Scan completed; zero or more devices reported |
| `1` | Runtime or configuration error, including an invalid `--network` |
| `2` | Command-line usage error reported by the argument parser |

A scan that finds nothing is not an error.

### 3.1 Text output

For zero devices, stdout is exactly:

```text
No ONVIF devices found.
```

For one or more devices, each device is one block. The first line is the
XAddrs joined by single spaces. When scopes are present, the next line is two
spaces followed by the scopes. Blocks are separated by a newline. The command
adds one trailing newline. Types are not printed in text mode.

### 3.2 JSON output

stdout is a pretty-printed JSON array, `[]` for zero devices. Each element is
an object with exactly these fields:

| Field | Type | Value |
|-------|------|-------|
| `xaddrs` | array of strings | XAddr values in response order |
| `scopes` | string | Scopes text, empty when absent |
| `types` | string | Types text, empty when absent |

Field order in the pretty output is `xaddrs`, `scopes`, `types`; consumers
MUST NOT depend on order. New optional fields may be added in a minor release.

## 4. Library contract

The crate is `alecto-discovery`; the library name is `alecto_discovery`.

### 4.1 Discovery

```rust
pub async fn discover(
    network: Option<&str>,
    wait: Duration,
) -> Result<Vec<DiscoveredDevice>>
```

`discover` implements section 2. `DiscoveredDevice` has public `xaddrs`,
`scopes` and `types` fields and implements `Serialize`.

`DiscoveryError::Config` is reserved for invalid input such as a malformed
CIDR. `DiscoveryError::Io` and `DiscoveryError::Http` wrap transport failures.
`DiscoveryError::Fail` is a runtime failure with a human-readable message.

### 4.2 Reporting

- `devices_json` returns one `serde_json::Value` object per device.
- `render_json` returns the pretty JSON array described in section 3.2.
- `render_text` returns the text body described in section 3.1, without the
  final newline.

These functions do no I/O.

### 4.3 Source resolution

```rust
pub async fn resolve(
    camera: &CameraConfig<'_>,
    start_utc: &str,
    end_utc: &str,
) -> Result<ResolvedSource>
```

`resolve` is a separate, authenticated operation. It:

1. Calls `GetDeviceInformation`, and uses the result for vendor, model and
   firmware. Missing values become `"unknown"`.
2. Calls `GetSystemDateAndTime`; a usable answer produces `time_source` and
   `clock_skew_secs`. Failure is not fatal.
3. Calls `GetCapabilities` to find the media service, falling back to the
   conventional `/onvif/media_service` path.
4. Calls `GetProfiles`, uses `onvif_profile` when supplied, otherwise the
   first profile token.
5. Calls `GetStreamUri`, then injects RTSP userinfo when the URI is `rtsp://`
   and has none.
6. Calls `GetCapabilities` and `GetReplayUri` when replay is advertised. A
   replay URI gets `starttime` and `endtime` query parameters; any existing
   query is discarded.
7. Falls back to the live stream URI when replay is absent or fails.

The HTTP client timeout is 8 seconds per request. The device endpoint is
`http://{host}:{port}/onvif/device_service`; TLS is not used by
`CameraConfig::device_url`.

`ResolvedSource::stream_uri_auth` contains credentials for RTSP sources.
`ResolvedSource::stream_uri_redacted` replaces the userinfo with `***`.
Callers MUST NOT log or persist the authenticated URI.

The SOAP username and profile token are XML-escaped. The password is not
placed in the document; it contributes only to the ONVIF password digest.

### 4.4 URI and time helpers

- `redact_uri` replaces everything before `@` in the authority with `***`.
  It does not redact query parameters or other secret material.
- `inject_if_rtsp` inserts `user:pass@` verbatim into an RTSP URI that has no
  `@`. It does not percent-encode. Empty username and password are a no-op.
- `host_from_url` and `port_from_url` are helpers for the
  `http://host:port/path` shape of ONVIF XAddrs, not general URL parsers.
- `rtsp_time_token` keeps ASCII digits, `T` and `Z`. It is not a validator.
- `with_rtsp_wall_clock_range` discards any existing query.
- `format_nvr_export_url` substitutes `{start}` and `{end}` in a template.

## 5. Safety properties

1. **Read-only.** The tool sends discovery probes and ONVIF query requests.
   It MUST NOT send a request that changes device configuration, recording
   state, users or firmware.
2. **No credential storage.** Discovery has no credential input. Resolution
   keeps the password only for the duration of the calls and the derived
   authenticated URI.
3. **No source writes.** There is no filesystem writer in the crate. Reports
   go to stdout or to the caller's own sink.
4. **No hidden network calls.** The only destinations are the fixed
   WS-Discovery multicast group and the device endpoints derived from the
   supplied host or from device responses during resolution.
5. **Explicit trust boundary.** Discovery output is unauthenticated. The tool
   MUST NOT present it as verified inventory.

## 6. Known limits

These are properties of 0.1.0, stated plainly.

### Discovery

- IPv4 only. IPv6 and mixed-stack networks are not covered.
- Multicast requires the scanner and devices to share a layer-2 boundary that
  forwards `239.255.255.250:3702`. Routed networks, VLAN boundaries and
  Wi-Fi client isolation can hide devices.
- One Probe and one timeout. A lost Probe or delayed response is missed.
- The receive buffer is the maximum UDP payload (65,507 bytes). A datagram
  cannot be larger, so this is not a truncation limit, but a device that
  fragments at the IP layer may still be lost.
- The XML reader is a substring/tag extractor, not a conforming parser. It is
  not namespace-aware, uses the first matching element, and does not resolve
  entities. Malformed, nested or hostile documents can be ignored or
  misreported.
- Only the first ProbeMatch payload's fields are read; multiple devices in one
  datagram are not enumerated.
- Duplicate responses appear as duplicate entries. There is no deduplication,
  sorting or stable device identity.
- `--network` filters on the advertised XAddr. A device advertising a hostname,
  IPv6 literal or no XAddr is dropped when the filter is set.
- Any local host can spoof a response. Scopes and types are self-asserted.
- There is no device-count cap or rate limit. A hostile host can make the
  scanner allocate many entries during the timeout.
- The scan does not identify an NVR as an NVR beyond what the device puts in
  its scopes and types.

### Resolution

- ONVIF only. Vendor-specific APIs that do not implement ONVIF are not
  supported.
- The first media profile is chosen when the caller does not name one. There
  is no profile browsing or codec selection.
- Replay support depends on `GetCapabilities` and `GetReplayUri` behaving as
  expected. The fallback is live-only and does not prove the device ignored a
  range request.
- `resolve` does not play the stream, check codecs, or prove that the returned
  URI is reachable after resolution.
- Requests go over plain HTTP to the device service. The ONVIF password digest
  is not transport encryption. A network observer can see usernames,
  responses and RTSP URIs.
- Credentials are inserted into the RTSP URI without percent-encoding.
  Reserved characters in a username or password can produce an invalid URI.
- The authenticated URI is held in memory as a normal `String`. There is no
  zeroization.
- The minimal HTTP/SOAP client has an 8-second per-request timeout and no
  retry policy.
- ONVIF responses are parsed with the same minimal XML reader as discovery.

### Reporting

- Text output omits types. Use `--json` when types matter.
- JSON is pretty-printed; consumers should parse it rather than diff bytes.
- Output is in response order, which is not deterministic across runs.

## 7. Compatibility

Within `0.x`, exit codes and the documented field meanings are stable.
Fields may be added to JSON objects in a minor release; consumers MUST ignore
unknown fields. Removing or renaming a field, changing an exit code, or
changing the meaning of `xaddrs`, `scopes` or `types` requires a major
release.

## 8. Test plan

CI runs on stable Rust with:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo package
```

The tests cover ProbeMatch parsing (including namespaced and malformed input),
URL and time helpers, credential injection and redaction, the exact text and
JSON report shapes, rejection of an invalid CIDR before network activity, and
the CLI help/version/error paths. CI does not require a live camera or a
multicast route.
