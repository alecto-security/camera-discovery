use std::net::Ipv4Addr;
use std::time::Duration;

use ipnet::Ipv4Net;
use serde::Serialize;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::error::{DiscoveryError, Result};
use crate::soap::SoapClient;

/// The IPv4 WS-Discovery multicast group and port.
pub const WS_DISCOVERY_ADDR: &str = "239.255.255.250:3702";

/// Largest possible UDP payload. A WS-Discovery ProbeMatch is normally tiny;
/// the full datagram is read so long scopes are not silently truncated.
const MAX_DATAGRAM: usize = 65_507;

/// One device that answered the WS-Discovery probe.
///
/// `xaddrs` are SOAP device service endpoints, not stream URLs. They are
/// reported exactly as the device sent them and are not verified.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredDevice {
    pub xaddrs: Vec<String>,
    pub scopes: String,
    pub types: String,
}

/// The narrow camera view resolution needs.
///
/// The caller owns credential lookup. Discovery itself never receives or sends
/// a password.
#[derive(Debug, Clone, Copy)]
pub struct CameraConfig<'a> {
    pub id: &'a str,
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
    /// ONVIF media profile token. `None` selects the device's first profile.
    pub onvif_profile: Option<&'a str>,
}

impl CameraConfig<'_> {
    /// The ONVIF device service endpoint for this camera or recorder.
    pub fn device_url(&self) -> String {
        format!("http://{}:{}/onvif/device_service", self.host, self.port)
    }
}

/// The stream information returned by [`resolve`].
///
/// `stream_uri_auth` contains the credentials used for the ONVIF request (for
/// RTSP sources). Do not log or persist it; use
/// [`stream_uri_redacted`](Self::stream_uri_redacted) in reports.
#[derive(Debug, Clone)]
pub struct ResolvedSource {
    /// Final stream URI, including credentials when the device returned RTSP.
    pub stream_uri_auth: String,
    /// The same URI with the userinfo component replaced by `***`.
    pub stream_uri_redacted: String,
    pub vendor: String,
    pub model: String,
    pub firmware: String,
    pub onvif_profile: String,
    pub time_source: String,
    /// `onvif-replay` or `rtsp-live`.
    pub extraction_mode: String,
    /// A short, stable description of how the URI was obtained.
    pub resolution_path: String,
    pub clock_skew_secs: Option<i64>,
}

/// Send one WS-Discovery probe and collect responses for `wait`.
///
/// The probe is a single UDP multicast datagram to
/// [`WS_DISCOVERY_ADDR`], sent from an ephemeral port. Devices must be on the
/// same layer-2 network as the caller for multicast to arrive.
///
/// When `network` is `Some`, it must be a valid IPv4 CIDR (for example
/// `192.168.1.0/24`). A response is kept only when at least one of its XAddrs
/// has an IPv4 host inside that network. Devices with no usable XAddr are
/// excluded by a filter. The filter is applied after the probe is received;
/// it does not narrow the multicast scope.
///
/// Discovery sends no credentials. Responses are not authenticated and are not
/// correlated to this probe's message id: any ProbeMatch seen during the window
/// is reported, so a host on the network can claim any XAddr, scope or type it
/// likes. A receive timeout completes the scan; any other socket error is
/// returned as an error rather than being reported as an empty result.
pub async fn discover(network: Option<&str>, wait: Duration) -> Result<Vec<DiscoveredDevice>> {
    // Validate before sending: a typo must not put traffic on the wire.
    let subnet: Option<Ipv4Net> = network
        .map(|n| {
            n.parse()
                .map_err(|_| DiscoveryError::config(format!("invalid network {n}")))
        })
        .transpose()?;

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;
    let probe = probe_xml();
    socket.send_to(probe.as_bytes(), WS_DISCOVERY_ADDR).await?;

    let mut devices = Vec::new();
    let started = tokio::time::Instant::now();
    // Saturate absurd waits rather than panicking on an overflowing deadline.
    let deadline = started
        .checked_add(wait)
        .unwrap_or_else(|| started + Duration::from_secs(365 * 24 * 60 * 60));
    let mut buf = vec![0u8; MAX_DATAGRAM];
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, _from))) => {
                if let Some(dev) = parse_probe_match(&String::from_utf8_lossy(&buf[..n])) {
                    if let Some(net) = subnet {
                        if !device_in_subnet(&dev, net) {
                            continue;
                        }
                    }
                    devices.push(dev);
                }
            }
            // No response before the deadline: the scan is complete.
            Err(_) => break,
            // A real socket failure is not "no devices found".
            Ok(Err(err)) => return Err(err.into()),
        }
    }
    Ok(devices)
}

/// Wall-clock replay query token: `20260914T221000Z` from `2026-09-14T22:10:00Z`.
///
/// This is a character filter, not a timestamp validator. Callers should pass
/// UTC RFC 3339 input.
pub fn format_nvr_export_url(template: &str, start_utc: &str, end_utc: &str) -> String {
    template
        .replace("{start}", &rtsp_time_token(start_utc))
        .replace("{end}", &rtsp_time_token(end_utc))
}

/// Strip separators from an RFC 3339 UTC timestamp for ONVIF/RTSP query use.
pub fn rtsp_time_token(utc_rfc3339: &str) -> String {
    utc_rfc3339
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T' || *c == 'Z')
        .collect()
}

/// Append ONVIF-style `starttime` / `endtime` range params for wall-clock RTSP replay.
///
/// Any query already present on `uri` is discarded.
pub fn with_rtsp_wall_clock_range(uri: &str, start_utc: &str, end_utc: &str) -> String {
    let base = uri.split('?').next().unwrap_or(uri);
    let start = rtsp_time_token(start_utc);
    let end = rtsp_time_token(end_utc);
    format!("{base}?starttime={start}&endtime={end}")
}

/// Resolve a camera's ONVIF media profile to a stream URI.
///
/// This performs the read-only ONVIF calls `GetDeviceInformation`,
/// `GetSystemDateAndTime`, `GetCapabilities`, `GetProfiles`, `GetStreamUri`,
/// and, when the device advertises replay, `GetReplayUri`. It never changes
/// device state.
///
/// The returned [`ResolvedSource::stream_uri_auth`] contains credentials for
/// RTSP sources. Treat it as a secret. The device is contacted over the
/// `http://` endpoint built by [`CameraConfig::device_url`]; ONVIF's password
/// digest protects the password value but does not encrypt the conversation.
/// Redirects and environment proxies are disabled; response bodies are capped
/// at 4 MiB. Service XAddrs returned by the device are followed as-is.
pub async fn resolve(
    camera: &CameraConfig<'_>,
    start_utc: &str,
    end_utc: &str,
) -> Result<ResolvedSource> {
    let soap = SoapClient::new()?;
    let device_url = camera.device_url();
    let info_xml = soap
        .get_device_information(&device_url, camera.username, camera.password)
        .await?;
    let skew = clock_skew_secs(&soap, &device_url, camera.username, camera.password).await;
    let vendor = xml_text(&info_xml, "Manufacturer").unwrap_or_else(|| "unknown".into());
    let model = xml_text(&info_xml, "Model").unwrap_or_else(|| "unknown".into());
    let firmware = xml_text(&info_xml, "FirmwareVersion").unwrap_or_else(|| "unknown".into());

    let media_url = media_service_url(&soap, &device_url, camera.username, camera.password)
        .await
        .unwrap_or_else(|_| format!("http://{}:{}/onvif/media_service", camera.host, camera.port));

    let profiles_xml = soap
        .get_profiles(&media_url, camera.username, camera.password)
        .await?;
    let tokens = xml_attrs(&profiles_xml, "Profiles", "token");
    let token = camera
        .onvif_profile
        .map(str::to_string)
        .or_else(|| tokens.first().cloned())
        .ok_or_else(|| DiscoveryError::fail(format!("camera {}: no ONVIF profiles", camera.id)))?;

    let stream_xml = soap
        .get_stream_uri(&media_url, camera.username, camera.password, &token)
        .await?;
    let uri = xml_text(&stream_xml, "Uri").ok_or_else(|| {
        DiscoveryError::fail(format!(
            "camera {}: GetStreamUri returned no Uri",
            camera.id
        ))
    })?;

    let auth_uri = inject_if_rtsp(&uri, camera.username, camera.password);
    let (stream_uri_auth, extraction_mode, resolution_path) = if let Some(replay_url) =
        replay_service_url(&soap, &device_url, camera.username, camera.password).await
    {
        match soap
            .get_replay_uri(&replay_url, camera.username, camera.password, &token)
            .await
        {
            Ok(replay_xml) => {
                if let Some(replay_uri) = xml_text(&replay_xml, "Uri") {
                    let ranged = with_rtsp_wall_clock_range(&replay_uri, start_utc, end_utc);
                    (
                        inject_if_rtsp(&ranged, camera.username, camera.password),
                        "onvif-replay".into(),
                        "onvif:GetReplayUri".into(),
                    )
                } else {
                    (
                        auth_uri.clone(),
                        "rtsp-live".into(),
                        "onvif:GetStreamUri+live".into(),
                    )
                }
            }
            Err(_) => (
                auth_uri.clone(),
                "rtsp-live".into(),
                "onvif:GetStreamUri+live".into(),
            ),
        }
    } else {
        (
            auth_uri.clone(),
            "rtsp-live".into(),
            "onvif:GetStreamUri+live".into(),
        )
    };

    let stream_uri_redacted = redact_uri(&stream_uri_auth);
    let time_source = match skew {
        Some(s) if s.abs() > 5 => format!("ntp; clock_skew_secs={s}"),
        Some(s) => format!("ntp; skew_ok={s}s"),
        None => "ntp".into(),
    };
    Ok(ResolvedSource {
        stream_uri_auth,
        stream_uri_redacted,
        vendor,
        model,
        firmware,
        onvif_profile: token,
        time_source,
        extraction_mode,
        resolution_path,
        clock_skew_secs: skew,
    })
}

// A plain rtsp_uri fallback has no declared replay capability: the caller must
// not append ONVIF wall-clock params to it. Servers that ignore them would hand
// us live bytes mislabeled as a past window; servers that reject them break the
// stream outright.
/// Mark a plain RTSP URI as live-only; no replay range is appended.
pub fn rtsp_live_fallback(auth_uri: &str) -> (String, String) {
    (auth_uri.to_string(), "rtsp-live".into())
}

async fn replay_service_url(
    soap: &SoapClient,
    device_url: &str,
    user: &str,
    pass: &str,
) -> Option<String> {
    let xml = soap.get_capabilities(device_url, user, pass).await.ok()?;
    xml_text(&xml, "Replay").or_else(|| {
        // Capabilities often nest Replay XAddr inside a block; scan for replay path.
        xml.lines()
            .find(|l| l.to_ascii_lowercase().contains("replay"))
            .and_then(|l| {
                if l.contains("http") {
                    xml_text(l, "XAddr")
                } else {
                    None
                }
            })
    })
}

/// Add RTSP userinfo to an RTSP URI when it has none.
///
/// The values are inserted verbatim; this helper does not percent-encode
/// reserved characters. An empty username and password leaves the URI alone.
pub fn inject_if_rtsp(uri: &str, user: &str, pass: &str) -> String {
    if user.is_empty() && pass.is_empty() {
        return uri.to_string();
    }
    if uri.starts_with("rtsp://") && !uri.contains('@') {
        if let Some(rest) = uri.strip_prefix("rtsp://") {
            return format!("rtsp://{user}:{pass}@{rest}");
        }
    }
    uri.to_string()
}

async fn media_service_url(
    soap: &SoapClient,
    device_url: &str,
    user: &str,
    pass: &str,
) -> Result<String> {
    let xml = soap.get_capabilities(device_url, user, pass).await?;
    xml_text(&xml, "XAddr")
        .or_else(|| xml_text(&xml, "Media"))
        .ok_or_else(|| DiscoveryError::fail("GetCapabilities returned no media XAddr"))
}

fn probe_xml() -> String {
    let uuid = uuid::Uuid::now_v7();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dn="http://www.onvif.org/ver10/network/wsdl">
  <s:Header>
    <a:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action>
    <a:MessageID>uuid:{uuid}</a:MessageID>
    <a:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</a:To>
  </s:Header>
  <s:Body>
    <d:Probe>
      <d:Types>dn:NetworkVideoTransmitter</d:Types>
    </d:Probe>
  </s:Body>
</s:Envelope>"#
    )
}

/// Parse one WS-Discovery ProbeMatch datagram.
///
/// Returns `None` when the payload does not look like a ProbeMatch. The parser
/// is intentionally small: it is a case-insensitive substring check followed
/// by tag extraction, not a conforming XML parser. See the known limits in
/// `docs/SPEC.md`.
pub fn parse_probe_match(xml: &str) -> Option<DiscoveredDevice> {
    if !xml.to_ascii_lowercase().contains("probematch") {
        return None;
    }
    let xaddrs = xml_text(xml, "XAddrs")
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();
    Some(DiscoveredDevice {
        xaddrs,
        scopes: xml_text(xml, "Scopes").unwrap_or_default(),
        types: xml_text(xml, "Types").unwrap_or_default(),
    })
}

fn device_in_subnet(dev: &DiscoveredDevice, net: Ipv4Net) -> bool {
    dev.xaddrs.iter().any(|addr| {
        host_from_url(addr)
            .and_then(|h| h.parse::<Ipv4Addr>().ok())
            .map(|ip| net.contains(&ip))
            .unwrap_or(false)
    })
}

/// Extract the host from a URL authority.
///
/// This is a small helper for the `http://host:port/path` XAddrs ONVIF
/// devices return. It does not parse IPv6 literals and does not remove
/// userinfo. See the known limits in `docs/SPEC.md`.
pub fn host_from_url(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let hostport = rest.split('/').next()?;
    let host = hostport.split(':').next()?;
    Some(host.to_string())
}

/// Port from a URL authority — explicit port, else scheme default (https 443, else 80).
///
/// Like [`host_from_url`], this is a small helper for ONVIF XAddrs, not a
/// general URL parser.
pub fn port_from_url(url: &str) -> u16 {
    let scheme_default = if url.starts_with("https://") { 443 } else { 80 };
    let Some(rest) = url.split("://").nth(1) else {
        return scheme_default;
    };
    let Some(hostport) = rest.split('/').next() else {
        return scheme_default;
    };
    hostport
        .split(':')
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(scheme_default)
}

/// Extract the text of the first element named `tag`.
///
/// Accepts both `<Tag>value</Tag>` and namespace-prefixed forms such as
/// `<d:Tag>value</d:Tag>`. This is a substring parser for the small,
/// device-generated documents ONVIF returns; it is not a general XML parser.
pub fn xml_text(xml: &str, tag: &str) -> Option<String> {
    xml_text_plain(xml, tag).or_else(|| xml_text_ns(xml, tag))
}

fn xml_text_plain(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let after = xml[start..].find('>')? + start + 1;
    let end = xml[after..].find(&close)? + after;
    let inner = xml[after..end].trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

fn xml_text_ns(xml: &str, tag: &str) -> Option<String> {
    let needle = format!("{tag}>");
    let mut search = xml;
    while let Some(idx) = search.find(&needle) {
        let after = idx + needle.len();
        if search[..idx].ends_with('<') || search[..idx].ends_with(':') {
            let rest = &search[after..];
            if let Some(end) = rest.find('<') {
                let inner = rest[..end].trim();
                if !inner.is_empty() {
                    return Some(inner.to_string());
                }
            }
        }
        search = &search[after..];
    }
    None
}

fn xml_attrs(xml: &str, tag: &str, attr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = xml;
    let open = format!("<{tag}");
    while let Some(idx) = search.find(&open).or_else(|| {
        search
            .find(&format!(":{tag}"))
            .filter(|&i| search[i..].contains(attr))
    }) {
        let slice = &search[idx..];
        if let Some(end) = slice.find('>') {
            let tag_src = &slice[..end];
            if let Some(val) = attr_value(tag_src, attr) {
                out.push(val);
            }
            search = &slice[end + 1..];
        } else {
            break;
        }
    }
    out
}

fn attr_value(tag_src: &str, attr: &str) -> Option<String> {
    let key = format!("{attr}=\"");
    let start = tag_src.find(&key)? + key.len();
    let end = tag_src[start..].find('"')? + start;
    Some(tag_src[start..end].to_string())
}

async fn clock_skew_secs(
    soap: &SoapClient,
    device_url: &str,
    user: &str,
    pass: &str,
) -> Option<i64> {
    let xml = soap
        .get_system_date_and_time(device_url, user, pass)
        .await
        .ok()?;
    let year = xml_text(&xml, "Year")?.parse::<i32>().ok()?;
    let month = xml_text(&xml, "Month")?.parse::<u32>().ok()?;
    let day = xml_text(&xml, "Day")?.parse::<u32>().ok()?;
    let hour = xml_text(&xml, "Hour")?.parse::<u32>().ok()?;
    let minute = xml_text(&xml, "Minute")?.parse::<u32>().ok()?;
    let second = xml_text(&xml, "Second")?.parse::<u32>().ok()?;
    let naive =
        chrono::NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, second)?;
    let camera = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc);
    Some((chrono::Utc::now() - camera).num_seconds())
}

/// Replace the userinfo in a URL with `***`, preserving scheme, host, path
/// and query. If the URL has no userinfo, it is returned unchanged.
pub fn redact_uri(uri: &str) -> String {
    if let Some(scheme_end) = uri.find("://") {
        let scheme = &uri[..scheme_end + 3];
        let rest = &uri[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
            return format!("{scheme}***@{}", &rest[at + 1..]);
        }
    }
    uri.to_string()
}
