# Security Policy

## Reporting a vulnerability

Please report vulnerabilities through GitHub's private security advisories:
open the repository's **Security** tab and choose **Report a vulnerability**.

Do not open a public issue for a security report. We aim to acknowledge
reports within 72 hours and will coordinate disclosure with you before any
public advisory.

## Scope

This tool sends network traffic and handles camera credentials, so these
classes of bug matter most:

- **Credential disclosure:** logging, error text or report output that exposes
  an authenticated RTSP URI, password or digest material.
- **Request injection:** a username, profile token or device response that
  changes the structure of an ONVIF SOAP request.
- **Unsafe parsing:** a panic, unbounded allocation or denial of service
  triggered by a hostile WS-Discovery or SOAP response.
- **Unexpected writes:** any request that changes camera or recorder state.
- **Path or shell injection:** command execution or filesystem writes from
  network-controlled values.

## Known limitations (not vulnerabilities)

- Discovery responses are unauthenticated. Any host on the local network can
  claim any XAddr, scope or type. The tool reports leads, not verified
  inventory.
- ONVIF requests use plain HTTP and password digest. The digest does not
  encrypt the conversation. Use the resolver only on networks you trust.
- The XML reader is intentionally minimal and not namespace-aware. It is
  documented in [docs/SPEC.md](docs/SPEC.md) section 6.
- There is no device-count cap or rate limit during discovery. A hostile local
  host can flood the scan with responses.
- Credentials are inserted into RTSP URIs verbatim; reserved characters in a
  username or password can produce an invalid URI.
- The tool scans networks the operator points it at. Authorization to scan is
  the operator's responsibility.

## Supported versions

Security fixes land on `main` and release as patch versions of the current
minor line. The `0.x` series does not promise wire or output compatibility
beyond the contract in [docs/SPEC.md](docs/SPEC.md).
