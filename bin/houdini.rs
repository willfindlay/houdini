// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

use std::process::exit;

use clap::StructOpt;
use houdini::*;

fn main() {
    // Parse command line arguments.
    let args = Cli::parse();
    // Initialize the "tracing" logger.
    houdini::logging::init(&args);

    // We want to log panics in debug mode, but produce a human panic message in release.
    log_panics::init();
    human_panic::setup_panic!();

    // After parsing arguments, we can consume them and run the corresponding subcommand.
    match args.run() {
        Ok(()) => {}
        // We want to print any fatal errors to the logs, rather tha simply printing them
        // to stderr.
        Err(e) => {
            tracing::error!(err = &*e.to_string(), "Fatal error");
            exit(1)
        }
    }
}
