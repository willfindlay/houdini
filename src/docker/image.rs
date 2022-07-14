// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Helpers for managing container images during exploit setup.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context as _, Result};
use bollard::image::BuildImageOptions;
use flate2::{read::GzEncoder, Compression};
use futures::StreamExt;
use hyper::Body;
use serde::{Deserialize, Serialize};
use tokio_util::codec::{BytesCodec, FramedRead};

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
        /// Path to Dockerfile.
        dockerfile: PathBuf,
        /// Arguments to pass to Docker build command.
        #[serde(default)]
        build_args: HashMap<String, String>,
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
                dockerfile,
                build_args,
            } => build_image(image, dockerfile, build_args).await,
        }
    }
}

async fn pull_image(
    image: &str,
    always: bool,
    sha256sum: Option<&str>,
    repo: Option<&str>,
) -> Result<()> {
    let tag = image.split_once(':').map(|x| x.1).unwrap_or("latest");

    let client = super::util::client()?;

    if let Ok(_) = client.inspect_image(image).await {
        if !always {
            return Ok(());
        }
    }

    let opts = bollard::image::CreateImageOptions {
        from_image: image,
        from_src: "",
        repo: repo.unwrap_or(""),
        tag,
        platform: "",
    };

    let mut stream = client.create_image(Some(opts), None, None);
    while let Some(res) = stream.next().await {
        let info = res.context("failed to pull image")?;
        if let Some(err) = info.error {
            return Err(anyhow::anyhow!("{}", err).context("failed to pull image"));
        }
        if let Some(status) = info.status {
            tracing::debug!(status = ?status, "image pull status")
        }
        if let Some(detail) = info.progress_detail {
            tracing::debug!(
                curr = detail.current,
                total = detail.total,
                "image pull progress"
            )
        }
    }

    let inspect = client
        .inspect_image(image)
        .await
        .context("image inspect error after pull")?;

    let digest = inspect
        .repo_digests
        .map(|l| l.get(0).cloned())
        .flatten()
        .map(|s| {
            if let Some((_, digest)) = s.split_once("sha256:") {
                Some(digest.to_owned())
            } else {
                None
            }
        })
        .flatten();

    match (sha256sum, digest.as_ref()) {
        (Some(d), None) => {
            bail!("expected image digest {} but found none", d)
        }
        (Some(d1), Some(d2)) if d1 != d2 => {
            bail!("image digest {} does not match expected digest {}", d2, d1)
        }
        _ => {
            // Digest matches expected or no expected digest provided
        }
    }

    Ok(())
}

async fn build_image<P: AsRef<Path>>(
    image: &str,
    dockerfile: P,
    build_args: &HashMap<String, String>,
) -> Result<()> {
    let client = super::util::client()?;

    let tag = image.split_once(':').map(|x| x.1).unwrap_or("latest");

    let image_options = BuildImageOptions {
        dockerfile: dockerfile.as_ref().to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "dockerfile path invalid `{}`",
                dockerfile.as_ref().display()
            )
        })?,
        t: tag,
        q: false,
        nocache: false,
        pull: true,
        rm: true,
        forcerm: false,
        buildargs: build_args
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect(),
        squash: true,
        ..Default::default()
    };

    let build_root = dockerfile.as_ref().parent().ok_or_else(|| {
        anyhow::anyhow!(
            "unable to get build root for dockerfile `{}`",
            dockerfile.as_ref().display()
        )
    })?;

    let tar_gz = tempfile::Builder::new()
        .suffix(".tar.gz")
        .tempfile()
        .context("failed to create temporary file")?
        .into_file();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(".", build_root)
        .context("failed to add buildroot to tar archive")?;
    let tar = tar.into_inner().context("failed to write tar archive")?;
    let tar_gz = tokio::fs::File::from_std(tar.into_inner());

    let stream = FramedRead::new(tar_gz, BytesCodec::new());
    let body = Body::wrap_stream(stream);

    let mut stream = client.build_image(image_options, None, Some(body));
    while let Some(res) = stream.next().await {
        let info = res.context("failed to build image")?;
        if let Some(err) = info.error {
            return Err(anyhow::anyhow!("{}", err).context("failed to build image"));
        }
        if let Some(status) = info.status {
            tracing::debug!(status = ?status, "image build status")
        }
        if let Some(detail) = info.progress_detail {
            tracing::debug!(
                curr = detail.current,
                total = detail.total,
                "image build progress"
            )
        }
    }

    Ok(())
}
