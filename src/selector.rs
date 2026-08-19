//! Constraint-based selection over a set of discovered installations.

use crate::error::{Error, Result};
use crate::model::{Architecture, DiscoverySource, JavaInstallation, JavaKind, Platform};
use crate::version::JavaVersion;

/// Constraints applied when picking an installation.
#[derive(Debug, Clone, Default)]
pub struct JavaQuery {
    major: Option<u32>,
    min_major: Option<u32>,
    max_major: Option<u32>,
    min_version: Option<JavaVersion>,
    kind: Option<JavaKind>,
    vendor: Option<String>,
    architecture: Option<Architecture>,
    platform: Option<Platform>,
    allow_prerelease: bool,
}

impl JavaQuery {
    /// An unconstrained query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Require an exact feature/major version.
    pub fn major(mut self, major: u32) -> Self {
        self.major = Some(major);
        self
    }

    /// Require at least this feature version.
    pub fn min_major(mut self, major: u32) -> Self {
        self.min_major = Some(major);
        self
    }

    /// Require at most this feature version.
    pub fn max_major(mut self, major: u32) -> Self {
        self.max_major = Some(major);
        self
    }

    /// Require a feature version within `[min, max]` inclusive.
    pub fn major_range(self, min: u32, max: u32) -> Self {
        self.min_major(min).max_major(max)
    }

    /// Require at least this full version.
    pub fn min_version(mut self, version: JavaVersion) -> Self {
        self.min_version = Some(version);
        self
    }

    /// Require a JDK (`javac` present).
    pub fn jdk(mut self) -> Self {
        self.kind = Some(JavaKind::Jdk);
        self
    }

    /// Require a JRE specifically.
    pub fn jre(mut self) -> Self {
        self.kind = Some(JavaKind::Jre);
        self
    }

    /// Require a vendor; matching is case-insensitive and substring-based.
    pub fn vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = Some(vendor.into().to_ascii_lowercase());
        self
    }

    /// Require a specific architecture.
    pub fn architecture(mut self, arch: Architecture) -> Self {
        self.architecture = Some(arch);
        self
    }

    /// Require the architecture of the running process.
    pub fn current_arch(self) -> Self {
        self.architecture(Architecture::current())
    }

    /// Require a specific platform.
    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = Some(platform);
        self
    }

    /// Allow early-access builds (excluded by default).
    pub fn allow_prerelease(mut self, allow: bool) -> Self {
        self.allow_prerelease = allow;
        self
    }

    /// `true` when the installation satisfies every constraint.
    pub fn matches(&self, install: &JavaInstallation) -> bool {
        if let Some(major) = self.major {
            if install.version.major() != major {
                return false;
            }
        }
        if let Some(min) = self.min_major {
            if install.version.major() < min {
                return false;
            }
        }
        if let Some(max) = self.max_major {
            if install.version.major() > max {
                return false;
            }
        }
        if let Some(min) = &self.min_version {
            if install.version < *min {
                return false;
            }
        }
        if let Some(kind) = self.kind {
            if install.kind != kind {
                return false;
            }
        }
        if let Some(vendor) = &self.vendor {
            let Some(actual) = &install.vendor else {
                return false;
            };
            if !actual.to_ascii_lowercase().contains(vendor) {
                return false;
            }
        }
        if let Some(arch) = self.architecture {
            // Unknown architecture is not treated as a mismatch: metadata
            // sources are allowed to be incomplete.
            if install.architecture != Architecture::Unknown && install.architecture != arch {
                return false;
            }
        }
        if let Some(platform) = self.platform {
            if install.platform != Platform::Unknown && install.platform != platform {
                return false;
            }
        }
        if !self.allow_prerelease && install.version.is_prerelease() {
            return false;
        }
        true
    }

    /// Deterministic ranking score; higher is better.
    fn score(&self, install: &JavaInstallation) -> (u8, u8, JavaVersion, i16) {
        (
            // Prefer JDKs unless a JRE was explicitly asked for.
            u8::from(self.kind != Some(JavaKind::Jre) && install.is_jdk()),
            // Prefer the installation JAVA_HOME points at.
            u8::from(install.source == DiscoverySource::JavaHome),
            install.version.clone(),
            -(install.source.priority() as i16),
        )
    }
}

/// Selection over a slice of installations.
pub struct Selector<'a> {
    installations: &'a [JavaInstallation],
    query: JavaQuery,
}

impl<'a> Selector<'a> {
    /// Start a selection over `installations`.
    pub fn new(installations: &'a [JavaInstallation]) -> Self {
        Selector {
            installations,
            query: JavaQuery::new(),
        }
    }

    /// Replace the query wholesale.
    pub fn with_query(mut self, query: JavaQuery) -> Self {
        self.query = query;
        self
    }

    /// Require an exact feature version.
    pub fn major(mut self, major: u32) -> Self {
        self.query = self.query.major(major);
        self
    }

    /// Require at least this feature version.
    pub fn min_major(mut self, major: u32) -> Self {
        self.query = self.query.min_major(major);
        self
    }

    /// Require a JDK.
    pub fn jdk(mut self) -> Self {
        self.query = self.query.jdk();
        self
    }

    /// Require a vendor.
    pub fn vendor(mut self, vendor: impl Into<String>) -> Self {
        self.query = self.query.vendor(vendor);
        self
    }

    /// Require the running process's architecture.
    pub fn current_arch(mut self) -> Self {
        self.query = self.query.current_arch();
        self
    }

    /// Every matching installation, best first.
    pub fn all(&self) -> Vec<&'a JavaInstallation> {
        let mut matches: Vec<&JavaInstallation> = self
            .installations
            .iter()
            .filter(|i| self.query.matches(i))
            .collect();
        matches.sort_by(|a, b| {
            self.query
                .score(b)
                .cmp(&self.query.score(a))
                .then_with(|| a.home.cmp(&b.home))
        });
        matches
    }

    /// The single best match, if any.
    pub fn best(&self) -> Result<&'a JavaInstallation> {
        self.all().into_iter().next().ok_or(Error::NoMatch)
    }
}

/// Convenience selection over owned collections.
pub trait SelectExt {
    /// Start a selection over this collection.
    fn select(&self) -> Selector<'_>;
}

impl SelectExt for Vec<JavaInstallation> {
    fn select(&self) -> Selector<'_> {
        Selector::new(self)
    }
}

impl SelectExt for [JavaInstallation] {
    fn select(&self) -> Selector<'_> {
        Selector::new(self)
    }
}
