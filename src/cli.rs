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

use clap_derive::Parser;

use crate::logging::LoggingFormat;

/// Describes Houdini's command line interface.
#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Cli {
    /// The subcommand to run.
    #[clap(subcommand)]
    subcmd: Cmd,
    /// Verbosity level (-1 or lower is silent, 0 is quiet, 1 is info, 2 is debug, 3 is trace).
    #[clap(long, short, default_value = "1")]
    pub verbose: i8,
    /// Format to use for logging. Auto implies pretty if stdout is a TTY and JSON
    /// otherwise.
    #[clap(arg_enum, long, short, default_value = "auto")]
    pub format: LoggingFormat,
}

/// Enumerates Houdini's various subcommands.
#[derive(Parser, Debug)]
enum Cmd {
    /// Run through the container-level test suite. Should be called from within
    /// a container.
    Container,
}

impl Cli {
    /// Consume the CLI object and run the corresponding subcommand.
    pub fn run(self) -> () {
        // TODO
    }
}
