use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
    Engine as _,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Read,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub title: String,
    pub announcement: String,
    pub support_url: String,
    pub web_page_url: String,
    pub update_hours: Option<u64>,
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    pub expires: Option<u64>,
    pub updated_at: u64,
}
fn text(value: &str, encoded: bool) -> String {
    let decoded = value
        .strip_prefix("base64:")
        .or(if encoded { Some(value) } else { None })
        .and_then(|s| {
            STANDARD
                .decode(s)
                .or_else(|_| STANDARD_NO_PAD.decode(s))
                .ok()
        })
        .and_then(|b| String::from_utf8(b).ok());
    decoded
        .as_deref()
        .unwrap_or(value)
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(4000)
        .collect()
}
fn https(value: &str) -> String {
    url::Url::parse(value)
        .ok()
        .filter(|u| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
        .map(|u| u.to_string())
        .unwrap_or_default()
}
fn headers(get: impl Fn(&str) -> Option<String>) -> Metadata {
    let usage = get("subscription-userinfo").unwrap_or_default();
    let fields: HashMap<_, _> = usage
        .split(';')
        .filter_map(|s| s.trim().split_once('='))
        .filter_map(|(k, v)| v.trim().parse::<u64>().ok().map(|n| (k.trim(), n)))
        .collect();
    Metadata {
        title: text(&get("profile-title").unwrap_or_default(), false),
        announcement: text(&get("announce").unwrap_or_default(), true),
        support_url: https(&get("support-url").unwrap_or_default()),
        web_page_url: https(&get("profile-web-page-url").unwrap_or_default()),
        update_hours: get("profile-update-interval")
            .and_then(|s| s.parse().ok())
            .filter(|n| *n > 0),
        upload: fields.get("upload").copied(),
        download: fields.get("download").copied(),
        total: fields.get("total").copied(),
        expires: fields.get("expire").copied(),
        updated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}
pub fn fetch(url: &str, hwid: &str) -> Result<(String, Metadata), String> {
    if !crate::hwid::valid(hwid) {
        return Err(crate::hwid::UNAVAILABLE.into());
    }
    if https(url).is_empty() {
        return Err("Use a secure subscription link.".into());
    }
    let tls = native_tls::TlsConnector::new().map_err(|_| "Could not connect. Try again later.")?;
    let agent = ureq::AgentBuilder::new()
        .tls_connector(Arc::new(tls))
        .timeout(Duration::from_secs(15))
        .build();
    let response = agent
        .get(url)
        .set(
            "User-Agent",
            concat!(
                "mihomo akimov-vpn/",
                env!("CARGO_PKG_VERSION"),
                " (windows)"
            ),
        )
        .set("x-hwid", hwid)
        .set("x-device-os", "Windows")
        .set("x-ver-os", "10")
        .set("x-device-model", "Windows PC")
        .call()
        .map_err(|_| "Could not update the subscription. Try again later.")?;
    let metadata = headers(|name| response.header(name).map(str::to_string));
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read the subscription. Try again later.")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("This subscription is too large.".into());
    }
    Ok((
        String::from_utf8(bytes).map_err(|_| "The subscription could not be read.")?,
        metadata,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remnawave_headers_decode_without_treating_unlimited_as_missing() {
        let values = HashMap::from([
            ("profile-title", "base64:VGVzdCBWUE4="),
            ("announce", "VXBkYXRlZCE="),
            (
                "subscription-userinfo",
                "upload=12; download=34; total=0; expire=0",
            ),
            ("profile-update-interval", "1"),
            ("support-url", "javascript:alert(1)"),
        ]);
        let m = headers(|k| values.get(k).map(|s| s.to_string()));
        assert_eq!(m.title, "Test VPN");
        assert_eq!(m.announcement, "Updated!");
        assert_eq!(m.total, Some(0));
        assert_eq!(m.download, Some(34));
        assert_eq!(m.expires, Some(0));
        assert_eq!(m.update_hours, Some(1));
        assert!(m.support_url.is_empty());
        assert_eq!(text("base64:VGVzdA", false), "Test");
    }
    #[test]
    fn malformed_headers_are_optional_and_plain_titles_survive() {
        assert!(fetch("https://example.invalid", "")
            .err()
            .is_some_and(|e| e == crate::hwid::UNAVAILABLE));
        let m = headers(|k| match k {
            "profile-title" => Some("My profile".into()),
            "subscription-userinfo" => Some("total=-1;expire=bad".into()),
            _ => None,
        });
        assert_eq!(m.title, "My profile");
        assert_eq!(m.total, None);
        assert_eq!(m.expires, None);
    }
}
