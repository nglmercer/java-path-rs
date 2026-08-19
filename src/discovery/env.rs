//! Discovery through the `JAVA_HOME` environment variable.

use crate::discovery::Collector;
use crate::model::DiscoverySource;

/// Add the installation pointed at by `JAVA_HOME`, if any.
pub fn collect(out: &mut Collector) {
    let Some(home) = std::env::var_os("JAVA_HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    out.consider(home, DiscoverySource::JavaHome);
}
