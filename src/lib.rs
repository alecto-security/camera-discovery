//! ONVIF WS-Discovery, SOAP source resolution and reporting for cameras and
//! NVRs.
//!
//! The crate is deliberately narrow:
//!
//! - [`discover`] sends a WS-Discovery probe and returns the devices that
//!   answer.
//! - [`resolve`] asks one device for its media profile and stream URIs.
//! - [`report`] renders the discovery result as text or JSON.
//!
//! Nothing here writes to a camera or recorder. Discovery is unauthenticated;
//! resolution sends the supplied ONVIF credentials to the device. See
//! [`docs/SPEC.md`](../docs/SPEC.md) in the repository for the output and
//! safety contract.
#![forbid(unsafe_code)]

pub mod error;
mod onvif;
pub mod report;
mod soap;

pub use error::{DiscoveryError, Result};
pub use onvif::{
    discover, format_nvr_export_url, host_from_url, inject_if_rtsp, parse_probe_match,
    port_from_url, redact_uri, resolve, rtsp_live_fallback, rtsp_time_token,
    with_rtsp_wall_clock_range, xml_text, CameraConfig, DiscoveredDevice, ResolvedSource,
    WS_DISCOVERY_ADDR,
};
pub use report::{devices_json, render_json, render_text};
