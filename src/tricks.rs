// SPDX-License-Identifier: Apache-2.0
//
// Houdini  A container escape artist
// Copyright (c) 2022  William Findlay
//
// February 25, 2022  William Findlay  Created this.
//

//! A [`Trick`] in Houdini is the list of steps required to perform a container exploit
//! (e.g. a container escape or privilege escalation). This module defines data structures
//! that represent a [`Trick`] and its [`Step`]s.

pub mod report;

mod steps;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use self::{
    report::{StepReport, TrickReport},
    status::Status,
    steps::Step,
};
use crate::docker::reap_container;

/// A series of steps for running and verifying the status of a container exploit.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Trick {
    pub name: String,
    steps: Vec<Step>,
}

impl Trick {
    /// Run every step of the trick plan, returning a final status in the end.
    /// If any step returns a final status, we return that status early.
    pub async fn run(&self) -> TrickReport {
        tracing::info!(name = ?&self.name, "running trick");

        let mut containers: HashSet<String> = HashSet::new();
        let mut status = Status::Undecided;

        let mut report = TrickReport::new(&self.name);
        report.set_system_info();

        for step in &self.steps {
            status = step.run().await;

            if let Step::SpawnContainer(step) = step {
                containers.insert(step.name.to_owned());
            }

            let step_report = StepReport::new(step, status);
            report.add(step_report);

            if status.is_final() {
                break;
            }
        }

        match status {
            Status::Undecided | Status::SetupFailure | Status::ExploitFailure => {
                tracing::info!(status = ?status, "trick execution FAILED");
            }
            Status::ExploitSuccess => {
                tracing::info!(status = ?status, "trick execution SUCCEEDED");
            }
            Status::Skip => {
                tracing::info!(status = ?status, "trick execution SKIPPED");
            }
        }

        report.set_status(status);

        // Clean up containers
        for id in &containers {
            if let Err(e) = reap_container(id).await {
                tracing::warn!(err = ?e, "failed to reap container");
            }
        }

        report
    }
}

pub(crate) mod status {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub enum Status {
        /// The status of the exploit test is undecided.
        Undecided,
        /// Setup has failed.
        /// This is a final status that stops the plan.
        SetupFailure,
        /// The exploit has succeeded.
        /// This is a final status that stops the plan.
        ExploitSuccess,
        /// The exploit has failed.
        /// This is a final status that stops the plan.
        ExploitFailure,
        /// Skip the exploit.
        /// Like SetupFailure but not considered a hard failure.
        Skip,
    }

    impl Status {
        pub fn is_final(&self) -> bool {
            match self {
                Status::Undecided => false,
                Status::SetupFailure => true,
                Status::ExploitSuccess => true,
                Status::ExploitFailure => true,
                Status::Skip => true,
            }
        }
    }

    impl Default for Status {
        fn default() -> Self {
            Status::Undecided
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        testutils::{assert_json_serialize, assert_yaml_deserialize},
        tricks::report::Report,
    };

    use super::*;
    use tokio::fs::File;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_yaml_plan_serde_smoke() {
        let yaml = r#"
            name: yaml smoke
            steps:
            - versionCheck:
                docker:
                  min: "1.40.0"
                  max: "1.42"
                kernel:
                  min: "5.14"
                  max: "5.18.8-arch1-1"
                runc:
                  max: "1.1.2"
            - spawnContainer:
                name: foo
                image: bar
                imagePolicy: never
            - spawnContainer:
                name: foo
                image: bar
                imagePolicy:
                    pull:
                        always: true
                        sha256sum: 4893916727cb83addc8ae68bad
                        repo: foobar.qux.com
            - spawnContainer:
                name: foo
                image: bar
                imagePolicy:
                    build:
                        dockerfile: /foo/bar/qux/Dockerfile
                        buildArgs:
                            foo: ""
                            bar: qux
                            baz: 1234
            - killContainer:
                name: foo
            - host:
                script:
                - command: docker
                  args: ["cp", "/etc/passwd", "bash:/passwd"]
                failure: setupFailure
            - container:
                name: bash
                script:
                - command: cat
                  args: ["/passwd"]
                failure: exploitFailure
                success: exploitSuccess
            - wait:
                for:
                    sleep: 2s
            - wait:
                for: input
            "#;
        assert_yaml_deserialize::<Trick>(yaml);
    }

    #[tokio::test]
    #[traced_test]
    #[serial_test::serial]
    async fn test_spawn_container() {
        let yaml = r#"
            name: spawn container test
            steps:
            - spawnContainer:
                name: bash
                image: bash
                cmd: sleep infinity
            - host:
                script:
                - command: echo
                  args: ["hello"]
                - command: echo
                  args: ["goodbye"]
                failure: exploitFailure
            - container:
                name: bash
                script:
                - command: echo
                  args: ["hello"]
                - command: echo
                  args: ["goodbye"]
                failure: exploitFailure
                success: exploitSuccess
            "#;

        let plan: Trick = assert_yaml_deserialize(yaml);
        let report = plan.run().await;
        assert!(
            matches!(report.status, Status::ExploitSuccess),
            "should succeed"
        );
    }

    #[tokio::test]
    #[traced_test]
    #[serial_test::serial]
    async fn test_exploits_work() {
        use std::path::PathBuf;

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("exploits");

        let mut report = Report::new();

        for entry in jwalk::WalkDir::new(d)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
        {
            let file = File::open(entry.path())
                .await
                .expect("file must open")
                .into_std()
                .await;
            let plan: Trick = serde_yaml::from_reader(&file).expect("should deserialize");
            let trick_report = plan.run().await;
            let status = trick_report.status;
            report.add(trick_report);
            assert!(
                matches!(status, Status::ExploitSuccess | Status::Skip),
                "exploit should succeed"
            );
        }

        assert_json_serialize(&report);
    }
}
