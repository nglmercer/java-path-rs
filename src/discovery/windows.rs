//! Well-known Windows JDK locations.
//!
//! The registry is queried through `reg.exe` so the crate needs no Windows
//! API dependency; deep filesystem search (Everything SDK and friends) is
//! deliberately not implemented.

use crate::discovery::Collector;
use crate::model::DiscoverySource;
use crate::process::hidden_command;
use std::path::{Path, PathBuf};

/// Vendor directories under the Program Files roots.
pub const VENDOR_DIRS: &[&str] = &[
    "Java",
    "Eclipse Adoptium",
    "AdoptOpenJDK",
    "Microsoft",
    "Amazon Corretto",
    "Zulu",
    "BellSoft",
    "RedHat",
];

/// Registry keys listing installed JDK/JRE homes.
pub const REGISTRY_KEYS: &[&str] = &[
    r"HKLM\SOFTWARE\JavaSoft\JDK",
    r"HKLM\SOFTWARE\JavaSoft\Java Development Kit",
    r"HKLM\SOFTWARE\JavaSoft\JRE",
    r"HKLM\SOFTWARE\JavaSoft\Java Runtime Environment",
    r"HKLM\SOFTWARE\Eclipse Adoptium\JDK",
    r"HKLM\SOFTWARE\Eclipse Foundation\JDK",
];

/// Scan Program Files vendor directories and the registry.
pub fn collect(out: &mut Collector) {
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        let Some(base) = std::env::var_os(var) else {
            continue;
        };
        let base = PathBuf::from(base);
        for vendor in VENDOR_DIRS {
            out.scan_children(&base.join(vendor), DiscoverySource::System);
        }
    }

    for home in registry_homes() {
        out.consider(home, DiscoverySource::Registry);
    }
}

/// Read `JavaHome` values out of the well-known registry keys via `reg.exe`.
fn registry_homes() -> Vec<PathBuf> {
    if !cfg!(target_os = "windows") {
        return Vec::new();
    }
    let mut homes = Vec::new();
    for key in REGISTRY_KEYS {
        // `hidden_command` carries CREATE_NO_WINDOW on Windows so each
        // `reg.exe` probe stays invisible in GUI sessions.
        let Ok(output) = hidden_command("reg")
            .args(["query", key, "/s", "/v", "JavaHome"])
            .output()
        else {
            continue;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let Some(idx) = line.find("REG_SZ") else {
                continue;
            };
            let value = line[idx + "REG_SZ".len()..].trim();
            if value.is_empty() {
                continue;
            }
            let path = Path::new(value);
            if path.is_dir() {
                homes.push(path.to_path_buf());
            }
        }
    }
    homes
}
