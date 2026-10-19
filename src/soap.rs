use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use sha1::{Digest, Sha1};

use crate::error::{DiscoveryError, Result};

/// ONVIF responses are small. Cap the body so a hostile or broken device
/// cannot make the client allocate without bound.
const MAX_SOAP_RESPONSE: usize = 4 * 1024 * 1024;

/// Minimal ONVIF SOAP client for device, media and replay services.
///
/// The client is read-only: every method builds an ONVIF request that queries
/// state. It does not expose a generic mutation call. Per-request timeouts are
/// 8 seconds; redirects and environment proxies are disabled so credentials
/// only go to the endpoint the caller named.
pub struct SoapClient {
    http: reqwest::Client,
}

impl SoapClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()?,
        })
    }

    pub async fn call(&self, url: &str, user: &str, pass: &str, body: &str) -> Result<String> {
        let envelope = soap_envelope(user, pass, body);
        let mut resp = self
            .http
            .post(url)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .body(envelope)
            .send()
            .await?;
        let status = resp.status();
        if let Some(len) = resp.content_length() {
            if len > MAX_SOAP_RESPONSE as u64 {
                return Err(DiscoveryError::fail(format!(
                    "ONVIF response from {url} is larger than {MAX_SOAP_RESPONSE} bytes"
                )));
            }
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            extend_capped(&mut bytes, &chunk, MAX_SOAP_RESPONSE)?;
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if !status.is_success() {
            return Err(DiscoveryError::fail(format!(
                "ONVIF {url} HTTP {status}: {text}"
            )));
        }
        Ok(text)
    }

    pub async fn get_device_information(
        &self,
        device_url: &str,
        user: &str,
        pass: &str,
    ) -> Result<String> {
        self.call(
            device_url,
            user,
            pass,
            r#"<GetDeviceInformation xmlns="http://www.onvif.org/ver10/device/wsdl"/>"#,
        )
        .await
    }

    pub async fn get_capabilities(
        &self,
        device_url: &str,
        user: &str,
        pass: &str,
    ) -> Result<String> {
        self.call(
            device_url,
            user,
            pass,
            r#"<GetCapabilities xmlns="http://www.onvif.org/ver10/device/wsdl"><Category>All</Category></GetCapabilities>"#,
        )
        .await
    }

    pub async fn get_profiles(&self, media_url: &str, user: &str, pass: &str) -> Result<String> {
        self.call(
            media_url,
            user,
            pass,
            r#"<GetProfiles xmlns="http://www.onvif.org/ver10/media/wsdl"/>"#,
        )
        .await
    }

    pub async fn get_stream_uri(
        &self,
        media_url: &str,
        user: &str,
        pass: &str,
        profile_token: &str,
    ) -> Result<String> {
        let body = format!(
            r#"<GetStreamUri xmlns="http://www.onvif.org/ver10/media/wsdl">
  <StreamSetup>
    <Stream xmlns="http://www.onvif.org/ver10/schema">RTP-Unicast</Stream>
    <Transport xmlns="http://www.onvif.org/ver10/schema"><Protocol>RTSP</Protocol></Transport>
  </StreamSetup>
  <ProfileToken>{}</ProfileToken>
</GetStreamUri>"#,
            xml_escape(profile_token)
        );
        self.call(media_url, user, pass, &body).await
    }

    pub async fn get_replay_uri(
        &self,
        replay_url: &str,
        user: &str,
        pass: &str,
        profile_token: &str,
    ) -> Result<String> {
        let body = format!(
            r#"<GetReplayUri xmlns="http://www.onvif.org/ver10/replay/wsdl">
  <ProfileToken>{}</ProfileToken>
</GetReplayUri>"#,
            xml_escape(profile_token)
        );
        self.call(replay_url, user, pass, &body).await
    }

    pub async fn get_system_date_and_time(
        &self,
        device_url: &str,
        user: &str,
        pass: &str,
    ) -> Result<String> {
        self.call(
            device_url,
            user,
            pass,
            r#"<GetSystemDateAndTime xmlns="http://www.onvif.org/ver10/device/wsdl"/>"#,
        )
        .await
    }
}

fn soap_envelope(user: &str, pass: &str, body: &str) -> String {
    let mut nonce = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce);
    let created = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let digest = password_digest(pass, &nonce, &created);
    let nonce_b64 = B64.encode(nonce);
    let user = xml_escape(user);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:wsse="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd" xmlns:wsu="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd">
  <s:Header>
    <wsse:Security>
      <wsse:UsernameToken>
        <wsse:Username>{user}</wsse:Username>
        <wsse:Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">{digest}</wsse:Password>
        <wsse:Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">{nonce_b64}</wsse:Nonce>
        <wsu:Created>{created}</wsu:Created>
      </wsse:UsernameToken>
    </wsse:Security>
  </s:Header>
  <s:Body>{body}</s:Body>
</s:Envelope>"#
    )
}

fn password_digest(password: &str, nonce: &[u8], created: &str) -> String {
    let mut h = Sha1::new();
    h.update(nonce);
    h.update(created.as_bytes());
    h.update(password.as_bytes());
    B64.encode(h.finalize())
}

fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn extend_capped(body: &mut Vec<u8>, chunk: &[u8], max: usize) -> Result<()> {
    if body.len().saturating_add(chunk.len()) > max {
        return Err(DiscoveryError::fail(format!(
            "ONVIF response is larger than {max} bytes"
        )));
    }
    body.extend_from_slice(chunk);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_neutralizes_markup() {
        assert_eq!(
            xml_escape(r#"a&b<c>d"e'f"#),
            "a&amp;b&lt;c&gt;d&quot;e&apos;f"
        );
    }

    #[test]
    fn envelope_escapes_the_username_but_not_the_password() {
        let envelope = soap_envelope("svc<&", "p<&", "<Body/>");
        assert!(envelope.contains("<wsse:Username>svc&lt;&amp;</wsse:Username>"));
        assert!(!envelope.contains("p<&"));
        assert!(envelope.contains("<s:Body><Body/></s:Body>"));
    }

    #[test]
    fn response_body_is_capped() {
        let mut body = Vec::new();
        extend_capped(&mut body, b"abc", 4).expect("fits");
        assert_eq!(body, b"abc");
        extend_capped(&mut body, b"d", 4).expect("fits exactly");
        let err = extend_capped(&mut body, b"e", 4).expect_err("over cap");
        assert!(err.to_string().contains("larger than 4 bytes"));
    }
}
