// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Helpers for using a virtio socket with Axum.

use std::{io, pin::Pin, sync::Arc, task::Poll};

use axum::{extract::connect_info, BoxError};
use futures::{ready, task::Context};
use hyper::{
    client::connect::{Connected, Connection},
    server::accept::Accept,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_vsock::{VsockListener, VsockStream};

/// A vsock address including cid and port number.
#[derive(Debug)]
pub struct VsockAddr {
    cid: u32,
    port: u32,
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

/// A client connection.
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

impl Connection for ClientConnection {
    fn connected(&self) -> Connected {
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

