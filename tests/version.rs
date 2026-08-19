use java_path::JavaVersion;
use std::collections::{BTreeSet, HashSet};

#[test]
fn parses_legacy_java_8() {
    let v = JavaVersion::parse("1.8.0_412").unwrap();
    assert_eq!(v.major(), 8);
    assert_eq!(v.patch(), 412);
    assert!(!v.is_prerelease());
}

#[test]
fn parses_legacy_java_8_with_build() {
    let v = JavaVersion::parse("1.8.0_412-b08").unwrap();
    assert_eq!(v.major(), 8);
    assert_eq!(v.patch(), 412);
    assert_eq!(v.build(), Some(8));
}

#[test]
fn parses_modern_versions() {
    let v = JavaVersion::parse("11.0.22+7").unwrap();
    assert_eq!(
        (v.major(), v.minor(), v.patch(), v.build()),
        (11, 0, 22, Some(7))
    );

    let v = JavaVersion::parse("17.0.10").unwrap();
    assert_eq!((v.major(), v.minor(), v.patch()), (17, 0, 10));

    let v = JavaVersion::parse("21").unwrap();
    assert_eq!((v.major(), v.minor(), v.patch()), (21, 0, 0));
}

/// JEP 322 version numbers are arbitrarily long sequences. Collapsing to
/// three elements made a patched JDK indistinguishable from an unpatched one.
#[test]
fn keeps_the_fourth_and_later_components() {
    let patched = JavaVersion::parse("17.0.10.1").unwrap();
    let base = JavaVersion::parse("17.0.10").unwrap();

    assert_eq!(patched.components(), &[17, 0, 10, 1]);
    assert_eq!(patched.component(3), 1);
    assert_ne!(patched, base);
    assert!(patched > base, "17.0.10.1 must sort above 17.0.10");

    let long = JavaVersion::parse("17.0.10.1.2").unwrap();
    assert_eq!(long.components(), &[17, 0, 10, 1, 2]);
    assert!(long > patched);
}

/// Regression: `Eq`/`Hash` were derived over `raw` while `Ord` ignored it, so
/// `17` and `17.0.0` were simultaneously equal and unequal. That breaks
/// `BTreeSet`, sorting and deduplication.
#[test]
fn eq_hash_and_ord_agree() {
    let a = JavaVersion::parse("17").unwrap();
    let b = JavaVersion::parse("17.0.0").unwrap();

    assert_eq!(a, b);
    assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
    assert_eq!(BTreeSet::from([a.clone(), b.clone()]).len(), 1);
    assert_eq!(HashSet::from([a.clone(), b.clone()]).len(), 1);

    // Distinct versions must stay distinct in both containers.
    let c = JavaVersion::parse("17.0.1").unwrap();
    assert_ne!(a, c);
    assert_eq!(BTreeSet::from([a.clone(), c.clone()]).len(), 2);
    assert_eq!(HashSet::from([a, c]).len(), 2);
}

#[test]
fn raw_string_is_preserved_but_not_part_of_identity() {
    let a = JavaVersion::parse("17").unwrap();
    let b = JavaVersion::parse("17.0.0").unwrap();
    assert_eq!(a.raw(), "17");
    assert_eq!(b.raw(), "17.0.0");
    assert_eq!(a.to_string(), "17");
    assert_eq!(a, b);
}

#[test]
fn parses_early_access() {
    let v = JavaVersion::parse("22-ea+15").unwrap();
    assert_eq!(v.major(), 22);
    assert_eq!(v.build(), Some(15));
    assert_eq!(v.pre(), Some("ea"));
    assert!(v.is_prerelease());
}

#[test]
fn rejects_garbage() {
    for bad in ["", "   ", "abc", "-1", "not.a.version", "0", "0.1"] {
        assert!(JavaVersion::parse(bad).is_err(), "{bad:?} should not parse");
    }
}

#[test]
fn orders_by_feature_then_patch() {
    let v8 = JavaVersion::parse("1.8.0_412").unwrap();
    let v11 = JavaVersion::parse("11.0.22").unwrap();
    let v17 = JavaVersion::parse("17.0.10").unwrap();
    let v17b = JavaVersion::parse("17.0.11").unwrap();
    assert!(v8 < v11);
    assert!(v11 < v17);
    assert!(v17 < v17b);
}

#[test]
fn ga_sorts_above_prerelease() {
    let ea = JavaVersion::parse("22-ea+15").unwrap();
    let ga = JavaVersion::parse("22").unwrap();
    assert!(ea < ga);
}

#[test]
fn build_number_breaks_ties() {
    let a = JavaVersion::parse("21.0.3+9").unwrap();
    let b = JavaVersion::parse("21.0.3+10").unwrap();
    assert!(a < b);
}

#[test]
fn sorting_is_total_and_consistent() {
    let mut versions: Vec<JavaVersion> = [
        "17.0.10",
        "1.8.0_412",
        "21",
        "17.0.10.1",
        "22-ea+15",
        "22",
        "11.0.22+7",
        "17",
    ]
    .iter()
    .map(|s| JavaVersion::parse(s).unwrap())
    .collect();
    versions.sort();

    let order: Vec<String> = versions.iter().map(|v| v.raw().to_string()).collect();
    assert_eq!(
        order,
        [
            "1.8.0_412",
            "11.0.22+7",
            "17",
            "17.0.10",
            "17.0.10.1",
            "21",
            "22-ea+15",
            "22"
        ]
    );
}
