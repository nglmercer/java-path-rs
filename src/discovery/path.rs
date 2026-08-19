//! Discovery through `java` executables found on `PATH`.

use crate::discovery::Collector;
use crate::inspect::{inspect_java_home_with, InspectOptions};
use crate::model::{DiscoverySource, JavaInstallation};
use std::path::PathBuf;

/// The name of the launcher executable on this platform.
fn launcher() -> &'static str {
    if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    }
}

/// The java homes on `PATH`, in the order the shell would search them.
fn homes_in_path_order() -> Vec<PathBuf> {
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut homes = Vec::new();
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(launcher());
        if !candidate.is_file() {
            continue;
        }
        // Resolve symlinks (e.g. /usr/bin/java -> .../jvm/.../bin/java) so we
        // land on the real Java home rather than on the symlink farm.
        let resolved = std::fs::canonicalize(&candidate).unwrap_or(candidate);
        if let Some(home) = resolved.parent().and_then(|p| p.parent()) {
            homes.push(home.to_path_buf());
        }
    }
    homes
}

/// The installation a plain `java` command would run.
///
/// Strictly the first match in `PATH` order — never the newest — because that
/// is what the shell itself does.
pub fn first_on_path() -> Option<JavaInstallation> {
    homes_in_path_order().into_iter().find_map(|home| {
        inspect_java_home_with(home, InspectOptions::default(), DiscoverySource::Path).ok()
    })
}

/// Add every `java` launcher reachable through `PATH`.
pub fn collect(out: &mut Collector) {
    for home in homes_in_path_order() {
        out.consider(home, DiscoverySource::Path);
    }
}
