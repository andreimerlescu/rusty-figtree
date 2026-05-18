# figtree-derive

figtree-derive is the procedural macro crate for figtree. It provides the
#[derive(Figtree)] macro and the #[figtree()] field attribute that the Rust
compiler expands at build time into typed getters, priority resolution logic,
a typed Mutation enum, validator invocations, and callback wiring.

## You Do Not Depend On This Crate Directly

figtree-derive is an implementation detail of the figtree crate. It is
re-exported transparently by figtree. Your Cargo.toml should only reference
figtree, never figtree-derive.

    [dependencies]
    figtree = "0.0.1"

figtree-derive will be pulled in automatically as a build-time dependency.
It contributes zero bytes to your final binary — it runs during compilation
and then disappears.

## What This Crate Does

When the Rust compiler encounters #[derive(Figtree)] on a struct, it invokes
the proc_macro_derive entry point in this crate with a token stream representing
your struct definition. This crate then:

    1.  Parses the token stream into a structured AST using the syn crate.
    2.  Reads each #[figtree()] attribute on the struct and on each field.
    3.  Validates that attribute arguments are well-formed and type-compatible.
    4.  Generates an impl block for your struct containing typed getters,
        a grow() constructor, and a store() method per field.
    5.  Generates a Mutation enum with one variant per field in your struct,
        each variant carrying typed old and new values.
    6.  Generates the priority resolution chain for each field — CLI flag
        first, then environment variable, then config files, then programmatic
        store, then default — as straight-line Rust code with no runtime
        dispatch.
    7.  Generates validator invocation calls in the grow() constructor body,
        so all validators run at startup before your application logic begins.
    8.  Generates callback wiring so on_verify, on_read, and on_change
        closures are called at the correct points in each key's lifecycle.
    9.  Emits the final token stream back to the compiler, which compiles it
        as if you had written it by hand.

## What Gets Generated

Given this struct definition:

    #[derive(Figtree)]
    #[figtree(tracking = true)]
    struct AppConfig {
        #[figtree(default = 10, env = "WORKERS", validate = assure_int_in_range(1, 64))]
        workers: i64,
    }

The macro generates approximately the following code:

    impl AppConfig {
        pub fn grow() -> Result<Self, figtree::FigtreeError> {
            let workers = {
                // priority 1: CLI flag
                if let Some(v) = figtree::sources::cli::get_i64("workers") {
                    v
                // priority 2: environment variable
                } else if let Ok(s) = std::env::var("WORKERS") {
                    s.parse::<i64>().map_err(figtree::FigtreeError::Parse)?
                // priority 3: default
                } else {
                    10i64
                }
            };
            figtree::validators::assure_int_in_range(1, 64)(workers)?;
            Ok(Self { workers })
        }

        pub fn workers(&self) -> i64 {
            self.workers
        }
    }

    pub enum Mutation {
        Workers { old: i64, new: i64 },
    }

The generated code is plain, readable Rust. You can inspect it with
cargo expand if you have cargo-expand installed.

## Inspecting Generated Code

    cargo install cargo-expand
    cargo expand

This prints the fully expanded output of all macros in your crate, including
everything figtree-derive generates for your struct. It is the most useful
debugging tool when working with or contributing to figtree-derive.

## Dependencies

figtree-derive depends on three crates that are standard in the Rust
procedural macro ecosystem.

    syn         Parses Rust token streams into a traversable AST. Used to
                read your struct definition and all #[figtree()] attributes.

    quote       Produces token streams from Rust code templates. Used to
                emit the generated impl blocks, enums, and function bodies.

    proc-macro2 Provides token stream types that work in both proc macro
                context and in regular code, enabling unit testing of the
                macro logic without invoking the full compiler.

## Contributing

Changes to figtree-derive must be made in conjunction with the figtree crate
when they affect the public API. Both crates are versioned together from the
single VERSION file at the repository root. A change to an attribute argument
in figtree-derive must have a corresponding runtime implementation in figtree,
and both must be covered by tests before merging.
