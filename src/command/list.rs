use std::os::unix::fs::MetadataExt;

use clap::Args;
use tokio::fs;

use crate::config::vm_config::Os;
use crate::config::vm_dir;
use crate::util::exception::Exception;

#[derive(Args)]
pub struct List;

impl List {
    pub async fn execute(&self) -> Result<(), Exception> {
        let home_dir = &vm_dir::home_dir();
        if !home_dir.exists() {
            return Err(Exception::new(format!("{} does not exist", home_dir.to_string_lossy())));
        }
        println!("{:<16}{:<8}{:<8}{:<8}{:<16}{:<16}", "name", "os", "cpu", "memory", "disk", "status");
        let mut entries = fs::read_dir(home_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().is_dir() {
                let dir = vm_dir::vm_dir(&entry.file_name().to_string_lossy());
                if dir.initialized() {
                    let name = dir.name();

                    let config = dir.load_config().await?;
                    let os = to_string(config.os);
                    let cpu = config.cpu;
                    let memory = format!("{:.2}G", config.memory as f32 / (1024.0 * 1024.0 * 1024.0));
                    let metadata = dir.disk_path.metadata()?;
                    let disk = format!(
                        "{:0.2}G/{:.2}G",
                        metadata.blocks() as f32 * 512.0 / 1_000_000_000.0,
                        metadata.len() as f32 / 1_000_000_000.0
                    );
                    let status = if dir.pid().is_some() { "running" } else { "stopped" };
                    println!("{:<16}{:<8}{:<8}{:<8}{:<16}{:<16}", name, os, cpu, memory, disk, status)
                }
            }
        }

        Ok(())
    }
}

fn to_string(os: Os) -> &'static str {
    match os {
        Os::Linux => "linux",
        Os::MacOS => "macOS",
    }
}