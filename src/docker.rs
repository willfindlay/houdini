// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Houdini's interaction with the Docker API.

mod container;
mod image;
pub mod util;

pub use container::{export_rootfs, kill_container, reap_container, run_command, spawn_container};
pub use image::{ImagePullPolicy, PullOpts};
