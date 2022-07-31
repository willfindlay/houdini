// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module defines the steps that manipulate the host system.

use std::{
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
};

use anyhow::{bail, Context as _, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{command::ShellCommand, RunStep};
use crate::tricks::status::Status;

/// Run a command or commands on the host.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Host {
    /// Script to run on the host. A non-zero exit status triggers `failure`,
    /// while a zero exit status triggers `success`.
    pub script: Vec<ShellCommand>,
    /// Failure mode for when this step fails. Default is Undecided.
    #[serde(default)]
    pub failure: Status,
    /// Success mode for when this step fails. Default is Undecided.
    #[serde(default)]
    pub success: Status,
}

#[async_trait]
impl RunStep for Host {
    async fn do_run(&self) -> Result<()> {
        for cmd in &self.script {
            let out = Command::new(&cmd.command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .args(&cmd.args)
                .output()
                .map_err(anyhow::Error::from)
                .context("failed to run command")?;

            match String::from_utf8(out.stdout) {
                Ok(stdout) => {
                    tracing::debug!(cmd = ?cmd.command, args = ?cmd.args, "command stdout:\n{}", stdout)
                }
                Err(e) => {
                    tracing::debug!(err = ?e, cmd = ?cmd.command, args = ?cmd.args, "failed to parse command stdout")
                }
            }

            match String::from_utf8(out.stderr) {
                Ok(stderr) => {
                    tracing::debug!(cmd = ?cmd.command, args = ?cmd.args, "command stderr:\n{}", stderr)
                }
                Err(e) => {
                    tracing::debug!(err = ?e, cmd = ?cmd.command, args = ?cmd.args, "failed to parse command stderr")
                }
            }

            let status = out.status;
            if !status.success() {
                match status.code() {
                    Some(code) => bail!("command failed with exit code: {}", code),
                    None => {
                        bail!(
                            "command exited with signal: {}",
                            status
                                .signal()
                                .expect("No signal or exit code for process!?")
                        )
                    }
                }
            }
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
