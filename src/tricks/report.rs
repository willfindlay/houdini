// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Generate reports summarizing exploit runs.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use anyhow::{Context, Result};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use versions::Versioning;

use crate::CONFIG;

use super::{
    status::Status,
    steps::version::{get_docker_version, get_linux_version, get_runc_version},
    Step,
};

/// A serializable report on one or more exploits.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    /// Date at which the report was generated.
    pub date: DateTime<chrono::Utc>,
    /// Information about the system
    pub system_info: SystemInfo,
    /// A series of reports on trick execution.
    pub exploits: Vec<TrickReport>,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            date: chrono::offset::Utc::now(),
            system_info: Default::default(),
            exploits: Default::default(),
        }
    }
}

impl Report {
    pub fn add(&mut self, exploit: TrickReport) {
        self.exploits.push(exploit)
    }

    pub async fn write_to_disk(&self) -> Result<()> {
        let mut s = DefaultHasher::new();
        self.date.hash(&mut s);
        let hash = s.finish();

        let filename = format!("report.{}.json", hash);
        let path = CONFIG.reports.dir.join(filename);

        let file = File::create(&path)
            .await
            .context(format!("failed to open file {:?}", &path))?;
        serde_json::to_writer(file.into_std().await, self).context("failed to write report")?;

        tracing::info!(file = ?&path, "wrote exploit report");

        Ok(())
    }
}

/// A serializable exploit report.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrickReport {
    /// Name of the exploit.
    pub name: String,
    /// A series of reports on exploit steps.
    pub steps: Vec<StepReport>,
    /// Final status of the exploit.
    pub status: Status,
}

impl TrickReport {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            steps: Default::default(),
            status: Default::default(),
        }
    }

    pub fn add(&mut self, step: StepReport) {
        self.steps.push(step)
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = status
    }
}

/// A serializable exploit step report.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepReport {
    /// Inner exploit step.
    #[serde(flatten)]
    inner: Step,
    /// Status of the exploit step.
    status: Status,
}

impl StepReport {
    pub(crate) fn new(step: &Step, status: Status) -> Self {
        Self {
            inner: step.to_owned(),
            status,
        }
    }
}

/// Information about the system that ran the exploits.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemInfo {
    /// Kernel version.
    #[serde(with = "super::steps::version::versioning_serde")]
    pub kernel: Option<Versioning>,
    /// Docker version.
    #[serde(with = "super::steps::version::versioning_serde")]
    pub docker: Option<Versioning>,
    /// Runc version.
    #[serde(with = "super::steps::version::versioning_serde")]
    pub runc: Option<Versioning>,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            kernel: get_linux_version().ok(),
            docker: get_docker_version().ok(),
            runc: get_runc_version().ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{testutils::assert_json_serialize, tricks::steps::host::Host};

    use super::*;

    #[test]
    fn report_serde_test() {
        let report = Report {
            date: chrono::Utc::now(),
            system_info: SystemInfo::default(),
            exploits: vec![TrickReport {
                name: "foo".into(),
                steps: vec![StepReport {
                    inner: Step::Host(Host {
                        script: vec![],
                        failure: Status::ExploitFailure,
                        success: Status::ExploitSuccess,
                    }),
                    status: Status::ExploitSuccess,
                }],
                status: Status::ExploitSuccess,
            }],
        };

        assert_json_serialize(&report);
    }
}
