//! Well-known macOS JDK locations.

use crate::discovery::Collector;
use crate::model::DiscoverySource;
use std::path::{Path, PathBuf};
use std::process::Command;

/// System-wide JVM bundle directory.
pub const SYSTEM_ROOT: &str = "/Library/Java/JavaVirtualMachines";

/// Scan the macOS bundle directories and ask `/usr/libexec/java_home`.
pub fn collect(out: &mut Collector) {
    out.scan_children(Path::new(SYSTEM_ROOT), DiscoverySource::System);
    if let Some(home) = crate::discovery::common_dirs::home_dir() {
        out.scan_children(
            &home
                .join("Library")
                .join("Java")
                .join("JavaVirtualMachines"),
            DiscoverySource::System,
        );
    }
    for home in java_home_tool() {
        out.consider(home, DiscoverySource::System);
    }
}

/// Query `/usr/libexec/java_home -V` for registered JVMs.
fn java_home_tool() -> Vec<PathBuf> {
    let tool = Path::new("/usr/libexec/java_home");
    if !tool.is_file() {
        return Vec::new();
    }
    let Ok(output) = Command::new(tool).arg("-V").output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stderr);
    text.lines()
        .filter_map(|line| {
            line.split_once(" /")
                .map(|(_, rest)| PathBuf::from(format!("/{rest}")))
        })
        .filter(|p| p.is_dir())
        .collect()
}
