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
use std::{ffi::OsString, fmt::Display, path::PathBuf};
use tracing::metadata::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Layer, Registry};

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

struct LevelFilterLayer {
    level: LevelFilter,
}

impl LevelFilterLayer {
    pub fn from_args(args: &cli::Cli) -> Self {
        let level = match args.verbose {
            n if n < 0 => LevelFilter::OFF,
            0 => LevelFilter::WARN,
            1 => LevelFilter::INFO,
            2 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        };
        Self { level }
    }

    // TODO: Allow this to be dead code for now. Will be used later.
    #[allow(dead_code)]
    pub fn from_cfg() -> Self {
        Self {
            level: CONFIG.log.level.into(),
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for LevelFilterLayer {
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.level.enabled(metadata, ctx)
    }
}

fn get_log_file() -> Result<(Option<PathBuf>, Option<OsString>)> {
    let file = &CONFIG.log.file;
    let file = match file {
        Some(f) => f,
        None => return Ok((None, None)),
    };

    let log_dir = file
        .parent()
        .ok_or_else(|| anyhow::anyhow!("unable to get log directory from config"))?;
    let log_file = file
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("unable to get log file name from config"))?;

    Ok((Some(log_dir.to_owned()), Some(log_file.to_owned())))
}

fn init_human(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let (log_dir, log_file) = get_log_file()?;

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
        .and_then(LevelFilterLayer::from_args(args));

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = Registry::default().with(stdout_layer).with(file_layer);
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(guard)
}

fn init_json(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let (log_dir, log_file) = get_log_file()?;

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
        .json()
        .and_then(LevelFilterLayer::from_args(args));

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = Registry::default().with(stdout_layer).with(file_layer);
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(guard)
}
fn init_compact(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let (log_dir, log_file) = get_log_file()?;

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
        .compact()
        .and_then(LevelFilterLayer::from_args(args));

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = Registry::default().with(stdout_layer).with(file_layer);
        tracing::subscriber::set_global_default(subscriber)?;
    }

    Ok(guard)
}
fn init_pretty(args: &cli::Cli) -> Result<Option<WorkerGuard>> {
    let (log_dir, log_file) = get_log_file()?;

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
        .pretty()
        .and_then(LevelFilterLayer::from_args(args));

    if let Some(file_appender) = file_appender {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .json();
        let subscriber = Registry::default().with(stdout_layer).with(file_layer);
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
