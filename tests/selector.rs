use java_path::{Architecture, Discovery, JavaKind, JavaQuery, SelectExt};
use std::path::{Path, PathBuf};

fn installs() -> Vec<java_path::JavaInstallation> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    Discovery::only_roots().root(fixtures).search().unwrap()
}

fn home_name(install: &java_path::JavaInstallation) -> String {
    PathBuf::from(&install.home)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn selects_exact_major() {
    let installs = installs();
    let java = installs.select().major(17).best().unwrap();
    assert_eq!(java.version.major(), 17);
}

#[test]
fn prefers_jdk_over_jre() {
    let installs = installs();
    let java = installs.select().best().unwrap();
    assert_eq!(java.kind, JavaKind::Jdk);
}

#[test]
fn jdk_only_excludes_the_jre() {
    let installs = installs();
    let matches = installs.select().jdk().all();
    assert!(matches.iter().all(|i| i.is_jdk()));
    assert!(!matches.is_empty());
}

#[test]
fn prerelease_excluded_by_default() {
    let installs = installs();
    assert!(installs
        .select()
        .all()
        .iter()
        .all(|i| !i.version.is_prerelease()));

    let with_ea = JavaQuery::new().allow_prerelease(true);
    assert!(installs
        .iter()
        .filter(|i| with_ea.matches(i))
        .any(|i| i.version.is_prerelease()));
}

#[test]
fn filters_by_vendor_case_insensitively() {
    let installs = installs();
    let java = installs.select().vendor("eclipse adoptium").best().unwrap();
    assert!(java
        .vendor
        .as_deref()
        .unwrap()
        .to_lowercase()
        .contains("adoptium"));
}

#[test]
fn filters_by_architecture() {
    let installs = installs();
    let query = JavaQuery::new().architecture(Architecture::Aarch64);
    let matches: Vec<_> = installs.iter().filter(|i| query.matches(i)).collect();
    assert!(matches
        .iter()
        .all(|i| i.architecture == Architecture::Aarch64));
    assert_eq!(matches.len(), 1);
    assert_eq!(home_name(matches[0]), "jdk-21.0.3");
}

#[test]
fn min_major_and_range() {
    let installs = installs();
    assert!(installs
        .iter()
        .filter(|i| JavaQuery::new().min_major(17).matches(i))
        .all(|i| i.version.major() >= 17));

    assert!(installs
        .iter()
        .filter(|i| JavaQuery::new().major_range(11, 17).matches(i))
        .all(|i| (11..=17).contains(&i.version.major())));
}

#[test]
fn no_match_is_an_error() {
    let installs = installs();
    assert!(installs.select().major(99).best().is_err());
}

#[test]
fn selection_is_deterministic() {
    let installs = installs();
    let a = installs.select().jdk().all();
    let b = installs.select().jdk().all();
    assert_eq!(a, b);
}

/// The fluent selector must expose everything `JavaQuery` supports; a split
/// API where some constraints are only reachable one way is a trap.
#[test]
fn selector_forwards_every_query_constraint() {
    let installs = installs();
    use java_path::{JavaVersion, Platform};

    assert!(installs
        .select()
        .max_major(11)
        .all()
        .iter()
        .all(|i| i.version.major() <= 11));
    assert!(installs
        .select()
        .major_range(11, 17)
        .all()
        .iter()
        .all(|i| (11..=17).contains(&i.version.major())));
    assert!(installs
        .select()
        .min_version(JavaVersion::parse("17.0.5").unwrap())
        .all()
        .iter()
        .all(|i| i.version >= JavaVersion::parse("17.0.5").unwrap()));
    assert!(installs.select().jre().all().iter().all(|i| !i.is_jdk()));
    assert!(installs
        .select()
        .architecture(Architecture::Aarch64)
        .all()
        .iter()
        .all(|i| i.architecture == Architecture::Aarch64));
    assert!(installs
        .select()
        .platform(Platform::MacOs)
        .all()
        .iter()
        .all(|i| i.platform == Platform::MacOs));
    assert!(installs
        .select()
        .allow_prerelease(true)
        .all()
        .iter()
        .any(|i| i.version.is_prerelease()));
}
