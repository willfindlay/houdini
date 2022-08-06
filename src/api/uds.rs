// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Helpers for using a Unix domain socket with Axum.
//! Mostly adapted from the [Axum examples][0].
//!
//! [0]: https://github.com/tokio-rs/axum/blob/79a0a54bc9f0f585c974b5e6793541baff980662/examples/unix-domain-socket/src/main.rs

use axum::{extract::connect_info, BoxError};
use futures::{ready, task::Context};
use hyper::{
    client::connect::{Connected, Connection},
    server::accept::Accept,
};
use std::{io, pin::Pin, sync::Arc, task::Poll};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{unix::UCred, UnixListener, UnixStream},
};

pub struct ServerAccept {
    pub uds: UnixListener,
}

impl Accept for ServerAccept {
    type Conn = UnixStream;
    type Error = BoxError;

    fn poll_accept(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let (stream, _addr) = ready!(self.uds.poll_accept(cx))?;
        Poll::Ready(Some(Ok(stream)))
    }
}

pub struct ClientConnection {
    stream: UnixStream,
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
pub struct UdsConnectInfo {
    peer_addr: Arc<tokio::net::unix::SocketAddr>,
    peer_cred: UCred,
}

impl connect_info::Connected<&UnixStream> for UdsConnectInfo {
    fn connect_info(target: &UnixStream) -> Self {
        let peer_addr = target.peer_addr().unwrap();
        let peer_cred = target.peer_cred().unwrap();

        Self {
            peer_addr: Arc::new(peer_addr),
            peer_cred,
        }
    }
}
