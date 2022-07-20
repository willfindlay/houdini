// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! An interface for running commands in a Docker container.

use std::ops::Deref;

use anyhow::Result;
use bollard::exec::{CreateExecOptions, StartExecOptions};
// use docker_api::{api::ContainerId, Docker, Exec, ExecContainerOpts};
use futures::StreamExt;

/// Determines what the Command does with stdio from the container exec.
pub enum Stdio {
    /// Ignore stdio
    Null,
    /// Use caller's stdio
    Inherit,
    /// Write stdio to vec
    Piped,
}

/// Wraps the exit code and stdio of the container exec.
#[derive(Default)]
pub struct Output {
    pub code: Option<ExitCode>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Wraps an exit code for a container exec.
pub struct ExitCode(pub i64);

impl ExitCode {
    /// Was the command successful?
    pub fn success(&self) -> bool {
        self.0 == 0
    }
}

impl Deref for ExitCode {
    type Target = i64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Command is a builder for running commands in a Docker container.
pub struct Command {
    id: String,
    command: String,
    args: Vec<String>,
    tty: bool,
    privileged: bool,
    // stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
}

impl Command {
    /// Construct a new command that runs `program` inside of `container` where `container`
    /// is a container name or container id.
    pub fn new<S: AsRef<str>>(container: String, program: S) -> Self {
        return Self {
            id: container,
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

    /// Sets program stdout to `stdout`.
    pub fn stdout(&mut self, stdout: Stdio) -> &mut Self {
        self.stdout = Some(stdout);
        self
    }

    /// Sets program stderr to `stderr`.
    pub fn stderr(&mut self, stderr: Stdio) -> &mut Self {
        self.stderr = Some(stderr);
        self
    }

    async fn exec(&mut self) -> Result<Output> {
        let client = super::util::client().await?;

        let opts = CreateExecOptions {
            attach_stdin: Some(false),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(self.tty),
            cmd: Some(
                std::iter::once(self.command.clone())
                    .chain(self.args.iter().cloned())
                    .collect(),
            ),
            privileged: Some(self.privileged),
            ..Default::default()
        };

        let exec = client.create_exec(&self.id, opts).await?.id;

        let opts = StartExecOptions {
            detach: false,
            ..Default::default()
        };
        let results = client.start_exec(&exec, Some(opts)).await?;

        let mut cmd_out = Output::default();

        match results {
            bollard::exec::StartExecResults::Attached { mut output, .. } => {
                while let Some(Ok(output)) = output.next().await {
                    match output {
                        bollard::container::LogOutput::StdErr { message } => cmd_out
                            .stdout
                            .append(&mut message.iter().cloned().collect()),
                        bollard::container::LogOutput::StdOut { message } => cmd_out
                            .stdout
                            .append(&mut message.iter().cloned().collect()),
                        _ => continue,
                    }
                }
            }
            bollard::exec::StartExecResults::Detached => unreachable!(),
        }

        let inspect = client.inspect_exec(&exec).await?;
        cmd_out.code = inspect.exit_code.map(ExitCode);

        Ok(cmd_out)
    }

    /// Run the program in the docker container and return its output.
    pub async fn output(&mut self) -> Result<Output> {
        self.exec().await
    }

    /// Run the program in the docker container and return its status.
    pub async fn status(&mut self) -> Result<Option<ExitCode>> {
        let output = self.exec().await?;
        Ok(output.code)
    }
}
