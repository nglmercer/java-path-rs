#![cfg(feature = "install")]

use java_path::provision::archive::{extract, find_extracted_home, safe_join, safe_link_target};
use java_path::provision::checksum::{sha256_file, verify_sha256};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("java-path-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn rejects_parent_and_absolute_entries() {
    let dest = Path::new("/tmp/dest");
    assert!(safe_join(dest, Path::new("../escape")).is_err());
    assert!(safe_join(dest, Path::new("a/../../escape")).is_err());
    assert!(safe_join(dest, Path::new("/etc/passwd")).is_err());
    assert_eq!(
        safe_join(dest, Path::new("jdk-21/bin/java")).unwrap(),
        dest.join("jdk-21/bin/java")
    );
}

#[test]
fn hashes_and_verifies_files() {
    let dir = scratch("checksum");
    let file = dir.join("artifact.bin");
    std::fs::write(&file, b"hello").unwrap();

    // Known SHA-256 of "hello".
    let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    assert_eq!(sha256_file(&file).unwrap(), expected);
    assert!(verify_sha256(&file, expected).is_ok());
    assert!(file.is_file());
}

#[test]
fn deletes_the_artifact_on_checksum_mismatch() {
    let dir = scratch("mismatch");
    let file = dir.join("artifact.bin");
    std::fs::write(&file, b"hello").unwrap();

    let err = verify_sha256(&file, &"0".repeat(64)).unwrap_err();
    assert!(matches!(err, java_path::Error::ChecksumMismatch { .. }));
    assert!(!file.exists(), "a bad artifact must not be left behind");
}

#[test]
fn rejects_unknown_archive_formats() {
    let dir = scratch("format");
    let file = dir.join("jdk.rar");
    std::fs::write(&file, b"x").unwrap();
    let err = extract(&file, &dir.join("out")).unwrap_err();
    assert!(
        matches!(err, java_path::Error::UnsupportedArchive(_)),
        "{err}"
    );
}

#[test]
fn extracts_a_tar_gz_and_finds_the_java_home() {
    let dir = scratch("targz");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let archive = dir.join("jdk.tar.gz");

    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&source)
        .arg("jdk-17.0.10+7")
        .status()
        .unwrap();
    assert!(status.success());

    let out = dir.join("out");
    extract(&archive, &out).unwrap();
    let home = find_extracted_home(&out).unwrap();
    assert!(home.join("bin/java").is_file());

    let install = java_path::inspect_java_home(&home).unwrap();
    assert_eq!(install.version.major, 17);
}

#[test]
fn refuses_a_tar_entry_that_escapes_the_destination() {
    let dir = scratch("traversal");
    let payload = dir.join("payload");
    std::fs::create_dir_all(&payload).unwrap();
    std::fs::write(payload.join("evil"), b"pwned").unwrap();

    let archive = dir.join("evil.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&payload)
        .arg("--transform")
        .arg("s|evil|../evil|")
        .arg("evil")
        .status()
        .unwrap();
    assert!(status.success());

    let out = dir.join("out");
    let err = extract(&archive, &out).unwrap_err();
    assert!(
        matches!(err, java_path::Error::UnsafeArchiveEntry(_)),
        "{err}"
    );
    assert!(!dir.join("evil").exists());
}

#[test]
fn find_extracted_home_errors_on_an_empty_directory() {
    let dir = scratch("empty");
    assert!(find_extracted_home(&dir).is_err());
}

/// Regression: real JDK tarballs contain relative symlinks such as
/// `legal/java.se/LICENSE -> ../java.base/LICENSE`. Rejecting every target
/// containing `..` broke extraction of every genuine Temurin archive.
#[test]
fn accepts_relative_symlinks_that_stay_inside_the_root() {
    let dest = Path::new("/tmp/dest");
    let resolved = safe_link_target(
        dest,
        Path::new("jdk/legal/java.se/LICENSE"),
        Path::new("../java.base/LICENSE"),
        true,
    )
    .unwrap();
    assert_eq!(resolved, dest.join("jdk/legal/java.base/LICENSE"));
}

#[test]
fn rejects_symlinks_that_escape_the_root() {
    let dest = Path::new("/tmp/dest");
    for link in ["../../../etc/passwd", "/etc/passwd"] {
        assert!(
            safe_link_target(dest, Path::new("jdk/bin/java"), Path::new(link), true).is_err(),
            "{link} should be rejected"
        );
    }
    // Relative to the archive root (hard link), `..` has nothing to pop.
    assert!(safe_link_target(dest, Path::new("jdk/x"), Path::new("../x"), false).is_err());
}

#[test]
fn extracts_an_archive_containing_a_relative_symlink() {
    let dir = scratch("symlink");
    let tree = dir.join("jdk");
    std::fs::create_dir_all(tree.join("legal/java.se")).unwrap();
    std::fs::create_dir_all(tree.join("legal/java.base")).unwrap();
    std::fs::write(tree.join("legal/java.base/LICENSE"), b"license").unwrap();
    std::os::unix::fs::symlink("../java.base/LICENSE", tree.join("legal/java.se/LICENSE")).unwrap();

    let archive = dir.join("jdk.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .arg("jdk")
        .status()
        .unwrap();
    assert!(status.success());

    let out = dir.join("out");
    extract(&archive, &out).unwrap();
    let link = out.join("jdk/legal/java.se/LICENSE");
    assert!(link.is_symlink());
    assert_eq!(std::fs::read_to_string(&link).unwrap(), "license");
}

#[test]
fn refuses_a_symlink_pointing_outside_the_destination() {
    let dir = scratch("evil-symlink");
    let tree = dir.join("jdk");
    std::fs::create_dir_all(&tree).unwrap();
    std::os::unix::fs::symlink("../../../../etc/passwd", tree.join("escape")).unwrap();

    let archive = dir.join("jdk.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .arg("jdk")
        .status()
        .unwrap();
    assert!(status.success());

    let err = extract(&archive, &dir.join("out")).unwrap_err();
    assert!(
        matches!(err, java_path::Error::UnsafeArchiveEntry(_)),
        "{err}"
    );
}
