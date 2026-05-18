This directory contains the complete implementation of the figtree
procedural macro. The entire macro lives in a single file: lib.rs.

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

## Why One File

Procedural macro crates are almost always a single lib.rs. The macro
entry point must be in lib.rs by Rust's proc macro rules. Private helper
functions live in the same file rather than submodules because the
entire crate is a build-time tool — it compiles, runs, generates code,
and disappears. It contributes zero bytes to the final binary. Splitting
it into multiple files would add navigation overhead with no architectural
benefit at this scale.

If the macro grows substantially — multiple derive macros, attribute
macros, complex code generation — splitting into submodules becomes
worthwhile. That threshold has not been reached.

## What lib.rs Does

lib.rs has one public entry point and several private helper functions.

The public entry point is the proc_macro_derive function registered as
Figtree. The Rust compiler calls this whenever it encounters
#[derive(Figtree)] on a struct. It receives a TokenStream representing
the struct definition and returns a TokenStream of generated code.

The private helper functions are:

    parse_fields()              Walks the DeriveInput AST produced by syn
                                and extracts FieldConfig values for each
                                field in the struct. Reads all #[figtree()]
                                attribute arguments — default, env, validate,
                                on_change, on_verify, on_read, rule.

    generate_getters()          Produces typed getter methods for each field.
                                The getter returns the field's Rust type
                                directly, not FigValue. The extraction of
                                the correct FigValue variant is generated
                                inline by the macro.

    generate_resolution()       Produces the PEMDAS resolution chain for
                                each field as straight-line Rust code inside
                                the grow() constructor. No runtime dispatch.
                                The compiler sees a simple chain of if-let
                                branches and inlines it completely.

    generate_mutation_enum()    Produces the Mutation enum with one variant
                                per field in the struct. Each variant carries
                                typed old and new values matching the field's
                                declared Rust type. The compiler enforces
                                exhaustive matching on this enum.

    generate_validator_calls()  Produces validator invocation code in the
                                grow() constructor body. Validators declared
                                with #[figtree(validate = ...)] are called
                                in declaration order. If any validator fails
                                grow() returns Err immediately.

    generate_callback_wiring()  Produces code that registers closures
                                declared in #[figtree(on_change = ...)] etc.
                                into the CallbackRegistry for each field.
                                The generated code wraps each user closure
                                in a FigValue extractor so the user closure
                                receives the typed inner value, not a
                                FigValue reference.

## The Three Crates

The macro depends on three crates that are the standard toolkit for
Rust procedural macros.

syn parses the incoming TokenStream into a traversable AST. Without syn
you would need to write a Rust parser from scratch. syn provides
DeriveInput, Field, Attribute, Meta, and all the other types needed to
walk a struct definition and read its attributes.

quote produces TokenStreams from Rust code templates using the quote!
macro. It handles escaping, repetition, and hygiene automatically.
Without quote you would need to construct TokenStreams by hand, which
is error-prone and unreadable.

proc-macro2 provides token stream types that work both in proc macro
context (at build time) and in regular code (at test time). This is
what makes it possible to unit test macro logic without invoking the
full compiler. The proc_macro crate from the standard library only
works inside a proc macro invocation.

## Inspecting What Gets Generated

cargo-expand prints the fully expanded output of all macros in your
crate including everything figtree-derive generates. It is the primary
debugging tool.

    cargo install cargo-expand
    cargo expand

The output is valid Rust that you could have written by hand. Reading
it is the fastest way to understand what the macro is doing and to
diagnose unexpected behavior.

## Design Principles

The macro generates plain readable Rust. There is no runtime magic,
no hidden indirection, no framework overhead. If you ran cargo expand
and removed the #[derive(Figtree)] attribute, the generated code would
work identically if pasted in by hand. This is intentional — the macro
is a code generation convenience, not a runtime dependency.

Type information flows from the struct field declarations into the
generated code. A field declared as i32 generates an i32 getter that
extracts FigValue::Int. A field declared as Vec<String> generates a
Vec<String> getter that extracts FigValue::ListString. The user never
writes FigValue in their own code.

Validator type checking happens at macro expansion time. If a validator
designed for strings is applied to an i32 field, the macro emits a
compile error before the program reaches the linker. This is the key
advantage over the raw Tree API where type mismatches are caught at
runtime.

The Mutation enum is generated per-struct. Each variant is named after
its field with PascalCase conversion. The compiler enforces exhaustive
matching — adding a new field to the struct and recompiling immediately
surfaces any match expressions that do not handle the new variant.

