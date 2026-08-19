//! Downloading and installing JDKs from a provider.

pub mod provider;

#[cfg(feature = "network")]
pub mod adoptium;
#[cfg(feature = "network")]
pub mod checksum;

#[cfg(feature = "install")]
pub mod archive;
#[cfg(feature = "install")]
pub mod download;

pub use provider::{JdkProvider, JdkRelease, ReleaseRequest, ReleaseType, Vendor, VersionSpec};

#[cfg(feature = "install")]
mod installer {
    use super::*;
    use crate::error::{Error, Result};
    use crate::inspect::{inspect_java_home_with, InspectOptions};
    use crate::model::{Architecture, DiscoverySource, JavaInstallation, JavaKind, Platform};
    use std::path::{Path, PathBuf};

    /// Stages an installation moves through.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum InstallEvent {
        /// Asking the provider which build to use.
        Resolving,
        /// Streaming the archive.
        Downloading {
            /// Bytes written so far.
            downloaded: u64,
            /// Total bytes, when known.
            total: Option<u64>,
        },
        /// Checking the SHA-256 digest.
        Verifying,
        /// Unpacking the archive.
        Extracting,
        /// Finished; the JDK lives at this path.
        Installed {
            /// Final Java home.
            path: PathBuf,
        },
    }

    /// Installs JDKs from a [`JdkProvider`] into a local directory.
    pub struct JavaInstaller<P> {
        provider: P,
        request: ReleaseRequest,
        install_dir: PathBuf,
        cache_dir: Option<PathBuf>,
        allow_unverified: bool,
        on_event: Box<dyn FnMut(InstallEvent) + Send>,
    }

    #[cfg(feature = "network")]
    impl JavaInstaller<adoptium::AdoptiumProvider> {
        /// An installer backed by the public Adoptium API.
        pub fn adoptium() -> Self {
            JavaInstaller::new(adoptium::AdoptiumProvider::new())
        }
    }

    impl<P: JdkProvider + Sync> JavaInstaller<P> {
        /// An installer using `provider`, defaulting to the host platform.
        pub fn new(provider: P) -> Self {
            JavaInstaller {
                provider,
                request: ReleaseRequest::default(),
                install_dir: default_install_dir(),
                cache_dir: None,
                allow_unverified: false,
                on_event: Box::new(|_| {}),
            }
        }

        /// Install a specific feature version.
        pub fn version(mut self, major: u32) -> Self {
            self.request.version = VersionSpec::Exact(major);
            self
        }

        /// Install the newest feature release, LTS or not.
        pub fn latest(mut self) -> Self {
            self.request.version = VersionSpec::Latest;
            self
        }

        /// Install the newest long-term-support release. This is the default.
        pub fn latest_lts(mut self) -> Self {
            self.request.version = VersionSpec::LatestLts;
            self
        }

        /// Install a JRE image instead of a JDK.
        pub fn jre(mut self) -> Self {
            self.request.kind = JavaKind::Jre;
            self
        }

        /// Target a platform other than the host's.
        ///
        /// Installing is only supported for the host platform; see
        /// [`JavaInstaller::install`]. This is useful with
        /// [`JdkProvider::resolve`] for inspecting other targets.
        pub fn platform(mut self, platform: Platform) -> Self {
            self.request.platform = platform;
            self
        }

        /// Target an architecture other than the host's.
        pub fn architecture(mut self, architecture: Architecture) -> Self {
            self.request.architecture = architecture;
            self
        }

        /// Where installations are placed.
        pub fn install_dir(mut self, dir: impl Into<PathBuf>) -> Self {
            self.install_dir = dir.into();
            self
        }

        /// Where downloaded archives are cached.
        pub fn cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
            self.cache_dir = Some(dir.into());
            self
        }

        /// Permit installing a release that publishes no SHA-256 checksum.
        ///
        /// Off by default: an unverified archive is installed without any
        /// integrity guarantee whatsoever.
        pub fn allow_unverified(mut self, allow: bool) -> Self {
            self.allow_unverified = allow;
            self
        }

        /// Subscribe to progress events.
        pub fn on_event(mut self, callback: impl FnMut(InstallEvent) + Send + 'static) -> Self {
            self.on_event = Box::new(callback);
            self
        }

        /// Resolve, download, verify, extract, validate and install a JDK.
        ///
        /// The install is idempotent: if the target directory already holds a
        /// JDK matching the request, it is returned without touching the
        /// network. Only the host platform can be installed for; a JDK for
        /// another operating system cannot be inspected or launched here, so
        /// requesting one is refused rather than silently producing a broken
        /// installation.
        pub async fn install(mut self) -> Result<JavaInstallation> {
            let host = Platform::current();
            if self.request.platform == Platform::Termux {
                return Err(Error::UnsupportedTarget(
                    "Termux JDKs must be installed with the Termux package manager".to_string(),
                ));
            }
            if self.request.platform != host {
                return Err(Error::UnsupportedTarget(format!(
                    "cannot install a {} jdk on {host}",
                    self.request.platform
                )));
            }

            (self.on_event)(InstallEvent::Resolving);
            let release = self.provider.resolve(self.request.clone()).await?;

            // Provider-supplied strings reach the filesystem, so they must be
            // exactly one ordinary path component.
            let file_name = safe_component("file_name", &release.file_name)?;
            safe_component("release_name", &release.release_name)?;

            let target = self.install_dir.join(target_dir_name(&release));
            if let Ok(existing) = inspect_java_home_with(
                &target,
                InspectOptions::default(),
                DiscoverySource::UserDirectory,
            ) {
                // A directory only counts as a hit if it really satisfies the
                // request; the name alone is not evidence.
                if validate(&existing, &release, &self.request).is_ok() {
                    (self.on_event)(InstallEvent::Installed {
                        path: existing.home.clone(),
                    });
                    return Ok(existing);
                }
            }

            if release.sha256.is_none() && !self.allow_unverified {
                return Err(Error::MissingChecksum(release.release_name.clone()));
            }

            let cache = self
                .cache_dir
                .clone()
                .unwrap_or_else(|| self.install_dir.join(".cache"));
            let client = reqwest::Client::new();

            let archive_path = {
                let on_event = &mut self.on_event;
                download::download_release(&client, &release, &cache, |p| {
                    on_event(InstallEvent::Downloading {
                        downloaded: p.downloaded,
                        total: p.total,
                    });
                })
                .await?
            };

            (self.on_event)(InstallEvent::Verifying);
            if let Some(expected) = &release.sha256 {
                checksum::verify_sha256(&archive_path, expected)?;
            }

            (self.on_event)(InstallEvent::Extracting);
            // A unique staging directory keeps concurrent installs of the same
            // release from colliding.
            let staging =
                self.install_dir
                    .join(format!(".staging-{}-{}", file_name, unique_suffix()));
            let _ = std::fs::remove_dir_all(&staging);
            let result =
                install_from_archive(&archive_path, &staging, &target, &release, &self.request);
            let _ = std::fs::remove_dir_all(&staging);
            let install = result?;

            (self.on_event)(InstallEvent::Installed {
                path: install.home.clone(),
            });
            Ok(install)
        }
    }

    /// Extract into staging, fully validate there, and only then move.
    ///
    /// Nothing reaches the final location until it has been inspected and
    /// matched against the request, so a failed install can never leave a
    /// broken JDK where callers would find it.
    fn install_from_archive(
        archive_path: &Path,
        staging: &Path,
        target: &Path,
        release: &JdkRelease,
        request: &ReleaseRequest,
    ) -> Result<JavaInstallation> {
        archive::extract(archive_path, staging)?;
        let extracted = archive::find_extracted_home(staging)?;

        let staged = inspect_java_home_with(
            &extracted,
            InspectOptions::default(),
            DiscoverySource::UserDirectory,
        )?;
        validate(&staged, release, request)?;

        // The macOS bundle layout puts the java home below the directory we
        // want to move, so move the top-level directory, not the home itself.
        let root = top_level_under(staging, &extracted);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        let _ = std::fs::remove_dir_all(target);
        move_dir(&root, target)?;

        let relative = extracted
            .strip_prefix(&root)
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        inspect_java_home_with(
            target.join(relative),
            InspectOptions::default(),
            DiscoverySource::UserDirectory,
        )
    }

    /// Check an installation against what was asked for.
    fn validate(
        install: &JavaInstallation,
        release: &JdkRelease,
        request: &ReleaseRequest,
    ) -> Result<()> {
        if install.kind != request.kind {
            return Err(Error::ValidationFailed(format!(
                "expected a {:?}, found a {:?}",
                request.kind, install.kind
            )));
        }
        if install.version.major() != release.major {
            return Err(Error::ValidationFailed(format!(
                "expected java {}, found {}",
                release.major, install.version
            )));
        }
        // Unknown metadata is tolerated; a positive mismatch is not.
        if install.architecture != Architecture::Unknown
            && install.architecture != request.architecture
        {
            return Err(Error::ValidationFailed(format!(
                "expected {}, found {}",
                request.architecture, install.architecture
            )));
        }
        if install.platform != Platform::Unknown
            && !install.platform.is_compatible_with(request.platform)
        {
            return Err(Error::ValidationFailed(format!(
                "expected {}, found {}",
                request.platform, install.platform
            )));
        }
        Ok(())
    }

    /// Directory name for an installation.
    ///
    /// Includes everything that distinguishes one build from another, so two
    /// releases that differ only by architecture or image type cannot collide.
    pub fn target_dir_name(release: &JdkRelease) -> String {
        // Adoptium semver carries build metadata ("25.0.4+7.0.LTS"); keep only
        // the numeric elements, plus any pre-release marker that distinguishes
        // an early-access build from the GA of the same number.
        let version = match crate::version::JavaVersion::parse(&release.version) {
            Ok(parsed) => {
                let numbers = parsed
                    .components()
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                match parsed.pre() {
                    Some(pre) => format!("{numbers}-{pre}"),
                    None => numbers,
                }
            }
            Err(_) => release
                .version
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect::<String>(),
        };
        let kind = match release.kind {
            JavaKind::Jdk => "jdk",
            JavaKind::Jre => "jre",
        };
        format!(
            "{}-{}-{}-{}-{}",
            release.vendor.slug(),
            version,
            release.platform,
            release.architecture,
            kind
        )
    }

    /// Reject anything that is not a single ordinary path component.
    fn safe_component<'a>(field: &'static str, value: &'a str) -> Result<&'a str> {
        let mut components = Path::new(value).components();
        let only = components.next();
        let unsafe_value = || Error::UnsafeProviderValue {
            field,
            value: value.to_string(),
        };
        match only {
            Some(std::path::Component::Normal(part))
                if components.next().is_none() && part == value =>
            {
                Ok(value)
            }
            _ => Err(unsafe_value()),
        }
    }

    fn unique_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("{}-{nanos}", std::process::id())
    }

    fn top_level_under(staging: &Path, extracted: &Path) -> PathBuf {
        let mut current = extracted.to_path_buf();
        while let Some(parent) = current.parent() {
            if parent == staging {
                return current;
            }
            if !parent.starts_with(staging) {
                break;
            }
            current = parent.to_path_buf();
        }
        extracted.to_path_buf()
    }

    fn move_dir(from: &Path, to: &Path) -> Result<()> {
        match std::fs::rename(from, to) {
            Ok(()) => Ok(()),
            // Cross-device moves need a copy; fall back to a recursive copy.
            Err(_) => {
                copy_dir(from, to)?;
                std::fs::remove_dir_all(from).map_err(|e| Error::io(from, e))
            }
        }
    }

    fn copy_dir(from: &Path, to: &Path) -> Result<()> {
        std::fs::create_dir_all(to).map_err(|e| Error::io(to, e))?;
        for entry in std::fs::read_dir(from).map_err(|e| Error::io(from, e))? {
            let entry = entry.map_err(|e| Error::io(from, e))?;
            let src = entry.path();
            let dst = to.join(entry.file_name());
            let file_type = entry.file_type().map_err(|e| Error::io(&src, e))?;
            if file_type.is_dir() {
                copy_dir(&src, &dst)?;
            } else if file_type.is_symlink() {
                #[cfg(unix)]
                {
                    let link = std::fs::read_link(&src).map_err(|e| Error::io(&src, e))?;
                    std::os::unix::fs::symlink(link, &dst).map_err(|e| Error::io(&dst, e))?;
                }
                #[cfg(not(unix))]
                {
                    std::fs::copy(&src, &dst).map_err(|e| Error::io(&dst, e))?;
                }
            } else {
                std::fs::copy(&src, &dst).map_err(|e| Error::io(&dst, e))?;
            }
        }
        Ok(())
    }

    /// The default per-user installation directory.
    pub fn default_install_dir() -> PathBuf {
        crate::discovery::common_dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(".java-path")
            .join("jdks")
    }
}

#[cfg(feature = "install")]
pub use installer::{default_install_dir, target_dir_name, InstallEvent, JavaInstaller};
