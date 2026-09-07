//! Well-known Windows JDK locations.
//!
//! The registry is read through the native Windows Registry API (`winreg`),
//! so discovery spawns no `reg.exe` helper and stays invisible in GUI
//! sessions; deep filesystem search (Everything SDK and friends) is
//! deliberately not implemented.

use crate::discovery::Collector;
use crate::model::DiscoverySource;
use std::path::PathBuf;

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

/// Strip the `HKLM\` prefix so the remainder opens under `HKEY_LOCAL_MACHINE`.
///
/// Only used by the Windows registry reader; kept compiled for tests so the
/// prefix handling stays covered on every platform.
#[cfg(any(windows, test))]
fn subkey_path(key: &str) -> Option<&str> {
    key.strip_prefix("HKLM\\")
        .or_else(|| key.strip_prefix("HKLM/"))
}

/// Read `JavaHome` values out of the well-known registry keys.
///
/// Both the 64-bit and the 32-bit registry view are consulted, matching what
/// `reg.exe query` used to observe. Enumeration is recursive with a small
/// depth bound: vendor keys nest one level per installed version.
#[cfg(windows)]
fn registry_homes() -> Vec<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut homes = Vec::new();
    for key in REGISTRY_KEYS {
        let Some(relative) = subkey_path(key) else {
            continue;
        };
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let Ok(base) = hklm.open_subkey_with_flags(relative, KEY_READ | view) else {
                continue;
            };
            walk_key(&base, &mut homes, 0);
        }
    }
    homes
}

/// Collect `JavaHome` values at `key` and below it, one version subkey at a time.
#[cfg(windows)]
fn walk_key(key: &winreg::RegKey, homes: &mut Vec<PathBuf>, depth: u8) {
    use std::path::Path;

    if depth > 4 {
        return;
    }
    if let Ok(home) = key.get_value::<String, _>("JavaHome") {
        let home = home.trim();
        if !home.is_empty() {
            let path = Path::new(home);
            if path.is_dir() {
                homes.push(path.to_path_buf());
            }
        }
    }
    let mut subkeys: Vec<String> = key.enum_keys().filter_map(|name| name.ok()).collect();
    subkeys.sort();
    for name in subkeys {
        if let Ok(sub) = key.open_subkey_with_flags(&name, winreg::enums::KEY_READ) {
            walk_key(&sub, homes, depth + 1);
        }
    }
}

/// Registry discovery is Windows-only; every other platform skips it.
#[cfg(not(windows))]
fn registry_homes() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_cover_the_known_vendors() {
        for key in [
            r"HKLM\SOFTWARE\JavaSoft\JDK",
            r"HKLM\SOFTWARE\Eclipse Adoptium\JDK",
            r"HKLM\SOFTWARE\Eclipse Foundation\JDK",
        ] {
            assert!(
                REGISTRY_KEYS.contains(&key),
                "expected {key} to stay discoverable"
            );
        }
    }

    #[test]
    fn subkey_paths_strip_the_hklm_prefix() {
        assert_eq!(
            subkey_path(r"HKLM\SOFTWARE\JavaSoft\JDK"),
            Some(r"SOFTWARE\JavaSoft\JDK")
        );
        assert_eq!(subkey_path(r"SOFTWARE\JavaSoft\JDK"), None);
    }

    #[test]
    fn registry_discovery_needs_no_subprocess_off_windows() {
        if !cfg!(target_os = "windows") {
            assert!(registry_homes().is_empty());
        }
    }
}
