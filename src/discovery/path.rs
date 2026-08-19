//! Discovery through `java` executables found on `PATH`.

use crate::discovery::Collector;
use crate::model::DiscoverySource;

/// Add every `java` launcher reachable through `PATH`.
pub fn collect(out: &mut Collector) {
    let Some(path) = std::env::var_os("PATH") else {
        return;
    };
    let exe = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(exe);
        if !candidate.is_file() {
            continue;
        }
        // Resolve symlinks (e.g. /usr/bin/java -> .../jvm/.../bin/java) so we
        // land on the real Java home rather than on the symlink farm.
        let resolved = std::fs::canonicalize(&candidate).unwrap_or(candidate);
        let home = resolved.parent().and_then(|p| p.parent());
        if let Some(home) = home {
            out.consider(home, DiscoverySource::Path);
        }
    }
}
