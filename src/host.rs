use std::path::PathBuf;
use tokio::fs::File;

use anyhow::{Context, Result};

use crate::{
    tricks::{report::Report, status::Status, Trick},
};

pub async fn main(exploits: Vec<PathBuf>) -> Result<()> {
    let mut report = Report::default();

    for exploit in exploits {
        let f = File::open(&exploit).await.context(format!(
            "could not open exploit file {}",
            &exploit.display()
        ))?;
        let plan: Trick = serde_yaml::from_reader(f.into_std().await).context(
            format!("failed to parse exploit file {}", &exploit.display()),
        )?;

        let status = plan.run(Some(&mut report)).await;
        match status {
            Status::Undecided | Status::SetupFailure | Status::ExploitFailure => {
                tracing::info!(status = ?status, "plan execution FAILED");
            }
            Status::ExploitSuccess => {
                tracing::info!(status = ?status, "plan execution SUCCEEDED");
            }
            Status::Skip => {
                tracing::info!(status = ?status, "plan execution SKIPPED");
            }
        }
    }

    report
        .write_to_disk()
        .await
        .context("failed to write report to disk")?;
        Ok(())
}