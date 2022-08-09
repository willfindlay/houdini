// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! The Houdini API.

pub mod client;

mod middleware;
mod uds;
mod vsock;

use std::path::Path;
use std::str;

use std::process::Command;
use std::process::Stdio;

use anyhow::{Context as _, Result};
use axum::{
    debug_handler,
    handler::Handler,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use hyper::StatusCode;
use tokio::net::UnixListener;
use tower::ServiceBuilder;

use crate::{
    tricks::{report::TrickReport, Trick},
    CONFIG,
};

use serde::Deserialize;
use serde::Serialize;

use tokio_vsock::VsockListener;
use tokio_vsock::VsockStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::StreamExt as _;

pub async fn serve(socket: Option<&Path>) -> Result<()> {
    let socket = if let Some(socket) = socket {
        socket
    } else {
        &CONFIG.api.socket
    };

    let _ = tokio::fs::remove_file(socket).await;
    if let Some(parent) = &CONFIG.api.socket.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("failed to create parent directory for Houdini socket")?
    }

    let uds = UnixListener::bind(socket).context("failed to bind to Houdini socket")?;

    // Add routes
    let app = Router::new()
        .route("/", get(ping))
        .route("/ping", get(ping))
        .route("/trick", post(run_trick));

    // Add fallback handler
    let app = app.fallback(not_found.into_service());

    // Add middleware
    let app = app.route_layer(
        ServiceBuilder::new().layer(axum::middleware::from_fn(middleware::log_connection)),
    );

    tracing::info!("server listening on {:?}...", socket);
    axum::Server::builder(uds::ServerAccept { uds })
        .serve(app.into_make_service_with_connect_info::<uds::UdsConnectInfo>())
        .await
        .context("failed to start Houdini API server")
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrickRequest {
    request_type: String,
    method: String,
    uri : String,
    body: Trick,
}

impl TrickRequest {
    pub fn new(body: Trick) -> Self {
        let request_type = String::from("REQUEST");
        let method = String::from("GET");
        let uri = String::from("\\\\trick");
        let body = body;

        return Self { request_type, method, uri, body};
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrickResponse {
    request_type: String,
    method: String,
    uri: String,
    body: TrickReport,
}

impl TrickResponse {
    pub fn new(body: TrickReport) -> Self {
        let request_type = String::from("RESPONSE");
        let method = String::from("POST");
        let uri = String::from("\\\\trick");
        let body = body;

        return Self { request_type, method, uri, body};
    }
}

//https://github.com/rust-vsock/tokio-vsock/blob/master/test_server/src/main.rs

pub async fn vsock_serve(cid: u32, port: u32) -> Result<()> {

    let mut listener = VsockListener::bind(cid, port)
        .expect("unable to bind virtio listener");
        println!("Listening for connections on port: {}", port);

    loop {
        let (mut stream, _) = listener.accept().await?;
        println!("Got connection ============");
        tokio::spawn(async move {
            process_socket(stream).await;
            println!("done task");
        });
        println!("made task");
    }
    println!("done here");
    Ok(())

}

async fn process_socket(mut stream: VsockStream){
    let mut buf = vec![0u8; 5000];
    println!("WAITING TO READ");
    let len = stream.read(&mut buf).await.unwrap();
    println!("READ SOMETHING");
    if len == 0 {
        println!("READ NOTHING");
    }
    buf.resize(len, 0);
    
    let request: TrickRequest = serde_json::from_slice(&buf).unwrap();

    println!("RECIEVED: {:?}",request);
    
    let trick: Trick = request.body;

    let report = trick.run(true).await;

    let payload = TrickResponse::new(report);

    println!("SENT: {:?}",payload);
    
    let payload = serde_json::to_vec(&payload).unwrap();

    stream.write_all(&payload).await.unwrap();

    println!("Finished Writing");
    println!("Shutting Down");
    poweroff();
}

fn poweroff(){
    let test_cmd = String::from("poweroff");
    let out = Command::new(&test_cmd)
                .stdout(Stdio::piped())
                .output()
                .map_err(anyhow::Error::from)
                .context("failed to run command");
}

async fn ping() -> &'static str {
    "pong"
}

#[debug_handler]
async fn run_trick(
    Json(trick): Json<Trick>,
) -> Result<Json<TrickReport>, (StatusCode, &'static str)> {
    let report = trick.run(false).await;
    Ok(Json(report))
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "bad endpoint")
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::*;
    use serial_test::serial;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    #[serial]
    async fn test_api_server_runs_smoke() {
        let path = tempfile::NamedTempFile::new()
            .unwrap()
            .into_temp_path()
            .to_path_buf();

        let jh =
            tokio::spawn(async move { serve(Some(&path)).await.expect("server should serve") });
        tokio::time::sleep(Duration::from_secs(1)).await;

        assert!(!jh.is_finished());
        let _ = jh.abort();
    }

    #[tokio::test]
    #[traced_test]
    #[serial]
    async fn test_api_ping() {
        let path = Arc::new(
            tempfile::NamedTempFile::new()
                .unwrap()
                .into_temp_path()
                .to_path_buf(),
        );

        let p = path.clone();
        let jh = tokio::spawn(async move { serve(Some(&p)).await.expect("server should serve") });
        tokio::time::sleep(Duration::from_secs(1)).await;

        let client = client::HoudiniClient::new(Some(&path)).expect("client should connect");
        client.ping().await.expect("ping should succeed");

        assert!(!jh.is_finished());
        let _ = jh.abort();
    }

    #[tokio::test]
    #[traced_test]
    #[serial]
    async fn test_api_trick() {
        let path = Arc::new(
            tempfile::NamedTempFile::new()
                .unwrap()
                .into_temp_path()
                .to_path_buf(),
        );

        let p = path.clone();
        let jh = tokio::spawn(async move { serve(Some(&p)).await.expect("server should serve") });
        tokio::time::sleep(Duration::from_secs(1)).await;

        let client = client::HoudiniClient::new(Some(&path)).expect("client should connect");

        let yaml = r#"
            name: foo
            steps: []
            "#;
        let trick = serde_yaml::from_str(yaml).expect("trick should deserialize");

        let report = client.trick(&trick).await.expect("trick should succeed");
        assert_eq!(report.name, "foo");
        assert_eq!(report.steps.len(), 0);

        assert!(!jh.is_finished());
        let _ = jh.abort();
    }
}
