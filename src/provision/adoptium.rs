//! The Eclipse Adoptium (Temurin) release provider.

use crate::error::{Error, Result};
use crate::model::{Architecture, JavaKind, Platform};
use crate::provision::provider::{JdkProvider, JdkRelease, ReleaseRequest, Vendor, VersionSpec};
use serde::Deserialize;

/// Default Adoptium API root.
pub const DEFAULT_API_BASE: &str = "https://api.adoptium.net/v3";

/// Feature versions known to be long-term support at the time of writing.
///
/// Only a fallback: [`AdoptiumProvider::lts_releases`] asks the API, so a new
/// LTS does not require a new release of this crate.
pub const KNOWN_LTS_VERSIONS: &[u32] = &[8, 11, 17, 21, 25];

/// Provider backed by the Adoptium v3 API.
#[derive(Debug, Clone)]
pub struct AdoptiumProvider {
    base_url: String,
    vendor: Vendor,
    client: reqwest::Client,
}

impl Default for AdoptiumProvider {
    fn default() -> Self {
        AdoptiumProvider {
            base_url: DEFAULT_API_BASE.to_string(),
            vendor: Vendor::Temurin,
            client: reqwest::Client::new(),
        }
    }
}

impl AdoptiumProvider {
    /// A provider pointing at the public API.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the API base URL (useful for mirrors and tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// Use a caller-supplied HTTP client.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// `true` when `major` is a long-term-support version according to the
    /// built-in list.
    ///
    /// Prefer [`AdoptiumProvider::lts_releases`], which asks the API and so
    /// stays correct as new LTS versions appear.
    pub fn is_known_lts(major: u32) -> bool {
        KNOWN_LTS_VERSIONS.contains(&major)
    }

    /// The LTS feature versions the API reports, newest first.
    pub async fn lts_releases(&self) -> Result<Vec<u32>> {
        let info: AvailableReleases = self
            .get_json(&format!("{}/info/available_releases", self.base_url))
            .await?;
        let mut versions = if info.available_lts_releases.is_empty() {
            info.available_releases
                .iter()
                .copied()
                .filter(|v| Self::is_known_lts(*v))
                .collect()
        } else {
            info.available_lts_releases
        };
        versions.sort_unstable_by(|a, b| b.cmp(a));
        Ok(versions)
    }

    /// Feature versions to try, newest first, for a version specification.
    ///
    /// A version can appear in `available_releases` without every OS and
    /// architecture combination having a binary, so callers walk this list
    /// until one actually yields a build for the requested target.
    async fn candidates(&self, spec: VersionSpec) -> Result<Vec<u32>> {
        Ok(match spec {
            VersionSpec::Exact(major) => vec![major],
            VersionSpec::LatestLts => self.lts_releases().await?,
            VersionSpec::Latest => self.available_releases().await?,
        })
    }

    /// Feature versions currently available, newest first.
    pub async fn available_releases(&self) -> Result<Vec<u32>> {
        let url = format!("{}/info/available_releases", self.base_url);
        let info: AvailableReleases = self.get_json(&url).await?;
        let mut versions = info.available_releases;
        versions.sort_unstable_by(|a, b| b.cmp(a));
        Ok(versions)
    }

    /// The most recent feature version, LTS or not.
    pub async fn latest_feature(&self) -> Result<u32> {
        let info: AvailableReleases = self
            .get_json(&format!("{}/info/available_releases", self.base_url))
            .await?;
        info.most_recent_feature_release
            .or_else(|| info.available_releases.iter().copied().max())
            .ok_or_else(|| Error::NoRelease("no feature release reported by the API".to_string()))
    }

    /// The most recent LTS feature version.
    pub async fn latest_lts(&self) -> Result<u32> {
        let info: AvailableReleases = self
            .get_json(&format!("{}/info/available_releases", self.base_url))
            .await?;
        info.most_recent_lts
            .or_else(|| {
                info.available_releases
                    .iter()
                    .copied()
                    .filter(|v| Self::is_known_lts(*v))
                    .max()
            })
            .ok_or_else(|| Error::NoRelease("no LTS version reported by the API".to_string()))
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;
        if !response.status().is_success() {
            return Err(Error::Network(format!(
                "{} returned HTTP {}",
                url,
                response.status()
            )));
        }
        response
            .json::<T>()
            .await
            .map_err(|e| Error::Network(format!("invalid response from {url}: {e}")))
    }
}

impl JdkProvider for AdoptiumProvider {
    async fn releases(&self, request: ReleaseRequest) -> Result<Vec<JdkRelease>> {
        let os = request.platform.adoptium_os().ok_or_else(|| {
            Error::NoRelease(format!("unsupported platform {}", request.platform))
        })?;
        let arch = request.architecture.adoptium_arch().ok_or_else(|| {
            Error::NoRelease(format!("unsupported architecture {}", request.architecture))
        })?;
        let image = match request.kind {
            JavaKind::Jdk => "jdk",
            JavaKind::Jre => "jre",
        };

        let candidates = self.candidates(request.version).await?;
        let release_type = request.release_type.api_name();

        // Walk newest-first until a version actually has a binary for this
        // target, rather than assuming the newest one does.
        let mut last_error = None;
        for major in candidates {
            match self
                .feature_release(major, release_type, os, arch, image, &request)
                .await
            {
                Ok(releases) if !releases.is_empty() => return Ok(releases),
                Ok(_) => {}
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error
            .unwrap_or_else(|| Error::NoRelease(format!("no {image} build for {os}/{arch}"))))
    }
}

impl AdoptiumProvider {
    /// Fetch and map the binaries of one feature release.
    async fn feature_release(
        &self,
        major: u32,
        release_type: &str,
        os: &str,
        arch: &str,
        image: &str,
        request: &ReleaseRequest,
    ) -> Result<Vec<JdkRelease>> {
        let lts = self.lts_releases().await.unwrap_or_default();

        let url = format!(
            "{base}/assets/feature_releases/{major}/{release_type}\
?architecture={arch}&image_type={image}&jvm_impl=hotspot&os={os}\
&vendor={vendor}&heap_size=normal&page=0&page_size=20&sort_order=DESC",
            base = self.base_url,
            vendor = self.vendor.api_name(),
        );

        let assets: Vec<FeatureRelease> = self.get_json(&url).await?;
        let mut releases = Vec::new();
        for asset in assets {
            for binary in asset.binaries {
                if binary.image_type != image {
                    continue;
                }
                let Some(package) = binary.package else {
                    continue;
                };
                releases.push(JdkRelease {
                    vendor: self.vendor,
                    release_name: asset.release_name.clone(),
                    version: asset
                        .version_data
                        .as_ref()
                        .and_then(|v| v.semver.clone())
                        .unwrap_or_else(|| asset.release_name.clone()),
                    major,
                    url: package.link,
                    file_name: package.name,
                    sha256: package.checksum,
                    size: package.size,
                    platform: Platform::parse(&binary.os),
                    architecture: Architecture::parse(&binary.architecture),
                    kind: request.kind,
                    lts: if lts.is_empty() {
                        Self::is_known_lts(major)
                    } else {
                        lts.contains(&major)
                    },
                });
            }
        }

        Ok(releases)
    }
}

#[derive(Debug, Deserialize)]
struct AvailableReleases {
    available_releases: Vec<u32>,
    #[serde(default)]
    available_lts_releases: Vec<u32>,
    most_recent_lts: Option<u32>,
    #[serde(default)]
    most_recent_feature_release: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FeatureRelease {
    release_name: String,
    #[serde(default)]
    version_data: Option<VersionData>,
    #[serde(default)]
    binaries: Vec<Binary>,
}

#[derive(Debug, Deserialize)]
struct VersionData {
    semver: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Binary {
    os: String,
    architecture: String,
    image_type: String,
    #[serde(default)]
    package: Option<Package>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    link: String,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}
