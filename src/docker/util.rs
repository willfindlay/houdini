// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Helpers for managing the Docker client. For internal use.

use anyhow::{Context, Result};
use bollard::{Docker, API_DEFAULT_VERSION};

use crate::config::CONFIG;

/// Spawn a bollard::Docker using the configured Unix socket and the default API version.
pub fn client() -> Result<Docker> {
    Docker::connect_with_unix(
        CONFIG
            .docker
            .socket
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("bad docker socket path in config"))?,
        60,
        API_DEFAULT_VERSION,
    )
    .context("failed to spawn client")
}
