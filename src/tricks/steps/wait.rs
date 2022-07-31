// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module defines a step that can be used to wait for a condition.

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use super::RunStep;
use crate::tricks::status::Status;

/// Pause Houdini until a condition occurs.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Wait {
    #[serde(rename = "for")]
    for_: WaitFor,
}

#[async_trait]
impl RunStep for Wait {
    async fn do_run(&self) -> Result<()> {
        match self.for_ {
            WaitFor::Sleep(dur) => tokio::time::sleep(dur).await,
            WaitFor::Input => {
                let _ = tokio::io::stdin().read(&mut [0]).await;
            }
        }
        Ok(())
    }

    fn on_success(&self) -> Status {
        Status::Undecided
    }

    fn on_failure(&self) -> Status {
        Status::Undecided
    }
}

/// A condition to wait for.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum WaitFor {
    #[serde(with = "humantime_serde")]
    Sleep(Duration),
    Input,
}
