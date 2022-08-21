// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Helpers to launch a guest environment that can run Houdini.

use anyhow::{Context as _, Result};
use std::{
    path::Path,
    process::{Command, Stdio},
};

pub(crate) fn create_buildroot_image(
    buildroot_folder: String,
    buildroot_config: String,
    kernel_config: String,
) -> Result<()> {
    let buildroot_config = format!("BR2_DEFCONFIG={}", buildroot_config);
    let kernel_config = format!("BR2_LINUX_KERNEL_CUSTOM_CONFIG_FILE={}", kernel_config);

    let out = Command::new("make")
        .arg("-C")
        .arg(buildroot_folder)
        .arg(buildroot_config)
        .arg(kernel_config)
        .output()
        .context("failed to run command")?;

    match out.status.success() {
        true => Ok(()),
        false => {
            anyhow::bail!("error while running buildroot: {}", out.status);
        }
    }
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
        .arg(memory.to_string().as_str())
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
        // TODO: I don't think we need port forwarding, delete this?
        // .arg("-netdev")
        // .arg("user,id=mynet0,hostfwd=tcp::30022-:22,hostfwd=tcp::32375-:2375")
        .arg("-device")
        .arg("virtio-net-pci,netdev=mynet0")
        .spawn()
        .map_err(anyhow::Error::from)
        .context("failed to run command")?;

    Ok(out)
}
