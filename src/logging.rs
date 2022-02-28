// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module contains helper functions to set up logging for Houdini.

use crate::cli;
use clap_derive::ArgEnum;
use std::fmt::Display;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{fmt::SubscriberBuilder, FmtSubscriber};

/// Formatter to use in the logging subscriber.
/// [`Auto`] implies pretty if the target is a TTY, JSON otherwise.
#[derive(Debug, ArgEnum, Clone, Copy)]
pub enum LoggingFormat {
    /// Implies Json if stderr is a file, else Full
    Auto,
    /// Pretty logs log messages on multiple lines
    Pretty,
    /// Human logs in a human-readable format
    Full,
    /// Compact is a more compact version of Full
    Compact,
    /// Json logs in a machine-readable JSON format
    Json,
}

impl Display for LoggingFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// It is necessary to split up the builder like this because mutating the formatter
// changes its type. See the beginning of init() for details.
fn builder(args: &cli::Cli) -> SubscriberBuilder {
    let builder = FmtSubscriber::builder();

    // Set verbosity
    let builder = match args.verbose {
        n if n < 0 => builder.with_max_level(LevelFilter::OFF),
        0 => builder.with_max_level(LevelFilter::WARN),
        1 => builder.with_max_level(LevelFilter::INFO),
        2 => builder.with_max_level(LevelFilter::DEBUG),
        _ => builder.with_max_level(LevelFilter::TRACE),
    };

    // Set remaining options
    let builder = builder
        .with_level(true)
        .with_thread_ids(false)
        .with_line_number(true)
        .with_thread_names(true);

    builder
}

/// Initialize the logger by setting the right subscriber.
pub fn init(args: &cli::Cli) {
    // Setting the formatter mutates the builder's type, so construct a unique subscriber
    // for each possible formatter type and set the correct susbcriber below.
    let human_subscriber = builder(args).with_writer(std::io::stderr).finish();
    let json_subscriber = builder(args).with_writer(std::io::stderr).json().finish();
    let pretty_subscriber = builder(args).with_writer(std::io::stderr).pretty().finish();
    let compact_subscriber = builder(args)
        .with_writer(std::io::stderr)
        .compact()
        .finish();

    let tracing_format = match args.format.clone() {
        LoggingFormat::Auto => {
            if atty::is(atty::Stream::Stderr) {
                LoggingFormat::Pretty
            } else {
                LoggingFormat::Json
            }
        }
        format => format,
    };

    match tracing_format {
        LoggingFormat::Auto => unreachable!(),
        LoggingFormat::Json => tracing::subscriber::set_global_default(json_subscriber)
            .expect("setting tracing default has failed"),
        LoggingFormat::Pretty => tracing::subscriber::set_global_default(pretty_subscriber)
            .expect("setting tracing default has failed"),
        LoggingFormat::Full => tracing::subscriber::set_global_default(human_subscriber)
            .expect("setting tracing default has failed"),
        LoggingFormat::Compact => tracing::subscriber::set_global_default(compact_subscriber)
            .expect("setting tracing default has failed"),
    }

    tracing_log::LogTracer::init().expect("failed to initialize tracing compatibility layer");
}
