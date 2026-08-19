//! The vendor-agnostic JDK provider abstraction.

use crate::error::Result;
use crate::model::{Architecture, JavaKind, Platform};
use std::future::Future;

/// A JDK distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Vendor {
    /// Eclipse Temurin, via the Adoptium API.
    #[default]
    Temurin,
}

impl Vendor {
    /// The value the Adoptium API's `vendor` query parameter expects.
    ///
    /// Note this is the *foundation* name, not the distribution name:
    /// Temurin builds are published under `eclipse`, and `temurin` is
    /// rejected with a 404.
    pub fn api_name(self) -> &'static str {
        match self {
            Vendor::Temurin => "eclipse",
        }
    }

    /// Short, filesystem-safe name used in installation directory names.
    pub fn slug(self) -> &'static str {
        match self {
            Vendor::Temurin => "temurin",
        }
    }
}

/// Which feature version to install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionSpec {
    /// A specific feature version, e.g. `21`.
    Exact(u32),
    /// The newest long-term-support release. The safe default.
    #[default]
    LatestLts,
    /// The newest feature release, LTS or not.
    Latest,
}

impl VersionSpec {
    /// The feature version, when it is already known without a network call.
    pub fn exact(self) -> Option<u32> {
        match self {
            VersionSpec::Exact(major) => Some(major),
            _ => None,
        }
    }
}

impl From<u32> for VersionSpec {
    fn from(major: u32) -> Self {
        VersionSpec::Exact(major)
    }
}

/// What a caller wants to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRequest {
    /// Which feature version to install.
    pub version: VersionSpec,
    /// Target platform.
    pub platform: Platform,
    /// Target architecture.
    pub architecture: Architecture,
    /// JDK or JRE image.
    pub kind: JavaKind,
    /// Which channel to query.
    pub release_type: ReleaseType,
}

/// Which release channel to query.
///
/// These are separate channels, not cumulative filters: `EarlyAccess` returns
/// early-access builds *instead of* general-availability ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReleaseType {
    /// General availability. The default.
    #[default]
    GeneralAvailability,
    /// Early access builds only.
    EarlyAccess,
}

impl ReleaseType {
    /// The Adoptium API token for this channel.
    pub fn api_name(self) -> &'static str {
        match self {
            ReleaseType::GeneralAvailability => "ga",
            ReleaseType::EarlyAccess => "ea",
        }
    }
}

impl Default for ReleaseRequest {
    fn default() -> Self {
        ReleaseRequest {
            version: VersionSpec::default(),
            platform: Platform::current(),
            architecture: Architecture::current(),
            kind: JavaKind::Jdk,
            release_type: ReleaseType::default(),
        }
    }
}

impl ReleaseRequest {
    /// Request a specific feature version.
    pub fn major(mut self, major: u32) -> Self {
        self.version = VersionSpec::Exact(major);
        self
    }

    /// Request the newest feature release, LTS or not.
    pub fn latest(mut self) -> Self {
        self.version = VersionSpec::Latest;
        self
    }

    /// Request the newest long-term-support release.
    pub fn latest_lts(mut self) -> Self {
        self.version = VersionSpec::LatestLts;
        self
    }

    /// Request a JRE image instead of a JDK.
    pub fn jre(mut self) -> Self {
        self.kind = JavaKind::Jre;
        self
    }

    /// Query the early-access channel instead of general availability.
    pub fn early_access(mut self) -> Self {
        self.release_type = ReleaseType::EarlyAccess;
        self
    }
}

/// A downloadable JDK build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JdkRelease {
    /// The distribution this build came from.
    pub vendor: Vendor,
    /// Release name as published by the vendor, e.g. `jdk-21.0.3+9`.
    pub release_name: String,
    /// Version string, e.g. `21.0.3+9`.
    pub version: String,
    /// Feature version.
    pub major: u32,
    /// Direct download URL of the archive.
    pub url: String,
    /// File name of the archive.
    pub file_name: String,
    /// SHA-256 hex digest of the archive, when published.
    pub sha256: Option<String>,
    /// Archive size in bytes, when published.
    pub size: Option<u64>,
    /// Target platform.
    pub platform: Platform,
    /// Target architecture.
    pub architecture: Architecture,
    /// JDK or JRE image.
    pub kind: JavaKind,
    /// `true` for long-term-support feature versions.
    pub lts: bool,
}

/// A source of downloadable JDK builds.
pub trait JdkProvider {
    /// List the releases matching `request`, newest first.
    fn releases(
        &self,
        request: ReleaseRequest,
    ) -> impl Future<Output = Result<Vec<JdkRelease>>> + Send;

    /// The single best release for `request`.
    fn resolve(&self, request: ReleaseRequest) -> impl Future<Output = Result<JdkRelease>> + Send
    where
        Self: Sync,
    {
        async move {
            let description = format!("{request:?}");
            self.releases(request)
                .await?
                .into_iter()
                .next()
                .ok_or(crate::error::Error::NoRelease(description))
        }
    }
}
