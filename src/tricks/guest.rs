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
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::docker::ImagePullPolicy;

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
            .map(|(kernel, rootfs)| Image { kernel, rootfs })
            .collect::<Vec<_>>()
            .into_iter()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Image {
    pub kernel: ImageSpec,
    pub rootfs: ImageSpec,
}

impl Image {
    async fn pull(&self) -> Result<()> {
        self.kernel
            .image_policy
            .acquire_image(&self.kernel.image)
            .await?;
        self.rootfs
            .image_policy
            .acquire_image(&self.rootfs.image)
            .await?;
        Ok(())
    }
}

/// Filesystems supported by Houdini.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum FileSystem {}

async fn oci_to_disk_image(image: &str, outfile: &str, filesystem: FileSystem) {}

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
        .arg("console=tty1 console=ttyS0")
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
