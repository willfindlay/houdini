// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Client logic for interacting with Houdini's API.

use std::path::Path;

use anyhow::{Context, Result};
use hyper::{Body, Request};
use hyperlocal::{UnixClientExt, UnixConnector, Uri};

use crate::{
    tricks::{report::TrickReport, Trick},
    CONFIG,
};

pub struct HoudiniClient<'a> {
    client: hyper::client::Client<UnixConnector>,
    socket: &'a Path,
}

impl<'a> HoudiniClient<'a> {
    pub fn new(socket: Option<&'a Path>) -> Result<Self> {
        let socket = if let Some(socket) = socket {
            socket
        } else {
            &CONFIG.api.socket
        };

        let client = hyper::client::Client::unix();

        Ok(Self { socket, client })
    }

    fn uri<S: AsRef<str>>(&self, endpoint: S) -> hyper::Uri {
        Uri::new(self.socket, endpoint.as_ref()).into()
    }

    pub async fn ping(&self) -> Result<()> {
        let res = self
            .client
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

    pub async fn trick(&self, trick: &Trick) -> Result<TrickReport> {
        let req = Request::builder()
            .header("content-type", "application/json")
            .method("POST")
            .uri(self.uri("/trick"))
            .body(Body::from(
                serde_json::to_vec(trick).context("failed to serialize trick")?,
            ))
            .expect("request builder");

        let res = self
            .client
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
