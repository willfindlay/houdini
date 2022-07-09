// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! The logic used to configure Houdini.

use anyhow::Result;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::path::PathBuf;

lazy_static! {
    /// The shared configuration object for Houdini.
    pub static ref CONFIG: Config = Config::new().expect("Failed to initialize config");
}

/// The base level config for Houdini.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Configuration specific to Docker.
    pub docker: Docker,
    /// Configuration specific to the logger.
    pub log: Log,
}

/// Configuration specific to Docker.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Docker {
    /// Name of the Docker client binary.
    pub client: String,
    /// Name of the Docker daemon binary.
    pub daemon: String,
    /// Name of the container runtime binary.
    pub runtime: String,
    /// Full path to the Docker socket.
    pub socket: PathBuf,
}

/// Configuration specific to Houdini's logger.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    /// Path to the log file.
    pub file: Option<PathBuf>,
    #[serde(default)]
    /// Log file verbosity.
    pub level: LevelFilter,
}

/// Level filter for logging.
#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub enum LevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LevelFilter {
    fn default() -> Self {
        LevelFilter::Info
    }
}

impl Into<tracing::metadata::LevelFilter> for LevelFilter {
    fn into(self) -> tracing::metadata::LevelFilter {
        match self {
            LevelFilter::Trace => tracing::metadata::LevelFilter::TRACE,
            LevelFilter::Debug => tracing::metadata::LevelFilter::DEBUG,
            LevelFilter::Info => tracing::metadata::LevelFilter::INFO,
            LevelFilter::Warn => tracing::metadata::LevelFilter::WARN,
            LevelFilter::Error => tracing::metadata::LevelFilter::ERROR,
        }
    }
}

impl Config {
    /// Construct a new Config.
    fn new() -> Result<Self> {
        let builder = config::Config::builder();

        // Add defaults
        let builder = builder.add_source(config::File::from_str(
            include_str!("config/defaults.toml"),
            config::FileFormat::Toml,
        ));
        // Add config file if it exists
        let builder = if let Some(config_file) = get_config_file() {
            let config_file = config_file.to_string_lossy();
            tracing::info!(file = debug(&config_file), "Reading config file");
            builder.add_source(config::File::with_name(&config_file).required(false))
        } else {
            builder
        };

        builder
            .build()?
            .try_deserialize()
            .map_err(anyhow::Error::from)
    }
}

/// Get the location for Houdini's config file. When compiled with the debug profile, this
/// returns a path relative to the project root. When compiled with the release profile,
/// this returns a directory in the OS's canonical config path.
fn get_config_file() -> Option<PathBuf> {
    let config_dir = if cfg!(debug_assertions) {
        PathBuf::from(file!())
            .parent()
            .and_then(|src| src.parent())
            .map(|p| p.to_owned())
    } else {
        ProjectDirs::from("com", "williamfindlay", "houdini").map(|d| d.config_dir().to_owned())
    };

    config_dir
        .map(|d| d.join("config.toml"))
        .and_then(|p| p.canonicalize().ok())
}
