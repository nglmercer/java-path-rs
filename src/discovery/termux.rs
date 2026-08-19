//! Termux (Android) environment detection and JDK discovery.
//!
//! Termux installs OpenJDK through its own package manager into
//! `$PREFIX/opt/openjdk`; Adoptium's generic Linux binaries do not run there,
//! so provisioning is intentionally not offered for this platform.

use crate::discovery::Collector;
use crate::model::DiscoverySource;
use std::path::PathBuf;

/// The Termux installation prefix, if we are running inside Termux.
pub fn prefix() -> Option<PathBuf> {
    let prefix = std::env::var_os("PREFIX").filter(|p| !p.is_empty())?;
    let prefix = PathBuf::from(prefix);
    let is_termux = prefix.to_str().is_some_and(|p| p.contains("com.termux"));
    is_termux.then_some(prefix)
}

/// `true` when the current process is running under Termux.
pub fn is_termux() -> bool {
    prefix().is_some()
        || std::env::var_os("TERMUX_VERSION").is_some()
        || PathBuf::from("/data/data/com.termux/files/usr").is_dir()
}

/// Path to the Termux `pkg` helper, when available.
pub fn package_manager() -> Option<PathBuf> {
    let prefix = prefix().unwrap_or_else(|| PathBuf::from("/data/data/com.termux/files/usr"));
    let pkg = prefix.join("bin").join("pkg");
    pkg.is_file().then_some(pkg)
}

/// Scan the Termux OpenJDK locations.
pub fn collect(out: &mut Collector) {
    if !is_termux() {
        return;
    }
    let prefix = prefix().unwrap_or_else(|| PathBuf::from("/data/data/com.termux/files/usr"));
    let opt = prefix.join("opt");
    out.scan_children(&opt, DiscoverySource::Termux);
    for name in ["openjdk", "openjdk-17", "java"] {
        out.consider(opt.join(name), DiscoverySource::Termux);
    }
}
