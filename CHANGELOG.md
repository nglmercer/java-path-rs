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

[Unreleased]: https://github.com/nglmercer/java-path-rs/commits/main
