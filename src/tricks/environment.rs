// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Helpers to launch a guest environment that can run Houdini.

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageOption {
    /// Package to install.
    pkg: String,
    /// Optional package version. Will default to latest.
    version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentOptions {
    #[serde(skip)]
    relative_dir: PathBuf,
    /// Kernel version to compile and use.
    kernel_tag: String,
    /// Path to kernel config.
    kconfig: Option<PathBuf>,
    /// Path to buildroot config.
    buildroot: Option<PathBuf>,
    /// Overrides for kernel config.
    #[serde(default)]
    kconfig_opts: HashMap<String, String>,
    /// Overrides for buildroot config.
    #[serde(default)]
    buildroot_opts: HashMap<String, String>,
    /// Additional packages to install.
    #[serde(default)]
    install: Vec<PackageOption>,
}

impl EnvironmentOptions {}

/// Parse a KEY=VAL config from a reader into a hashmap.
async fn parse_config<T: AsyncBufRead + Unpin>(
    reader: BufReader<T>,
) -> Result<HashMap<String, String>> {
    let mut res = HashMap::default();

    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            res.insert(key.trim().to_owned(), val.trim().to_owned());
        }
    }

    Ok(res)
}

async fn write_config<T: AsyncWrite + Unpin>(
    map: &HashMap<String, String>,
    mut writer: BufWriter<T>,
) -> Result<()> {
    for (k, v) in map.iter() {
        let line = format!("{}={}\n", k, v);
        writer.write(line.as_bytes()).await?;
    }
    writer.flush().await?;
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_parse_config() {
        let config = r#"
            BR2_HAVE_DOT_CONFIG=y
            BR2_EXTERNAL_HOUDINI_PATH="/foo/var/houdini"
            BR2_HOST_GCC_AT_LEAST_4_9=y
            BR2_HOST_GCC_AT_LEAST_5=y
            BR2_HOST_GCC_AT_LEAST_6=y
            BR2_HOST_GCC_AT_LEAST_7=y
            BR2_HOST_GCC_AT_LEAST_8=y
            BR2_HOST_GCC_AT_LEAST_9=y
            # BR2_OPTIMIZE_0 is not set
            # BR2_OPTIMIZE_1 is not set
            # BR2_OPTIMIZE_2 is not set
            # BR2_OPTIMIZE_3 is not set
            # BR2_OPTIMIZE_G is not set
            # foo = qux
            foooooooooooooooooooooooooobaaaaaaaaaaaaaaaaarquuuuuuuuuuuuuuuux
            "#;
        let reader = tokio::io::BufReader::new(config.as_bytes());
        let config_map = parse_config(reader).await.expect("should parse");

        let expected_config_map = HashMap::from(
            [
                ("BR2_HAVE_DOT_CONFIG", "y"),
                ("BR2_EXTERNAL_HOUDINI_PATH", "\"/foo/var/houdini\""),
                ("BR2_HOST_GCC_AT_LEAST_4_9", "y"),
                ("BR2_HOST_GCC_AT_LEAST_5", "y"),
                ("BR2_HOST_GCC_AT_LEAST_6", "y"),
                ("BR2_HOST_GCC_AT_LEAST_7", "y"),
                ("BR2_HOST_GCC_AT_LEAST_8", "y"),
                ("BR2_HOST_GCC_AT_LEAST_9", "y"),
            ]
            .map(|(k, v)| (k.to_string(), v.to_string())),
        );

        assert_eq!(
            config_map, expected_config_map,
            "parsed config map should be same as expected"
        );

        let mut buf = Vec::with_capacity(config.as_bytes().len());
        let writer = BufWriter::new(&mut buf);
        write_config(&config_map, writer)
            .await
            .expect("should write");

        let expected_output = r#"BR2_HAVE_DOT_CONFIG=y
BR2_EXTERNAL_HOUDINI_PATH="/foo/var/houdini"
BR2_HOST_GCC_AT_LEAST_4_9=y
BR2_HOST_GCC_AT_LEAST_5=y
BR2_HOST_GCC_AT_LEAST_6=y
BR2_HOST_GCC_AT_LEAST_7=y
BR2_HOST_GCC_AT_LEAST_8=y
BR2_HOST_GCC_AT_LEAST_9=y
"#;

        assert_eq!(
            String::from_utf8(buf).expect("should be UTF-8"),
            expected_output
        );
    }
}
