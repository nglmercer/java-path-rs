//! Locating Java installations on the current machine.
//!
//! Discovery never performs a full filesystem scan: it consults `JAVA_HOME`,
//! `PATH`, well-known platform directories, version-manager stores, and any
//! roots the caller supplies.

pub mod common_dirs;
pub mod env;
pub mod linux;
pub mod macos;
pub mod path;
pub mod termux;
pub mod windows;

use crate::error::Result;
use crate::inspect::{inspect_java_home_with, InspectOptions};
use crate::model::{DiscoverySource, JavaInstallation};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Accumulates candidates during discovery, de-duplicating by canonical home.
pub struct Collector {
    options: InspectOptions,
    found: HashMap<PathBuf, JavaInstallation>,
    seen: Vec<PathBuf>,
}

impl Collector {
    fn new(options: InspectOptions) -> Self {
        Collector {
            options,
            found: HashMap::new(),
            seen: Vec::new(),
        }
    }

    /// Inspect one candidate home and keep it if it is a real installation.
    ///
    /// When the same home is reached twice, the higher-priority discovery
    /// source wins, which keeps results deterministic.
    pub fn consider(&mut self, path: impl Into<OsString>, source: DiscoverySource) {
        let path = PathBuf::from(path.into());
        if self.seen.contains(&path) {
            return;
        }
        self.seen.push(path.clone());

        let Ok(install) = inspect_java_home_with(&path, self.options, source) else {
            return;
        };
        match self.found.get(&install.home) {
            Some(existing) if existing.source.priority() <= source.priority() => {}
            _ => {
                self.found.insert(install.home.clone(), install);
            }
        }
    }

    /// Inspect every direct child of `root` (one level only, never recursive).
    pub fn scan_children(&mut self, root: &Path, source: DiscoverySource) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        children.sort();
        for child in children {
            self.consider(child, source);
        }
    }

    fn finish(self) -> Vec<JavaInstallation> {
        let mut installs: Vec<JavaInstallation> = self.found.into_values().collect();
        // Deterministic ordering: newest version first, then source priority,
        // then path.
        installs.sort_by(|a, b| {
            b.version
                .cmp(&a.version)
                .then_with(|| a.source.priority().cmp(&b.source.priority()))
                .then_with(|| a.home.cmp(&b.home))
        });
        installs
    }
}

/// Builder for a discovery run.
#[derive(Debug, Clone)]
pub struct Discovery {
    options: InspectOptions,
    roots: Vec<PathBuf>,
    use_env: bool,
    use_path: bool,
    use_system: bool,
    use_tool_stores: bool,
}

impl Default for Discovery {
    fn default() -> Self {
        Discovery {
            options: InspectOptions::default(),
            roots: Vec::new(),
            use_env: true,
            use_path: true,
            use_system: true,
            use_tool_stores: true,
        }
    }
}

impl Discovery {
    /// A discovery run with every default source enabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Only look at explicitly supplied roots.
    pub fn only_roots() -> Self {
        Discovery {
            use_env: false,
            use_path: false,
            use_system: false,
            use_tool_stores: false,
            ..Self::default()
        }
    }

    /// Add a directory to scan. Both the directory itself and its direct
    /// children are considered.
    pub fn root(mut self, root: impl Into<PathBuf>) -> Self {
        self.roots.push(root.into());
        self
    }

    /// Set the metadata-extraction options used for each candidate.
    pub fn inspect_options(mut self, options: InspectOptions) -> Self {
        self.options = options;
        self
    }

    /// Enable or disable `JAVA_HOME`.
    pub fn java_home(mut self, enabled: bool) -> Self {
        self.use_env = enabled;
        self
    }

    /// Enable or disable `PATH` scanning.
    pub fn path(mut self, enabled: bool) -> Self {
        self.use_path = enabled;
        self
    }

    /// Enable or disable platform system directories.
    pub fn system_dirs(mut self, enabled: bool) -> Self {
        self.use_system = enabled;
        self
    }

    /// Enable or disable SDKMAN!/mise/Gradle/JBang stores.
    pub fn tool_stores(mut self, enabled: bool) -> Self {
        self.use_tool_stores = enabled;
        self
    }

    /// Run discovery.
    pub fn search(self) -> Result<Vec<JavaInstallation>> {
        let mut collector = Collector::new(self.options);

        if self.use_env {
            env::collect(&mut collector);
        }
        if self.use_path {
            path::collect(&mut collector);
        }
        if self.use_system {
            if cfg!(target_os = "windows") {
                windows::collect(&mut collector);
            } else if cfg!(target_os = "macos") {
                macos::collect(&mut collector);
            } else {
                linux::collect(&mut collector);
                termux::collect(&mut collector);
            }
        }
        if self.use_tool_stores {
            common_dirs::collect(&mut collector);
        }
        for root in &self.roots {
            collector.consider(root.clone(), DiscoverySource::UserDirectory);
            collector.scan_children(root, DiscoverySource::UserDirectory);
        }

        Ok(collector.finish())
    }
}

/// Discover every Java installation using the default sources.
pub fn discover() -> Result<Vec<JavaInstallation>> {
    Discovery::new().search()
}
