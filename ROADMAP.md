# Roadmap

Checkboxes are ticked only when the acceptance criteria pass
(`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test --all-features`).

## Phase 0 — Specification ✅

- [x] Supported OSes: Windows, Linux, macOS
- [x] Experimental platform: Termux/Android
- [x] Supported architectures
- [x] Minimum Rust version (1.75)
- [x] Define `JavaVersion`
- [x] Define `JavaInstallation`
- [x] JDK vs JRE semantics (presence of `bin/javac`)
- [x] Discovery source priority
- [x] Duplicate-resolution rules
- [x] Selection/ranking rules
- [x] `ARCHITECTURE.md`
- [x] `AGENTS.md`
- [x] Fixture directory layouts

## Phase 1 — Core models ✅ (v0.1.0)

- [x] `JavaVersion` with `1.8.0_x`, `11.0.x`, `17.0.x`, `21`, EA parsing
- [x] `Architecture`, `Platform`, `JavaKind`, `DiscoverySource`
- [x] `JavaInstallation`, `JavaMetadata`
- [x] Structured error enum
- [x] Tests: Java 8 / 11+ / 21+ / EA / bad strings / unknown architectures

## Phase 2 — Installation inspection ✅ (v0.1.0)

- [x] Identify candidate `JAVA_HOME` (incl. macOS `Contents/Home`)
- [x] Find `bin/java` and `bin/javac`
- [x] Parse the JDK `release` file
- [x] Determine JDK vs JRE
- [x] Extract version, vendor, architecture
- [x] Optional subprocess fallback (off by default)
- [x] Canonicalise paths, handle symlinks safely
- [x] `inspect_java_home` / `inspect_java_home_with`

## Phase 3 — Basic discovery ✅ (v0.2.0)

- [x] `JAVA_HOME`
- [x] `java` on `PATH`, with symlink resolution
- [x] Duplicate removal and deterministic ordering
- [x] Caller-supplied roots
- [x] No recursive filesystem scanning

## Phase 4 — Native platform discovery ✅ (v0.2.0)

- [x] Linux system roots
- [x] SDKMAN!, mise/asdf, Gradle, JBang stores
- [x] macOS bundle directories and `/usr/libexec/java_home`
- [x] Windows Program Files vendor directories
- [x] Windows registry via `reg.exe` (Everything SDK deliberately not required)

## Phase 5 — Selection engine ✅ (v0.3.0)

- [x] Exact major, min major, max major, range
- [x] Minimum full version
- [x] JDK/JRE, vendor, architecture, platform
- [x] Prefer `JAVA_HOME`, prefer newest patch
- [x] Deterministic scoring

## Phase 6 — Adoptium provider ✅ (v0.4.0)

- [x] `JdkProvider` trait
- [x] Available releases, latest GA, latest LTS, LTS data (`VersionSpec`)
- [x] OS and architecture mapping
- [x] JDK/JRE image selection, HotSpot
- [x] Release metadata, artifact size, checksum

## Phase 7 — Secure download ✅ (v0.5.0)

- [x] Streaming download to a `.partial` file
- [x] SHA-256 verification before commit
- [x] Progress callbacks
- [x] Failed downloads cleaned up; bad artifacts deleted
- [x] Atomic rename only after verification

## Phase 8 — Secure extraction/install ✅ (v0.6.0)

- [x] `.zip` and `.tar.gz`
- [x] Relative symlinks inside the root preserved (real JDK archives rely on them)
- [x] Traversal protection for both, including symlink targets
- [x] Extract to a staging directory, inspect, validate
- [x] Atomic move into the final location, cleanup on failure
- [x] Idempotent installs

## Phase 9 — Async + progress ✅ (v0.6.0)

- [x] Discovery stays synchronous
- [x] Networking is async
- [x] `InstallEvent` progress reporting

## Phase 10 — Termux ✅ (v0.7.0)

- [x] Detect the Termux environment
- [x] Detect `pkg`
- [x] Discover Termux OpenJDK installs
- [x] Refuse Adoptium provisioning on Termux
- [ ] Integration test on real Android/Termux CI

## Phase 11 — CLI (optional, v0.8.0)

- [ ] Split into a `java-path` / `java-path-cli` workspace
- [ ] `list`, `home [major]`, `inspect <path>`, `install <major>`, `doctor`
- [ ] Stable `--json` output for agent consumption

## Phase 12 — Quality / release

- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test --all-features`
- [x] `cargo doc --no-deps`
- [x] CI matrix definition (ubuntu / windows / macOS)
- [ ] `cargo deny check` wired into CI
- [ ] Verified against real Java 8 / 11 / 17 / 21 / 25 installs on every CI target

## Milestones

| Version | Contents | Status |
| ------- | -------- | ------ |
| v0.1.0 | Core models + inspection | done |
| v0.2.0 | Cross-platform discovery | done |
| v0.3.0 | Selection/query API | done |
| v0.4.0 | Adoptium release API | done |
| v0.5.0 | Download + verification | done |
| v0.6.0 | Installation/extraction | done |
| v0.7.0 | Termux + extra discovery sources | done |
| v0.8.0 | CLI / JSON agent interface | not started |
| v0.9.x | API stabilisation + ecosystem testing | not started |
| v1.0.0 | Stable discovery/provisioning API | blocked on real-world use |

Nothing is tagged `1.0` until the selection/model API has been used by real projects.
