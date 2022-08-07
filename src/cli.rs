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
    api,
    logging::LoggingFormat,
    tricks::{report::Report, Trick},
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
        /// The exploits to run.
        #[clap(min_values = 1, required = true)]
        tricks: Vec<PathBuf>,
    },
    /// The Houdini API.
    Api {
        /// The subcommand to run.
        #[clap(subcommand)]
        subcmd: ApiCmd,
        /// The path to the Houdini socket. Defaults to the value in Houdini configs.
        #[clap(global = true, long, short)]
        socket: Option<PathBuf>,
    },
}

/// Subcommands for Houdini API server.
#[derive(Parser, Debug)]
enum ApiCmd {
    /// Run the Houdini API server.
    Serve,
    /// Interact with the Houdini API server.
    Client {
        /// The operation to perform.
        #[clap(subcommand)]
        operation: ClientOperation,
    },
}

/// Operations for Houdini client.
#[derive(Parser, Debug)]
enum ClientOperation {
    /// Ping the Houdini API server.
    Ping,
    /// Run a trick on the server and get back the result.
    Trick {
        /// The exploit to run.
        trick: PathBuf,
    },
}

impl Cli {
    /// Consume the CLI object and run the corresponding subcommand.
    pub async fn run(self) -> Result<()> {
        match self.subcmd {
            Cmd::Run { tricks } => {
                let mut report = Report::default();

                for file in tricks {
                    let f = File::open(&file)
                        .await
                        .context(format!("could not open trick file {}", &file.display()))?;

                    let trick: Trick = serde_yaml::from_reader(f.into_std().await)
                        .context(format!("failed to parse trick {}", &file.display()))?;

                    report.add(trick.run().await);
                }

                report
                    .write_to_disk()
                    .await
                    .context("failed to write report to disk")?;
            }
            Cmd::Api {
                subcmd: ApiCmd::Serve,
                socket,
            } => {
                api::serve(socket.as_deref()).await?;
            }
            Cmd::Api {
                subcmd: ApiCmd::Client { operation },
                socket,
            } => {
                let client = api::client::HoudiniClient::new(socket.as_deref())
                    .context("failed to parse API socket URL")?;

                match operation {
                    ClientOperation::Ping => client.ping().await?,
                    ClientOperation::Trick { trick } => {
                        let f = File::open(&trick)
                            .await
                            .context(format!("could not open trick file {}", &trick.display()))?;

                        let trick: Trick = serde_yaml::from_reader(f.into_std().await)
                            .context(format!("failed to parse trick {}", &trick.display()))?;

                        let report = client.trick(&trick).await?;
                        let out = serde_json::to_string_pretty(&report)?;

                        println!("{}", out);
                    }
                }
            }
        }

        Ok(())
    }
}
