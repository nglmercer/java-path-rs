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

pub use provider::{JdkProvider, JdkRelease, ReleaseRequest, Vendor};

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
                on_event: Box::new(|_| {}),
            }
        }

        /// Install a specific feature version.
        pub fn version(mut self, major: u32) -> Self {
            self.request.major = Some(major);
            self
        }

        /// Install a JRE image instead of a JDK.
        pub fn jre(mut self) -> Self {
            self.request.kind = JavaKind::Jre;
            self
        }

        /// Target a platform other than the host's.
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

        /// Subscribe to progress events.
        pub fn on_event(mut self, callback: impl FnMut(InstallEvent) + Send + 'static) -> Self {
            self.on_event = Box::new(callback);
            self
        }

        /// Resolve, download, verify, extract and install a JDK.
        ///
        /// The install is idempotent: if the target directory already holds a
        /// valid Java home, it is returned without touching the network.
        pub async fn install(mut self) -> Result<JavaInstallation> {
            if self.request.platform == Platform::Termux {
                return Err(Error::NoRelease(
                    "Termux JDKs must be installed with the Termux package manager".to_string(),
                ));
            }

            (self.on_event)(InstallEvent::Resolving);
            let release = self.provider.resolve(self.request.clone()).await?;

            let target = self.install_dir.join(&release.release_name);
            if let Ok(install) = inspect_java_home_with(
                &target,
                InspectOptions::default(),
                DiscoverySource::UserDirectory,
            ) {
                (self.on_event)(InstallEvent::Installed {
                    path: install.home.clone(),
                });
                return Ok(install);
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
            let staging = self
                .install_dir
                .join(format!(".staging-{}", release.release_name));
            let _ = std::fs::remove_dir_all(&staging);
            let result = install_from_archive(&archive_path, &staging, &target);
            let _ = std::fs::remove_dir_all(&staging);
            let home = result?;

            let install = inspect_java_home_with(
                &home,
                InspectOptions::default(),
                DiscoverySource::UserDirectory,
            )?;
            (self.on_event)(InstallEvent::Installed {
                path: install.home.clone(),
            });
            Ok(install)
        }
    }

    /// Extract into staging, validate, then move atomically into place.
    fn install_from_archive(archive_path: &Path, staging: &Path, target: &Path) -> Result<PathBuf> {
        archive::extract(archive_path, staging)?;
        let extracted = archive::find_extracted_home(staging)?;

        // The bundle layout means the java home may sit below the directory
        // we want to move; move the top-level directory, not the home itself.
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
        Ok(target.join(relative))
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
pub use installer::{default_install_dir, InstallEvent, JavaInstaller};
