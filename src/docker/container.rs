// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Helpers for spawning and interacting with containers.

use anyhow::{Context as _, Result};
use bollard::{
    container::{
        Config, CreateContainerOptions, DownloadFromContainerOptions, RemoveContainerOptions,
        WaitContainerOptions,
    },
    exec::{CreateExecOptions, StartExecOptions, StartExecResults},
    models::HostConfig,
};
use futures::StreamExt;
use scopeguard::defer;
use std::{fmt::Display, ops::Deref, path::Path, sync::Arc};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::{util::client, ImagePullPolicy};

/// A wrapper for a container ID.
pub struct ContainerId(String);

impl From<String> for ContainerId {
    fn from(value: String) -> Self {
        ContainerId(value)
    }
}

impl From<ContainerId> for String {
    fn from(value: ContainerId) -> Self {
        value.0
    }
}

impl Display for ContainerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Export root filesystem from a container image.
pub async fn export_rootfs<P: AsRef<Path>>(image_name: &str, file_name: P) -> Result<()> {
    let client = Arc::new(client()?);

    let container_name = Arc::new(Uuid::new_v4().to_string());

    client
        .create_container(
            Some(CreateContainerOptions {
                name: container_name.as_ref(),
            }),
            Config {
                image: Some(image_name),
                ..Default::default()
            },
        )
        .await?;

    let executor = tokio::runtime::Handle::current();
    defer! {
        let client_c = client.clone();
        let container_name_c = container_name.clone();
        executor.spawn(async move {
            let _ = client_c.remove_container(&container_name_c, None).await;
        });
    }

    let mut file = tokio::fs::File::create(file_name)
        .await
        .context("Failed to create file for image dump")?;

    let mut stream = client.download_from_container(
        &container_name,
        Some(DownloadFromContainerOptions { path: "/" }),
    );
    while let Some(res) = stream.next().await {
        let bytes = res?;
        file.write_all(&bytes).await?;
    }

    Ok(())
}

/// Clean up a container by removing it and waiting for it.
pub async fn reap_container(name: &str) -> Result<()> {
    let client = client()?;

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

    let client = client()?;

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
    let client = client()?;

    client
        .kill_container::<&str>(name, None)
        .await
        .context("failed to kill container")
}

/// Run a command in a container.
pub async fn run_command(
    name: &str,
    cmd: &str,
    args: &[&str],
    privileged: bool,
    tty: bool,
) -> Result<()> {
    let client = client()?;

    let opts = CreateExecOptions {
        attach_stdin: Some(false),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(tty),
        cmd: Some(
            std::iter::once(cmd)
                .chain(args.iter().copied())
                .collect::<Vec<&str>>(),
        ),
        privileged: Some(privileged),
        ..Default::default()
    };

    let exec = client
        .create_exec(name, opts)
        .await
        .context("failed to create exec object")?
        .id;

    let opts = StartExecOptions {
        detach: false,
        ..Default::default()
    };

    let results = client
        .start_exec(&exec, Some(opts))
        .await
        .context("failed to start exec")?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    match results {
        StartExecResults::Attached { mut output, .. } => {
            while let Some(Ok(output)) = output.next().await {
                match output {
                    bollard::container::LogOutput::StdErr { message } => {
                        stderr.append(&mut message.iter().cloned().collect())
                    }
                    bollard::container::LogOutput::StdOut { message } => {
                        stdout.append(&mut message.iter().cloned().collect())
                    }
                    _ => continue,
                }
            }
        }
        StartExecResults::Detached => unreachable!(),
    }

    match String::from_utf8(stdout) {
        Ok(stdout) => tracing::debug!(cmd = ?cmd, args = ?args, "command stdout:\n{}", stdout),
        Err(e) => {
            tracing::debug!(err = ?e, cmd = ?cmd, args = ?args, "failed to parse command stdout")
        }
    }

    match String::from_utf8(stderr) {
        Ok(stderr) => tracing::debug!(cmd = ?cmd, args = ?args, "command stderr:\n{}", stderr),
        Err(e) => {
            tracing::debug!(err = ?e, cmd = ?cmd, args = ?args, "failed to parse command stderr")
        }
    }

    let inspect = client
        .inspect_exec(&exec)
        .await
        .context("failed to inspect exec result")?;
    let code = inspect.exit_code.map(ExitCode);

    match code {
        None => anyhow::bail!("unknown exit status"),
        Some(c) if !c.success() => anyhow::bail!("command failed with {}", *c),
        Some(_) => Ok(()),
    }
}

/// Wraps an exit code for a container exec.
pub struct ExitCode(pub i64);

impl ExitCode {
    /// Was the command successful?
    pub fn success(&self) -> bool {
        self.0 == 0
    }
}

impl Deref for ExitCode {
    type Target = i64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
