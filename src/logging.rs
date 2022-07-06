// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! This module contains helper functions to set up logging for Houdini.

use crate::{cli, CONFIG};
use anyhow::Result;
use clap_derive::ArgEnum;
use std::{fmt::Display, path::PathBuf};
use tracing::metadata::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::Filtered, layer::Layered, prelude::__tracing_subscriber_SubscriberExt, EnvFilter,
    Layer, Registry,
};

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

fn registry(
    args: &cli::Cli,
) -> Layered<Filtered<EnvFilter, tracing::level_filters::LevelFilter, Registry>, Registry> {
    let filter = EnvFilter::default();

    // Set verbosity
    let filter = match args.verbose {
        n if n < 0 => filter.with_filter(LevelFilter::OFF),
        0 => filter.with_filter(LevelFilter::WARN),
        1 => filter.with_filter(LevelFilter::INFO),
        2 => filter.with_filter(LevelFilter::DEBUG),
        _ => filter.with_filter(LevelFilter::TRACE),
    };

    Registry::default().with(filter)
}

fn init_human(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let log_dir = CONFIG.log.file.parent();
    let log_file = CONFIG.log.file.file_name();

    let (file_appender, guard) = if let (Some(log_dir), Some(log_file)) = (log_dir, log_file) {
        let file_appender = tracing_appender::rolling::daily(log_dir, log_file);
        let (file_appender, guard) = tracing_appender::non_blocking(file_appender);
        (Some(file_appender), Some(guard))
    } else {
        (None, None)
    };

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_level(true)
        .with_thread_ids(false)
        .with_line_number(true)
        .with_thread_names(true);

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = registry(args).with(stdout_layer).with(file_layer);
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(guard)
}

fn init_json(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let file = &CONFIG.log.file.to_string_lossy();
    let file = PathBuf::from(shellexpand::tilde(file).as_ref());
    let log_dir = file.parent();
    let log_file = file.file_name();

    let (file_appender, guard) = if let (Some(log_dir), Some(log_file)) = (log_dir, log_file) {
        let file_appender = tracing_appender::rolling::daily(log_dir, log_file);
        let (file_appender, guard) = tracing_appender::non_blocking(file_appender);
        (Some(file_appender), Some(guard))
    } else {
        (None, None)
    };

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_level(true)
        .with_thread_ids(false)
        .with_line_number(true)
        .with_thread_names(true)
        .json();

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = registry(args).with(stdout_layer).with(file_layer);
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(guard)
}
fn init_compact(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let log_dir = CONFIG.log.file.parent();
    let log_file = CONFIG.log.file.file_name();

    let (file_appender, guard) = if let (Some(log_dir), Some(log_file)) = (log_dir, log_file) {
        let file_appender = tracing_appender::rolling::daily(log_dir, log_file);
        let (file_appender, guard) = tracing_appender::non_blocking(file_appender);
        (Some(file_appender), Some(guard))
    } else {
        (None, None)
    };

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_level(true)
        .with_thread_ids(false)
        .with_line_number(true)
        .with_thread_names(true)
        .compact();

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = registry(args).with(stdout_layer).with(file_layer);
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(guard)
}
fn init_pretty(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let log_dir = CONFIG.log.file.parent();
    let log_file = CONFIG.log.file.file_name();

    let (file_appender, guard) = if let (Some(log_dir), Some(log_file)) = (log_dir, log_file) {
        let file_appender = tracing_appender::rolling::daily(log_dir, log_file);
        let (file_appender, guard) = tracing_appender::non_blocking(file_appender);
        (Some(file_appender), Some(guard))
    } else {
        (None, None)
    };

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_level(true)
        .with_thread_ids(false)
        .with_line_number(true)
        .with_thread_names(true)
        .pretty();

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = registry(args).with(stdout_layer).with(file_layer);
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(guard)
}

/// Initialize the logger by setting the right subscriber.
pub fn init(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let tracing_format = match args.format {
        LoggingFormat::Auto => {
            if atty::is(atty::Stream::Stderr) {
                LoggingFormat::Pretty
            } else {
                LoggingFormat::Json
            }
        }
        format => format,
    };

    let guard = match tracing_format {
        LoggingFormat::Auto => unreachable!(),
        LoggingFormat::Json => init_json(args)?,
        LoggingFormat::Pretty => init_pretty(args)?,
        LoggingFormat::Full => init_human(args)?,
        LoggingFormat::Compact => init_compact(args)?,
    };

    tracing_log::LogTracer::init()?;

    Ok(guard)
}
