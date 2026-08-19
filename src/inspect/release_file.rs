//! Parsing of the `<JAVA_HOME>/release` metadata file.
//!
//! This is the authoritative metadata source: it requires no subprocess.

use crate::error::{Error, Result};
use crate::model::{Architecture, JavaKind, JavaMetadata, Platform};
use crate::version::JavaVersion;
use std::collections::BTreeMap;
use std::path::Path;

/// Parsed `KEY="VALUE"` pairs from a `release` file.
pub type ReleaseProperties = BTreeMap<String, String>;

/// Parse the raw contents of a `release` file.
///
/// Unknown keys are kept; malformed lines are ignored.
pub fn parse_release_contents(contents: &str) -> ReleaseProperties {
    let mut map = BTreeMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = value.trim().trim_matches('"').to_string();
        map.insert(key.to_string(), value);
    }
    map
}

/// `true` when the failure was simply that no `release` file exists.
///
/// A missing file is an ordinary situation that may fall back to another
/// metadata source; a file that exists but is invalid is an error.
pub fn is_missing(error: &Error) -> bool {
    matches!(
        error,
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound
    )
}

/// Read and parse `<home>/release`.
pub fn read_release_file(home: &Path) -> Result<ReleaseProperties> {
    let path = home.join("release");
    let contents = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let props = parse_release_contents(&contents);
    if props.is_empty() {
        return Err(Error::MalformedReleaseFile {
            path,
            reason: "no key=value pairs found".to_string(),
        });
    }
    Ok(props)
}

/// Build metadata from an already-parsed `release` file.
///
/// `kind` is decided by the caller from the presence of `bin/javac`, since the
/// `release` file does not always distinguish JDK from JRE reliably.
pub fn metadata_from_properties(
    path: &Path,
    props: &ReleaseProperties,
    kind: JavaKind,
) -> Result<JavaMetadata> {
    let raw_version = props
        .get("JAVA_VERSION")
        .ok_or_else(|| Error::MalformedReleaseFile {
            path: path.to_path_buf(),
            reason: "missing JAVA_VERSION".to_string(),
        })?;

    let version = JavaVersion::parse(raw_version).map_err(|_| Error::MalformedReleaseFile {
        path: path.to_path_buf(),
        reason: format!("invalid JAVA_VERSION {raw_version:?}"),
    })?;

    let vendor = props
        .get("IMPLEMENTOR")
        .or_else(|| props.get("JAVA_VENDOR"))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let architecture = props
        .get("OS_ARCH")
        .map(|a| Architecture::parse(a))
        .unwrap_or(Architecture::Unknown);

    let platform = props
        .get("OS_NAME")
        .map(|o| Platform::parse(o))
        .unwrap_or(Platform::Unknown);

    Ok(JavaMetadata {
        version,
        vendor,
        architecture,
        platform,
        kind,
    })
}

/// Read `<home>/release` and turn it into metadata.
pub fn inspect_release_file(home: &Path, kind: JavaKind) -> Result<JavaMetadata> {
    let props = read_release_file(home)?;
    metadata_from_properties(&home.join("release"), &props, kind)
}
