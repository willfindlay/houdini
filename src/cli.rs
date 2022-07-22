// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module comprises Houdini's CLI, including arguments, subcommands, and main
//! entrypoint logic. Its public interface is [`Cli::run()`], which consumes [`Cli`]
//! and executes the corresponding subcommand.

use std::path::PathBuf;
use tokio::fs::File;

use anyhow::{Context, Result};
use clap_derive::Parser;

use crate::{
    exploits::{report::Report, Plan},
    logging::LoggingFormat,
};

/// Describes Houdini's command line interface.
#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Cli {
    /// The subcommand to run.
    #[clap(subcommand)]
    subcmd: Cmd,
    /// Verbosity level (-1 or lower is silent, 0 is quiet, 1 is info, 2 is debug, 3 is trace).
    #[clap(global = true, long, short, default_value = "1")]
    pub verbose: i8,
    /// Format to use for logging. Auto implies pretty if stdout is a TTY and JSON
    /// otherwise.
    #[clap(global = true, arg_enum, long, short, default_value = "auto")]
    pub format: LoggingFormat,
}

/// Enumerates Houdini's various subcommands.
#[derive(Parser, Debug)]
enum Cmd {
    /// Run one or more container exploits and test whether they complete successfully.
    Run {
        /// The exploit to run.
        #[clap(min_values = 1, required = true)]
        exploits: Vec<PathBuf>,
    },
}

impl Cli {
    /// Consume the CLI object and run the corresponding subcommand.
    pub async fn run(self) -> Result<()> {
        use crate::exploits::ExploitStatus;
        match self.subcmd {
            Cmd::Run { exploits } => {
                let mut report = Report::default();

                for exploit in exploits {
                    let f = File::open(&exploit).await.context(format!(
                        "could not open exploit file {}",
                        &exploit.display()
                    ))?;
                    let plan: Plan = serde_yaml::from_reader(f.into_std().await).context(
                        format!("failed to parse exploit file {}", &exploit.display()),
                    )?;

                    let status = plan.run(Some(&mut report)).await;
                    match status {
                        ExploitStatus::Undecided
                        | ExploitStatus::SetupFailure
                        | ExploitStatus::ExploitFailure => {
                            tracing::info!(status = ?status, "plan execution FAILED");
                        }
                        ExploitStatus::ExploitSuccess => {
                            tracing::info!(status = ?status, "plan execution SUCCEEDED");
                        }
                        ExploitStatus::Skip => {
                            tracing::info!(status = ?status, "plan execution SKIPPED");
                        }
                    }
                }

                report
                    .write_to_disk()
                    .await
                    .context("failed to write report to disk")?;
            }
        }

        Ok(())
    }
}

// fn path_validator(path: &str) -> Result<PathBuf, std::io::Error> {
//     let path = PathBuf::from(path);
//     if !path.exists() {
//         return Err();
//     }
//     Ok(path)
// }
