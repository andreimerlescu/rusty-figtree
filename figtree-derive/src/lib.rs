//! figtree-derive — procedural macro crate for figtree.
//!
//! Provides #[derive(Figtree)] which generates:
//!   - A struct wrapping Arc<RwLock<figtree::Tree>>
//!   - A grow() constructor that registers all fields and calls parse()
//!   - Typed getters for all fields (all take &self — pure reads)
//!   - A typed Mutation enum with one variant per field
//!   - A typed MutationReceiver wrapper
//!   - pollinate(), store(), usage(), problems(), history() delegations
//!
//! This crate is a build-time dependency only. It contributes zero bytes
//! to the final binary. End users depend on figtree, not figtree-derive.
//! figtree re-exports the Figtree derive macro transparently.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

mod error;
mod field;
mod generate;

use field::{StructConfig, parse_fields};
use generate::{
    generate_mutation_enum,
    generate_struct_definition,
    generate_struct_impl,
};

/// The #[derive(Figtree)] macro entry point.
///
/// Applied to a struct whose fields declare configuration keys:
///
/// ```ignore
///     use figtree::prelude::*;
///
///     #[derive(Figtree)]
///     #[figtree(
///         file      = "/etc/myapp/config.yaml",
///         tracking  = true,
///         pollinate = true,
///     )]
///     struct AppConfig {
///         #[figtree(
///             default     = 10,
///             env         = "WORKERS",
///             validate    = assure_int_in_range(1, 64),
///             description = "number of worker threads",
///         )]
///         workers: i32,
///
///         #[figtree(
///             default     = "http://localhost:8080",
///             env         = "ENDPOINT",
///             validate    = assure_string_has_prefix("http"),
///             validate    = assure_string_not_empty,
///             description = "api endpoint",
///         )]
///         endpoint: String,
///
///         #[figtree(
///             default     = false,
///             env         = "DEBUG",
///             description = "enable debug mode",
///         )]
///         debug: bool,
///     }
///
///     fn main() -> figtree::FigtreeResult<()> {
///         let config = AppConfig::grow()?;
///
///         println!("workers:  {}", config.workers());
///         println!("endpoint: {}", config.endpoint());
///         println!("debug:    {}", config.debug());
///
///         // typed mutation tracking
///         let rx = config.mutations().unwrap();
///         std::thread::spawn(move || {
///             for mutation in rx.iter() {
///                 match mutation {
///                     AppConfigMutation::Workers  { old, new } => { }
///                     AppConfigMutation::Endpoint { old, new } => { }
///                     AppConfigMutation::Debug    { old, new } => { }
///                 }
///             }
///         });
///
///         Ok(())
///     }
/// ```
///
/// ## What Gets Generated
///
/// Run `cargo expand` in a crate that uses #[derive(Figtree)] to see
/// the full generated output. The output is plain, readable Rust that
/// you could write by hand.
///
/// ## Field Attributes
///
/// ```ignore
///     default     = <expr>        default value (omit for required keys)
///     env         = "VAR_NAME"    environment variable name
///     validate    = <expr>        validator function or closure (repeatable)
///     on_change   = <closure>     fires on store() - receives typed inner value
///     on_verify   = <closure>     fires on parse()/load() - receives typed inner value
///     on_read     = <closure>     fires on every getter call - receives typed inner value
///     rule        = RuleXxx       behavioral rule (one per field)
///     description = "text"        shown in usage() output
///     key         = "custom_key"  override the Tree key name (default: field name)
/// ```
///
/// ## Struct Attributes
///
/// ```ignore
///     file      = "path"    config file to load (repeatable, in order)
///     tracking  = true      enable mutation tracking
///     pollinate = true      enable pollination via pollinate()
///     germinate = true      ignore -test. flags from the test runner
/// ```
#[proc_macro_derive(Figtree, attributes(figtree))]
pub fn derive_figtree(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // parse struct-level attributes
    let struct_config = match StructConfig::from_attrs(&input.attrs) {
        Ok(cfg) => cfg,
        Err(e)  => return error::to_compile_error(e).into(),
    };

    // parse field-level attributes
    let fields = match parse_fields(&input) {
        Ok(f)  => f,
        Err(e) => return error::to_compile_error(e).into(),
    };

    let struct_name = &input.ident;

    // generate all pieces
    let struct_definition = generate_struct_definition(struct_name);
    let struct_impl       = generate_struct_impl(struct_name, &struct_config, &fields);
    let mutation_enum     = generate_mutation_enum(struct_name, &fields);

    let expanded = quote! {
        // The generated struct replaces the user's field-based declaration.
        // Fields in the original struct are used as a schema only — they
        // drive registration, getters, and the Mutation enum but do not
        // become actual struct fields. The struct wraps Arc<RwLock<Tree>>.
        #struct_definition

        // impl block: grow(), getters, mutations(), pollinate(), store(),
        // usage(), problems(), history(), history_log(), fig()
        #struct_impl

        // Typed Mutation enum + MutationReceiver wrapper
        #mutation_enum
    };

    expanded.into()
}