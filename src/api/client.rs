// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Client logic for interacting with Houdini's API.

use std::{
    path::{Path, PathBuf},
    thread, time,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use hyper::{Body, Request};

use hyperlocal::{UnixClientExt, UnixConnector, Uri};

use crate::{
    tricks::{report::TrickReport, Trick},
    CONFIG,
};

use super::{vsock::VsockConnector, Socket, VsockAddr};

#[async_trait]
pub trait HoudiniClient<T>
where
    T: hyper::client::connect::Connect + Clone + std::marker::Send + std::marker::Sync + 'static,
{
    fn client(&self) -> &hyper::client::Client<T>;
    fn uri<S: AsRef<str>>(&self, endpoint: S) -> hyper::Uri;

    async fn ping(&self) -> Result<()> {
        let res = self
            .client()
            .get(self.uri("/ping"))
            .await
            .context("ping failed")?;

        if !res.status().is_success() {
            anyhow::bail!("ping failed with status code {}", res.status())
        } else {
            tracing::info!("server responsed to ping, all is well");
            Ok(())
        }
    }

    async fn trick(&self, trick: &Trick) -> Result<TrickReport> {
        let req = Request::builder()
            .header("content-type", "application/json")
            .method("POST")
            .uri(self.uri("/trick"))
            .body(Body::from(
                serde_json::to_vec(trick).context("failed to serialize trick")?,
            ))
            .expect("request builder");

        let res = self
            .client()
            .request(req)
            .await
            .context("trick request failed")?;

        if !res.status().is_success() {
            anyhow::bail!("request failed with status code {}", res.status())
        }

        let body = hyper::body::to_bytes(res.into_body()).await?.to_vec();
        serde_json::from_slice(body.as_slice()).context("failed to deserialize response")
    }
}

pub struct HoudiniUnixClient<'a> {
    client: hyper::client::Client<UnixConnector>,
    socket: &'a Path,
}

impl<'a> HoudiniUnixClient<'a> {
    pub fn new(socket: Option<&'a Path>) -> Result<Self> {
        let client = hyper::client::Client::unix();

        Ok(Self {
            socket: socket.unwrap_or(&CONFIG.api.socket),
            client: client.into(),
        })
    }
}

impl<'a> HoudiniClient<UnixConnector> for HoudiniUnixClient<'a> {
    fn client(&self) -> &hyper::client::Client<UnixConnector> {
        &self.client
    }

    fn uri<S: AsRef<str>>(&self, endpoint: S) -> hyper::Uri {
        hyperlocal::Uri::new(self.socket, endpoint.as_ref()).into()
    }
}

pub struct HoudiniVsockClient {
    cid: u32,
    port: u32,
    client: hyper::client::Client<VsockConnector>,
}

impl HoudiniVsockClient {
    pub fn new(cid: u32, port: u32) -> Result<Self> {
        let client: hyper::Client<VsockConnector> =
            hyper::client::Client::builder().build(VsockConnector);

        Ok(Self {
            cid,
            port,
            client: client.into(),
        })
    }
}

impl HoudiniClient<VsockConnector> for HoudiniVsockClient {
    fn client(&self) -> &hyper::client::Client<VsockConnector> {
        &self.client
    }

    fn uri<S: AsRef<str>>(&self, endpoint: S) -> hyper::Uri {
        super::VsockUri::new(self.cid, self.port, endpoint.as_ref()).into()
    }
}
