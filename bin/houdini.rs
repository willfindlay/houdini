// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

use anyhow::{Context, Result};
use clap::StructOpt;
use houdini::{config, Cli};
use std::{fs::DirBuilder, os::unix::fs::DirBuilderExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments.
    let args = Cli::parse();

    // Initialize the "tracing" logger.
    let _guard = houdini::logging::init(&args).await?;

    // We want to log panics in debug mode, but produce a human panic message in release.
    log_panics::init();
    human_panic::setup_panic!();

    // Log initial configs
    tracing::debug!(args = ?&args, "cli args");
    tracing::debug!(config = ?&*config().await, "houdini config");

    init().await.context("failed to initialize environment")?;

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

async fn init() -> Result<()> {
    // Create reports dir
    let dir = &config().await.reports.dir;
    DirBuilder::new()
        .recursive(true)
        .mode(0o755)
        .create(dir)
        .context(format!("failed to create reports dir {}", dir.display()))?;

    // Create log dir dir
    if let Some(file) = &config().await.log.file {
        let dir = file
            .parent()
            .ok_or_else(|| anyhow::anyhow!("no parent directory for log file"))?;

        DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(dir)
            .context(format!("failed to create log dir {}", dir.display()))?;
    }

    Ok(())
}
