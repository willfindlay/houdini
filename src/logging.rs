// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

use crate::cli;
use clap_derive::ArgEnum;
use std::fmt::Display;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{fmt::SubscriberBuilder, FmtSubscriber};

/// Formatter to use in the logging subscriber.
/// [`Auto`] implies pretty if the target is a TTY, JSON otherwise.
#[derive(Debug, ArgEnum, Clone)]
pub enum LoggingFormat {
    Auto,
    Pretty,
    Human,
    Compact,
    Json,
}

impl Display for LoggingFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

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
        .with_thread_names(true);

    builder
}

pub fn init(args: &cli::Cli) {
    let human_subscriber = builder(args).finish();
    let json_subscriber = builder(args).json().finish();
    let pretty_subscriber = builder(args).pretty().with_thread_ids(true).finish();
    let compact_subscriber = builder(args).compact().finish();

    let tracing_format = match args.format.clone() {
        LoggingFormat::Auto => {
            if atty::is(atty::Stream::Stdout) {
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
        LoggingFormat::Human => tracing::subscriber::set_global_default(human_subscriber)
            .expect("setting tracing default has failed"),
        LoggingFormat::Compact => tracing::subscriber::set_global_default(compact_subscriber)
            .expect("setting tracing default has failed"),
    }
}
