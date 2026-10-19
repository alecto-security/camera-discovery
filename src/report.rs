//! Presentation for discovery results.
//!
//! The functions here are the output contract of the `camera-discovery` CLI:
//! [`render_text`] for humans, [`render_json`] for machines. Both are pure and
//! leave the caller to decide where the bytes go.

use serde_json::{json, Value};

use crate::onvif::DiscoveredDevice;

/// One JSON object per device: `{xaddrs, scopes, types}`.
pub fn devices_json(devices: &[DiscoveredDevice]) -> Vec<Value> {
    devices
        .iter()
        .map(|d| {
            json!({
                "xaddrs": d.xaddrs,
                "scopes": d.scopes,
                "types": d.types,
            })
        })
        .collect()
}

/// Pretty JSON array, as `camera-discovery --json` prints it.
pub fn render_json(devices: &[DiscoveredDevice]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&devices_json(devices))
}

/// Human output, without the trailing newline: one XAddr per line followed by
/// its scopes when present. Empty result prints one flat sentence.
pub fn render_text(devices: &[DiscoveredDevice]) -> String {
    if devices.is_empty() {
        return "No ONVIF devices found.".into();
    }
    let mut out = String::new();
    for (i, d) in devices.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&d.xaddrs.join(" "));
        if !d.scopes.is_empty() {
            out.push('\n');
            out.push_str("  ");
            out.push_str(&d.scopes);
        }
    }
    out
}
