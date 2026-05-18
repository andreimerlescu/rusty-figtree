# figtree/src

This directory contains the complete runtime implementation of the figtree
crate. Each file is a self-contained module with a single responsibility.
Files are listed here in dependency order — each module only imports from
modules listed above it.

## Module Dependency Order

    error.rs        No internal dependencies. Defines FigtreeError and
                    FigtreeResult. Every other module imports from here.
                    This is the bedrock of the crate — get it right first.

    rules.rs        No internal dependencies. Defines the Rule enum that
                    governs per-key behavioral restrictions. Pure data,
                    no logic beyond helper predicate methods.

    fig.rs          Imports error and rules. Defines FigValue (the typed
                    value enum including i128 and f128 variants), FigSource
                    (which source in the PEMDAS chain provided the value),
                    FigState (semantic description of each history entry),
                    FigHistoryEntry (one point in a Fig's history), and
                    Fig itself (the container for a single config value).
                    History is append-only. Every state transition is
                    recorded including rejections and rule blocks.

    mutation.rs     Imports fig. Defines Mutation (an outbound change
                    notification distinct from history), MutationSender,
                    MutationReceiver, and the mutation_channel() constructor.
                    Mutations are only emitted on actual value changes —
                    not on rejections or no-ops.

    callbacks.rs    Imports error and fig. Defines CallbackPhase (AfterVerify,
                    AfterRead, AfterChange), CallbackFn (the boxed closure
                    type), Callback (phase + function), and CallbackRegistry
                    (ordered collection of callbacks for one Fig). Callbacks
                    fire in registration order. First failure halts the chain.

    validators.rs   Imports error and fig. Defines ValidatorFn (the boxed
                    closure type), NamedValidator, ValidatorRegistry, and all
                    built-in assure_type_rule functions. Validators receive
                    &FigValue and return FigtreeResult<()>. The full set
                    covers String, Bool, Int, Int64, Int128, Float64,
                    Float128, Duration, all List variants, and all Map
                    variants.

    priority.rs     Imports error, fig, rules. Defines the Source trait
                    (the contract any config source must satisfy), the
                    ResolutionResult struct (winning value + winning source),
                    resolve() (single-Fig PEMDAS resolution), and
                    resolve_all() (full-tree resolution used by parse/load).
                    This is the only place PEMDAS order is codified. All
                    sources depend on this module, never the reverse.

    sources/        A subdirectory of source implementations. Each file
                    implements the Source trait from priority.rs for one
                    specific origin — environment variables, CLI flags,
                    YAML files, JSON files, TOML files, INI files, plist
                    files, dotenv files, RON files, and embedded hardware.
                    See sources/README.md for details.

    tree.rs         Imports everything. The Tree struct is the public API
                    surface of the crate — it owns a collection of Figs,
                    holds sources, wires validators and callbacks, drives
                    resolution via priority.rs, and emits mutations when
                    tracking is enabled. It is built last because it
                    assembles all other modules.

## Design Principles Held Throughout

Single responsibility per file. No file reaches into another file's
internal implementation — only public interfaces are used. This means
each module can be tested in isolation before the next one is written.

Priority resolution is pure. The resolve() function in priority.rs is
a pure function — it takes inputs and returns a result without side
effects. The Tree calls it and acts on the result. This separation means
the PEMDAS contract is fully testable without a Tree.

History is append-only. Fig.history only grows. No entry is ever removed
or modified. This gives a complete audit trail of every value a Fig has
held, including rejections and rule blocks, from initialization onward.

Option A for heterogeneous collections. Callbacks and validators both
receive &FigValue rather than typed inner values. This is the correct
choice for the runtime layer because it allows storing mixed callbacks
in a Vec without fighting the type system. The proc macro layer
(figtree-derive) generates typed wrappers that extract the correct
variant before calling user closures, so end users never write FigValue
match arms themselves.

i128 and f128 are first-class. Go's numeric ceiling is i64 and f64.
Rust's type system supports i128 natively. figtree includes Int128,
Float128, ListInt128, ListFloat128, MapInt128, and MapFloat128 variants
throughout. Float128 is currently stored as f64 pending stabilization
of Rust's native f128 type — the variant name is reserved for forward
compatibility.

No_std embedded path. The embedded feature flag disables all std-dependent
sources (file reading, environment variables) and enables the EmbeddedSource
stub which reads from static config tables, EEPROM, flash, or any other
hardware-backed store accessible without an allocator.

## What Was Rejected And Why

A generic Fig<T> was considered and rejected. Making Fig generic over its
value type would require Tree to be generic too, and storing mixed-type
Figs in a single collection without dynamic dispatch becomes architecturally
painful. The FigValue enum approach matches Go's interface{} intent while
staying idiomatic in Rust.

Per-mutagenesis validator types were considered and rejected for the
runtime layer. Having IntValidatorFn, StringValidatorFn etc. would prevent
storing all validators for a Fig in a single Vec. The proc macro layer
handles typed validation at the call site — the runtime layer stays uniform.

A separate ValidatorFn crate was considered and rejected. The complexity
does not justify the split. Validators are pure functions with no external
dependencies beyond FigValue and FigtreeError, both of which are in this
crate.
