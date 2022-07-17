// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Utility functions for writing Houdini unit tests.

#![cfg(test)]

use std::fmt::Debug;

use serde::{de::DeserializeOwned, Serialize};
use serde_yaml::{from_str, to_string};

/// Deserialize a yaml string, serialize it back, and deserialize it again ensuring the
/// two resulting objects are equal and returning the first object.
pub fn assert_serde_yaml<T: Serialize + DeserializeOwned + Debug + Eq>(yaml: &str) -> T {
    let obj1: T = from_str(yaml).expect("should deserialize");
    let yaml = to_string(&obj1).expect("should serialize");
    let obj2: T = from_str(&yaml).expect("should deserialize back");
    assert_eq!(obj1, obj2, "deserialized structs should be the same");
    obj1
}
