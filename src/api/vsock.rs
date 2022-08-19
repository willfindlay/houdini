// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Helpers for using a virtio socket with Axum.

use std::{future::Future, io, pin::Pin, sync::Arc, task::Poll};

use axum::{extract::connect_info, BoxError};
use futures::{ready, task::Context};
use hex::FromHex;
use hyper::{
    client::connect::Connected, server::accept::Accept, service::Service, Uri as HyperUri,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_vsock::{VsockListener, VsockStream};

/// A vsock address including cid and port number.
#[derive(Debug)]
pub struct VsockAddr {
    pub cid: u32,
    pub port: u32,
}

/// Accepts the connection on behalf of the server.
pub struct ServerAccept {
    pub virtio_sock: VsockListener,
}

impl Accept for ServerAccept {
    type Conn = VsockStream;
    type Error = BoxError;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let (stream, addr) = ready!(self.virtio_sock.poll_accept(cx))?;
        tracing::debug!(stream = ?stream, addr = ?addr, local_addr
            = ?stream.local_addr(), peer_addr = ?stream.peer_addr());
        Poll::Ready(Some(Ok(stream)))
    }
}

pub struct ClientConnection {
    stream: VsockStream,
}

impl AsyncWrite for ClientConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl AsyncRead for ClientConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl hyper::client::connect::Connection for ClientConnection {
    fn connected(&self) -> hyper::client::connect::Connected {
        Connected::new()
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct VsockConnectInfo {
    peer_addr: Arc<tokio_vsock::SockAddr>,
}

impl connect_info::Connected<&VsockStream> for VsockConnectInfo {
    fn connect_info(target: &VsockStream) -> Self {
        let peer_addr = target.peer_addr().unwrap();
        Self {
            peer_addr: Arc::new(peer_addr),
        }
    }
}

/// the `[VsockConnector]` can be used to construct a `[hyper::Client]` which can
/// speak to a virtio socket.
#[derive(Clone, Copy, Debug, Default)]
pub struct VsockConnector;

impl Unpin for VsockConnector {}

impl Service<HyperUri> for VsockConnector {
    type Response = ClientConnection;

    type Error = io::Error;

    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: HyperUri) -> Self::Future {
        let fut = async move {
            let (cid, port) = parse_vsock_path(&req)?;
            Ok(ClientConnection {
                stream: VsockStream::connect(cid, port).await?,
            })
        };
        Box::pin(fut)
    }
}

fn parse_vsock_path(uri: &HyperUri) -> Result<(u32, u32), io::Error> {
    if uri.scheme_str() != Some("vsock") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid URL, scheme must be vsock",
        ));
    }

    let host = uri.host().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid URL, host must be present",
        )
    })?;

    let bytes = Vec::from_hex(host).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid URL, host must be a hex-encoded path",
        )
    })?;

    let s = String::from_utf8_lossy(&bytes).into_owned();
    let (cid, port) = s.split_once(':').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid URL, host must be a hex-encoded cid:port",
        )
    })?;

    let cid = cid.parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid URL, failed to parse CID: {}", e),
        )
    })?;

    let port = port.parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid URL, failed to parse port: {}", e),
        )
    })?;

    Ok((cid, port))
}

#[derive(Debug, Clone)]
pub struct Uri {
    hyper_uri: HyperUri,
}

impl Uri {
    pub fn new(cid: u32, port: u32, endpoint: &str) -> Self {
        let host = hex::encode(format!("{}:{}", cid, port).as_bytes());
        let s = format!("vsock://{}:0{}", host, endpoint);
        let hyper_uri: HyperUri = s.parse().expect("failed to parse hyper uri");

        Self { hyper_uri }
    }
}

impl From<Uri> for HyperUri {
    fn from(uri: Uri) -> Self {
        uri.hyper_uri
    }
}
