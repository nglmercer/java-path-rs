# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing has been published to crates.io yet. Everything below describes the
state of `main`.

### Added

* Discovery across `JAVA_HOME`, `PATH`, Linux/macOS/Windows system locations,
  the Windows registry, SDKMAN!, mise, asdf, Gradle, JBang and Termux.
* Inspection with a strict metadata priority: `release` file, then JVM system
  properties, then `java -version`, then directory-name heuristics.
* `JavaQuery`/`Selector` constraint-based selection with deterministic ranking.
* `JdkProvider` trait and an Adoptium implementation, behind `network`.
* `JavaInstaller`: download, SHA-256 verification, safe extraction and
  validated installation, behind `install`.
* `VersionSpec::{Exact, LatestLts, Latest}` and `ReleaseType`.
* `Platform::AlpineLinux` for musl targets.
* `Java::preferred()`, the `JAVA_HOME`-first answer that `Java::current()`
  used to give.

### Fixed

* `JavaVersion` had inconsistent `Eq`/`Hash` versus `Ord`, and discarded the
  fourth and later version components, so `17.0.10.1` compared equal to
  `17.0.10`.
* The Adoptium `vendor` query parameter needs the foundation name `eclipse`;
  `temurin` returned HTTP 404 for every request.
* Archive extraction rejected the relative symlinks that real JDK archives
  contain, so no genuine Temurin archive could be unpacked.
* Provider-supplied file names could escape the installation directory.
* Installations were moved into place before being validated.
* Releases without a checksum were installed unverified.
* A malformed `release` file silently fell back to directory-name guessing.
* Failed `java` invocations were parsed instead of being treated as errors.
* `JavaVersion`'s derived `Deserialize` could build component vectors the
  parser never produces, reintroducing the `Eq`/`Hash` mismatch. It is now
  serialised as its version string and read back through `JavaVersion::parse`.
* The installer trusted the release a provider resolved; a Java 17 build could
  satisfy a request for Java 21. Releases are now validated against the
  request before download.
* A failed cross-device move fell back to a recursive copy that could leave a
  partial JDK at the final location. The commit is now a rename or nothing.
* Concurrent installs of the same build could race on the final target; the
  commit now takes a per-target lock.

### Changed

* `Java::current()` now resolves `PATH` the way a shell does — the first
  `java` on `PATH`, not the newest — matching what its documentation claimed.
  Use `Java::preferred()` for the previous `JAVA_HOME`-first, newest-version
  behaviour.

[Unreleased]: https://github.com/nglmercer/java-path-rs/commits/main
