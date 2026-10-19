use std::time::Duration;

use alecto_discovery::{
    devices_json, discover, format_nvr_export_url, host_from_url, inject_if_rtsp,
    parse_probe_match, port_from_url, redact_uri, render_json, render_text, rtsp_time_token,
    with_rtsp_wall_clock_range, xml_text, CameraConfig, DiscoveryError,
};

const PROBE_MATCH: &str = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery">
  <s:Body>
    <d:ProbeMatches>
      <d:ProbeMatch>
        <d:Types>dn:NetworkVideoTransmitter</d:Types>
        <d:Scopes>onvif://www.onvif.org/name/CAM onvif://www.onvif.org/type/video_encoder</d:Scopes>
        <d:XAddrs>http://192.168.1.64/onvif/device_service http://[2001:db8::1]/onvif/device_service</d:XAddrs>
      </d:ProbeMatch>
    </d:ProbeMatches>
  </s:Body>
</s:Envelope>"#;

#[test]
fn probe_match_parses_xaddrs_scopes_and_types() {
    let dev = parse_probe_match(PROBE_MATCH).expect("probe match");
    assert_eq!(
        dev.xaddrs,
        vec![
            "http://192.168.1.64/onvif/device_service",
            "http://[2001:db8::1]/onvif/device_service"
        ]
    );
    assert!(dev.scopes.contains("onvif://www.onvif.org/name/CAM"));
    assert_eq!(dev.types, "dn:NetworkVideoTransmitter");
}

#[test]
fn non_probe_payload_is_ignored() {
    assert!(parse_probe_match("<Envelope><Body>Hello</Body></Envelope>").is_none());
    assert!(parse_probe_match("").is_none());
}

#[test]
fn probe_match_without_xaddrs_still_reports_the_device() {
    let xml = "<d:ProbeMatch><d:Scopes>scope</d:Scopes></d:ProbeMatch>";
    let dev = parse_probe_match(xml).expect("probe match");
    assert!(dev.xaddrs.is_empty());
    assert_eq!(dev.scopes, "scope");
}

#[test]
fn xml_text_handles_plain_and_namespaced_tags() {
    assert_eq!(
        xml_text("<XAddrs>http://cam/onvif/device_service</XAddrs>", "XAddrs").as_deref(),
        Some("http://cam/onvif/device_service")
    );
    assert_eq!(
        xml_text("<d:Manufacturer>Axis</d:Manufacturer>", "Manufacturer").as_deref(),
        Some("Axis")
    );
    assert!(xml_text("<XAddrs></XAddrs>", "XAddrs").is_none());
    assert!(xml_text("<Other>value</Other>", "XAddrs").is_none());
}

#[test]
fn url_helpers_follow_the_xaddr_shape() {
    assert_eq!(
        host_from_url("http://192.168.1.64:8080/onvif/device_service").as_deref(),
        Some("192.168.1.64")
    );
    assert_eq!(
        host_from_url("https://nvr.example/onvif/device_service").as_deref(),
        Some("nvr.example")
    );
    assert_eq!(
        port_from_url("http://192.168.1.64:8080/onvif/device_service"),
        8080
    );
    assert_eq!(
        port_from_url("http://192.168.1.64/onvif/device_service"),
        80
    );
    assert_eq!(
        port_from_url("https://192.168.1.64/onvif/device_service"),
        443
    );
    assert_eq!(port_from_url("not-a-url"), 80);
}

#[test]
fn credentials_are_injected_and_redacted() {
    let uri = inject_if_rtsp("rtsp://192.168.1.64/stream", "svc", "secret");
    assert_eq!(uri, "rtsp://svc:secret@192.168.1.64/stream");
    assert_eq!(redact_uri(&uri), "rtsp://***@192.168.1.64/stream");

    let already_authed = "rtsp://other:pass@192.168.1.64/stream";
    assert_eq!(
        inject_if_rtsp(already_authed, "svc", "secret"),
        already_authed
    );
    assert_eq!(
        inject_if_rtsp("http://192.168.1.64/stream", "svc", "secret"),
        "http://192.168.1.64/stream"
    );
    assert_eq!(
        inject_if_rtsp("rtsp://192.168.1.64/stream", "", ""),
        "rtsp://192.168.1.64/stream"
    );
}

#[test]
fn wall_clock_helpers_use_compact_utc_tokens() {
    assert_eq!(rtsp_time_token("2026-09-14T22:10:00Z"), "20260914T221000Z");

    let export = format_nvr_export_url(
        "http://192.168.1.100/export?start={start}&end={end}",
        "2026-09-14T22:10:00Z",
        "2026-09-14T22:35:00Z",
    );
    assert_eq!(
        export,
        "http://192.168.1.100/export?start=20260914T221000Z&end=20260914T223500Z"
    );

    let range = with_rtsp_wall_clock_range(
        "rtsp://192.168.1.64/Streaming/Channels/101?old=1",
        "2026-09-14T22:10:00Z",
        "2026-09-14T22:35:00Z",
    );
    assert_eq!(
        range,
        "rtsp://192.168.1.64/Streaming/Channels/101?starttime=20260914T221000Z&endtime=20260914T223500Z"
    );
}

#[test]
fn device_url_is_the_onvif_device_service() {
    let config = CameraConfig {
        id: "CAM-W01",
        host: "192.168.1.64",
        port: 8080,
        username: "svc",
        password: "secret",
        onvif_profile: None,
    };
    assert_eq!(
        config.device_url(),
        "http://192.168.1.64:8080/onvif/device_service"
    );
}

#[test]
fn text_report_is_stable() {
    assert_eq!(render_text(&[]), "No ONVIF devices found.");

    let devices = parse_probe_match(PROBE_MATCH).map(|d| vec![d]).unwrap();
    assert_eq!(
        render_text(&devices),
        "http://192.168.1.64/onvif/device_service http://[2001:db8::1]/onvif/device_service\n  onvif://www.onvif.org/name/CAM onvif://www.onvif.org/type/video_encoder"
    );
}

#[test]
fn json_report_matches_the_documented_shape() {
    let devices = parse_probe_match(PROBE_MATCH).map(|d| vec![d]).unwrap();
    let json = render_json(&devices).expect("json");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let array = value.as_array().expect("array");
    assert_eq!(array.len(), 1);
    assert_eq!(
        array[0]["xaddrs"][0].as_str(),
        Some("http://192.168.1.64/onvif/device_service")
    );
    assert_eq!(
        array[0]["types"].as_str(),
        Some("dn:NetworkVideoTransmitter")
    );
    assert_eq!(devices_json(&devices)[0]["scopes"], array[0]["scopes"]);
}

#[tokio::test]
async fn invalid_network_is_rejected_before_any_probe() {
    let err = discover(Some("not-a-cidr"), Duration::from_secs(1))
        .await
        .expect_err("invalid CIDR");
    assert!(matches!(err, DiscoveryError::Config(_)));
    assert!(err.to_string().contains("invalid network not-a-cidr"));
}
