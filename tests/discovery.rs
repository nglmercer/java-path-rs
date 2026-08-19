use java_path::{Discovery, DiscoverySource};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn finds_every_fixture_under_a_custom_root() {
    let installs = Discovery::only_roots().root(fixtures()).search().unwrap();
    let majors: Vec<u32> = installs.iter().map(|i| i.version.major).collect();
    for expected in [8, 11, 17, 21, 22] {
        assert!(
            majors.contains(&expected),
            "missing java {expected} in {majors:?}"
        );
    }
    assert!(installs
        .iter()
        .all(|i| i.source == DiscoverySource::UserDirectory));
}

#[test]
fn results_are_sorted_newest_first_and_deterministic() {
    let first = Discovery::only_roots().root(fixtures()).search().unwrap();
    let second = Discovery::only_roots().root(fixtures()).search().unwrap();
    assert_eq!(first, second);

    let versions: Vec<_> = first.iter().map(|i| i.version.clone()).collect();
    let mut sorted = versions.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(versions, sorted);
}

#[test]
fn homes_are_unique() {
    let installs = Discovery::only_roots()
        .root(fixtures())
        // The same root passed twice must not produce duplicates.
        .root(fixtures())
        .search()
        .unwrap();
    let mut homes: Vec<_> = installs.iter().map(|i| i.home.clone()).collect();
    let count = homes.len();
    homes.sort();
    homes.dedup();
    assert_eq!(homes.len(), count);
}

#[test]
fn missing_roots_are_ignored() {
    let installs = Discovery::only_roots()
        .root(fixtures().join("does-not-exist"))
        .search()
        .unwrap();
    assert!(installs.is_empty());
}

#[test]
fn default_discovery_does_not_error() {
    // The host may or may not have a JDK; discovery must never fail because of it.
    assert!(Discovery::new().search().is_ok());
}
