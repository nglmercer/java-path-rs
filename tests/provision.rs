#![cfg(feature = "install")]

use java_path::provision::archive::{extract, find_extracted_home, safe_join, safe_link_target};
use java_path::provision::checksum::{sha256_file, verify_sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("java-path-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// One entry in a synthetic archive.
enum Entry<'a> {
    File(&'a str, &'a [u8]),
    /// `(path, target)` — a symlink, written only where the format supports it.
    Symlink(&'a str, &'a str),
    /// A file whose name is written straight into the header, bypassing the
    /// `tar` crate's own validation. Needed to forge the hostile archives a
    /// real attacker would produce, which the crate refuses to build normally.
    RawFile(&'a str, &'a [u8]),
}

/// Build a `.tar.gz` in-process. Shelling out to `tar` is not portable:
/// Windows has no `tar` with these flags and macOS ships BSD tar, which has
/// no `--transform`.
fn write_tar_gz(path: &Path, entries: &[Entry]) {
    let file = std::fs::File::create(path).unwrap();
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);

    for entry in entries {
        match entry {
            Entry::File(name, data) => {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, name, *data).unwrap();
            }
            Entry::Symlink(name, target) => {
                let mut header = tar::Header::new_gnu();
                header.set_size(0);
                header.set_mode(0o777);
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_cksum();
                builder.append_link(&mut header, name, target).unwrap();
            }
            Entry::RawFile(name, data) => {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_entry_type(tar::EntryType::Regular);
                {
                    // Write the path bytes directly: set_path() rejects `..`.
                    let old = header.as_old_mut();
                    let bytes = name.as_bytes();
                    old.name[..bytes.len()].copy_from_slice(bytes);
                }
                header.set_cksum();
                builder.append(&header, *data).unwrap();
            }
        }
    }
    builder.into_inner().unwrap().finish().unwrap();
}

/// Build a `.zip`, the format Windows JDKs ship in.
fn write_zip(path: &Path, entries: &[Entry]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().unix_permissions(0o755);

    for entry in entries {
        match entry {
            Entry::File(name, data) => {
                zip.start_file(*name, options).unwrap();
                zip.write_all(data).unwrap();
            }
            Entry::RawFile(name, data) => {
                // The zip crate permits the name verbatim.
                zip.start_file(*name, options).unwrap();
                zip.write_all(data).unwrap();
            }
            // Zip archives from the JDK vendors carry no symlinks.
            Entry::Symlink(..) => {}
        }
    }
    zip.finish().unwrap();
}

/// The minimum layout `resolve_layout` accepts as a Java home.
fn jdk_entries(prefix: &str, windows: bool) -> Vec<Entry<'static>> {
    let java: &'static str = Box::leak(
        format!("{prefix}/bin/{}", if windows { "java.exe" } else { "java" }).into_boxed_str(),
    );
    let javac: &'static str = Box::leak(
        format!(
            "{prefix}/bin/{}",
            if windows { "javac.exe" } else { "javac" }
        )
        .into_boxed_str(),
    );
    let release: &'static str = Box::leak(format!("{prefix}/release").into_boxed_str());
    vec![
        Entry::File(java, b"#!/bin/sh\n"),
        Entry::File(javac, b"#!/bin/sh\n"),
        Entry::File(
            release,
            b"JAVA_VERSION=\"21.0.3\"\nOS_ARCH=\"x86_64\"\nOS_NAME=\"Linux\"\nIMPLEMENTOR=\"Eclipse Adoptium\"\n",
        ),
    ]
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
    let archive = dir.join("jdk.tar.gz");
    write_tar_gz(&archive, &jdk_entries("jdk-21.0.3", false));

    let out = dir.join("out");
    extract(&archive, &out).unwrap();
    let home = find_extracted_home(&out).unwrap();

    let install = java_path::inspect_java_home(&home).unwrap();
    assert_eq!(install.version.major(), 21);
    assert!(install.is_jdk());
}

/// Windows JDKs ship as `.zip`, so this path needs real coverage rather than
/// relying on a CI leg that may never run.
#[test]
fn extracts_a_zip_and_finds_the_java_home() {
    let dir = scratch("zip");
    let archive = dir.join("jdk.zip");
    write_zip(&archive, &jdk_entries("jdk-21.0.3", cfg!(windows)));

    let out = dir.join("out");
    extract(&archive, &out).unwrap();
    let home = find_extracted_home(&out).unwrap();

    let install = java_path::inspect_java_home(&home).unwrap();
    assert_eq!(install.version.major(), 21);
    assert_eq!(install.vendor.as_deref(), Some("Eclipse Adoptium"));
}

#[test]
fn refuses_a_zip_entry_that_escapes_the_destination() {
    let dir = scratch("zip-traversal");
    let archive = dir.join("evil.zip");
    write_zip(&archive, &[Entry::RawFile("../evil", b"pwned")]);

    let out = dir.join("out");
    let err = extract(&archive, &out).unwrap_err();
    assert!(
        matches!(err, java_path::Error::UnsafeArchiveEntry(_)),
        "{err}"
    );
    assert!(!dir.join("evil").exists());
}

#[test]
fn refuses_a_tar_entry_that_escapes_the_destination() {
    let dir = scratch("traversal");
    let archive = dir.join("evil.tar.gz");
    write_tar_gz(&archive, &[Entry::RawFile("../evil", b"pwned")]);

    let err = extract(&archive, &dir.join("out")).unwrap_err();
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
    let archive = dir.join("jdk.tar.gz");
    write_tar_gz(
        &archive,
        &[
            Entry::File("jdk/legal/java.base/LICENSE", b"license"),
            Entry::Symlink("jdk/legal/java.se/LICENSE", "../java.base/LICENSE"),
        ],
    );

    let out = dir.join("out");
    extract(&archive, &out).unwrap();
    let link = out.join("jdk/legal/java.se/LICENSE");

    // Windows needs a privilege to create symlinks, so only assert the link
    // nature where it is guaranteed; the content check holds everywhere.
    #[cfg(unix)]
    assert!(link.is_symlink());
    assert_eq!(std::fs::read_to_string(&link).unwrap(), "license");
}

#[test]
fn refuses_a_symlink_pointing_outside_the_destination() {
    let dir = scratch("evil-symlink");
    let archive = dir.join("jdk.tar.gz");
    write_tar_gz(
        &archive,
        &[Entry::Symlink("jdk/escape", "../../../../etc/passwd")],
    );

    let err = extract(&archive, &dir.join("out")).unwrap_err();
    assert!(
        matches!(err, java_path::Error::UnsafeArchiveEntry(_)),
        "{err}"
    );
}

// --- installer trust boundary -------------------------------------------------

use java_path::provision::provider::{JdkProvider, JdkRelease, ReleaseRequest, Vendor};
use java_path::{Architecture, JavaInstaller, JavaKind, Platform};

fn release(file_name: &str, release_name: &str, sha256: Option<&str>) -> JdkRelease {
    JdkRelease {
        vendor: Vendor::Temurin,
        release_name: release_name.to_string(),
        version: "21.0.3+9".to_string(),
        major: 21,
        url: "http://127.0.0.1:1/x".to_string(),
        file_name: file_name.to_string(),
        sha256: sha256.map(|s| s.to_string()),
        size: None,
        platform: Platform::current(),
        architecture: Architecture::current(),
        kind: JavaKind::Jdk,
        lts: true,
    }
}

/// A provider that hands back exactly what the test wants, including hostile
/// values a real provider should never send.
struct FakeProvider(JdkRelease);

impl JdkProvider for FakeProvider {
    async fn releases(&self, _request: ReleaseRequest) -> java_path::Result<Vec<JdkRelease>> {
        Ok(vec![self.0.clone()])
    }
}

#[test]
fn target_dir_name_distinguishes_builds_that_differ_only_by_target() {
    let mut a = release("x.tar.gz", "jdk-21.0.3+9", None);
    let name_a = java_path::target_dir_name(&a);
    assert!(name_a.starts_with("temurin-21.0.3"), "{name_a}");

    // Same release name, different image type and architecture: must not collide.
    a.kind = JavaKind::Jre;
    assert_ne!(name_a, java_path::target_dir_name(&a));

    let mut b = release("x.tar.gz", "jdk-21.0.3+9", None);
    b.architecture = Architecture::Aarch64;
    assert_ne!(name_a, java_path::target_dir_name(&b));

    let mut c = release("x.tar.gz", "jdk-21.0.3+9", None);
    c.platform = Platform::Windows;
    assert_ne!(name_a, java_path::target_dir_name(&c));
}

#[tokio::test]
async fn refuses_provider_filenames_that_escape_the_install_directory() {
    let dir = scratch("hostile-name");
    for (file_name, release_name) in [
        ("../../evil.tar.gz", "jdk-21.0.3+9"),
        ("jdk.tar.gz", "../../evil"),
        ("/etc/passwd", "jdk-21.0.3+9"),
        ("jdk.tar.gz", "a/b"),
    ] {
        let err = JavaInstaller::new(FakeProvider(release(
            file_name,
            release_name,
            Some(&"a".repeat(64)),
        )))
        .install_dir(&dir)
        .install()
        .await
        .unwrap_err();
        assert!(
            matches!(err, java_path::Error::UnsafeProviderValue { .. }),
            "{file_name:?}/{release_name:?} produced {err}"
        );
    }
}

#[tokio::test]
async fn refuses_a_release_without_a_checksum() {
    let dir = scratch("no-checksum");
    let err = JavaInstaller::new(FakeProvider(release("jdk.tar.gz", "jdk-21.0.3+9", None)))
        .install_dir(&dir)
        .install()
        .await
        .unwrap_err();
    assert!(matches!(err, java_path::Error::MissingChecksum(_)), "{err}");

    // The escape hatch gets past the checksum gate (and fails later, on the
    // unreachable download, which is what proves it got past it).
    let err = JavaInstaller::new(FakeProvider(release("jdk.tar.gz", "jdk-21.0.3+9", None)))
        .install_dir(&dir)
        .allow_unverified(true)
        .install()
        .await
        .unwrap_err();
    assert!(matches!(err, java_path::Error::Network(_)), "{err}");
}

#[tokio::test]
async fn refuses_to_install_for_another_operating_system() {
    let dir = scratch("cross-os");
    let other = if Platform::current() == Platform::Windows {
        Platform::Linux
    } else {
        Platform::Windows
    };

    let err = JavaInstaller::new(FakeProvider(release("jdk.zip", "jdk-21.0.3+9", Some("x"))))
        .install_dir(&dir)
        .platform(other)
        .install()
        .await
        .unwrap_err();
    assert!(
        matches!(err, java_path::Error::UnsupportedTarget(_)),
        "{err}"
    );

    let err = JavaInstaller::new(FakeProvider(release(
        "jdk.tar.gz",
        "jdk-21.0.3+9",
        Some("x"),
    )))
    .install_dir(&dir)
    .platform(Platform::Termux)
    .install()
    .await
    .unwrap_err();
    assert!(
        matches!(err, java_path::Error::UnsupportedTarget(_)),
        "{err}"
    );
}

#[tokio::test]
async fn does_not_reuse_a_directory_whose_contents_do_not_match() {
    let dir = scratch("stale-target");
    let rel = release("jdk.tar.gz", "jdk-21.0.3+9", Some(&"a".repeat(64)));

    // Plant a Java 17 JRE where the Java 21 JDK install would land.
    let target = dir.join(java_path::target_dir_name(&rel));
    std::fs::create_dir_all(target.join("bin")).unwrap();
    std::fs::write(target.join("bin/java"), b"#!/bin/sh\n").unwrap();
    std::fs::write(
        target.join("release"),
        b"JAVA_VERSION=\"17.0.10\"\nOS_ARCH=\"x86_64\"\nOS_NAME=\"Linux\"\n",
    )
    .unwrap();

    // It must not be accepted as an already-satisfied install: the run should
    // proceed to the download instead of returning the stale directory.
    let err = JavaInstaller::new(FakeProvider(rel))
        .install_dir(&dir)
        .install()
        .await
        .unwrap_err();
    assert!(
        matches!(err, java_path::Error::Network(_)),
        "reused a mismatched install: {err}"
    );
}

#[test]
fn target_dir_name_is_clean_for_adoptium_semver() {
    let mut rel = release("x.tar.gz", "jdk-25.0.4+7", None);
    // What the Adoptium API actually returns in version_data.semver.
    rel.version = "25.0.4+7.0.LTS".to_string();
    rel.major = 25;
    rel.kind = JavaKind::Jre;
    rel.platform = Platform::Linux;
    rel.architecture = Architecture::X86_64;
    assert_eq!(
        java_path::target_dir_name(&rel),
        "temurin-25.0.4-linux-x64-jre"
    );

    rel.version = "26-ea+15".to_string();
    assert_eq!(
        java_path::target_dir_name(&rel),
        "temurin-26-ea-linux-x64-jre"
    );
}
