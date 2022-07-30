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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// Configuration specific to Docker.
    pub docker: DockerConfig,
    /// Configuration specific to the logger.
    pub log: LogConfig,
    /// Configuration specific to exploit reports.
    pub reports: ReportConfig,
}

/// Configuration specific to Docker.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DockerConfig {
    /// Name of the Docker client binary.
    pub client: String,
    /// Name of the Docker daemon binary.
    pub daemon: String,
    /// Name of the container runtime binary.
    pub runtime: String,
    /// Full path to the Docker socket.
    #[serde(deserialize_with = "serde_helpers::expand_pathbuf")]
    pub socket: PathBuf,
}

/// Configuration specific to Houdini's logger.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogConfig {
    /// Path to the log file.
    #[serde(default)]
    #[serde(deserialize_with = "serde_helpers::expand_option_pathbuf")]
    pub file: Option<PathBuf>,
    #[serde(default)]
    /// Log file verbosity.
    pub level: LevelFilter,
}

/// Configuration specific to Houdini's exploit reports.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportConfig {
    /// Path to the exploit reports dir.
    #[serde(deserialize_with = "serde_helpers::expand_pathbuf")]
    pub dir: PathBuf,
}

/// Level filter for logging.
#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

impl From<LevelFilter> for tracing::metadata::LevelFilter {
    fn from(f: LevelFilter) -> Self {
        match f {
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

mod serde_helpers {
    use serde::{Deserialize, Deserializer};
    use std::path::PathBuf;

    pub fn expand_pathbuf<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        let p = PathBuf::deserialize(deserializer)?;
        let p = shellexpand::full(
            p.to_str()
                .ok_or_else(|| serde::de::Error::custom("path is not a UTF-8 string"))?,
        )
        .map_err(serde::de::Error::custom)?;
        let p = p.as_ref();
        Ok(PathBuf::from(p))
    }

    pub fn expand_option_pathbuf<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let p: Option<PathBuf> = Option::deserialize(deserializer)?;
        if let Some(p) = p {
            let p = shellexpand::full(
                p.to_str()
                    .ok_or_else(|| serde::de::Error::custom("path is not a UTF-8 string"))?,
            )
            .map_err(serde::de::Error::custom)?;
            let p = p.as_ref();
            Ok(Some(PathBuf::from(p)))
        } else {
            Ok(None)
        }
    }
}
