use super::{ResourceFile, manifest, remote_url};
use anyhow::{Context as _, ensure};
use futures::AsyncReadExt as _;
use gpui_kit::http_client::{HttpClient, HttpRequestExt as _, RedirectPolicy, Request};
use std::{sync::Arc, time::Duration};

mod client;
pub(super) use client::Client;

pub(super) async fn download(
    client: Arc<dyn HttpClient>,
    file: &ResourceFile,
) -> anyhow::Result<Vec<u8>> {
    let url = remote_url(&manifest()?.storage, file)?;
    fetch(client, file, url, super::credentials::token()).await
}

async fn fetch(
    client: Arc<dyn HttpClient>,
    file: &ResourceFile,
    url: String,
    token: Option<String>,
) -> anyhow::Result<Vec<u8>> {
    let mut builder = Request::builder()
        .method("GET")
        .uri(url)
        .follow_redirects(RedirectPolicy::FollowLimit(8))
        .timeout(Duration::from_secs(120));
    if let Some(token) = token {
        builder = builder.header("Authorization", format!("Bearer {token}"));
    }
    let request = builder.body(().into())?;
    // HttpClient strips sensitive headers on cross-origin redirects. Do not expose
    // signed CDN URLs or response bodies in errors shown to the user.
    let mut response = client
        .send(request)
        .await
        .map_err(|_| anyhow::anyhow!("资源服务器暂时无法连接"))?;
    let status = response.status().as_u16();
    ensure!(
        status != 401 && status != 403,
        "私有资源仓库需要本机已配置的访问凭据"
    );
    ensure!(status != 404, "所需资源尚未上传或资源路径不存在");
    ensure!(status != 429, "资源服务器请求受限，请稍后重试");
    ensure!(
        response.status().is_success(),
        "资源服务器返回错误（HTTP {status}）"
    );
    if let Some(length) = response.headers().get("Content-Length") {
        let length: u64 = length.to_str()?.parse()?;
        ensure!(length == file.bytes, "远程资源长度与清单不一致");
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .take(file.bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .await
        .context("资源读取中断")?;
    ensure!(super::verified(file, &bytes), "远程资源校验失败");
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{executor::block_on, future::BoxFuture};
    use gpui_kit::http_client::{AsyncBody, Response, Url, http::HeaderValue};
    use sha2::{Digest as _, Sha256};

    struct ResponseClient {
        status: u16,
        length: Option<&'static str>,
        bytes: Vec<u8>,
    }
    impl HttpClient for ResponseClient {
        fn user_agent(&self) -> Option<&HeaderValue> {
            None
        }
        fn proxy(&self) -> Option<&Url> {
            None
        }
        fn send(
            &self,
            request: Request<AsyncBody>,
        ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
            assert_eq!(request.method(), "GET");
            assert_eq!(request.uri().path(), "/one-product/one-file.svg");
            assert!(request.headers().get("Authorization").is_none());
            let mut response = Response::builder().status(self.status);
            if let Some(length) = self.length {
                response = response.header("Content-Length", length);
            }
            let result = response.body(self.bytes.clone().into()).map_err(Into::into);
            Box::pin(async move { result })
        }
    }

    #[test]
    fn individual_download_requires_success_exact_length_and_matching_content() {
        let bytes = b"verified";
        let file = ResourceFile {
            path: "assets/synapse/products/756/one-file.svg".into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        };
        for (status, length, payload, expected) in [
            (200, Some("8"), bytes.to_vec(), true),
            (200, None, bytes.to_vec(), true),
            (200, Some("7"), bytes.to_vec(), false),
            (200, None, b"truncated".to_vec(), false),
            (200, Some("8"), b"modified".to_vec(), false),
            (401, None, b"private response".to_vec(), false),
            (403, None, Vec::new(), false),
            (404, None, Vec::new(), false),
            (429, None, Vec::new(), false),
            (500, None, b"private response".to_vec(), false),
        ] {
            let client = Arc::new(ResponseClient {
                status,
                length,
                bytes: payload,
            });
            let result = block_on(fetch(
                client,
                &file,
                "https://example.invalid/one-product/one-file.svg".into(),
                None,
            ));
            assert_eq!(result.is_ok(), expected, "HTTP {status}");
            if let Err(error) = result {
                assert!(!error.to_string().contains("private response"));
                assert!(!error.to_string().contains("example.invalid"));
            }
        }
    }
}
