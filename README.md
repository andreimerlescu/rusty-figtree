# rusty-figtree

A layered runtime configuration system for Rust, inspired by the Go figtree package.
Supports multiple configuration sources with explicit priority resolution, per-key
validators, per-key callbacks, and live mutation tracking via channels.

## Repository Structure

    rusty-figtree/
    ├── Cargo.toml           # workspace root — ties both crates together
    ├── VERSION              # single version file for both crates
    ├── build.rs             # reads VERSION, exposes APP_VERSION at compile time
    ├── .gitignore
    ├── LICENSE
    ├── CHANGELOG.md
    ├── figtree/             # primary library crate
    │   ├── Cargo.toml
    │   ├── README.md
    │   └── src/
    │       ├── lib.rs
    │       ├── error.rs
    │       ├── tree.rs
    │       ├── fig.rs
    │       ├── mutation.rs
    │       ├── priority.rs
    │       ├── rules.rs
    │       ├── validators.rs
    │       ├── callbacks.rs
    │       ├── sources/
    │       │   ├── mod.rs
    │       │   ├── env.rs
    │       │   ├── cli.rs
    │       │   ├── yaml.rs
    │       │   ├── json.rs
    │       │   ├── ini.rs
    │       │   └── embedded.rs
    │       └── tests/
    │           ├── tree_test.rs
    │           ├── validators_test.rs
    │           ├── priority_test.rs
    │           ├── sources_test.rs
    │           └── callbacks_test.rs
    └── figtree-derive/      # procedural macro crate
        ├── Cargo.toml
        ├── README.md
        └── src/
            └── lib.rs

## Two Crates, One Repository

This repository is a Cargo workspace containing two crates that are always versioned,
built, tested, and published together.

figtree is the library your application depends on at runtime. figtree-derive is the
procedural macro crate the Rust compiler consumes at build time to expand the
#[derive(Figtree)] decorator. End users never reference figtree-derive directly in
their own Cargo.toml — it is re-exported transparently by figtree.

## Priority Resolution

figtree resolves configuration values in a fixed priority order. This order is
inspired by PEMDAS — just as multiplication always precedes addition regardless
of left-to-right reading, CLI flags always precede environment variables regardless
of declaration order.

    CLI flags
      > Environment Variables
        > Configuration Files (in load order)
          > Programmatic Store()
            > Default Values

The source that wins is the one highest in this hierarchy that has a value defined.
A default only speaks when nothing above it does.

## Feature Flags

    Feature       What It Enables
    -------       ---------------
    std           Standard library support (enabled by default)
    env           Environment variable source (enabled by default)
    yaml          YAML config file source via serde_yaml
    json          JSON config file source via serde_json
    ini           INI config file source via rust-ini
    cli           CLI flag source via clap
    async         Async mutation channel via tokio
    embedded      Bare metal source support, implies no_std

## Versioning

Both crates share a single VERSION file at the repository root. The build.rs script
reads this file at compile time and exposes the version as APP_VERSION. Bumping the
version with the bump tool updates both crates atomically.

    bump -patch -write
    cargo build --release

## License

Apache 2.0
