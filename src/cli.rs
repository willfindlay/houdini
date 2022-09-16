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

use std::{path::PathBuf, time::Duration};
use tokio::fs::File;

use anyhow::{Context, Result};
use clap::AppSettings;
use clap_derive::Parser;

use crate::{
    api,
    api::{
        client::{
            HoudiniClient, HoudiniUnixClient, HoudiniVsockClient, Wrapper as HoudiniClientWrapper,
        },
        Socket,
    },
    logging::LoggingFormat,
    tricks::{environment::launch_guest, report::Report, Trick},
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
        socket: Option<Socket>,
    },
    /// Run houdini in Guest OS mode. This subcommand will spin up a virtio socket API
    /// server and wait for instructions to come over the socket.
    #[clap(setting = AppSettings::Hidden)]
    Guest {
        /// CID for the virtio socket connection.
        cid: u32,
        /// Port for the virtio socket connection.
        port: u32,
    },
    /// Development and debugging-related subcommands.
    #[clap(setting = AppSettings::Hidden)]
    Debug {
        /// The subcommand to run.
        #[clap(subcommand)]
        subcmd: DebugCmd,
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

/// Debugging and development subcommands for Houdini.
#[derive(Parser, Debug)]
enum DebugCmd {
    /// Run the Houdini API server.
    RunGuest {
        /// CID for the virtio socket connection.
        #[clap(long, short, default_value = "3")]
        cid: u32,
        /// Port for the virtio socket connection.
        #[clap(long, short, default_value = "2375")]
        port: u32,
        /// Path to kernel bzImage.
        #[clap(long, short)]
        bzimage: PathBuf,
        /// Path to init ramdisk.
        #[clap(long, short)]
        initrd: PathBuf,
        /// RAM in GiB to use for the VM.
        #[clap(long, default_value = "4")]
        ram: u32,
        /// Number of CPU cores to use for the VM.
        #[clap(long, default_value = "4")]
        cpu: u32,
        /// The exploit to run.
        trick: PathBuf,
    },
}

impl Cli {
    /// Consume the CLI object and run the corresponding subcommand.
    pub async fn run(self) -> Result<()> {
        match self.subcmd {
            Cmd::Run { tricks } => {
                let mut report = Report::new();

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
            } => api::serve(socket).await?,
            Cmd::Api {
                subcmd: ApiCmd::Client { operation },
                socket,
            } => {
                let client = match socket {
                    Some(Socket::Unix(path)) => {
                        HoudiniClientWrapper::HoudiniUnixClient(HoudiniUnixClient::new(Some(path))?)
                    }
                    Some(Socket::Vsock(cid, port)) => HoudiniClientWrapper::HoudiniVsockClient(
                        HoudiniVsockClient::new(cid, port)?,
                    ),
                    None => HoudiniClientWrapper::HoudiniUnixClient(HoudiniUnixClient::new(None)?),
                };

                match operation {
                    ClientOperation::Ping => match client {
                        HoudiniClientWrapper::HoudiniUnixClient(client) => client.ping().await?,
                        HoudiniClientWrapper::HoudiniVsockClient(client) => client.ping().await?,
                    },
                    ClientOperation::Trick { trick } => {
                        let f = File::open(&trick)
                            .await
                            .context(format!("could not open trick file {}", &trick.display()))?;

                        let trick: Trick = serde_yaml::from_reader(f.into_std().await)
                            .context(format!("failed to parse trick {}", &trick.display()))?;

                        let report = match client {
                            HoudiniClientWrapper::HoudiniUnixClient(client) => {
                                client.trick(&trick).await?
                            }
                            HoudiniClientWrapper::HoudiniVsockClient(client) => {
                                client.trick(&trick).await?
                            }
                        };

                        let out = serde_json::to_string_pretty(&report)?;

                        println!("{}", out);
                    }
                }
            }
            Cmd::Guest { cid, port } => {
                tracing::info!(cid = cid, port = port, "spinning up a guest API server");
                api::serve(Some(api::Socket::Vsock(cid, port))).await?
            }
            Cmd::Debug { subcmd } => match subcmd {
                DebugCmd::RunGuest {
                    cid,
                    port,
                    bzimage,
                    initrd,
                    ram,
                    cpu,
                    trick,
                } => {
                    let mut guest = launch_guest(cid, cpu, ram, bzimage, initrd)?;
                    std::thread::sleep(Duration::from_secs(3));
                    let client = HoudiniVsockClient::new(cid, port)?;

                    let f = File::open(&trick)
                        .await
                        .context(format!("could not open trick file {}", &trick.display()))?;

                    let trick: Trick = serde_yaml::from_reader(f.into_std().await)
                        .context(format!("failed to parse trick {}", &trick.display()))?;

                    let report = client.trick(&trick).await?;
                    let out = serde_json::to_string_pretty(&report)?;
                    println!("{}", out);
                    let _ = guest.kill();
                }
            },
        }

        Ok(())
    }
}
