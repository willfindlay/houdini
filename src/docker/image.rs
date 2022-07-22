// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Helpers for managing container images during exploit setup.

use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Context as _, Result};
use bollard::image::BuildImageOptions;
use flate2::{write::GzEncoder, Compression};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

/// Defines policy for what to do about acquiring a container image for an exploit step.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ImagePullPolicy {
    /// Never build or pull the image.
    /// Requires that a viable image is always present locally.
    Never,
    /// Pull the image from a container repository.
    Pull(PullOpts),
    /// Build the container image from a local Dockerfile.
    Build(BuildOpts),
}

impl Default for ImagePullPolicy {
    fn default() -> Self {
        Self::Pull(PullOpts {
            always: false,
            sha256sum: None,
            repo: None,
        })
    }
}

impl ImagePullPolicy {
    /// Acquire a Docker image according to the ImagePullPolicy.
    pub async fn acquire_image(&self, image: &str) -> Result<()> {
        match self {
            ImagePullPolicy::Never => Ok(()),
            ImagePullPolicy::Pull(opts) => opts.pull(image).await.context("failed to pull image"),
            ImagePullPolicy::Build(opts) => {
                opts.build(image).await.context("failed to build image")
            }
        }
    }
}

/// Options for pulling an image.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PullOpts {
    #[serde(default = "crate::serde_defaults::default_true")]
    /// Should we always pull even when the image exists on the host? Defaults to true.
    always: bool,
    #[serde(alias = "sha256")]
    /// SHA256 sum to use to verify the container image.
    sha256sum: Option<String>,
    /// Name of the container repo to use. Defaults to docker hub.
    repo: Option<String>,
}

impl PullOpts {
    pub async fn pull(&self, image: &str) -> Result<()> {
        let tag = image.split_once(':').map(|x| x.1).unwrap_or("latest");

        let client = super::util::client()?;

        if client.inspect_image(image).await.is_ok() && !self.always {
            return Ok(());
        }

        let opts = bollard::image::CreateImageOptions {
            from_image: image,
            from_src: "",
            repo: self.repo.as_deref().unwrap_or_default(),
            tag,
            platform: "",
        };

        let mut stream = client.create_image(Some(opts), None, None);
        while let Some(res) = stream.next().await {
            let info = res.context("failed to send request")?;
            if let Some(err) = info.error {
                return Err(anyhow::anyhow!("{}", err).context("error from docker"));
            }
            if let Some(status) = info.status {
                tracing::trace!(status = ?status, "image pull status")
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
            .and_then(|l| l.get(0).cloned())
            .and_then(|s| {
                if let Some((_, digest)) = s.split_once("sha256:") {
                    Some(digest.to_owned())
                } else {
                    None
                }
            });

        match (&self.sha256sum, digest.as_ref()) {
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
}

/// Options for building an image.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BuildOpts {
    /// Path to Dockerfile.
    dockerfile: PathBuf,
    /// Arguments to pass to Docker build command.
    #[serde(default)]
    build_args: HashMap<String, String>,
}

impl BuildOpts {
    async fn build(&self, image: &str) -> Result<()> {
        let client = super::util::client()?;

        let image_options = BuildImageOptions {
            dockerfile: self
                .dockerfile
                .file_name()
                .and_then(|f| f.to_str())
                .ok_or_else(|| {
                    anyhow::anyhow!("dockerfile path invalid `{}`", self.dockerfile.display())
                })?,
            t: image,
            q: false,
            nocache: false,
            pull: true,
            rm: true,
            forcerm: false,
            buildargs: self
                .build_args
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect(),
            squash: false,
            ..Default::default()
        };

        let build_root = self.dockerfile.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "unable to get build root for dockerfile `{}`",
                self.dockerfile.display()
            )
        })?;

        let dockerignore = build_root.join(".dockerignore");
        let ignore = dockerignore
            .exists()
            .then(|| gitignore::File::new(&dockerignore).ok())
            .flatten();

        let mut buf = Vec::new();
        let enc = GzEncoder::new(&mut buf, Compression::default());
        let mut tar = tar::Builder::new(enc);

        jwalk::WalkDir::new(build_root)
        .skip_hidden(false)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|p| {
            let p = p.path();
            let name = p.strip_prefix(&build_root).unwrap_or(&p);
            if name.components().take(1).next().is_none() {
                tracing::trace!(host_path = ?p, tar_path = ?name, "skipping empty filename");
                return None;
            }
            if let Some(ref ignore) = ignore {
                match ignore.is_excluded(&p) {
                    Ok(true) => {
                        tracing::trace!(host_path = ?p, tar_path = ?name, "skipping ignored filename");
                        return None;
                    },
                    Err(e) => return Some(Err(e).map_err(anyhow::Error::from)),
                    _ => {}
                }
            }
            tracing::debug!(host_path = ?p, tar_path = ?name, "adding file to archive");
            Some(
                tar.append_path_with_name(&p, &name)
                    .with_context(|| format!("failed to add file to tar archive {:?}", &p)),
            )
        })
        .collect::<Result<_>>()?;

        tar.append_dir_all(".", build_root)
            .context("failed to add buildroot to tar archive")?;

        // FIXME: would be nice if we didn't have to clone here
        let buf = tar
            .into_inner()
            .and_then(|t| t.finish())
            .context("failed to write to tar archive")?
            .clone();

        let mut stream = client.build_image(image_options, None, Some(buf.into()));
        while let Some(res) = stream.next().await {
            let info = res.context("failed to send request")?;
            if let Some(err) = info.error {
                return Err(anyhow::anyhow!("{}", err).context("error from docker"));
            }
            if let Some(status) = info.status {
                tracing::trace!(status = ?status, "image build status")
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
}

#[cfg(test)]
mod tests {
    use crate::{docker::util::client, testutils::assert_yaml_deserialize};

    use super::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_image_pull_policy_serde() {
        let p = "never";
        assert_yaml_deserialize::<ImagePullPolicy>(p);

        let p = "
        pull:
            always: true
            sha256: 10203040deadbeef
            repo: quay.io/foobar
        ";
        assert_yaml_deserialize::<ImagePullPolicy>(p);

        let p = "
        build:
            dockerfile: /foo/bar/qux/Dockerfile
            buildArgs:
                foo: 1
                bar: 2
                baz: qux
        ";
        assert_yaml_deserialize::<ImagePullPolicy>(p);
    }

    #[tokio::test]
    #[traced_test]
    #[serial_test::serial]
    async fn test_image_build() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("testdata/imgbuild/Dockerfile");

        let opts = BuildOpts {
            dockerfile: d,
            build_args: HashMap::default(),
        };

        opts.build("foo").await.expect("image should build");

        let client = client().expect("failed to get client");

        client
            .inspect_image("foo")
            .await
            .expect("image should exist");
    }
}
