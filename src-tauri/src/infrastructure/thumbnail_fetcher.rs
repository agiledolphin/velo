use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{Client, header};
use serde::Serialize;
use url::Url;

const MAX_THUMBNAIL_BYTES: usize = 5 * 1024 * 1024;
const MAX_REDIRECTS: usize = 3;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
pub struct ThumbnailFetcher;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ThumbnailError {
    pub code: &'static str,
    pub message: &'static str,
}

impl ThumbnailError {
    fn unavailable() -> Self {
        Self {
            code: "thumbnail_unavailable",
            message: "暂时无法加载视频封面。",
        }
    }

    fn invalid_url() -> Self {
        Self {
            code: "invalid_thumbnail_url",
            message: "视频封面地址无效。",
        }
    }

    fn connection_failed() -> Self {
        Self {
            code: "thumbnail_connection_failed",
            message: "无法连接到视频封面服务器。",
        }
    }

    fn unsupported_response() -> Self {
        Self {
            code: "unsupported_thumbnail_response",
            message: "视频封面格式不受支持。",
        }
    }

    fn too_large() -> Self {
        Self {
            code: "thumbnail_too_large",
            message: "视频封面文件过大。",
        }
    }
}

impl ThumbnailFetcher {
    pub async fn fetch_data_url(&self, source: &str) -> Result<String, ThumbnailError> {
        let mut url = parse_public_web_url(source)?;

        for redirect_count in 0..=MAX_REDIRECTS {
            let response = send_request(&url).await?;
            if response.status().is_redirection() {
                if redirect_count == MAX_REDIRECTS {
                    return Err(ThumbnailError::unavailable());
                }
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(ThumbnailError::unavailable)?;
                url = parse_public_web_url(
                    url.join(location)
                        .map_err(|_| ThumbnailError::invalid_url())?
                        .as_str(),
                )?;
                continue;
            }

            if !response.status().is_success() {
                return Err(ThumbnailError::unavailable());
            }

            return response_to_data_url(response).await;
        }

        Err(ThumbnailError::unavailable())
    }
}

async fn send_request(url: &Url) -> Result<reqwest::Response, ThumbnailError> {
    let host = url.host_str().ok_or_else(ThumbnailError::invalid_url)?;
    let port = url
        .port_or_known_default()
        .ok_or_else(ThumbnailError::invalid_url)?;
    let addresses = resolve_public_addresses(host, port).await?;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .user_agent("Velo/0.1 thumbnail fetcher")
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| ThumbnailError::connection_failed())?;

    client
        .get(url.clone())
        .header(
            header::ACCEPT,
            "image/avif,image/webp,image/png,image/jpeg,image/gif",
        )
        .send()
        .await
        .map_err(|_| ThumbnailError::connection_failed())
}

async fn resolve_public_addresses(
    host: &str,
    port: u16,
) -> Result<Vec<SocketAddr>, ThumbnailError> {
    if let Ok(address) = host.parse::<IpAddr>() {
        return is_public_ip(address)
            .then_some(vec![SocketAddr::new(address, port)])
            .ok_or_else(ThumbnailError::invalid_url);
    }

    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| ThumbnailError::unavailable())?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(ThumbnailError::invalid_url());
    }

    Ok(addresses)
}

async fn response_to_data_url(mut response: reqwest::Response) -> Result<String, ThumbnailError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_THUMBNAIL_BYTES as u64)
    {
        return Err(ThumbnailError::too_large());
    }

    let mime = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(allowed_image_mime)
        .ok_or_else(ThumbnailError::unsupported_response)?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ThumbnailError::unavailable())?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_THUMBNAIL_BYTES {
            return Err(ThumbnailError::too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err(ThumbnailError::unavailable());
    }

    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

fn allowed_image_mime(content_type: &str) -> Option<&'static str> {
    match content_type
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/avif" => Some("image/avif"),
        "image/webp" => Some("image/webp"),
        "image/png" => Some("image/png"),
        "image/jpeg" => Some("image/jpeg"),
        "image/gif" => Some("image/gif"),
        _ => None,
    }
}

fn parse_public_web_url(source: &str) -> Result<Url, ThumbnailError> {
    let url = Url::parse(source).map_err(|_| ThumbnailError::invalid_url())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ThumbnailError::invalid_url());
    }
    Ok(url)
}

fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    !(address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || address.is_unspecified()
        || first == 0
        || (first == 100 && (64..=127).contains(&second))
        || (first == 192 && second == 0)
        || (first == 198 && second == 51)
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 203 && second == 0)
        || first >= 240)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(address) = address.to_ipv4_mapped() {
        return is_public_ipv4(address);
    }
    let segments = address.segments();
    !(address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_safe_thumbnail_urls() {
        assert!(parse_public_web_url("https://cdn.example/thumb.jpg").is_ok());
        assert!(parse_public_web_url("file:///tmp/thumb.jpg").is_err());
        assert!(parse_public_web_url("https://user:secret@example.com/thumb.jpg").is_err());
    }

    #[test]
    fn rejects_local_and_reserved_networks() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.1.1",
            "192.168.1.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!is_public_ip(
                address.parse().expect("address should parse")
            ));
        }
        assert!(is_public_ip(
            "1.1.1.1".parse().expect("address should parse")
        ));
        assert!(is_public_ip(
            "2606:4700:4700::1111"
                .parse()
                .expect("address should parse")
        ));
    }

    #[test]
    fn accepts_only_supported_image_mime_types() {
        assert_eq!(
            allowed_image_mime("image/jpeg; charset=binary"),
            Some("image/jpeg")
        );
        assert_eq!(allowed_image_mime("image/svg+xml"), None);
        assert_eq!(allowed_image_mime("text/html"), None);
    }

    #[tokio::test]
    #[ignore = "requires an explicit public image URL and network access"]
    async fn fetches_configured_real_thumbnail() {
        let url = std::env::var("VELO_INTEGRATION_THUMBNAIL_URL")
            .expect("VELO_INTEGRATION_THUMBNAIL_URL must be set explicitly");
        let data_url = ThumbnailFetcher
            .fetch_data_url(&url)
            .await
            .expect("configured thumbnail should load");

        assert!(data_url.starts_with("data:image/"));
        assert!(data_url.len() <= MAX_THUMBNAIL_BYTES * 2);
    }
}
