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
    exec::{CreateExecOptions, StartExecOptions, StartExecResults},
    models::HostConfig,
};
use futures::StreamExt;
use std::ops::Deref;

use super::{util::client, ImagePullPolicy};

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
    app_armor: Option<&str>,
) -> Result<()> {
    image_policy
        .acquire_image(image)
        .await
        .context("failed to acquire container image")?;

    let client = client()?;

    let mut security_options = security_options.to_owned();
    if let Some(app_armor) = app_armor {
        security_options.push(format!("apparmor={}", app_armor))
    }

    let opts = CreateContainerOptions { name };
    let host_config = HostConfig {
        binds: Some(volumes.to_owned()),
        auto_remove: Some(true),
        security_opt: Some(security_options),
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
