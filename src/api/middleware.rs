// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Middleware for the Houdini API.

use crate::api::{uds::UdsConnectInfo, vsock::VsockConnectInfo};
use axum::{
    extract::{ConnectInfo, RequestParts},
    http::Request,
    middleware::Next,
    response::Response,
};

pub async fn log_uds_connection<B>(request: Request<B>, next: Next<B>) -> Response
where
    B: Send,
{
    let mut parts = RequestParts::new(request);

    match parts.extract::<ConnectInfo<UdsConnectInfo>>().await {
        Ok(info) => tracing::info!("new connection from {:?}", info),
        Err(e) => tracing::warn!(err = ?e, "failed to extract connection info"),
    };

    let request = parts.try_into_request().expect("body extracted");
    next.run(request).await
}

pub async fn log_vsock_connection<B>(request: Request<B>, next: Next<B>) -> Response
where
    B: Send,
{
    let mut parts = RequestParts::new(request);

    match parts.extract::<ConnectInfo<VsockConnectInfo>>().await {
        Ok(info) => tracing::info!("new connection from {:?}", info),
        Err(e) => tracing::warn!(err = ?e, "failed to extract connection info"),
    };

    let request = parts.try_into_request().expect("body extracted");
    next.run(request).await
}
