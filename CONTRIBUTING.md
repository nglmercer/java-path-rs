# Contributing

## Setup

```bash
cargo build --all-features
cargo test --all-features
```

Tests are hermetic: they run against the synthetic Java homes in `tests/fixtures/` and
need no JDK installed. The one test that touches the network is `#[ignore]`d:

```bash
cargo test --all-features -- --ignored   # hits api.adoptium.net
```

## Before opening a PR

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

Also check that the crate still builds with each feature in isolation — networking must
stay optional:

```bash
cargo check --no-default-features
cargo check --features serde
cargo check --features network
cargo check --features install
```

## Conventions

* Read `ARCHITECTURE.md` first; the module layering is deliberate.
* `AGENTS.md` lists the hard rules (no `unwrap` in library code, no full filesystem
  scans, metadata priority, traversal protection). They apply to humans too.
* New behaviour needs a test. New dependencies need a justification.
* Update `ROADMAP.md` when a task actually passes its acceptance criteria.

## Adding a JDK provider

Implement `provision::provider::JdkProvider` and map the vendor's OS/architecture tokens
onto `Platform`/`Architecture`. Do not change the public installer API to accommodate a
vendor — that is what the trait is for.

## Adding a discovery source

Add a module under `src/discovery/`, expose a `collect(&mut Collector)` function, wire it
into `Discovery::search`, and add a variant to `DiscoverySource` in the correct priority
position. Remember that the enum's declaration order is its priority order.
