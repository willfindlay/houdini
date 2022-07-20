// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Helpers for spawning and interacting with containers.

use anyhow::{Context as _, Result};
use bollard::{
    container::{Config, CreateContainerOptions, RemoveContainerOptions, WaitContainerOptions},
    models::HostConfig,
};
use futures::StreamExt;

use super::{util::client, ImagePullPolicy};

/// Clean up a container by removing it and waiting for it.
pub async fn reap_container(name: &str) -> Result<()> {
    let client = client().await?;

    let opts = RemoveContainerOptions {
        v: true,
        force: true,
        link: false,
    };
    client.remove_container(name, Some(opts)).await?;

    let opts = WaitContainerOptions {
        condition: "removed",
    };
    let mut stream = client.wait_container(name, Some(opts));

    while stream.next().await.is_some() {
        // The could return an error if the container has already been removed, but we
        // don't care. So do nothing here.
    }

    Ok(())
}

/// Spawn a new container.
pub async fn spawn_container(
    name: &str,
    image: &str,
    image_policy: &ImagePullPolicy,
    cmd: Option<&str>,
    volumes: &[String],
    privileged: bool,
    security_options: &[String],
) -> Result<()> {
    image_policy
        .acquire_image(image)
        .await
        .context("failed to acquire container image")?;

    let client = client().await?;

    let opts = CreateContainerOptions { name };
    let host_config = HostConfig {
        binds: Some(volumes.to_owned()),
        auto_remove: Some(true),
        security_opt: Some(security_options.to_owned()),
        // mounts: todo!(),
        // cap_add: todo!(),
        // cap_drop: todo!(),
        privileged: Some(privileged),
        // publish_all_ports: todo!(),
        ..Default::default()
    };
    let config = Config {
        // env: todo!(),
        cmd: cmd.map(|cmd| cmd.split_whitespace().collect()),
        image: Some(image),
        // working_dir: todo!(),
        // entrypoint: todo!(),
        // labels: todo!(),
        // shell: todo!(),
        host_config: Some(host_config),
        // networking_config: todo!(),
        ..Default::default()
    };

    client
        .create_container(Some(opts), config)
        .await
        .context("failed to create container")?;

    client
        .start_container::<&str>(name, None)
        .await
        .context("failed to start container")
}

/// Kill a container.
pub async fn kill_container(name: &str) -> Result<()> {
    let client = client().await?;

    client
        .kill_container::<&str>(name, None)
        .await
        .context("failed to kill container")
}
