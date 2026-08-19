# java-path

A Rust library for **discovering, inspecting, selecting, downloading and provisioning JVM/JDK installations** across platforms.

It is library-first: everything is available as typed API, no CLI required.

```rust
use java_path::{discover, SelectExt};

let installs = discover()?;
let java = installs.select().major(21).jdk().current_arch().best()?;
println!("{}", java.home.display());
```

## Why this crate

| Capability                | java-locator | java-manager | jbx      | java-path |
| ------------------------- | -----------: | -----------: | -------: | --------: |
| Find active Java          | ✅ | ✅ | ✅ | ✅ |
| Find many JDKs            | limited | ✅ | ✅ | ✅ |
| Version/vendor/arch       | limited | ✅ | ✅ | ✅ |
| Download JDK              | ❌ | ❌ | ✅ | ✅ |
| Adoptium API as a library | ❌ | ❌ | internal | **✅** |
| Verify downloads          | ❌ | ❌ | ✅ | ✅ |
| Library-first API         | ✅ | ✅ | no | **✅** |
| Termux/Android            | ❌ | ❌ | no | **✅** |
| Select by constraints     | basic | some | CLI-oriented | **✅** |

## Features

| Feature     | Default | What it adds |
| ----------- | ------- | ------------ |
| `discovery` | yes     | Local discovery and inspection. No network, no subprocesses by default. |
| `serde`     | no      | `Serialize`/`Deserialize` for the model types. |
| `network`   | no      | The Adoptium release API (`AdoptiumProvider`). |
| `install`   | no      | Download, checksum-verify and extract JDKs (`JavaInstaller`). |
| `termux`    | no      | Termux-specific behaviour. |

```toml
[dependencies]
java-path = { version = "0.1", features = ["install"] }
```

## Discovery

Discovery never scans the whole filesystem. It consults, in priority order:

`JAVA_HOME` → `PATH` → platform system directories → SDKMAN!/mise/Gradle/JBang stores → caller-supplied roots.

```rust
use java_path::Discovery;

let installs = Discovery::new()
    .root("/opt/my-jdks")
    .tool_stores(false)
    .search()?;
```

Platform coverage:

* **Linux** — `/usr/lib/jvm`, `/usr/java`, `/usr/local/java`, `/opt/java`, `/opt/jdk(s)`
* **macOS** — `/Library/Java/JavaVirtualMachines`, the per-user equivalent, and `/usr/libexec/java_home`
* **Windows** — Program Files vendor directories and the registry (via `reg.exe`; no Everything SDK dependency)
* **Termux** — `$PREFIX/opt`, detected from the Termux prefix

## Metadata priority

Metadata is never guessed from a directory name when something authoritative exists:

1. `<JAVA_HOME>/release`
2. `java -XshowSettings:properties -version`
3. `java -version`
4. directory-name heuristics — last resort only

Subprocess execution is **off by default** so filesystem scans never launch arbitrary
executables. Opt in with `InspectOptions::thorough()`, or turn heuristics off entirely
with `InspectOptions::strict()`.

## Selection

```rust
use java_path::{discover, JavaQuery, SelectExt};

let installs = discover()?;
let java = installs
    .select()
    .min_major(17)
    .jdk()
    .vendor("Eclipse Adoptium")
    .current_arch()
    .best()?;
```

Ranking is deterministic: JDK over JRE, then the `JAVA_HOME` installation, then the
newest version, then discovery-source priority, then path. Early-access builds are
excluded unless `allow_prerelease(true)` is set.

## Provisioning

```rust
use java_path::{InstallEvent, JavaInstaller};

let java = JavaInstaller::adoptium()
    .version(21)
    .on_event(|e| eprintln!("{e:?}"))
    .install()
    .await?;
```

The install pipeline is: resolve → download to `.partial` → SHA-256 verify → extract to
a staging directory → inspect → atomic move into place. A failed checksum deletes the
artifact; archive entries that would escape the extraction root are rejected. Installs
are idempotent — an existing valid Java home in the target directory is returned without
touching the network.

Termux is deliberately excluded from provisioning: generic Adoptium Linux binaries do not
run there, so use the Termux package manager instead.

## Examples

```bash
cargo run --example discover
cargo run --example select_java_21
cargo run --example install_temurin --features install
```

## License

MIT. Ported in spirit — not line by line — from the MIT-licensed
[`nglmercer/java-path`](https://github.com/nglmercer/java-path) TypeScript package.
