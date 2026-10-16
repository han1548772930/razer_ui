//! Resource HTTP runs on its own I/O driver, independently of GPUI's null default.
use futures::{AsyncRead, Stream as _, future::BoxFuture};
use gpui_kit::http_client::{
    AsyncBody, HttpClient, Inner, RedirectPolicy, Request, RequestTimeout, Response, Url,
    http::{HeaderValue, Method},
};
use std::{
    io::{self, Cursor, Read as _},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

pub(in crate::remote) struct Client {
    network: reqwest::Client,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Client {
    pub(in crate::remote) fn new() -> anyhow::Result<Arc<dyn HttpClient>> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("resource-http")
            .enable_all()
            .build()?;
        let network = reqwest::Client::builder()
            .redirect_policy(reqwest::redirect::Policy::limited(8))
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(120))
            .user_agent(concat!("RazerUI/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Arc::new(Self {
            network,
            runtime: Some(runtime),
        }))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl HttpClient for Client {
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
        let (parts, body) = request.into_parts();
        if parts.method != Method::GET
            || !matches!(body.0, Inner::Empty)
            || parts.uri.scheme_str() != Some("https")
            || parts.extensions.get::<RedirectPolicy>() != Some(&RedirectPolicy::FollowLimit(8))
        {
            return Box::pin(async { anyhow::bail!("Invalid resource HTTP request") });
        }
        let timeout = parts
            .extensions
            .get::<RequestTimeout>()
            .map_or(Duration::from_secs(120), |value| value.0);
        let request = self
            .network
            .get(parts.uri.to_string())
            .headers(parts.headers)
            .timeout(timeout);
        let (mut reply, received) = tokio::sync::oneshot::channel();
        self.runtime
            .as_ref()
            .expect("HTTP driver exists until drop")
            .spawn(async move {
                let response = tokio::select! {
                    _ = reply.closed() => return,
                    response = request.send() => response,
                };
                let mut response = match response {
                    Ok(response) => response,
                    Err(_) => {
                        let _ = reply.send(Err(anyhow::anyhow!("资源服务器暂时无法连接")));
                        return;
                    }
                };
                let (sender, receiver) = async_channel::bounded(2);
                let mut builder = Response::builder().status(response.status());
                *builder.headers_mut().expect("Valid response status") = response.headers().clone();
                let head = builder
                    .body(AsyncBody::from_reader(Body {
                        receiver: Box::pin(receiver),
                        bytes: Cursor::new(Vec::new()),
                    }))
                    .map_err(Into::into);
                if reply.send(head).is_err() {
                    return;
                }
                loop {
                    let chunk = tokio::select! {
                        _ = sender.closed() => return,
                        chunk = response.chunk() => chunk,
                    };
                    match chunk {
                        Ok(Some(bytes)) => {
                            if sender.send(Ok(bytes.to_vec())).await.is_err() {
                                return;
                            }
                        }
                        Ok(None) => return,
                        Err(_) => {
                            let _ = sender.send(Err(io::Error::other("资源读取中断"))).await;
                            return;
                        }
                    }
                }
            });
        Box::pin(async move {
            received
                .await
                .map_err(|_| anyhow::anyhow!("资源加载已取消"))?
        })
    }
}

struct Body {
    receiver: Pin<Box<async_channel::Receiver<io::Result<Vec<u8>>>>>,
    bytes: Cursor<Vec<u8>>,
}

impl AsyncRead for Body {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if output.is_empty() {
            return Poll::Ready(Ok(0));
        }
        loop {
            if self.bytes.position() < self.bytes.get_ref().len() as u64 {
                return Poll::Ready(self.bytes.read(output));
            }
            match self.receiver.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => self.bytes = Cursor::new(bytes),
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Err(error)),
                Poll::Ready(None) => return Poll::Ready(Ok(0)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{AsyncReadExt as _, executor::block_on};

    #[test]
    fn body_keeps_chunk_order_and_reports_stream_errors_without_urls() {
        let (sender, receiver) = async_channel::bounded(3);
        sender.try_send(Ok(b"one".to_vec())).unwrap();
        sender.try_send(Ok(Vec::new())).unwrap();
        sender.try_send(Ok(b"two".to_vec())).unwrap();
        drop(sender);
        let mut body = Body {
            receiver: Box::pin(receiver),
            bytes: Cursor::new(Vec::new()),
        };
        let mut output = Vec::new();
        block_on(body.read_to_end(&mut output)).unwrap();
        assert_eq!(output, b"onetwo");
        let (sender, receiver) = async_channel::bounded(1);
        sender
            .try_send(Err(io::Error::other("interrupted")))
            .unwrap();
        let mut body = Body {
            receiver: Box::pin(receiver),
            bytes: Cursor::new(Vec::new()),
        };
        assert!(block_on(body.read_to_end(&mut Vec::new())).is_err());
        drop(body);
        assert!(sender.is_closed());
    }

    #[test]
    fn configured_driver_rejects_unscoped_requests_without_contacting_network() {
        let client = Client::new().unwrap();
        let request = Request::builder()
            .uri("http://example.invalid")
            .body(().into())
            .unwrap();
        assert!(
            block_on(client.send(request))
                .err()
                .unwrap()
                .to_string()
                .contains("Invalid resource")
        );
    }
}
