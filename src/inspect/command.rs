//! Fallback metadata extraction by invoking the `java` launcher.
//!
//! Used only when `<JAVA_HOME>/release` is missing or unusable.

use crate::error::{Error, Result};
use crate::model::{Architecture, JavaKind, JavaMetadata, Platform};
use crate::version::JavaVersion;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// Run `java -XshowSettings:properties -version` and parse the system properties.
///
/// Falls back to `java -version` when the settings flag is unsupported.
pub fn inspect_with_command(java: &Path, kind: JavaKind) -> Result<JavaMetadata> {
    if let Ok(props) = system_properties(java) {
        if let Some(meta) = metadata_from_system_properties(&props, kind) {
            return Ok(meta);
        }
    }
    metadata_from_version_output(&version_output(java)?, kind)
}

/// Parse the output of `java -XshowSettings:properties -version`.
pub fn system_properties(java: &Path) -> Result<BTreeMap<String, String>> {
    let output = run(java, &["-XshowSettings:properties", "-version"])?;
    let mut props = BTreeMap::new();
    let mut last_key: Option<String> = None;
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some((key, value)) = trimmed.split_once(" = ") {
            last_key = Some(key.trim().to_string());
            props.insert(key.trim().to_string(), value.trim().to_string());
        } else if trimmed.is_empty() {
            last_key = None;
        } else if let Some(key) = &last_key {
            // Continuation lines (e.g. multi-entry paths); keep the first value.
            let _ = key;
        }
    }
    if props.is_empty() {
        return Err(Error::Command {
            program: java.display().to_string(),
            reason: "no system properties in output".to_string(),
        });
    }
    Ok(props)
}

fn metadata_from_system_properties(
    props: &BTreeMap<String, String>,
    kind: JavaKind,
) -> Option<JavaMetadata> {
    let version = props
        .get("java.version")
        .and_then(|v| JavaVersion::parse(v).ok())?;
    Some(JavaMetadata {
        version,
        vendor: props.get("java.vendor").cloned().filter(|v| !v.is_empty()),
        architecture: props
            .get("os.arch")
            .map(|a| Architecture::parse(a))
            .unwrap_or(Architecture::Unknown),
        platform: props
            .get("os.name")
            .map(|o| Platform::parse(o))
            .unwrap_or(Platform::Unknown),
        kind,
    })
}

/// Capture the combined output of `java -version`.
pub fn version_output(java: &Path) -> Result<String> {
    run(java, &["-version"])
}

/// Parse the classic `java -version` banner.
pub fn metadata_from_version_output(output: &str, kind: JavaKind) -> Result<JavaMetadata> {
    let raw = output
        .lines()
        .find_map(|line| {
            let start = line.find('"')?;
            let rest = &line[start + 1..];
            let end = rest.find('"')?;
            Some(rest[..end].to_string())
        })
        .ok_or_else(|| Error::InvalidVersion(output.to_string()))?;

    let version = JavaVersion::parse(&raw)?;
    let vendor = output
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .map(|w| w.to_string())
        .filter(|w| !w.is_empty() && w != "java" && w != "openjdk");

    Ok(JavaMetadata {
        version,
        vendor,
        architecture: Architecture::Unknown,
        platform: Platform::Unknown,
        kind,
    })
}

fn run(java: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(java)
        .args(args)
        .output()
        .map_err(|e| Error::Command {
            program: java.display().to_string(),
            reason: e.to_string(),
        })?;
    let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    if text.trim().is_empty() {
        return Err(Error::Command {
            program: java.display().to_string(),
            reason: "empty output".to_string(),
        });
    }
    Ok(text)
}
