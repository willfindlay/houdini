// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Default helpers for serde types.

use crate::tricks::status::Status;

pub fn default_true() -> bool {
    true
}

pub fn default_false() -> bool {
    false
}

pub fn default_setup_failure() -> Status {
    Status::SetupFailure
}

pub fn default_skip() -> Status {
    Status::Skip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        assert_eq!(default_true(), true);
        assert_eq!(default_false(), false);

        assert_eq!(default_setup_failure(), Status::SetupFailure);

        assert_eq!(default_skip(), Status::Skip);
    }
}
