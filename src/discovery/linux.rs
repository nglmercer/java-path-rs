//! Well-known Linux JDK locations.

use crate::discovery::Collector;
use crate::model::DiscoverySource;
use std::path::Path;

/// Directories distributions commonly install JDKs into.
pub const ROOTS: &[&str] = &[
    "/usr/lib/jvm",
    "/usr/lib64/jvm",
    "/usr/java",
    "/usr/local/java",
    "/opt/java",
    "/opt/jdk",
    "/opt/jdks",
];

/// Scan the well-known Linux roots.
pub fn collect(out: &mut Collector) {
    for root in ROOTS {
        out.scan_children(Path::new(root), DiscoverySource::System);
    }
}
