use java_path::JavaVersion;

#[test]
fn parses_legacy_java_8() {
    let v = JavaVersion::parse("1.8.0_412").unwrap();
    assert_eq!(v.major, 8);
    assert_eq!(v.patch, 412);
    assert!(!v.is_prerelease());
}

#[test]
fn parses_legacy_java_8_with_build() {
    let v = JavaVersion::parse("1.8.0_412-b08").unwrap();
    assert_eq!(v.major, 8);
    assert_eq!(v.patch, 412);
    assert_eq!(v.build, Some(8));
}

#[test]
fn parses_modern_versions() {
    let v = JavaVersion::parse("11.0.22+7").unwrap();
    assert_eq!((v.major, v.minor, v.patch, v.build), (11, 0, 22, Some(7)));

    let v = JavaVersion::parse("17.0.10").unwrap();
    assert_eq!((v.major, v.minor, v.patch), (17, 0, 10));

    let v = JavaVersion::parse("21").unwrap();
    assert_eq!((v.major, v.minor, v.patch), (21, 0, 0));
}

#[test]
fn parses_early_access() {
    let v = JavaVersion::parse("22-ea+15").unwrap();
    assert_eq!(v.major, 22);
    assert_eq!(v.build, Some(15));
    assert!(v.is_prerelease());
}

#[test]
fn rejects_garbage() {
    for bad in ["", "   ", "abc", "-1", "not.a.version"] {
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
fn keeps_raw_string() {
    assert_eq!(
        JavaVersion::parse("17.0.10+7").unwrap().to_string(),
        "17.0.10+7"
    );
}
