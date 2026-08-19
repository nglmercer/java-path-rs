//! Inspecting a candidate Java home and extracting authoritative metadata.
//!
//! Metadata sources are tried in strict priority order:
//!
//! 1. `<JAVA_HOME>/release`
//! 2. `java -XshowSettings:properties -version`
//! 3. `java -version`
//! 4. directory-name heuristics (last resort only)

pub mod command;
pub mod release_file;

use crate::error::{Error, Result};
use crate::model::{
    Architecture, DiscoverySource, JavaInstallation, JavaKind, JavaMetadata, Platform,
};
use crate::version::JavaVersion;
use std::path::{Path, PathBuf};

/// Controls which metadata sources `inspect_java_home` may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectOptions {
    /// Allow invoking the `java` launcher when the `release` file is unusable.
    pub allow_subprocess: bool,
    /// Allow falling back to directory-name heuristics.
    pub allow_heuristics: bool,
}

impl Default for InspectOptions {
    fn default() -> Self {
        // Subprocess execution is off by default: filesystem scans should not
        // launch arbitrary executables.
        InspectOptions {
            allow_subprocess: false,
            allow_heuristics: true,
        }
    }
}

impl InspectOptions {
    /// Allow every metadata source, including subprocess execution.
    pub fn thorough() -> Self {
        InspectOptions {
            allow_subprocess: true,
            allow_heuristics: true,
        }
    }

    /// Only accept authoritative metadata (`release` file, or `java` if enabled).
    pub fn strict() -> Self {
        InspectOptions {
            allow_subprocess: false,
            allow_heuristics: false,
        }
    }
}

/// The executables inside a Java home.
#[derive(Debug, Clone)]
pub struct JavaLayout {
    /// Canonicalised Java home.
    pub home: PathBuf,
    /// Path to `bin/java`.
    pub java: PathBuf,
    /// Path to `bin/javac`, when present.
    pub javac: Option<PathBuf>,
    /// JDK when `javac` exists, otherwise JRE.
    pub kind: JavaKind,
}

fn exe(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Locate `bin/java` and `bin/javac` under a candidate home.
///
/// On macOS, a `Contents/Home` bundle layout is resolved transparently.
pub fn resolve_layout(path: impl AsRef<Path>) -> Result<JavaLayout> {
    let raw = path.as_ref();
    let mut home = raw.to_path_buf();

    if !home.join("bin").join(exe("java")).is_file() {
        let bundle = home.join("Contents").join("Home");
        if bundle.join("bin").join(exe("java")).is_file() {
            home = bundle;
        }
    }

    // Callers sometimes pass `<home>/bin` or the launcher itself.
    if !home.join("bin").join(exe("java")).is_file() {
        if home.file_name().and_then(|n| n.to_str()) == Some("bin") {
            if let Some(parent) = home.parent() {
                home = parent.to_path_buf();
            }
        } else if home.is_file() {
            if let Some(parent) = home.parent().and_then(|p| p.parent()) {
                home = parent.to_path_buf();
            }
        }
    }

    let java = home.join("bin").join(exe("java"));
    if !java.is_file() {
        return Err(Error::NotAJavaHome {
            path: raw.to_path_buf(),
            reason: "bin/java not found",
        });
    }

    let javac_path = home.join("bin").join(exe("javac"));
    let javac = javac_path.is_file().then_some(javac_path);
    let kind = if javac.is_some() {
        JavaKind::Jdk
    } else {
        JavaKind::Jre
    };

    // Canonicalise so symlinked duplicates collapse; fall back on failure.
    let home = std::fs::canonicalize(&home).unwrap_or(home);
    let java = std::fs::canonicalize(&java).unwrap_or(java);

    Ok(JavaLayout {
        home,
        java,
        javac,
        kind,
    })
}

/// Inspect a candidate Java home with the default options.
pub fn inspect_java_home(path: impl AsRef<Path>) -> Result<JavaInstallation> {
    inspect_java_home_with(
        path,
        InspectOptions::default(),
        DiscoverySource::UserDirectory,
    )
}

/// Inspect a candidate Java home, controlling which metadata sources are used.
pub fn inspect_java_home_with(
    path: impl AsRef<Path>,
    options: InspectOptions,
    source: DiscoverySource,
) -> Result<JavaInstallation> {
    let layout = resolve_layout(path)?;
    let meta = metadata_for(&layout, options)?;
    Ok(JavaInstallation::from_metadata(
        layout.home,
        layout.java,
        layout.javac,
        meta,
        source,
    ))
}

fn metadata_for(layout: &JavaLayout, options: InspectOptions) -> Result<JavaMetadata> {
    let mut last_error = match release_file::inspect_release_file(&layout.home, layout.kind) {
        Ok(mut meta) => {
            fill_gaps(&mut meta, &layout.home);
            return Ok(meta);
        }
        Err(e) => e,
    };

    if options.allow_subprocess {
        match command::inspect_with_command(&layout.java, layout.kind) {
            Ok(mut meta) => {
                fill_gaps(&mut meta, &layout.home);
                return Ok(meta);
            }
            Err(e) => last_error = e,
        }
    }

    if options.allow_heuristics {
        if let Some(mut meta) = metadata_from_path(&layout.home, layout.kind) {
            fill_gaps(&mut meta, &layout.home);
            return Ok(meta);
        }
    }

    Err(last_error)
}

/// Fill in unknown platform/architecture with the host's values.
fn fill_gaps(meta: &mut JavaMetadata, home: &Path) {
    if meta.architecture == Architecture::Unknown {
        if let Some(a) = arch_from_path(home) {
            meta.architecture = a;
        }
    }
    if meta.platform == Platform::Unknown {
        if let Some(p) = platform_from_path(home) {
            meta.platform = p;
        }
    }
}

/// Last-resort metadata inference from a directory name.
///
/// Recognises names such as `jdk-21.0.3+9`, `java-11-openjdk`, `openjdk-17`
/// and `8_x86_64_windows`.
pub fn metadata_from_path(home: &Path, kind: JavaKind) -> Option<JavaMetadata> {
    let name = home.file_name()?.to_str()?;
    let version = version_from_name(name)?;
    Some(JavaMetadata {
        version,
        vendor: vendor_from_name(name),
        architecture: arch_from_path(home).unwrap_or(Architecture::Unknown),
        platform: platform_from_path(home).unwrap_or(Platform::Unknown),
        kind,
    })
}

fn version_from_name(name: &str) -> Option<JavaVersion> {
    // Prefer the longest dotted numeric run, else the first standalone number.
    let mut best: Option<JavaVersion> = None;
    for token in name.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '_' || c == '+')) {
        let token = token.trim_matches(|c: char| c == '.' || c == '_' || c == '+');
        if token.is_empty() || !token.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        if let Ok(v) = JavaVersion::parse(token) {
            let better = match &best {
                None => true,
                Some(prev) => token.matches('.').count() > prev.raw.matches('.').count(),
            };
            if better {
                best = Some(v);
            }
        }
    }
    best
}

fn vendor_from_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    [
        ("temurin", "Eclipse Adoptium"),
        ("adoptium", "Eclipse Adoptium"),
        ("adoptopenjdk", "AdoptOpenJDK"),
        ("corretto", "Amazon"),
        ("zulu", "Azul Systems, Inc."),
        ("graalvm", "GraalVM Community"),
        ("microsoft", "Microsoft"),
        ("liberica", "BellSoft"),
        ("semeru", "IBM"),
        ("oracle", "Oracle Corporation"),
    ]
    .into_iter()
    .find(|(needle, _)| lower.contains(needle))
    .map(|(_, vendor)| vendor.to_string())
}

fn arch_from_path(home: &Path) -> Option<Architecture> {
    let lower = home.to_str()?.to_ascii_lowercase();
    for token in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
        let arch = Architecture::parse(token);
        if arch != Architecture::Unknown {
            return Some(arch);
        }
    }
    None
}

fn platform_from_path(home: &Path) -> Option<Platform> {
    let lower = home.to_str()?.to_ascii_lowercase();
    for token in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
        let platform = Platform::parse(token);
        if platform != Platform::Unknown {
            return Some(platform);
        }
    }
    None
}
