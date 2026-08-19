Yes. After inspecting `nglmercer/java-path` and the current Rust ecosystem, I **would build a Rust version—but I would not do a literal 1:1 port**.

Your TypeScript package is really a Java/JDK management library: discovery, platform detection, local installation scanning, Adoptium release lookup/download, checksum validation, extraction, task progress, Termux support, plus generic filesystem/command helpers.  The published package is currently `1.1.3`.

## Does something similar already exist?

The closest match is **`java-manager`**. Its current repository declares version `0.3.0`; it discovers Java through `PATH`, `JAVA_HOME`, system scans, Windows registry/Everything, extracts version/vendor/architecture metadata, and can launch Java applications.

There is also **`java-locator`**, but it is much narrower: find the active Java home, `libjvm`, or another file under the Java installation. ([Docs.rs][1])

And **`jbx`** already has fairly serious JDK management from a CLI perspective: it discovers JDKs from `JAVA_HOME`, PATH, SDKMAN, mise, Gradle, JBang and system directories; it can install Temurin from Adoptium and verifies SHA-256. ([Docs.rs][2])

So the landscape is approximately:

| Capability                | java-locator | java-manager |               jbx | Your Rust library |
| ------------------------- | -----------: | -----------: | ----------------: | ----------------: |
| Find active Java          |            ✅ |            ✅ |                 ✅ |                 ✅ |
| Find many JDKs            |      limited |            ✅ |                 ✅ |                 ✅ |
| Version/vendor/arch       |      limited |            ✅ |                 ✅ |                 ✅ |
| Cross-platform discovery  |            ✅ |            ✅ |                 ✅ |                 ✅ |
| Download JDK              |            ❌ |            ❌ |                 ✅ |                 ✅ |
| Adoptium API as library   |            ❌ |            ❌ |          internal |             **✅** |
| Verify downloads          |            ❌ |            ❌ |                 ✅ |                 ✅ |
| Install/extract JDK       |            ❌ |            ❌ |                 ✅ |                 ✅ |
| Library-first API         |            ✅ |            ✅ | not its main goal |             **✅** |
| Termux/Android            |            ❌ |            ❌ |       not a focus |             **✅** |
| Select JDK by constraints |        basic |         some |      CLI-oriented |             **✅** |
| Pluggable vendors         |            ❌ |            ❌ |    mostly Temurin |    **eventually** |

That last column is where I think your project can have a reason to exist.

# Recommended project direction

I would position it as:

> **A Rust library for discovering, inspecting, selecting, downloading, and provisioning JVM/JDK installations across platforms.**

Not:

> A Rust rewrite of every utility in `java-path`.

For example, don't migrate generic `FileUtils`, `FolderUtils`, shell wrappers, backup helpers, etc. Rust's ecosystem already handles those things well. Your `index.ts` currently exposes essentially everything from platform, service, command, file, folder, and validation modules.  That makes the JS package convenient but would make a Rust crate unnecessarily broad.

---

# Proposed architecture

Start with **one crate**, not a complicated workspace:

```text
java-path-rs/
├── Cargo.toml
├── README.md
├── ROADMAP.md
├── CONTRIBUTING.md
├── AGENTS.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── model.rs
│   ├── version.rs
│   │
│   ├── inspect/
│   │   ├── mod.rs
│   │   ├── release_file.rs
│   │   └── command.rs
│   │
│   ├── discovery/
│   │   ├── mod.rs
│   │   ├── env.rs
│   │   ├── path.rs
│   │   ├── common_dirs.rs
│   │   ├── macos.rs
│   │   ├── linux.rs
│   │   ├── windows.rs
│   │   └── termux.rs
│   │
│   ├── selector.rs
│   │
│   └── provision/
│       ├── mod.rs
│       ├── provider.rs
│       ├── adoptium.rs
│       ├── download.rs
│       ├── checksum.rs
│       └── archive.rs
│
├── tests/
│   ├── fixtures/
│   ├── discovery.rs
│   ├── metadata.rs
│   ├── selector.rs
│   └── adoptium.rs
│
└── examples/
    ├── discover.rs
    ├── select_java_21.rs
    └── install_temurin.rs
```

Later, when it proves useful:

```text
workspace/
├── java-path/          # library
└── java-path-cli/      # optional CLI
```

Don't start with multiple crates.

---

# Core Rust model

The centerpiece should be something along these lines:

```rust
pub struct JavaInstallation {
    pub home: PathBuf,
    pub java: PathBuf,
    pub javac: Option<PathBuf>,

    pub version: JavaVersion,
    pub vendor: Option<String>,
    pub architecture: Architecture,
    pub platform: Platform,

    pub kind: JavaKind,
    pub source: DiscoverySource,
}

pub enum JavaKind {
    Jdk,
    Jre,
}

pub enum DiscoverySource {
    JavaHome,
    Path,
    System,
    Registry,
    Sdkman,
    Mise,
    Gradle,
    Jbang,
    UserDirectory,
    Termux,
}
```

Then selection becomes clean:

```rust
let java = JavaFinder::new()
    .major_version(21)
    .require_jdk(true)
    .current_architecture(true)
    .find_best()?;
```

And installation eventually:

```rust
let java = JavaInstaller::new()
    .version(21)
    .vendor(Vendor::Temurin)
    .install()
    .await?;
```

That API is a stronger reason for the crate to exist than merely reproducing Node functions.

---

# Important change from the TypeScript version

Don't infer metadata primarily from directory names.

The existing scanner has patterns such as:

```text
jdk-21.0.3+9
8_x86_64_windows
java-11-openjdk
openjdk-17
```

and uses names to infer version/architecture/OS.

That works as a fallback but should not be authoritative.

For Rust, use this priority:

```text
1. <JAVA_HOME>/release file
2. java -XshowSettings:properties -version
3. java -version
4. directory-name heuristics only as last fallback
```

The JDK `release` file gives you metadata without launching arbitrary executables, which is especially useful during filesystem scans.

---

# LLM implementation roadmap

I would give coding agents the following roadmap.

## Phase 0 — Specification

**Goal:** freeze scope before generating code.

TODO:

* [ ] Define supported OSes: Windows, Linux, macOS.
* [ ] Define experimental platform: Termux/Android.
* [ ] Define supported architectures.
* [ ] Decide minimum Rust version.
* [ ] Define `JavaVersion`.
* [ ] Define `JavaInstallation`.
* [ ] Define JDK vs JRE semantics.
* [ ] Define discovery source priority.
* [ ] Define duplicate-resolution rules.
* [ ] Define selection/ranking rules.
* [ ] Create `ARCHITECTURE.md`.
* [ ] Create `AGENTS.md`.
* [ ] Create fixture directory layouts.

**Exit criteria:**

```text
No networking.
No downloading.
No CLI.
All public core types agreed upon.
```

---

# Phase 1 — Core models

Implement:

```text
src/error.rs
src/model.rs
src/version.rs
```

TODO:

* [ ] `JavaVersion`
* [ ] parse `1.8.0_...`
* [ ] parse `11.0.x`
* [ ] parse `17.0.x`
* [ ] parse `21`
* [ ] early-access versions
* [ ] `Architecture`
* [ ] `Platform`
* [ ] `JavaKind`
* [ ] `JavaInstallation`
* [ ] `DiscoverySource`
* [ ] structured error enum

Tests:

```text
Java 8 legacy versions
Java 11+
Java 21+
EA builds
bad version strings
unknown architectures
```

**LLM rule:** no discovery logic until version parsing has >90% meaningful branch coverage.

---

# Phase 2 — Installation inspection

Create:

```text
inspect/release_file.rs
inspect/command.rs
```

TODO:

* [ ] identify candidate JAVA_HOME
* [ ] find `bin/java`
* [ ] find `bin/javac`
* [ ] parse JDK `release`
* [ ] determine JDK vs JRE
* [ ] extract version
* [ ] extract vendor
* [ ] extract architecture
* [ ] optional subprocess fallback
* [ ] canonicalize paths
* [ ] handle symlinks safely

API:

```rust
pub fn inspect_java_home(path: impl AsRef<Path>)
    -> Result<JavaInstallation>;
```

Do this **before discovery**.

---

# Phase 3 — Basic discovery

Sources:

```text
JAVA_HOME
PATH
explicit user directories
```

TODO:

* [ ] `JAVA_HOME`
* [ ] executable search through PATH
* [ ] symlink resolution
* [ ] duplicate removal
* [ ] deterministic ordering
* [ ] custom roots

API:

```rust
let installs = discover()?;

let installs = Discovery::new()
    .root("/custom/jdks")
    .search()?;
```

No recursive full-disk scanning yet.

---

# Phase 4 — Native platform discovery

### Linux

Search things such as:

```text
/usr/lib/jvm
/usr/java
/usr/local/java
/opt/java
```

plus integrations:

```text
~/.sdkman/candidates/java
~/.local/share/mise/installs/java
~/.gradle/jdks
~/.jbang/jdks
```

`jbx` already demonstrates the usefulness of checking SDKMAN, mise, Gradle and JBang stores. ([Docs.rs][2])

### macOS

Support:

```text
/Library/Java/JavaVirtualMachines
~/Library/Java/JavaVirtualMachines
```

and macOS's:

```text
/usr/libexec/java_home
```

### Windows

Support:

```text
JAVA_HOME
PATH
Program Files/Java
Program Files/Eclipse Adoptium
Microsoft JDK directories
Windows Registry
```

Do **not** make Everything SDK mandatory. `java-manager` currently exposes an Everything-based deep search on Windows, which is useful but is also an external dependency.

---

# Phase 5 — Selection engine

This is one of the features I'd emphasize.

```rust
JavaQuery::new()
    .version(21)
    .jdk_only()
    .vendor("Eclipse Adoptium")
    .current_arch()
```

TODO:

* [ ] major version
* [ ] minimum version
* [ ] version range
* [ ] JDK/JRE
* [ ] vendor
* [ ] architecture
* [ ] platform
* [ ] valid-only
* [ ] prefer `JAVA_HOME`
* [ ] prefer newest patch
* [ ] deterministic scoring

Example:

```rust
let java = installations
    .select()
    .major(21)
    .jdk()
    .best()?;
```

This is a good differentiator from simple locator crates.

---

# Phase 6 — Adoptium provider

Your current TS code calls the Adoptium API to enumerate releases and retrieve binaries.  Preserve that idea, but isolate it behind a provider abstraction.

```rust
pub trait JdkProvider {
    async fn releases(
        &self,
        request: ReleaseRequest,
    ) -> Result<Vec<JdkRelease>>;
}
```

Then:

```rust
pub struct AdoptiumProvider;
```

TODO:

* [ ] available releases
* [ ] latest GA
* [ ] LTS data
* [ ] OS mapping
* [ ] architecture mapping
* [ ] JDK/JRE image selection
* [ ] HotSpot selection
* [ ] release metadata
* [ ] artifact size
* [ ] checksum

This makes future providers possible:

```text
Temurin
Microsoft
Amazon Corretto
Azul Zulu
Oracle
GraalVM
```

without rewriting the public API.

---

# Phase 7 — Secure download

Do not mix discovery and downloading.

TODO:

* [ ] streaming download
* [ ] temp file
* [ ] maximum/error handling
* [ ] SHA-256 verification
* [ ] progress callbacks
* [ ] cancellation
* [ ] cleanup failed downloads
* [ ] rename only after verification

Conceptually:

```rust
download -> .partial
         -> hash
         -> verify
         -> atomic rename
```

Your TypeScript implementation already deletes the downloaded artifact when integrity verification fails; keep that behavior.

---

# Phase 8 — Secure extraction/install

Support:

```text
.zip
.tar.gz
```

TODO:

* [ ] zip traversal protection
* [ ] tar traversal protection
* [ ] symlink safety
* [ ] extract to temporary directory
* [ ] inspect extracted JDK
* [ ] validate Java home
* [ ] atomic move to final installation directory
* [ ] cleanup on failure
* [ ] idempotent installs

Never extract untrusted archive paths directly.

Installation flow:

```text
resolve release
      ↓
download
      ↓
verify SHA-256
      ↓
extract temporary
      ↓
inspect JDK
      ↓
validate
      ↓
atomic install
```

---

# Phase 9 — Async + progress

Only add async where it actually helps.

Core discovery can remain synchronous:

```rust
discover() -> Result<Vec<JavaInstallation>>
```

Networking:

```rust
provider.releases().await
installer.install().await
```

Progress:

```rust
pub enum InstallEvent {
    Resolving,
    Downloading { downloaded: u64, total: u64 },
    Verifying,
    Extracting,
    Installed { path: PathBuf },
}
```

This is cleaner than recreating a generic task manager inside the library.

---

# Phase 10 — Termux

Your existing library explicitly identifies Termux and uses its package manager model.

Keep it, but behind:

```rust
#[cfg(feature = "termux")]
```

TODO:

* [ ] detect Termux environment
* [ ] detect `pkg`
* [ ] find OpenJDK packages
* [ ] inspect Termux Java
* [ ] don't assume normal Adoptium binaries work
* [ ] integration test on Android/Termux CI if feasible

Termux can become one of your differentiators.

---

# Phase 11 — CLI, optional

Only after the library is stable.

```bash
java-path list
java-path home
java-path home 21
java-path inspect /some/jdk
java-path install 21
java-path install 17 --vendor temurin
java-path doctor
```

And agent-friendly:

```bash
java-path list --json
java-path install 21 --json
```

For LLM usage, stable JSON is far more useful than pretty terminal tables.

---

# Phase 12 — Quality / release

Required CI matrix:

```text
ubuntu x86_64
ubuntu aarch64 if practical
windows x86_64
macOS x86_64
macOS arm64
```

Test with:

```text
Java 8
Java 11
Java 17
Java 21
Java 25+
```

And:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
cargo deny check
```

---

# AGENTS.md rules for LLMs

This part matters a lot if Claude/Codex/ChatGPT will be building it.

I would put rules like these in `AGENTS.md`:

```text
1. Read ARCHITECTURE.md and ROADMAP.md before editing.

2. Work on exactly one roadmap task at a time.

3. Do not change public API unless the task explicitly requires it.

4. Every new behavior requires tests.

5. Do not use unwrap(), expect(), panic!(), or todo!()
   in production library code.

6. Do not construct shell commands through string concatenation.
   Use std::process::Command with separate arguments.

7. Never infer Java metadata from directory names unless no
   authoritative metadata source exists.

8. Prefer:
   JDK release file
   > JVM properties
   > java -version
   > path heuristics.

9. Platform-specific behavior must live behind an abstraction.

10. No full filesystem scans in the default discovery path.

11. Network functionality must be optional.

12. Downloads must be checksum verified before installation.

13. Archive extraction must prevent path traversal.

14. New dependencies require justification.

15. cargo fmt, cargo clippy, and cargo test must pass before
    declaring a task complete.

16. Update ROADMAP.md checkboxes only after acceptance criteria pass.

17. Never silently ignore errors in library code.
```

That will significantly reduce LLM-generated architectural drift.

---

# Issue format for each LLM task

Give agents issues like:

```text
PHASE 2 / TASK 2.3 — Parse JDK release file

Objective:
Parse metadata from <JAVA_HOME>/release.

Input:
Path to candidate Java home.

Output:
JavaMetadata containing version, vendor, architecture.

Requirements:
- Support quoted KEY="VALUE".
- Ignore unknown keys.
- Missing optional fields are allowed.
- Invalid JAVA_VERSION returns typed error.
- No subprocess execution.

Tests:
- Temurin 8 fixture
- Temurin 17 fixture
- Oracle 21 fixture
- malformed fixture

Out of scope:
- PATH discovery
- java -version
- networking
- downloads

Done when:
cargo test release_file
cargo clippy -- -D warnings
passes.
```

LLMs perform dramatically better with tasks this small than with:

> "Port java-path to Rust."

---

# Suggested milestones

I would map releases like this:

```text
v0.1.0
Core models + inspect existing JDK

v0.2.0
Cross-platform discovery

v0.3.0
JDK selection/query API

v0.4.0
Adoptium release API

v0.5.0
Download + verification

v0.6.0
Installation/extraction

v0.7.0
Termux + extra discovery sources

v0.8.0
CLI / JSON agent interface

v0.9.x
API stabilization + ecosystem testing

v1.0.0
Stable discovery/provisioning API
```

Don't call anything `1.0` until the selection/model API has been used by real projects.

## What I would **not** migrate

From the existing project:

```text
❌ generic FileUtils
❌ generic FolderUtils
❌ generic backup service
❌ generic task manager
❌ generic command runner
❌ generic response wrapper
```

Those aren't the unique value of the library.

I would migrate/reimplement:

```text
✅ environment detection
✅ Java/JDK inspection
✅ Java installation discovery
✅ Java selection
✅ architecture matching
✅ platform-specific discovery
✅ Adoptium integration
✅ JDK download
✅ checksum verification
✅ extraction/install
✅ Termux support
```

That gives you a much tighter Rust crate.

## My recommendation

I would **not compete with `java-manager` purely on discovery**. Its `0.3.0` already covers a lot of that area.

Instead, make the project:

**`java-manager`-style discovery + a polished library-first Adoptium provisioning API + robust selection + Termux.**

Something like:

```rust
let java = Java::discover()
    .select()
    .major(21)
    .jdk()
    .or_install(Adoptium::default())
    .await?;
```

That single workflow would clearly communicate why the crate exists.

Also, your current repo is MIT-licensed, so the existing implementation can serve as a migration/reference source, though I'd treat the Rust implementation as a redesign rather than line-by-line translation.

If you want to run this largely through coding agents, the best next step would be to create **`ROADMAP.md`, `ARCHITECTURE.md`, `AGENTS.md`, and the first ~20 implementation tasks/issues** from the plan above; that would give Codex/Claude a concrete queue instead of one giant migration prompt.

[1]: https://docs.rs/java-locator/latest/java_locator/?utm_source=chatgpt.com "java_locator - Rust"
[2]: https://docs.rs/crate/jbx/latest "jbx 0.6.3 - Docs.rs"
