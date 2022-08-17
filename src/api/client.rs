// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Client logic for interacting with Houdini's API.

use std::path::Path;
use std::{thread, time};

use anyhow::{Context, Result};
use hyper::{Body, Request};
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use tokio_vsock::VsockStream;

use tokio_vsock::VsockListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::StreamExt as _;
use std::str;

use crate::api::TrickResponse;
use crate::api::TrickRequest;
//use crate::api::create_payload;


use httparse;

use crate::{
    tricks::{report::TrickReport, Trick},
    CONFIG,
};

pub struct HoudiniClient<'a> {
    client: hyper::client::Client<UnixConnector>,
    socket: &'a Path,
}

pub struct HoudiniVsockClient{
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

impl HoudiniVsockClient{

    pub async fn new(cid: u32, port: u32) -> Result<HoudiniVsockClient> {
        let connected = 0;
        let mut client: VsockStream;
        
        while connected == 0 {
            if let Ok(client) = VsockStream::connect(cid, port)
            .await{
                println!("CLIENT CONNECTION CID: {} PORT: {}",cid,port);
                return Ok(Self { cid, port, client})
            }
        }

        client = VsockStream::connect(cid, port)
            .await
            .expect("connection failed");
        Ok(Self { cid, port, client})
    }

    pub async fn ping(&self) -> Result<()> {

        //let mut rx_blob = vec![];
        
        let req = b"
        {
            \"request_type\": \"Request\",
            \"method\": \"GET\",
            \"uri\": \"\\\\ping\",
            \"body\": \"[]\"
        }";

        /*let req = Request::builder()
            .header("content-type", "application/json")
            .method("GET")
            .uri(self.uri("/ping"))
            .body(Body::default()).unwrap();*/

        println!("PINGING CID: {} PORT: {}",self.cid, self.port);
        println!("REQUEST {:?}",req);

        //let payload = format!("{:?}",req);
        let payload = req;

        /*let payload = String::from("ping");
        let mut response = String::from("");

        let mut return_bytes = 0;
        let mut sent_bytes = 0;

        let mut stream = VsockStream::connect(self.cid, self.port)
        .await
        .expect("connection failed");

        while sent_bytes == 0{
            let sent = stream
                .write(payload.as_bytes())
                .await
                .context("ping failed")?;
            sent_bytes += sent;
            println!("SENT: {} -- SENT_BYTES: {}",sent,sent_bytes);
            println!("SENT AND NOW WAITING FOR RESPONSE");
            while return_bytes == 0{
                println!("WAITING FOR RESPONSE");
                
                let res = stream
                    .read_to_string(&mut response)
                    .await
                    .expect("read failed");
                return_bytes += res;
                println!("READ: {} -- READ_BYTES: {}",res,return_bytes);
                println!("{:?}",response);
            }
        }

        if return_bytes == 0 {
            tracing::info!("Stream closed");
            Ok(())
        } else {
            tracing::info!("server responsed to ping, all is well");
            Ok(())
        }*/

        //let mut rng = rand::thread_rng();
        //let mut blob: Vec<u8> = vec![];
        //let mut blob: &[u8]= payload.as_bytes();
        let mut blob: &[u8]= payload;
        let test_blob_size: usize = blob.len();
        let test_block_size: usize = blob.len();
        let mut rx_blob = vec![];
        let mut tx_pos = 0;

        rx_blob.resize(test_blob_size, 0);
        //rng.fill_bytes(&mut blob);

        let mut stream = VsockStream::connect(self.cid, self.port)
            .await
            .expect("connection failed");
        
        //need to have logic to create http responses/requests

        while tx_pos < test_blob_size {
            let written_bytes = stream
                .write(&blob)
                .await
                .expect("write failed");
            if written_bytes == 0 {
                panic!("stream unexpectedly closed");
            }

            let mut rx_pos = tx_pos;
            while rx_pos < (tx_pos + written_bytes) {
                let read_bytes = stream.read(&mut rx_blob).await.unwrap();

                if read_bytes == 0 {
                    panic!("stream unexpectedly closed");
                }
                rx_pos += read_bytes;
                let s = match str::from_utf8(&rx_blob) {
                    Ok(v) => v,
                    Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
                };

                println!("Recieved: {} bytes", read_bytes);
                println!("Recieved: {:?}", rx_blob);
                println!("Recieved: {:?}", s);

                //logic goes here
                //parse received data which should be http
                

                
            }

            tx_pos += written_bytes;
        }
        Ok(())
    }

    fn uri<S: AsRef<str>>(&self, endpoint: S) -> hyper::Uri {
        let mut authority = String::default();
        authority.push_str(&self.cid.to_string());
        authority.push_str(":");
        authority.push_str(&self.port.to_string());

        hyper::Uri::builder().scheme("vsock").authority(authority).path_and_query(endpoint.as_ref()).build().unwrap()
    }

    pub async fn trick(&self, trick: Vec<u8>) -> Result<TrickReport> {

        let trick: Trick = serde_json::from_slice(&trick).unwrap();
        //let trick = serde_json::to_string(&trick).unwrap();

        let req = b"
        {
            \"request_type\": \"Request\",
            \"method\": \"GET\",
            \"uri\": \"\\\\trick\",
            \"body\": \"START\"
        }";

        println!("PINGING CID: {} PORT: {}",self.cid, self.port);
        println!("REQUEST {:?}",req);
        println!("REQUEST {:?}",String::from_utf8_lossy(req));

        //let req2 = create_payload(String::from("Request"), String::from("GET"), String::from("\\\\trick"), trick).await.unwrap();

        let req2 = TrickRequest::new(trick);
        let req2 = serde_json::to_vec(&req2).unwrap();

        println!("REQUEST {:?}",req2);
        println!("REQUEST {:?}",String::from_utf8_lossy(&req2));

        //let payload = format!("{:?}",req);
        let payload = req2;

        let mut blob = payload;
        let test_blob_size: usize = blob.len();
        let test_block_size: usize = blob.len();
        let mut rx_blob = vec![];
        let mut tx_pos = 0;

        rx_blob.resize(5000, 0);
        //rng.fill_bytes(&mut blob);

        let mut stream = VsockStream::connect(self.cid, self.port)
            .await
            .expect("connection failed");

        println!("SENDING OVER TRICK");
        write_to_stream(&mut stream, blob).await;
        println!("SENT OVER TRICK");
        let mut response: Vec<u8> = vec![];

        let mut valid = 0;
        let mut rx_pos = 0;
        
        while valid == 0 {
            read_from_stream(&mut stream, &mut response, rx_pos).await;

            if let Ok(payload) = serde_json::from_slice(&response){
                println!("PAYLOAD OK");
                let v: TrickResponse = payload;
                println!("{:?}", v);
                valid = 1;
                let trick_report: TrickReport = v.body;
                return Ok(trick_report)
            }
            else {
                println!("ISSUE READING");
            }
            println!("STILL READING");

        }



        
        
        
        //need to have logic to create http responses/requests

        /*while tx_pos < test_blob_size {
            let written_bytes = stream
                .write(&blob)
                .await
                .expect("write failed");
            if written_bytes == 0 {
                panic!("stream unexpectedly closed");
            }

            let mut rx_pos = 0;
            let mut valid = 0;
            while valid == 0 {
                let read_bytes = stream.read(&mut rx_blob[rx_pos..]).await.unwrap();

                if read_bytes == 0 {
                    panic!("stream unexpectedly closed");
                }
                rx_pos += read_bytes;
                rx_blob.resize(rx_pos, 0);
                let s = match str::from_utf8(&rx_blob) {
                    Ok(v) => v,
                    Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
                };

                println!("Recieved: {} bytes", read_bytes);
                println!("Recieved: {:?}", rx_blob);
                println!("Recieved: {:?}", s);

                if let Ok(payload) = serde_json::from_slice(&rx_blob){
                    let v: TrickResponse = payload;
                    println!("{:?}", v);
                    valid = 1;
                    let trick: Trick = v.body;
                    //logic goes here
                    //parse received data which should be http
                    let report = trick.run().await;
                }
                rx_blob.resize(5000, 0);
                
            }

            tx_pos += written_bytes;
        }

        while tx_pos < test_blob_size {
            let written_bytes = stream
                .write(&blob)
                .await
                .expect("write failed");
            if written_bytes == 0 {
                panic!("stream unexpectedly closed");
            }

            tx_pos += written_bytes;
        }*/

        Ok((TrickReport::new("")))
    }
}

async fn write_to_stream(stream: &mut VsockStream, payload: Vec<u8>) -> Result<()> {
    let mut tx_pos = 0;
    println!("BEGINNING TRANSMISSION");
    println!("{:?}",stream.peer_addr());
    println!("{:?}",stream.local_addr());
    while tx_pos < payload.len() {
        println!("WRITING...");
        let written_bytes = stream
            .write(&payload)
            .await
            .expect("write failed");
        println!("WROTE {} BYTES",written_bytes);
        if written_bytes == 0 {
            panic!("stream unexpectedly closed");
        }
        tx_pos += written_bytes;
    }
    Ok(())
}

async fn read_from_stream(stream: &mut VsockStream, payload: &mut Vec<u8>, mut rx_pos: usize) -> Result<()> {

    payload.resize(5000, 0);

    println!("SENT OVER TRICK");
    let read_bytes = stream.read(&mut payload[rx_pos..]).await.unwrap();

    if read_bytes == 0 {
        panic!("stream unexpectedly closed");
    }
    rx_pos += read_bytes;
    payload.resize(rx_pos, 0);

    println!("Recieved: {} bytes", read_bytes);
    println!("Recieved: {:?}", payload);

    Ok(())
}