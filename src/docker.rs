// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Houdini's interaction with the Docker API.

mod cmd;
mod image;

pub use cmd::{Command, ExitCode, Stdio};
pub use image::ImagePullPolicy;
