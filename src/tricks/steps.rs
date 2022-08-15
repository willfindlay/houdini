// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module defines the steps used in Houdini [`super::Trick`]s.

use std::fmt::Debug;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::status::Status;

use self::{
    container::{Container, KillContainer, SpawnContainer},
    host::Host,
    version::VersionCheck,
    wait::Wait,
    environment::CreateEnvironment,
};

pub mod environment;
pub mod command;
pub mod container;
pub mod host;
pub mod version;
pub mod wait;

/// A series of steps for running and verifying the status of a container exploit.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Step {
    createEnvironment(CreateEnvironment),
    VersionCheck(Box<VersionCheck>),
    SpawnContainer(SpawnContainer),
    KillContainer(KillContainer),
    Container(Container),
    Host(Host),
    Wait(Wait),
}

impl Step {
    pub async fn run(&self) -> Status {
        match self {
            Step::createEnvironment(step) => step.run(),
            Step::VersionCheck(step) => step.run(),
            Step::SpawnContainer(step) => step.run(),
            Step::KillContainer(step) => step.run(),
            Step::Container(step) => step.run(),
            Step::Host(step) => step.run(),
            Step::Wait(step) => step.run(),
        }
        .await
    }
}

#[async_trait]
pub trait RunStep: Debug {
    /// Run the step, returning the corresponding exploit status depending on whether it
    /// succeeded or failed.
    async fn run(&self) -> Status {
        match self.do_run().await {
            Ok(_) => {
                let status = self.on_success();
                tracing::info!(step = ?self, status = ?status, "step succeeded");
                status
            }
            Err(e) => {
                let status = self.on_failure();
                tracing::info!(error = ?e, step = ?self, status = ?status, "step failed");
                status
            }
        }
    }

    /// Internal implementation of [`RunStep::run`].
    async fn do_run(&self) -> Result<()>;

    /// This function is run on success and should return the appropriate status.
    fn on_success(&self) -> Status;

    /// This function is run on failure and should return the appropriate status.
    fn on_failure(&self) -> Status;
}
