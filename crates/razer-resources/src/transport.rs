use crate::{
    catalog::{ResourceFile, manifest, remote_url},
    runtime::Downloader,
};
use anyhow::{Context as _, ensure};
use futures::future::BoxFuture;
use std::time::Duration;

#[derive(Default)]
pub(crate) struct HttpDownloader {
    client: Option<reqwest::Client>,
}

impl Downloader for HttpDownloader {
    fn fetch(&mut self, file: ResourceFile) -> BoxFuture<'static, anyhow::Result<Vec<u8>>> {
        let prepare = || -> anyhow::Result<reqwest::Client> {
            reqwest::Client::builder()
                .redirect_policy(reqwest::redirect::Policy::limited(8))
                .connect_timeout(Duration::from_secs(20))
                .timeout(Duration::from_secs(120))
                .user_agent(concat!("RazerUI/", env!("CARGO_PKG_VERSION")))
                .build()
                .context("资源 HTTP 客户端初始化失败")
        };
        if self.client.is_none() {
            match prepare() {
                Ok(client) => self.client = Some(client),
                Err(error) => return Box::pin(async { Err(error) }),
            }
        }
        let client = self
            .client
            .as_ref()
            .expect("HTTP client initialized")
            .clone();
        Box::pin(async move {
            let url = remote_url(&manifest()?.storage, &file)?;
            let mut request = client.get(url);
            if let Some(token) = crate::credentials::token() {
                request = request.bearer_auth(token);
            }
            // Reqwest removes sensitive headers on cross-origin redirects. Errors must
            // never expose signed CDN URLs, response bodies or credentials.
            let mut response = request
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("资源服务器暂时无法连接"))?;
            validate_head(
                response.status().as_u16(),
                response
                    .headers()
                    .get("Content-Length")
                    .map(|value| value.to_str())
                    .transpose()
                    .context("无效资源长度")?,
                &file,
            )?;
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| anyhow::anyhow!("资源读取中断"))?
            {
                ensure!(
                    (bytes.len() as u64).saturating_add(chunk.len() as u64) <= file.bytes,
                    "远程资源长度与清单不一致"
                );
                bytes.extend_from_slice(&chunk);
            }
            ensure!(crate::cache::verified(&file, &bytes), "远程资源校验失败");
            crate::trace("download", &file.path, format!("ok {}", bytes.len()));
            Ok(bytes)
        })
    }
}

fn validate_head(status: u16, length: Option<&str>, file: &ResourceFile) -> anyhow::Result<()> {
    ensure!(
        status != 401 && status != 403,
        "私有资源仓库需要本机已配置的访问凭据"
    );
    ensure!(status != 404, "所需资源尚未上传或资源路径不存在");
    ensure!(status != 429, "资源服务器请求受限，请稍后重试");
    ensure!(
        (200..300).contains(&status),
        "资源服务器返回错误（HTTP {status}）"
    );
    if let Some(length) = length {
        let length: u64 = length.parse().context("无效资源长度")?;
        ensure!(length == file.bytes, "远程资源长度与清单不一致");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_requires_success_and_exact_declared_length_without_private_details() {
        let file = crate::cache::tests::file("assets/icon.svg", b"verified");
        for (status, length, valid) in [
            (200, Some("8"), true),
            (200, None, true),
            (200, Some("7"), false),
            (200, Some("bad"), false),
            (401, None, false),
            (403, None, false),
            (404, None, false),
            (429, None, false),
            (500, None, false),
        ] {
            assert_eq!(validate_head(status, length, &file).is_ok(), valid);
        }
    }
}
