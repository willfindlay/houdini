// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! An interface for running commands in a Docker container.

use std::{io::Write, ops::Deref};

use anyhow::{bail, Result};
use docker_api::{api::ContainerId, Docker, Exec, ExecContainerOpts};
use futures::StreamExt;

use crate::CONFIG;

pub struct ExitCode(pub u64);

impl ExitCode {
    pub fn success(&self) -> bool {
        self.0 == 0
    }
}

impl Deref for ExitCode {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Command is a builder for running commands in a Docker container.
pub struct Command {
    id: ContainerId,
    command: String,
    args: Vec<String>,
    tty: bool,
    privileged: bool,
    // stdin: Option<Stdio>,
    stdout: Option<Box<dyn std::io::Write>>,
    stderr: Option<Box<dyn std::io::Write>>,
}

impl Command {
    /// Construct a new command that runs `program` inside of `container` where `container`
    /// is a container name or container id.
    pub fn new<S: AsRef<str>, ID: Into<ContainerId>>(container: ID, program: S) -> Self {
        return Self {
            id: container.into(),
            command: program.as_ref().to_owned(),
            args: Default::default(),
            tty: false,
            privileged: false,
            stdout: None,
            stderr: None,
        };
    }

    /// Adds a single argument to be passed to the program.
    pub fn arg<S: AsRef<str>>(&mut self, arg: S) -> &mut Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    /// Adds multiple arguments to be passed to the program.
    pub fn args<S: AsRef<str>, I: IntoIterator<Item = S>>(&mut self, args: I) -> &mut Self {
        for arg in args {
            self.args.push(arg.as_ref().to_owned())
        }
        self
    }

    /// Attach a TTY for this exec.
    pub fn tty(&mut self, tty: bool) -> &mut Self {
        self.tty = tty;
        self
    }

    /// Run this command with elevated privilegeds.
    pub fn privileged(&mut self, privileged: bool) -> &mut Self {
        self.privileged = privileged;
        self
    }

    // /// Sets program stdin to `stdin`.
    // pub fn stdin(&mut self, stdin: Box<dyn std::io::Read>) -> &mut Self {
    //     self.stdin = Some(stdin);
    //     self
    // }

    /// Sets program stdout to `stdout`.
    pub fn stdout(&mut self, stdout: Box<dyn std::io::Write>) -> &mut Self {
        self.stdout = Some(stdout);
        self
    }

    /// Sets program stderr to `stderr`.
    pub fn stderr(&mut self, stderr: Box<dyn std::io::Write>) -> &mut Self {
        self.stderr = Some(stderr);
        self
    }

    /// Run the program in the docker container and return its status.
    ///
    /// TODO: Have this return a Status object instead
    pub async fn status(&mut self) -> Result<ExitCode> {
        let client = Docker::unix(&CONFIG.docker.socket);
        let cmd = std::iter::once(&self.command).chain(&self.args);

        let builder = ExecContainerOpts::builder()
            .cmd(cmd)
            .tty(self.tty)
            .privileged(self.privileged);
        let opts = builder.build();

        let exec = Exec::create(client, &self.id, &opts).await?;
        let mut stream = exec.start();
        while let Some(res) = stream.next().await {
            if let Ok(tty) = res {
                match tty {
                    docker_api::conn::TtyChunk::StdIn(buf) => {
                        // docker_api doesn't seem to support this from exec endpoint atm
                        // TODO maybe switch to bollard which does support this
                        unreachable!()
                    }
                    docker_api::conn::TtyChunk::StdOut(buf) => {
                        if let Some(writer) = self.stdout.as_mut() {
                            writer.write_all(&buf)?;
                        }
                    }
                    docker_api::conn::TtyChunk::StdErr(buf) => {
                        if let Some(writer) = self.stderr.as_mut() {
                            writer.write_all(&buf)?;
                        }
                    }
                }
            }
        }

        let info = exec.inspect().await?;
        if let Some(code) = info.exit_code {
            Ok(ExitCode(code))
        } else {
            bail!("command did not complete")
        }
    }
}
