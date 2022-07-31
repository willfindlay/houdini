// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! Helper types for defining commands to run.

use serde::{Deserialize, Serialize};

/// Defines a command to run in a container or on the host.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShellCommand {
    pub command: String,
    pub args: Vec<String>,
}
