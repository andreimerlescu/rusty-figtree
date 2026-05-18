// src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Figtree, attributes(figtree))]
pub fn derive_figtree(input: TokenStream) -> TokenStream {
    let input  = parse_macro_input!(input as DeriveInput);
    let name   = &input.ident;
    let fields = parse_fields(&input);

    let getters        = generate_getters(&fields);
    let resolution     = generate_resolution(&fields);
    let mutation_enum  = generate_mutation_enum(&name, &fields);
    let validators     = generate_validator_calls(&fields);
    let callbacks      = generate_callback_wiring(&fields);

    let expanded = quote! {
        impl #name {
            pub fn grow() -> Result<Self, figtree::Error> {
                #resolution
                #validators
                #callbacks
            }
            #(#getters)*
        }
        #mutation_enum
    };

    expanded.into()
}

// each of these is a private function in the same file
fn parse_fields(input: &DeriveInput) -> Vec<FieldConfig> { ... }
fn generate_getters(fields: &[FieldConfig]) -> Vec<...> { ... }
fn generate_resolution(fields: &[FieldConfig]) -> ... { ... }
fn generate_mutation_enum(name: &Ident, fields: &[FieldConfig]) -> ... { ... }
fn generate_validator_calls(fields: &[FieldConfig]) -> ... { ... }
fn generate_callback_wiring(fields: &[FieldConfig]) -> ... { ... }
