//! Core data model: platforms, architectures and installations.

use crate::version::JavaVersion;
use std::fmt;
use std::path::PathBuf;

/// Operating system family a JDK targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum Platform {
    /// Linux (glibc or musl).
    Linux,
    /// macOS / Darwin.
    MacOs,
    /// Microsoft Windows.
    Windows,
    /// Android under Termux.
    Termux,
    /// Anything else.
    Unknown,
}

impl Platform {
    /// The platform this binary is running on.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "android") || crate::discovery::termux::is_termux() {
            Platform::Termux
        } else if cfg!(target_os = "linux") {
            Platform::Linux
        } else {
            Platform::Unknown
        }
    }

    /// Parse an OS token as found in `release` files or directory names.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "linux" => Platform::Linux,
            "mac" | "macos" | "darwin" | "osx" => Platform::MacOs,
            "windows" | "win" | "win32" => Platform::Windows,
            "android" | "termux" => Platform::Termux,
            _ => Platform::Unknown,
        }
    }

    /// The token the Adoptium API uses for this platform.
    pub fn adoptium_os(self) -> Option<&'static str> {
        match self {
            Platform::Linux | Platform::Termux => Some("linux"),
            Platform::MacOs => Some("mac"),
            Platform::Windows => Some("windows"),
            Platform::Unknown => None,
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Platform::Linux => "linux",
            Platform::MacOs => "mac",
            Platform::Windows => "windows",
            Platform::Termux => "termux",
            Platform::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// CPU architecture a JDK targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum Architecture {
    /// 64-bit x86.
    X86_64,
    /// 32-bit x86.
    X86,
    /// 64-bit ARM.
    Aarch64,
    /// 32-bit ARM.
    Arm,
    /// 64-bit PowerPC little-endian.
    Ppc64le,
    /// IBM Z.
    S390x,
    /// 64-bit RISC-V.
    Riscv64,
    /// Unrecognised architecture.
    Unknown,
}

impl Architecture {
    /// The architecture this binary was compiled for.
    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "x86_64" => Architecture::X86_64,
            "x86" => Architecture::X86,
            "aarch64" => Architecture::Aarch64,
            "arm" => Architecture::Arm,
            "powerpc64" => Architecture::Ppc64le,
            "s390x" => Architecture::S390x,
            "riscv64" => Architecture::Riscv64,
            _ => Architecture::Unknown,
        }
    }

    /// Parse an architecture token from a `release` file or a directory name.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "x86_64" | "amd64" | "x64" => Architecture::X86_64,
            "x86" | "i386" | "i586" | "i686" | "x32" => Architecture::X86,
            "aarch64" | "arm64" => Architecture::Aarch64,
            "arm" | "arm32" | "armv7l" | "armhf" => Architecture::Arm,
            "ppc64le" | "ppc64el" => Architecture::Ppc64le,
            "s390x" => Architecture::S390x,
            "riscv64" => Architecture::Riscv64,
            _ => Architecture::Unknown,
        }
    }

    /// The token the Adoptium API uses for this architecture.
    pub fn adoptium_arch(self) -> Option<&'static str> {
        match self {
            Architecture::X86_64 => Some("x64"),
            Architecture::X86 => Some("x32"),
            Architecture::Aarch64 => Some("aarch64"),
            Architecture::Arm => Some("arm"),
            Architecture::Ppc64le => Some("ppc64le"),
            Architecture::S390x => Some("s390x"),
            Architecture::Riscv64 => Some("riscv64"),
            Architecture::Unknown => None,
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.adoptium_arch().unwrap_or("unknown"))
    }
}

/// Whether an installation can compile Java or only run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum JavaKind {
    /// Full development kit: `bin/javac` present.
    Jdk,
    /// Runtime only.
    Jre,
}

/// Where an installation was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[non_exhaustive]
pub enum DiscoverySource {
    /// The `JAVA_HOME` environment variable.
    JavaHome,
    /// A `java` executable on `PATH`.
    Path,
    /// A well-known system directory for the platform.
    System,
    /// The Windows registry.
    Registry,
    /// SDKMAN! candidate store.
    Sdkman,
    /// mise install store.
    Mise,
    /// Gradle's JDK cache.
    Gradle,
    /// JBang's JDK cache.
    Jbang,
    /// A root supplied by the caller.
    UserDirectory,
    /// Termux packages.
    Termux,
}

impl DiscoverySource {
    /// Priority used to break ties between duplicate installations.
    ///
    /// Lower is better; the enum's declaration order is the priority order.
    pub fn priority(self) -> u8 {
        self as u8
    }
}

/// Metadata about a Java installation, independent of where it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JavaMetadata {
    /// Parsed version.
    pub version: JavaVersion,
    /// Vendor/implementor string, when known.
    pub vendor: Option<String>,
    /// Target architecture.
    pub architecture: Architecture,
    /// Target platform.
    pub platform: Platform,
    /// JDK or JRE.
    pub kind: JavaKind,
}

/// A concrete Java installation on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JavaInstallation {
    /// The Java home directory.
    pub home: PathBuf,
    /// Path to the `java` launcher.
    pub java: PathBuf,
    /// Path to `javac`, when this is a JDK.
    pub javac: Option<PathBuf>,
    /// Parsed version.
    pub version: JavaVersion,
    /// Vendor/implementor string, when known.
    pub vendor: Option<String>,
    /// Target architecture.
    pub architecture: Architecture,
    /// Target platform.
    pub platform: Platform,
    /// JDK or JRE.
    pub kind: JavaKind,
    /// Where it was discovered.
    pub source: DiscoverySource,
}

impl JavaInstallation {
    /// `true` when `bin/javac` is present.
    pub fn is_jdk(&self) -> bool {
        self.kind == JavaKind::Jdk
    }

    /// Feature/major version number.
    pub fn major(&self) -> u32 {
        self.version.major
    }

    pub(crate) fn from_metadata(
        home: PathBuf,
        java: PathBuf,
        javac: Option<PathBuf>,
        meta: JavaMetadata,
        source: DiscoverySource,
    ) -> Self {
        JavaInstallation {
            home,
            java,
            javac,
            version: meta.version,
            vendor: meta.vendor,
            architecture: meta.architecture,
            platform: meta.platform,
            kind: meta.kind,
            source,
        }
    }
}
