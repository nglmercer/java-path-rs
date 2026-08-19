//! Discovery through version-manager and tool JDK stores.

use crate::discovery::Collector;
use crate::model::DiscoverySource;
use std::path::PathBuf;

/// Home directory of the current user, if determinable.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Scan SDKMAN!, mise, Gradle and JBang JDK stores.
pub fn collect(out: &mut Collector) {
    let Some(home) = home_dir() else {
        return;
    };

    let stores: [(PathBuf, DiscoverySource); 5] = [
        (
            home.join(".sdkman").join("candidates").join("java"),
            DiscoverySource::Sdkman,
        ),
        (
            home.join(".local")
                .join("share")
                .join("mise")
                .join("installs")
                .join("java"),
            DiscoverySource::Mise,
        ),
        (
            home.join(".asdf").join("installs").join("java"),
            DiscoverySource::Mise,
        ),
        (home.join(".gradle").join("jdks"), DiscoverySource::Gradle),
        (
            home.join(".jbang").join("cache").join("jdks"),
            DiscoverySource::Jbang,
        ),
    ];

    for (root, source) in stores {
        out.scan_children(&root, source);
    }
}
