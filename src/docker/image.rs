// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Helpers for managing container images during exploit setup.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use docker_api::{
    api::{ImageBuildChunk, PullOpts},
    Docker,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::CONFIG;

/// Defines policy for what to do about acquiring a container image for an exploit step.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImagePullPolicy {
    /// Never build or pull the image.
    /// Requires that a viable image is always present locally.
    Never,
    /// Pull the image from a container repository.
    Pull {
        /// Should we always pull even when the image exists on the host?
        always: bool,
        /// SHA256 sum to use to verify the container image.
        sha256sum: Option<String>,
        /// Name of the container repo to use. Defaults to docker hub.
        repo: Option<String>,
    },
    /// Build the container image from a local Dockerfile.
    Build {
        /// Root directory for build context.
        build_root: PathBuf,
        /// Path to Dockerfile.
        dockerfile: PathBuf,
    },
}

impl Default for ImagePullPolicy {
    fn default() -> Self {
        Self::Pull {
            always: false,
            sha256sum: None,
            repo: None,
        }
    }
}

impl ImagePullPolicy {
    /// Acquire a Docker image according to the ImagePullPolicy.
    pub async fn acquire_image(&self, image: &str) -> Result<()> {
        match self {
            ImagePullPolicy::Never => Ok(()),
            ImagePullPolicy::Pull {
                always,
                sha256sum,
                repo,
            } => pull_image(image, *always, sha256sum.as_deref(), repo.as_deref()).await,
            ImagePullPolicy::Build {
                build_root,
                dockerfile,
            } => build_image(image, build_root, dockerfile).await,
        }
    }
}

async fn pull_image(
    image: &str,
    always: bool,
    sha256sum: Option<&str>,
    repo: Option<&str>,
) -> Result<()> {
    let mut image = image;
    let mut tag = None;
    if let Some((image_, tag_)) = image.split_once(':') {
        image = image_;
        tag = Some(tag_);
    }
    let client = Docker::unix(&CONFIG.docker.socket);

    // Stop early if we already have the image in question and "always" is not set.
    let images = client.images();
    let check = images.get(image);
    if let Ok(_) = check.inspect().await {
        if !always {
            return Ok(());
        }
    }

    let mut builder = PullOpts::builder().image(image);
    if let Some(repo) = repo {
        builder = builder.repo(repo);
    }
    if let Some(tag) = tag {
        builder = builder.tag(tag)
    } else {
        builder = builder.tag("latest")
    }
    let opts = builder.build();

    let images = client.images();
    let mut stream = images.pull(&opts);

    // FIXME: this hangs forever
    while let Some(res) = stream.next().await {
        match res {
            Ok(output) => match output {
                ImageBuildChunk::Update { stream } => {
                    tracing::debug!(output = ?stream, "image pull output");
                }
                ImageBuildChunk::Error {
                    error,
                    error_detail,
                } => {
                    anyhow::bail!(format!("{}: {}", error, error_detail.message));
                }
                ImageBuildChunk::Digest { aux } => {
                    tracing::debug!(digest = ?aux, "got container image digest");
                }
                ImageBuildChunk::PullStatus {
                    status,
                    id,
                    progress,
                    progress_detail,
                } => {
                    // tracing::debug!(status = ?status, id = ?id, progress = ?progress,
                    //     progress_detail = ?progress_detail, "image pull status");
                }
            },
            Err(e) => return Err(anyhow::Error::from(e).context("failed to pull container image")),
        }
    }

    let inspect = images
        .get(image)
        .inspect()
        .await
        .context("image inspect error after pull")?;
    tracing::debug!(digests = ?inspect.repo_digests, "got image digests");
    //inspect.repo_digests.get(0)

    Ok(())
}

async fn build_image<P: AsRef<Path>>(image: &str, build_root: P, dockerfile: P) -> Result<()> {
    todo!()
}
