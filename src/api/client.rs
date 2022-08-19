// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Client logic for interacting with Houdini's API.

use std::{path::Path, thread, time};

use anyhow::{Context, Result};
use hyper::{Body, Request};
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use tokio_vsock::VsockStream;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    api::{TrickRequest, TrickResponse},
    tricks::{report::TrickReport, Trick},
    CONFIG,
};

pub struct HoudiniClient<'a> {
    client: hyper::client::Client<UnixConnector>,
    socket: &'a Path,
}

pub struct HoudiniVsockClient {
    client: VsockStream,
    cid: u32,
    port: u32,
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

impl HoudiniVsockClient {
    pub async fn new(cid: u32, port: u32) -> Result<HoudiniVsockClient> {
        let connected = 0;
        let client: VsockStream;

        while connected == 0 {
            if let Ok(client) = VsockStream::connect(cid, port).await {
                println!("CLIENT CONNECTION CID: {} PORT: {}", cid, port);
                return Ok(Self { cid, port, client });
            }
        }

        client = VsockStream::connect(cid, port)
            .await
            .expect("connection failed");
        Ok(Self { cid, port, client })
    }

    pub async fn ping(&self) -> Result<()> {
        println!("UNSUPPORTED FUNCTION");
        Ok(())
    }

    pub async fn trick(&mut self, trick: Vec<u8>) -> Result<TrickReport> {
        let mut success = false;

        println!("SENDING TRICK TO CID: {} PORT: {}", self.cid, self.port);
        let trick: Trick = serde_json::from_slice(&trick).unwrap();
        println!("TRICK: {:#?}", trick);
        let payload = TrickRequest::new(trick);

        while !success {
            let payload = serde_json::to_vec(&payload).unwrap();

            let mut rx_blob = vec![];

            rx_blob.resize(5000, 0);

            success = write_to_stream(&mut self.client, payload).await.unwrap();

            if !success {
                self.client = VsockStream::connect(self.cid, self.port).await.unwrap();
            }
        }

        println!("SENT OVER TRICK");

        let mut response: Vec<u8> = vec![];

        let valid = 0;
        let mut rx_pos = 0;

        while valid == 0 {
            rx_pos = read_from_stream(&mut self.client, &mut response, rx_pos).await;

            if let Ok(payload) = serde_json::from_slice(&response) {
                println!("PAYLOAD OK");
                let response: TrickResponse = payload;
                println!("RECIEVED {:?}", response);
                let trick_report: TrickReport = response.body;
                return Ok(trick_report);
            } else {
                println!("ISSUE READING");
            }
            println!("STILL READING");
        }

        Ok(TrickReport::new(""))
    }
}

async fn write_to_stream(stream: &mut VsockStream, payload: Vec<u8>) -> Result<bool> {
    let mut tx_pos = 0;
    thread::sleep(time::Duration::from_millis(100));
    while tx_pos < payload.len() {
        if let Ok(written_bytes) = stream.write(&payload).await {
            println!("WROTE {} BYTES", written_bytes);
            if written_bytes == 0 {
                panic!("stream unexpectedly closed");
            }
            tx_pos += written_bytes;
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn read_from_stream(
    stream: &mut VsockStream,
    payload: &mut Vec<u8>,
    mut rx_pos: usize,
) -> usize {
    payload.resize(5000, 0);

    println!("READING...");
    let read_bytes = stream.read(&mut payload[rx_pos..]).await.unwrap();

    if read_bytes == 0 {
        panic!("stream unexpectedly closed");
    }
    rx_pos += read_bytes;
    payload.resize(rx_pos, 0);

    println!("READ: {} bytes", read_bytes);

    rx_pos
}

