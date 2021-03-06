// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module defines a step that can be used as a version check.

use std::{io::BufRead, process::Command};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use nix::sys::utsname::uname;
use serde::{Deserialize, Serialize};
use versions::Versioning;

use crate::tricks::{status::Status, steps::RunStep};

/// Check software versions in the exploit environment. This can be useful for skipping
/// tests or failing setup when a required minimum version is not met.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct VersionCheck {
    /// Version check for.
    pub kernel: Option<VersionComparison>,
    pub docker: Option<VersionComparison>,
    pub runc: Option<VersionComparison>,
    /// Status on failure. Default is Skip.
    #[serde(default = "crate::serde_defaults::default_skip")]
    pub failure: Status,
    /// Status on success. Default is Undecided.
    #[serde(default)]
    pub success: Status,
}

#[async_trait]
impl RunStep for VersionCheck {
    async fn do_run(&self) -> Result<()> {
        if let Some(kernel) = &self.kernel {
            let version = get_linux_version().context("failed to get Linux version")?;
            kernel
                .compare(version)
                .context("Linux version check failed")?;
        }

        if let Some(docker) = &self.docker {
            let version = get_docker_version().context("failed to get docker version")?;
            docker
                .compare(version)
                .context("docker version check failed")?;
        }

        if let Some(runc) = &self.runc {
            let version = get_runc_version().context("failed to get runc version")?;
            runc.compare(version).context("runc version check failed")?;
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

/// Specify a minimum and/or maximum version to compare to.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionComparison {
    #[serde(default)]
    #[serde(with = "versioning_serde")]
    #[serde(alias = "minimum")]
    pub min: Option<Versioning>,
    #[serde(default)]
    #[serde(with = "versioning_serde")]
    #[serde(alias = "maximum")]
    pub max: Option<Versioning>,
}

impl VersionComparison {
    /// Compare against a known version, returning `Ok` if acceptable or `Err` otherwise.
    pub fn compare(&self, version: Versioning) -> Result<()> {
        let version = strip_version(version);

        if let Some(max) = &self.max {
            let max = &strip_version(max.clone());
            if &version > max {
                bail!("version {:?} is greater than maximum {:?}", version, max);
            }
        }

        if let Some(min) = &self.min {
            let min = &strip_version(min.clone());
            if &version < min {
                bail!("version {:?} is less than minimum {:?}", version, min);
            }
        }

        Ok(())
    }
}

/// Get running Linux kernel version.
pub fn get_linux_version() -> Result<Versioning> {
    let version = uname().context("failed to call uname")?;
    let version = version.release();
    let version = version
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF-8 version string"))?;
    parse_version(version)
}

/// Get runc version.
pub fn get_runc_version() -> Result<Versioning> {
    let output = Command::new("runc")
        .arg("--version")
        .output()
        .context("failed to spawn runc command")?;
    if !output.status.success() {
        bail!("runc command failed with {}", output.status);
    }
    for line in output.stdout.lines() {
        let line = line.context("failed to read line from stdout")?;
        if let Some(version) = line.strip_prefix("runc version").map(|line| line.trim()) {
            return parse_version(version);
        }
    }
    bail!("failed to find runc version")
}

/// Get dockerd version.
pub fn get_docker_version() -> Result<Versioning> {
    let output = Command::new("docker")
        .arg("--version")
        .output()
        .context("failed to spawn docker command")?;
    if !output.status.success() {
        bail!("docker command failed with {}", output.status);
    }
    for line in output.stdout.lines() {
        let line = line.context("failed to read line from stdout")?;
        if let Some(version) = line
            .strip_prefix("Docker version")
            .and_then(|line| line.trim().split_once(',').map(|s| s.0))
        {
            return parse_version(version);
        }
    }
    bail!("failed to find docker version")
}

/// Parse a version from a string.
fn parse_version(v: &str) -> Result<Versioning> {
    Versioning::new(v).ok_or_else(|| anyhow::anyhow!("invalid version string {}", v))
}

/// Strip a version of its pre-release and meta numbers.
fn strip_version(v: Versioning) -> Versioning {
    if let versions::Versioning::Ideal(version) = v {
        let mut version = version;
        version.pre_rel = None;
        version.meta = None;
        return versions::Versioning::Ideal(version);
    }

    if let versions::Versioning::General(version) = v {
        let mut version = version;
        version.meta = None;
        return versions::Versioning::General(version);
    }

    v
}

pub mod versioning_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use versions::Versioning;

    pub fn serialize<S>(versioning: &Option<Versioning>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(versioning) = &versioning {
            versioning.to_string().serialize(serializer)
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Versioning>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;

        if let Some(s) = s {
            if let Ok((_, ideal)) = versions::SemVer::parse(&s) {
                return Ok(Some(versions::Versioning::Ideal(ideal)));
            }

            if let Ok((_, general)) = versions::Version::parse(&s) {
                return Ok(Some(versions::Versioning::General(general)));
            }

            if let Ok((_, complex)) = versions::Mess::parse(&s) {
                return Ok(Some(versions::Versioning::Complex(complex)));
            }

            Err(serde::de::Error::custom(format!(
                "unable to parse {} as a version",
                &s
            )))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compare() {
        let vc = VersionComparison {
            min: None,
            max: Some("5.18.9-arch1-1".try_into().unwrap()),
        };

        vc.compare(Versioning::try_from("5.18.9-arch1-1").unwrap())
            .expect("identical version should be ok");
        vc.compare(Versioning::try_from("5.18.9-foobar").unwrap())
            .expect("should ignore pre-rel");
        vc.compare(Versioning::try_from("5.18.9").unwrap())
            .expect("without pre-rel should be ok");

        vc.compare(Versioning::try_from("5.18.10").unwrap())
            .expect_err("higher patch should be err");
        vc.compare(Versioning::try_from("5.19.0").unwrap())
            .expect_err("higher minor should be err");
        vc.compare(Versioning::try_from("6.0.0").unwrap())
            .expect_err("higher major should be err");
    }

    #[test]
    fn test_get_linux_version() {
        let version = get_linux_version().expect("should be able to get linux version");
        assert!(version.is_ideal());
    }

    #[test]
    fn test_get_runc_version() {
        let version = get_runc_version().expect("should be able to get runc version");
        assert!(version.is_ideal());
    }

    #[test]
    fn test_get_docker_version() {
        let version = get_docker_version().expect("should be able to get docker version");
        assert!(version.is_ideal());
    }
}
