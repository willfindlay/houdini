// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module defines the steps that manipulate containers.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{command::ShellCommand, RunStep};
use crate::{
    docker::{kill_container, run_command, spawn_container, ImagePullPolicy},
    tricks::status::Status,
};

/// Spawn a container using the docker api.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SpawnContainer {
    /// Name to assign the container. This is done using the docker api, so commands
    /// like `docker exec -it <name> <command>` will work.
    pub name: String,
    /// Container image to use for the spawned container.
    pub image: String,
    /// A policy for what to do when an image is not available. Defaults to pulling
    /// from docker hub if the image does not exist and _no_ SHA256 verification.
    #[serde(default)]
    pub image_policy: ImagePullPolicy,
    /// Command to run in the container.
    pub cmd: Option<String>,
    /// Docker volumes for the container.
    #[serde(default)]
    pub volumes: Vec<String>,
    /// List of string options to customize LSM systems like SELinux.
    #[serde(default)]
    pub security: Vec<String>,
    /// Spawn the container with extra privileges.
    #[serde(default = "crate::serde_defaults::default_false")]
    pub privileged: bool,
    /// Status on failure. Default is SetupFailure.
    #[serde(default = "crate::serde_defaults::default_setup_failure")]
    pub failure: Status,
    /// Status on success. Default is Undecided.
    #[serde(default)]
    pub success: Status,
}

#[async_trait]
impl RunStep for SpawnContainer {
    async fn do_run(&self) -> Result<()> {
        spawn_container(
            &self.name,
            &self.image,
            &self.image_policy,
            self.cmd.as_deref(),
            &self.volumes,
            self.privileged,
            &self.security,
        )
        .await
    }

    fn on_success(&self) -> Status {
        self.success
    }

    fn on_failure(&self) -> Status {
        self.failure
    }
}

/// Kill a container using the docker api.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct KillContainer {
    /// Name of the container to kill.
    pub name: String,
    /// Status on failure. Default is Undecided.
    #[serde(default)]
    pub failure: Status,
    /// Status on success. Default is Undecided.
    #[serde(default)]
    pub success: Status,
}

#[async_trait]
impl RunStep for KillContainer {
    async fn do_run(&self) -> Result<()> {
        kill_container(&self.name).await
    }

    fn on_success(&self) -> Status {
        self.success
    }

    fn on_failure(&self) -> Status {
        self.failure
    }
}

/// Run a command in a spawned container using the docker api.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Container {
    /// Name of the container to run the command in. Must be the name of a previously
    /// spawned container.
    pub name: String,
    /// Script to run in the container. A non-zero exit status triggers `failure`,
    /// while a zero exit status triggers `success`.
    pub script: Vec<ShellCommand>,
    /// Should we run the commands with elevated privileges in the container?
    #[serde(default = "crate::serde_defaults::default_false")]
    pub privileged: bool,
    /// Should we spawn and attach a TTY for these commands?
    #[serde(default = "crate::serde_defaults::default_true")]
    pub tty: bool,
    /// Status on failure. Default is Undecided.
    #[serde(default)]
    pub failure: Status,
    /// Status on success. Default is Undecided.
    #[serde(default)]
    pub success: Status,
}

#[async_trait]
impl RunStep for Container {
    async fn do_run(&self) -> Result<()> {
        for cmd in &self.script {
            run_command(
                &self.name,
                &cmd.command,
                &cmd.args.iter().map(|x| &**x).collect::<Vec<_>>(),
                self.privileged,
                self.tty,
            )
            .await?;
        }

        Ok(())
    }

    fn on_success(&self) -> Status {
        self.success
    }

    fn on_failure(&self) -> Status {
        self.failure
    }
}
