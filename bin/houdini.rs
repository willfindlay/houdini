// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

use anyhow::Result;
use clap::StructOpt;
use houdini::{Cli, CONFIG};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments.
    let args = Cli::parse();

    // Initialize the "tracing" logger.
    let _guard = houdini::logging::init(&args)?;

    // We want to log panics in debug mode, but produce a human panic message in release.
    log_panics::init();
    human_panic::setup_panic!();

    // Initialize config file
    let _span = tracing::trace_span!("main", args = ?&args, config = ?&*CONFIG).entered();

    // After parsing arguments, we can consume them and run the corresponding subcommand.
    match args.run().await {
        Ok(()) => Ok(()),
        // We want to print any fatal errors to the logs, rather tha simply printing them
        // to stderr.
        Err(e) => {
            let errs: Vec<_> = e.chain().skip(1).map(|e| e.to_string()).collect();
            tracing::error!(err = &*e.to_string(), caused_by = ?errs, "fatal error");
            Err(e)
        }
    }
}
