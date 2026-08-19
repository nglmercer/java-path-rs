use java_path::inspect::release_file::{metadata_from_properties, parse_release_contents};
use java_path::{inspect_java_home, Architecture, JavaKind, Platform};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn parses_quoted_values_and_ignores_junk() {
    let props = parse_release_contents(
        "# comment\nJAVA_VERSION=\"17.0.10\"\nnonsense line\nOS_ARCH=\"x86_64\"\n",
    );
    assert_eq!(props.get("JAVA_VERSION").unwrap(), "17.0.10");
    assert_eq!(props.get("OS_ARCH").unwrap(), "x86_64");
    assert_eq!(props.len(), 2);
}

#[test]
fn rejects_invalid_java_version() {
    let props = parse_release_contents("JAVA_VERSION=\"nope\"\n");
    let err = metadata_from_properties(Path::new("release"), &props, JavaKind::Jdk).unwrap_err();
    assert!(err.to_string().contains("invalid JAVA_VERSION"));
}

#[test]
fn inspects_temurin_8() {
    let install = inspect_java_home(fixture("jdk-8u412-b08")).unwrap();
    assert_eq!(install.version.major, 8);
    assert_eq!(install.vendor.as_deref(), Some("Temurin"));
    assert_eq!(install.architecture, Architecture::X86_64);
    assert_eq!(install.platform, Platform::Linux);
    assert!(install.is_jdk());
}

#[test]
fn inspects_temurin_17() {
    let install = inspect_java_home(fixture("jdk-17.0.10+7")).unwrap();
    assert_eq!(install.version.major, 17);
    assert_eq!(install.vendor.as_deref(), Some("Eclipse Adoptium"));
    assert!(install.javac.is_some());
}

#[test]
fn inspects_oracle_21() {
    let install = inspect_java_home(fixture("jdk-21.0.3")).unwrap();
    assert_eq!(install.version.major, 21);
    assert_eq!(install.architecture, Architecture::Aarch64);
    assert_eq!(install.platform, Platform::MacOs);
}

#[test]
fn detects_jre_without_javac() {
    let install = inspect_java_home(fixture("jre-11.0.22+7")).unwrap();
    assert_eq!(install.kind, JavaKind::Jre);
    assert!(install.javac.is_none());
}

#[test]
fn detects_early_access_build() {
    let install = inspect_java_home(fixture("openjdk-22-ea")).unwrap();
    assert!(install.version.is_prerelease());
}

#[test]
fn malformed_release_falls_back_to_path_heuristics() {
    // The directory name carries no version, so there is nothing to fall back on.
    assert!(inspect_java_home(fixture("malformed-jdk")).is_err());
}

#[test]
fn missing_release_uses_directory_name() {
    let install = inspect_java_home(fixture("no-release/java-11-openjdk-amd64")).unwrap();
    assert_eq!(install.version.major, 11);
    assert_eq!(install.architecture, Architecture::X86_64);
}

#[test]
fn heuristics_can_be_disabled() {
    use java_path::{inspect_java_home_with, DiscoverySource, InspectOptions};
    let err = inspect_java_home_with(
        fixture("no-release/java-11-openjdk-amd64"),
        InspectOptions::strict(),
        DiscoverySource::UserDirectory,
    );
    assert!(err.is_err());
}

#[test]
fn rejects_non_java_directory() {
    assert!(inspect_java_home(fixture("..")).is_err());
}
