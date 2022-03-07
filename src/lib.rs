// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! A stub library which is intended to be consumed by Houdini. Not intended for use by
//! other projects. (Although you should feel free to ignore this notice and use it
//! anyway---just be warned that many aspects of this library are specific to Houdini.)

#![deny(missing_docs)]

#[macro_use]
extern crate async_trait;

mod cli;
pub mod config;
mod exploits;
pub mod logging;
mod report;

pub use crate::config::CONFIG;
pub use cli::Cli;
