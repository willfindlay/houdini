
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::process::Stdio;
use anyhow::{bail, Context as _, Result};

use super::{command::ShellCommand, RunStep};

use crate::{
    tricks::status::Status,
};

/// Spawn a container using the docker api.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateEnvironment {

    pub kernel_tag: String,

    pub kconfig: String,

    pub bconfig: String,

    /// Status on failure. Default is SetupFailure.
    #[serde(default = "crate::serde_defaults::default_setup_failure")]
    pub failure: Status,
    /// Status on success. Default is Undecided.
    #[serde(default)]
    pub success: Status,
}

#[async_trait]
impl RunStep for CreateEnvironment {
    async fn do_run(&self) -> Result<()> {
        let bconfig = String::from(&self.bconfig);
        let kconfig = String::from(&self.kconfig);
        create_buildroot_image(bconfig,kconfig);
        launch_image();
        Ok(())
    }

    fn on_success(&self) -> Status {
        self.success
    }

    fn on_failure(&self) -> Status {
        self.failure
    }
}

fn create_buildroot_image(bconfig: String, kconfig: String){
    let buildroot_folder = String::from("~/Desktop/buildroot-bpfcontain/buildroot");

    let mut buildroot_config = String::from("BR2_DEFCONFIG=");
    buildroot_config.push_str(&bconfig);

    let mut kernel_config = String::from("BR2_LINUX_KERNEL_CUSTOM_CONFIG_FILE=");
    kernel_config.push_str(&kconfig);

    let test_cmd = String::from("make");
    let mut test_args = Vec::new();
    test_args.push(String::from("-C"));
    test_args.push(buildroot_folder);
    test_args.push(buildroot_config);
    test_args.push(kernel_config);

    run_environment_command(test_cmd, test_args);


}

fn launch_image(){
    let test_cmd = String::from("qemu-system-x86_64");
    let out = Command::new(&test_cmd)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .arg("-M")
                .arg("pc")
                .arg("-m")
                .arg("2048")
                .arg("-nographic")
                .arg("-smp")
                .arg("1")
                .arg("-kernel")
                .arg("~/Desktop/buildroot-bpfcontain/buildroot/output/images/bzImage")
                .arg("-initrd")
                .arg("~/Desktop/buildroot-bpfcontain/buildroot/output/images/rootfs.cpio")
                .arg("-append")
                .arg("console=tty1 console=ttyS0")
                .arg("-netdev")
                .arg("user,id=n1")
                .arg("-device")
                .arg("e1000,netdev=n1")
                .arg("-device")
                .arg("vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid=3")
                .arg("-netdev")
                .arg("user,id=mynet0,hostfwd=tcp::30022-:22,hostfwd=tcp::32375-:2375")
                .arg("-device")
                .arg("virtio-net-pci,netdev=mynet0")
                .arg("&")
                .output()
                .map_err(anyhow::Error::from)
                .context("failed to run command");
}

fn run_environment_command(cmd: String, args: Vec<String>){
    println!("{}", cmd);
    println!("{:?}", args);
    println!("run_environment_command executing");
    let out = Command::new(&cmd)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .args(&args)
                .output()
                .map_err(anyhow::Error::from)
                .context("failed to run command");
}
