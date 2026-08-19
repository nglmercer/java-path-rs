# Architecture

`java-path` is a single crate with a strict layering: nothing lower in the list may
depend on anything above it.

```
version  →  model  →  inspect  →  discovery  →  selector
                          ↓
                      provision  (network / install features only)
```

## Scope

**In scope:** environment detection, JDK inspection, installation discovery, selection,
architecture matching, platform-specific discovery, the Adoptium provider, JDK download,
checksum verification, extraction/install, Termux support.

**Out of scope:** generic filesystem helpers, generic shell/command runners, generic
backup helpers, generic task managers, generic response wrappers. Rust's ecosystem
already covers those, and including them would make the public API unnecessarily broad.

## Supported targets

| OS | Status |
| -- | ------ |
| Linux glibc (x86_64, aarch64) | supported |
| Linux musl / Alpine | supported — a separate Adoptium target (`alpine-linux`); glibc builds do not run there |
| macOS (x86_64, arm64) | supported |
| Windows (x86_64) | supported |
| Termux / Android | experimental — discovery only, no provisioning |

Architectures modelled: `x86_64`, `x86`, `aarch64`, `arm`, `ppc64le`, `s390x`, `riscv64`,
plus `Unknown`. Minimum supported Rust version, both verified in CI:

| Build | MSRV | Limited by |
| ----- | ---- | ---------- |
| default (no features) | **1.75** | async fn in trait |
| `network` / `install` | **1.88** | `reqwest` → `icu_properties_data` |

## Modules

### `version`
`JavaVersion` parses both the legacy `1.8.0_412-b08` scheme and JEP 223
(`11.0.22+7`, `21`, `22-ea+15`). `1.x` normalises to feature version `x`, and the
legacy `_NNN` update suffix becomes the patch level. Ordering is
`(major, minor, patch, GA-before-EA, build)`.

### `model`
`Platform`, `Architecture`, `JavaKind`, `DiscoverySource`, `JavaMetadata`,
`JavaInstallation`. `DiscoverySource`'s declaration order **is** its priority order:
`JavaHome < Path < System < Registry < Sdkman < Mise < Gradle < Jbang < UserDirectory < Termux`.

### `inspect`
Turns a candidate path into a `JavaInstallation`.

* `resolve_layout` finds `bin/java` and `bin/javac`, transparently resolving the macOS
  `Contents/Home` bundle layout and callers who pass `<home>/bin` or the launcher itself.
  Paths are canonicalised so symlinked duplicates collapse.
* Metadata sources, in strict order: `release` file → `-XshowSettings:properties` →
  `java -version` → directory-name heuristics.
* `InspectOptions` gates subprocess execution (off by default) and heuristics.
* JDK vs JRE is decided by the presence of `bin/javac`, not by the `release` file, which
  is not reliable on that point.

### `discovery`
`Discovery` is a builder over independent source modules (`env`, `path`, `linux`,
`macos`, `windows`, `termux`, `common_dirs`). Each source pushes candidates into a
`Collector`, which:

* de-duplicates by canonical home,
* resolves conflicts by discovery-source priority,
* sorts results newest-version-first, then by source priority, then by path.

`scan_children` is one level deep only. **There is no recursive filesystem scan in the
default path.**

### `selector`
`JavaQuery` holds the constraints; `Selector` applies and ranks them. Ranking key,
highest first: is-a-JDK (unless a JRE was requested) → is the `JAVA_HOME` installation →
version → source priority → path. Unknown architecture/platform is treated as
*compatible*, not as a mismatch, since metadata sources are allowed to be incomplete.

### `provision`
`JdkProvider` is the vendor abstraction (async fn in trait, no `async-trait`
dependency). `AdoptiumProvider` implements it against the Adoptium v3 API. The LTS list comes from
the API's `available_lts_releases` rather than a hardcoded constant, and "latest"
resolution walks candidate feature versions newest-first until one actually has a binary
for the requested OS and architecture — a version can appear in `available_releases`
without every combination existing. Adding
Corretto, Zulu, Microsoft or GraalVM means adding a provider, not changing the public
API.

`JavaInstaller` orchestrates:

```
resolve → check provider strings → reuse check → download (.partial)
        → verify SHA-256 → extract to staging → inspect → validate
        → atomic move → cleanup
```

Security properties that must not regress:

* a download is only renamed to its final name **after** checksum verification;
* a checksum mismatch **deletes** the artifact;
* every archive entry path — including link targets — is validated against the
  extraction root before anything is written. Link targets are resolved *lexically*:
  a symlink target is relative to the link's own directory (so `../java.base/LICENSE`,
  which real JDK archives contain, is legitimate), while a hard-link target is relative
  to the archive root. Only targets that escape the root are rejected;
* extraction happens in a unique staging directory, removed on both success and failure;
* inspection *and* validation happen entirely inside staging, so a JDK that does not match
  the request never reaches the final location;
* provider-supplied `file_name` and `release_name` must each be a single ordinary path
  component before they touch the filesystem — a provider is not trusted;
* a release without a published SHA-256 is refused unless `allow_unverified(true)`;
* an existing target directory is reused only after its contents are validated against
  the request, never on the strength of its name;
* installing for a platform other than the host is refused: the resulting JDK could
  not be inspected or launched here anyway.

The installation directory name encodes vendor, version, platform, architecture and
image type (`temurin-25.0.4-linux-x64-jre`), so builds that differ only by target
cannot collide.

## Feature gating

Networking is optional and never on by default. `discovery` pulls in no dependencies
beyond `thiserror`; `network` adds `reqwest`/`serde`/`sha2`; `install` adds
`zip`/`tar`/`flate2`/`futures-util`.

## Error handling

One structured `Error` enum, `#[non_exhaustive]`. Library code contains no `unwrap`,
`expect`, `panic!` or `todo!`, and `unsafe` is forbidden crate-wide. Errors are never
silently swallowed except where a source is legitimately optional (a missing directory
during discovery is not an error).
