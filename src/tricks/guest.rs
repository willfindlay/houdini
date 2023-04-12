// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Helpers to launch a guest environment that can run Houdini.

use anyhow::{Context as _, Result};
use itertools::Itertools;
use scopeguard::defer;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tar::Archive;

use crate::docker::{export_rootfs, ImagePullPolicy};

pub trait IntoQemuArg {
    fn into_qemu_arg(&self) -> String;
}

/// Possible sources for kernel and rootfs images.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields, untagged)]
pub enum GuestSource {
    Single(Image),
    List(Vec<Image>),
    Matrix(ImageMatrix),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageSpec {
    /// Container image to use.
    pub image: String,
    /// A policy for what to do when an image is not available. Defaults to pulling
    /// from docker hub if the image does not exist and _no_ SHA256 verification.
    #[serde(default)]
    pub image_policy: ImagePullPolicy,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageMatrix {
    /// List of OCI images that contain a kernel bzImage for the guest.
    pub kernel: Vec<ImageSpec>,
    /// List of OCI images that contain a root filesystem for the guest.
    pub rootfs: Vec<ImageSpec>,
}

impl IntoIterator for ImageMatrix {
    type Item = Image;

    type IntoIter = std::vec::IntoIter<Image>;

    fn into_iter(self) -> Self::IntoIter {
        self.kernel
            .into_iter()
            .cartesian_product(self.rootfs)
            .into_iter()
            .map(|(kernel, rootfs)| Image {
                kernel: Some(kernel),
                rootfs,
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Image {
    pub kernel: Option<ImageSpec>,
    pub rootfs: ImageSpec,
}

impl Image {
    async fn maybe_pull_kernel(&self) -> Result<()> {
        if let Some(ref kernel) = self.kernel {
            kernel
                .image_policy
                .acquire_image(&kernel.image)
                .await
                .context("Failed to acquire kernel OCI image")?;
        }
        Ok(())
    }

    async fn pull_rootfs(&self) -> Result<()> {
        self.rootfs
            .image_policy
            .acquire_image(&self.rootfs.image)
            .await
            .context("Failed to acquire root filesystem OCI image")?;
        Ok(())
    }

    pub async fn create_and_populate_filesystem(&self, size: usize) -> Result<PathBuf> {
        // Create root filesystem
        let fs_path = create_filesystem(size, FileSystem::Ext4)
            .await
            .context("Failed to create root filesystem")?;

        // Create a temporary directory and mount to it
        let dir = tempfile::TempDir::new().context("Failed to create temporary directory")?;
        let dir_path = dir.path();
        let status = Command::new("mount")
            .arg(format!("{}", fs_path.display()))
            .arg(format!("{}", dir_path.display()))
            .status()?;
        if !status.success() {
            anyhow::bail!("failed to mount root filesystem")
        }

        defer! {
            let _ = std::process::Command::new("umount")
                .arg(format!("{}", dir_path.display()))
                .status();
        }

        // Populate the root filesystem
        let file = tempfile::NamedTempFile::new()?;
        self.pull_rootfs().await?;
        export_rootfs(&self.rootfs.image, file.path()).await?;

        let mut archive = Archive::new(file);
        for entry in archive.entries()? {
            let mut entry = entry?;
            let _ = entry.unpack_in(dir_path);
        }

        Ok(fs_path)
    }
}

/// Create an empty file to back a filesystem
async fn create_empty_file(size: usize) -> Result<PathBuf> {
    let file = tempfile::NamedTempFile::new_in(std::env::current_dir()?)?;
    let path = file.into_temp_path().to_path_buf();
    let file = tokio::fs::File::create(&path).await?;
    file.set_len(size as u64).await?;
    file.sync_all().await?;
    Ok(path)
}

/// Create an empty ext4 filesystem
async fn create_filesystem(size: usize, fs_type: FileSystem) -> Result<PathBuf> {
    let path = create_empty_file(size).await?;
    let status = tokio::process::Command::new("mkfs")
        .arg("-t")
        .arg(&fs_type.to_string())
        .arg(&path.display().to_string())
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("Failed to run mkfs")
    }
    Ok(path)
}

/// Filesystems supported by Houdini.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum FileSystem {
    Ext4,
}

impl Display for FileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileSystem::Ext4 => write!(f, "ext4"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuestOptions {
    /// Name of OCI image that contains a kernel for the guest.
    pub kernel_image: String,
    /// Name of OCI image that contains a root filesystem for the guest.
    pub rootfs_image: String,
    /// Number of CPUs to use for the guest. Default is 1.
    #[serde(default = "crate::serde_defaults::default_one_u32")]
    pub ncpus: u32,
    /// Memory to assign to the VM in GB. Default is 2GB.
    #[serde(default = "crate::serde_defaults::default_two_u32")]
    pub memory: u32,
}

pub(crate) fn launch_guest<P: AsRef<Path>>(
    cid: u32,
    ncpus: u32,
    memory: u32,
    kernel_image: P,
    initrd: P,
) -> Result<std::process::Child> {
    let test_cmd = String::from("qemu-system-x86_64");
    let out = Command::new(&test_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-M")
        .arg("pc")
        .arg("-m")
        .arg(format!("{}G", memory.to_string()))
        .arg("-nographic")
        .arg("-smp")
        .arg(ncpus.to_string().as_str())
        .arg("-kernel")
        .arg(kernel_image.as_ref().display().to_string().as_str())
        .arg("-initrd")
        .arg(initrd.as_ref().display().to_string().as_str())
        .arg("-append")
        .arg("console=ttyS0")
        .arg("-netdev")
        .arg("user,id=n1")
        .arg("-device")
        .arg("e1000,netdev=n1")
        .arg("-device")
        .arg(format!(
            "vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid={}",
            cid
        ))
        .arg("-netdev")
        .arg("user,id=mynet0")
        .arg("-device")
        .arg("virtio-net-pci,netdev=mynet0")
        .spawn()
        .map_err(anyhow::Error::from)
        .context("failed to run command")?;

    Ok(out)
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageOption {
    /// Package to install.
    pub pkg: String,
    /// Optional package version. Will default to latest.
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_empty_file_test() {
        let path = create_empty_file(13)
            .await
            .expect("file creation should succeed");

        println!("{}", path.display());
        assert_eq!(
            tokio::fs::File::open(&path)
                .await
                .expect("file should open")
                .metadata()
                .await
                .expect("should fetch metadata")
                .len(),
            13
        );

        tokio::fs::remove_file(&path)
            .await
            .expect("file deletion should succeed");
    }

    #[tokio::test]
    async fn create_filesystem_test() {
        let path = create_filesystem(4 * 1024_usize.pow(2), FileSystem::Ext4)
            .await
            .expect("file creation should succeed");
        std::fs::remove_file(path).expect("file should be removed");
    }

    #[tokio::test]
    async fn create_and_populate_filesystem_test() {
        let image = Image {
            kernel: None,
            rootfs: ImageSpec {
                image: "houndini-guest".into(),
                image_policy: ImagePullPolicy::Never,
            },
        };
        image
            .create_and_populate_filesystem(40 * 1024_usize.pow(2))
            .await
            .expect("filesystem creation should succeed");
    }
}
