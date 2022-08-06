// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! The Houdini API.

mod uds;

use anyhow::{Context as _, Result};
use axum::{extract::ConnectInfo, routing::get, Router};
use tokio::net::UnixListener;

use crate::CONFIG;

pub async fn serve() -> Result<()> {
    let _ = tokio::fs::remove_file(&CONFIG.api.socket).await;
    if let Some(parent) = &CONFIG.api.socket.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("failed to create parent directory for Houdini socket")?
    }

    let uds = UnixListener::bind(&CONFIG.api.socket).context("failed to bind to Houdini socket")?;

    let app = Router::new().route("/", get(ping));

    tracing::info!("server listening on {:?}...", &CONFIG.api.socket);
    axum::Server::builder(uds::ServerAccept { uds })
        .serve(app.into_make_service_with_connect_info::<uds::UdsConnectInfo>())
        .await
        .context("failed to start Houdini API server")
}

async fn ping(ConnectInfo(info): ConnectInfo<uds::UdsConnectInfo>) -> &'static str {
    tracing::info!("new connection from {:?}", info);
    "pong"
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use serial_test::serial;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    #[serial]
    async fn test_api_server_runs_smoke() {
        let jh = tokio::spawn(async { serve().await.expect("server should serve") });
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(!jh.is_finished());
        let _ = jh.abort();
    }
}
