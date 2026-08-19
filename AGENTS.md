# Rules for coding agents

1. Read `ARCHITECTURE.md` and `ROADMAP.md` before editing.
2. Work on exactly one roadmap task at a time.
3. Do not change the public API unless the task explicitly requires it.
4. Every new behaviour requires tests.
5. No `unwrap()`, `expect()`, `panic!()` or `todo!()` in production library code.
   Tests may use them.
6. Do not build shell commands by string concatenation. Use `std::process::Command`
   with separate arguments.
7. Never infer Java metadata from directory names when an authoritative source exists.
8. Metadata priority is fixed: JDK `release` file > JVM properties > `java -version` >
   path heuristics.
9. Platform-specific behaviour lives in its own module behind an abstraction.
10. No full filesystem scans in the default discovery path.
11. Network functionality stays behind the `network`/`install` features.
12. Downloads must be checksum verified before installation.
13. Archive extraction must prevent path traversal, symlink targets included.
14. New dependencies require justification in the PR description.
15. `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings` and
    `cargo test --all-features` must pass before a task is called complete.
16. Update `ROADMAP.md` checkboxes only after the acceptance criteria pass.
17. Never silently ignore errors in library code. A missing optional discovery source is
    not an error; a malformed `release` file is.

## Task format

Tasks should be scoped like this, not like "port java-path to Rust":

```
PHASE 2 / TASK 2.3 — Parse JDK release file

Objective: parse metadata from <JAVA_HOME>/release.
Input:     path to a candidate Java home.
Output:    JavaMetadata with version, vendor, architecture.

Requirements:
- Support quoted KEY="VALUE".
- Ignore unknown keys.
- Missing optional fields are allowed.
- Invalid JAVA_VERSION returns a typed error.
- No subprocess execution.

Tests: Temurin 8, Temurin 17, Oracle 21, malformed fixtures.
Out of scope: PATH discovery, java -version, networking, downloads.
Done when: cargo test release_file && cargo clippy -- -D warnings pass.
```

## Fixtures

`tests/fixtures/` holds synthetic Java homes (`release` file plus stub `bin/java`,
`bin/javac`). They are never executed — inspection defaults to no subprocess — so a
`#!/bin/sh` stub is enough. Add a fixture rather than depending on a JDK being installed
on the machine running the tests.
