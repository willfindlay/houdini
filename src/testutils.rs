// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.

//! Utility functions for writing Houdini unit tests.

#![cfg(test)]
#![allow(dead_code)]

use std::fmt::Debug;

use serde::{de::DeserializeOwned, Serialize};

/// Deserialize a yaml string, serialize it back, and deserialize it again ensuring the
/// two resulting objects are equal and returning the first object.
pub fn assert_yaml_deserialize<T: Serialize + DeserializeOwned + Debug + Eq>(yaml: &str) -> T {
    let obj1: T = serde_yaml::from_str(yaml).expect("should deserialize");
    assert_yaml_serialize(&obj1);
    obj1
}

/// Serialize a T as a yaml string, and deserialize it back ensuring the two
/// resulting objects are equal and returning the yaml string.
pub fn assert_yaml_serialize<T: Serialize + DeserializeOwned + Debug + Eq>(obj: &T) -> String {
    let yaml = serde_yaml::to_string(obj).expect("should serialize");
    let obj2: T = serde_yaml::from_str(&yaml).expect("should deserialize back");
    assert_eq!(obj, &obj2, "deserialized structs should be the same");
    yaml
}

/// Deserialize a json string, serialize it back, and deserialize it again ensuring the
/// two resulting objects are equal and returning the first object.
pub fn assert_json_deserialize<T: Serialize + DeserializeOwned + Debug + Eq>(json: &str) -> T {
    let obj1: T = serde_json::from_str(json).expect("should deserialize");
    assert_json_serialize(&obj1);
    obj1
}

/// Serialize a T as a json string, and deserialize it back ensuring the two
/// resulting objects are equal and returning the json string.
pub fn assert_json_serialize<T: Serialize + DeserializeOwned + Debug + Eq>(obj: &T) -> String {
    let json = serde_json::to_string(obj).expect("should serialize");
    let obj2: T = serde_json::from_str(&json).expect("should deserialize back");
    assert_eq!(obj, &obj2, "deserialized structs should be the same");
    json
}
