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

use std::path::{Path, PathBuf};

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

use serde::{Deserialize, Serialize};

use tokio_vsock::VsockListener;

pub use vsock::VsockAddr;

/// Houdini API server supported socket types.
#[derive(Debug)]
pub enum Socket {
    Unix(PathBuf),
    Vsock(VsockAddr),
}

pub async fn serve(socket: Option<Socket>) -> Result<()> {
    if socket.is_none() {}

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

    match socket {
        Some(Socket::Unix(path)) => uds_serve(path, app).await,
        Some(Socket::Vsock(addr)) => vsock_serve(&addr, app).await,
        None => uds_serve(&CONFIG.api.socket, app).await,
    }
    .context("failed to start Houdini API server")
}

async fn uds_serve<P: AsRef<Path>>(path: P, app: Router) -> Result<()> {
    let _ = tokio::fs::remove_file(path.as_ref()).await;
    if let Some(parent) = path.as_ref().parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("failed to create parent directory for Houdini socket")?
    }
    let uds = UnixListener::bind(path.as_ref()).context("failed to bind to Houdini socket")?;

    tracing::info!("server listening on {:?}...", path.as_ref());
    axum::Server::builder(uds::ServerAccept { uds })
        .serve(app.into_make_service_with_connect_info::<uds::UdsConnectInfo>())
        .await
        .map_err(anyhow::Error::from)
}

async fn vsock_serve(VsockAddr { cid, port }: &VsockAddr, app: Router) -> Result<()> {
    let virtio_sock = VsockListener::bind(*cid, *port).expect("unable to bind virtio listener");

    tracing::info!("server listening on {}:{}...", cid, port);
    axum::Server::builder(vsock::ServerAccept { virtio_sock })
        .serve(app.into_make_service_with_connect_info::<vsock::VsockConnectInfo>())
        .await
        .map_err(anyhow::Error::from)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrickRequest {
    request_type: String,
    method: String,
    uri: String,
    body: Trick,
}

impl TrickRequest {
    pub fn new(body: Trick) -> Self {
        let request_type = String::from("REQUEST");
        let method = String::from("GET");
        let uri = String::from("\\\\trick");
        let body = body;

        return Self {
            request_type,
            method,
            uri,
            body,
        };
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

        return Self {
            request_type,
            method,
            uri,
            body,
        };
    }
}

// fn poweroff() {
//     let test_cmd = String::from("poweroff");
//     let out = Command::new(&test_cmd)
//         .stdout(Stdio::piped())
//         .output()
//         .map_err(anyhow::Error::from)
//         .context("failed to run command");
// }

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

        let jh = tokio::spawn(async move {
            serve(Some(Socket::Unix(path)))
                .await
                .expect("server should serve")
        });
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
        let jh = tokio::spawn(async move {
            serve(Some(Socket::Unix(*p.clone())))
                .await
                .expect("server should serve")
        });
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
        let jh = tokio::spawn(async move {
            serve(Some(Socket::Unix(*p.clone())))
                .await
                .expect("server should serve")
        });
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
