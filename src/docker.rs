// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Houdini's interaction with the Docker API.

mod cmd;
mod container;
mod image;
mod util;

pub use cmd::{Command, ExitCode, Stdio};
pub use container::{kill_container, reap_container, spawn_container};
pub use image::ImagePullPolicy;
