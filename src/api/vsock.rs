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

use tokio_vsock::VsockListener;
use tokio_vsock::VsockStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::StreamExt as _;

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
        println!("POLL ACCEPT");
        let (stream, _addr) = ready!(self.virtio_sock.poll_accept(cx))?;
        println!("test 0 {:?} -- {:?}", stream, _addr);
        println!("test 1 {:?}", stream.local_addr());
        println!("test 2 {:?}", stream.peer_addr());
        println!("test 3 {:?}", _addr.to_string());
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
        println!("POLL WRITE");
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        println!("POLL FLUSH");
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        println!("POLL SHUTDOWN");
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl AsyncRead for ClientConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        println!("ASYNC READ");
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl Connection for ClientConnection {
    fn connected(&self) -> Connected {
        println!("CONNECTION");
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
        println!("CONNECT INFO");
        let peer_addr = target.peer_addr().unwrap();
        println!("{:?}", peer_addr);
        Self {
            peer_addr: Arc::new(peer_addr),
        }
    }
}