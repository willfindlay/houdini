use anyhow::{Context, Result};
use loopdev::{LoopControl, LoopDevice};
use mbrman::MBR;
use scopeguard::defer;
use std::path::{Path, PathBuf};
use tempfile::Builder;

const MBR_SECTOR_SIZE: u32 = 512;
const LINUX_FILESYSTEM: u8 = 0x83;

fn bootstrap_disk_image(size: usize) -> Result<PathBuf> {
    let path = new_image_path()?;
    create_empty_file(&path, size).context("failed to create empty file")?;

    let starting_lba = partition(&path).context("failed to partition disk")?;

    let ld = acquire_loopback_device(&path)?;
    defer! {
        let _ = ld.detach();
    }

    let ld_path = ld
        .path()
        .ok_or_else(|| anyhow::anyhow!("no path for loopback device"))?;

    println!("{}", ld_path.display());

    create_ext4_filesystem(&ld_path, starting_lba).context("failed to create ext4 filesystem")?;
    mount_filesystem(&ld_path).context("failed to mount filesystem")?;

    Ok(path)
}

/// Return a new image path relative to PWD.
fn new_image_path() -> Result<PathBuf> {
    Ok(Builder::new()
        .prefix("")
        .rand_bytes(6)
        .suffix(".houdini.img")
        .tempfile_in(std::env::current_dir()?)?
        .into_temp_path()
        .to_path_buf())
}

/// Create an empty file to back a disk.
fn create_empty_file(path: &Path, size: usize) -> Result<()> {
    let file = std::fs::File::create(&path)?;
    file.set_len(size as u64)?;
    file.sync_all()?;
    Ok(())
}

/// Acquire loopback device on a file.
fn acquire_loopback_device(path: &Path) -> Result<LoopDevice> {
    let lc = LoopControl::open().context("failed to open loopback control")?;
    let ld = lc
        .next_free()
        .context("failed to get next free loopback device")?;
    ld.attach_file(path)
        .context("failed to attach loopback device")?;

    Ok(ld)
}
/// Partition the disk.
fn partition(path: &Path) -> Result<u32> {
    let mut file = std::fs::File::options()
        .read(true)
        .write(true)
        .open(path)
        .context("failed to open disk")?;
    let mut mbr = MBR::new_from(&mut file, MBR_SECTOR_SIZE, [0xff; 4])
        .context("failed to create partition table")?;
    mbr.write_into(&mut file)
        .context("failed to write partition table to disk")?;

    let free_partition_number = mbr
        .iter()
        .find(|(_, p)| p.is_unused())
        .map(|(i, _)| i)
        .context("no more free partition numbers")?;
    let sectors = mbr
        .get_maximum_partition_size()
        .context("no space on disk")?;
    let starting_lba = mbr
        .find_optimal_place(sectors)
        .context("could not find starting place for partition")?;

    mbr[free_partition_number] = mbrman::MBRPartitionEntry {
        boot: mbrman::BOOT_INACTIVE,     // boot flag
        first_chs: mbrman::CHS::empty(), // first CHS address (only useful for old computers)
        sys: LINUX_FILESYSTEM,           // Linux filesystem
        last_chs: mbrman::CHS::empty(),  // last CHS address (only useful for old computers)
        starting_lba,                    // the sector where the partition starts
        sectors,                         // the number of sectors in that partition
    };

    Ok(starting_lba)
}

fn create_ext4_filesystem(ld_path: &Path, offset: u32) -> Result<()> {
    let status = std::process::Command::new("mkfs")
        .args(&[
            "-t",
            "ext4",
            "-E",
            format!("offset={}", offset.to_string().as_str(),).as_str(),
            ld_path.to_str().unwrap(),
        ])
        .status()
        .context("failed to start mkfs")?;

    if !status.success() {
        anyhow::bail!("failed to run mkfs: {}", status)
    }

    Ok(())
}

fn mount_filesystem(ld_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_disk_image_test() {
        bootstrap_disk_image(4 * 1024_usize.pow(2)).expect("bootstrapping should succeed");
    }
}
