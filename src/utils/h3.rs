use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use axum::body::Bytes;
use axum::extract::ConnectInfo;
use bytes::Buf as _;
use h3_util::server_body::send_h3_server_body;
use tower::ServiceExt as _;

use crate::prelude::*;

/// Spawn an HTTP/3 QUIC listener on `addr`, reusing the certificates in `tls`.
pub fn spawn(addr: std::net::SocketAddr, tls: rustls::ServerConfig, router: axum::Router) {
    tokio::spawn(async move {
        if let Err(err) = serve(addr, tls, router).await {
            tracing::error!("http/3: {}", err.message());
        }
    });
}

async fn serve(
    addr: std::net::SocketAddr, mut tls: rustls::ServerConfig, router: axum::Router,
) -> Result<()> {
    tls.alpn_protocols = vec![b"h3".to_vec()];
    let tls = quinn::crypto::rustls::QuicServerConfig::try_from(tls)?;
    let endpoint = quinn::Endpoint::server(quinn::ServerConfig::with_crypto(Arc::new(tls)), addr)?;

    while let Some(incoming) = endpoint.accept().await {
        let router = router.clone();
        tokio::spawn(async move {
            if let Err(err) = serve_connection(incoming, router).await {
                tracing::debug!("h3 connection: {}", err.message());
            }
        });
    }
    Ok(())
}

async fn serve_connection(incoming: quinn::Incoming, router: axum::Router) -> Result<()> {
    let conn = incoming.await?;
    let client = conn.remote_address();
    let mut conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;
    loop {
        match conn.accept().await {
            Ok(Some(resolver)) => {
                let router = router.clone();
                tokio::spawn(async move {
                    if let Err(err) = serve_request(resolver, client, router).await {
                        tracing::debug!("h3 request: {}", err.message());
                    }
                });
            }
            Ok(None) => return Ok(()),
            Err(err) if err.is_h3_no_error() => return Ok(()),
            Err(err) => return Err(err.into()),
        }
    }
}

async fn serve_request(
    resolver: h3::server::RequestResolver<h3_quinn::Connection, Bytes>, client: std::net::SocketAddr,
    router: axum::Router,
) -> Result<()> {
    let (req, stream) = resolver.resolve_request().await?;
    let (mut send, recv) = stream.split();

    let (parts, _) = req.into_parts();
    let mut req = Request::from_parts(parts, axum::body::Body::new(IncomingBody::new(recv)));
    req.extensions_mut().insert(ConnectInfo(client));
    let res = router.oneshot(req).await?;

    let (parts, body) = res.into_parts();
    send.send_response(Response::from_parts(parts, ())).await?;
    send_h3_server_body::<axum::body::Body, h3_quinn::BidiStream<Bytes>>(&mut send, body)
        .await
        .map_err(|err| any!("send body: {}", err))?;
    Ok(())
}

struct IncomingBody {
    stream: h3::server::RequestStream<h3_quinn::RecvStream, Bytes>,
    done: bool,
}

impl IncomingBody {
    fn new(stream: h3::server::RequestStream<h3_quinn::RecvStream, Bytes>) -> Self {
        Self { stream, done: false }
    }
}

impl Drop for IncomingBody {
    fn drop(&mut self) {
        let mut cx = Context::from_waker(Waker::noop());
        while !self.done {
            match self.stream.poll_recv_data(&mut cx) {
                Poll::Ready(Ok(Some(_))) => continue,
                Poll::Ready(Ok(None)) | Poll::Ready(Err(_)) => return,
                Poll::Pending => {
                    self.stream.stop_sending(h3::error::Code::H3_NO_ERROR);
                    return;
                }
            }
        }
    }
}

impl http_body::Body for IncomingBody {
    type Data = Bytes;
    type Error = h3::error::StreamError;

    fn poll_frame(
        self: Pin<&mut Self>, cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        if !this.done {
            match futures::ready!(this.stream.poll_recv_data(cx)) {
                Ok(Some(mut data)) => {
                    return Poll::Ready(Some(Ok(http_body::Frame::data(
                        data.copy_to_bytes(data.remaining()),
                    ))));
                }
                Ok(None) => this.done = true,
                Err(err) => return Poll::Ready(Some(Err(err))),
            }
        }
        match futures::ready!(this.stream.poll_recv_trailers(cx)) {
            Ok(Some(trailers)) => Poll::Ready(Some(Ok(http_body::Frame::trailers(trailers)))),
            Ok(None) => Poll::Ready(None),
            Err(err) => Poll::Ready(Some(Err(err))),
        }
    }
}
