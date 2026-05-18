use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::field::{FieldConfig, FieldMutagenesis, StructConfig};

// ── generate_tree_construction ────────────────────────────────────────────────

pub fn generate_tree_construction(struct_config: &StructConfig) -> TokenStream {
    let constructor = if struct_config.tracking {
        quote! { figtree::Tree::grow() }
    } else {
        quote! { figtree::Tree::new() }
    };

    let file_registrations: Vec<TokenStream> = struct_config.files.iter().map(|path| {
        let lower = path.to_lowercase();
        if lower.ends_with(".yaml") || lower.ends_with(".yml") {
            quote! { __tree.with_yaml_file(#path).map_err(|e| e)?; }
        } else if lower.ends_with(".json") {
            quote! { __tree.with_json_file(#path).map_err(|e| e)?; }
        } else if lower.ends_with(".toml") {
            quote! { __tree.with_toml_file(#path).map_err(|e| e)?; }
        } else if lower.ends_with(".ini") {
            quote! { __tree.with_ini_file(#path).map_err(|e| e)?; }
        } else if lower.ends_with(".plist") {
            quote! { __tree.with_plist_file(#path).map_err(|e| e)?; }
        } else if lower.ends_with(".env") {
            quote! { __tree.with_dotenv_file(#path).map_err(|e| e)?; }
        } else if lower.ends_with(".ron") {
            quote! { __tree.with_ron_file(#path).map_err(|e| e)?; }
        } else {
            quote! {}
        }
    }).collect();

    quote! {
        let mut __tree = #constructor;
        #(#file_registrations)*
    }
}

// ── generate_registration ─────────────────────────────────────────────────────

pub fn generate_registration(fields: &[FieldConfig]) -> TokenStream {
    let calls: Vec<TokenStream> = fields.iter().map(|field| {
        let key         = &field.key;
        let description = &field.description;
        let method      = format_ident!("{}", field.mutagenesis.tree_registration_method());

        match &field.default {
            Some(default_expr) => quote! {
                __tree.#method(#key, #default_expr, #description);
            },
            None => quote! {
                __tree.new_string_required(#key, #description);
            },
        }
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_validator_calls ──────────────────────────────────────────────────

pub fn generate_validator_calls(fields: &[FieldConfig]) -> TokenStream {
    let calls: Vec<TokenStream> = fields.iter().flat_map(|field| {
        let key = &field.key;
        field.validators.iter().enumerate().map(move |(i, validator_expr)| {
            let name = format!("validator_{}", i);
            quote! {
                __tree.with_validator(
                    #key,
                    #name,
                    {
                        let __validator = #validator_expr;
                        move |__fig_value| __validator(__fig_value)
                    }
                ).map_err(|e| e)?;
            }
        })
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_callback_wiring ──────────────────────────────────────────────────

pub fn generate_callback_wiring(fields: &[FieldConfig]) -> TokenStream {
    let calls: Vec<TokenStream> = fields.iter().flat_map(|field| {
        let key     = &field.key;
        let variant = format_ident!("{}", field.mutagenesis.fig_value_variant());
        let mut field_calls: Vec<TokenStream> = Vec::new();

        if let Some(on_change) = &field.on_change {
            field_calls.push(quote! {
                __tree.with_callback(
                    #key,
                    figtree::CallbackPhase::AfterChange,
                    {
                        let __cb = #on_change;
                        move |__fig_value| {
                            if let figtree::FigValue::#variant(ref __inner) = __fig_value {
                                __cb(__inner)
                            } else {
                                Ok(())
                            }
                        }
                    }
                ).map_err(|e| e)?;
            });
        }

        if let Some(on_verify) = &field.on_verify {
            field_calls.push(quote! {
                __tree.with_callback(
                    #key,
                    figtree::CallbackPhase::AfterVerify,
                    {
                        let __cb = #on_verify;
                        move |__fig_value| {
                            if let figtree::FigValue::#variant(ref __inner) = __fig_value {
                                __cb(__inner)
                            } else {
                                Ok(())
                            }
                        }
                    }
                ).map_err(|e| e)?;
            });
        }

        if let Some(on_read) = &field.on_read {
            field_calls.push(quote! {
                __tree.with_callback(
                    #key,
                    figtree::CallbackPhase::AfterRead,
                    {
                        let __cb = #on_read;
                        move |__fig_value| {
                            if let figtree::FigValue::#variant(ref __inner) = __fig_value {
                                __cb(__inner)
                            } else {
                                Ok(())
                            }
                        }
                    }
                ).map_err(|e| e)?;
            });
        }

        field_calls
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_rule_calls ───────────────────────────────────────────────────────

pub fn generate_rule_calls(fields: &[FieldConfig]) -> TokenStream {
    let calls: Vec<TokenStream> = fields.iter().filter_map(|field| {
        let rule = field.rule.as_ref()?;
        let key  = &field.key;
        let rule_ts = rule.to_token_stream();
        Some(quote! {
            __tree.with_rule(#key, #rule_ts).map_err(|e| e)?;
        })
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_resolution ───────────────────────────────────────────────────────

pub fn generate_resolution() -> TokenStream {
    quote! { __tree.parse()?; }
}

// ── generate_getters ──────────────────────────────────────────────────────────

pub fn generate_getters(fields: &[FieldConfig]) -> Vec<TokenStream> {
    fields.iter().map(|field| {
        let getter_name = &field.ident;
        let key         = &field.key;
        let tree_method = format_ident!("{}", field.mutagenesis.tree_getter_method());
        let panic_msg   = format!(
            "{}: type contract guaranteed by #[derive(Figtree)]",
            field.key
        );
        let return_type = rust_return_type(&field.mutagenesis);

        if field.mutagenesis.needs_clone() {
            quote! {
                pub fn #getter_name(&self) -> #return_type {
                    self.__tree
                        .read()
                        .unwrap()
                        .#tree_method(#key)
                        .expect(#panic_msg)
                        .to_owned()
                }
            }
        } else {
            quote! {
                pub fn #getter_name(&self) -> #return_type {
                    self.__tree
                        .read()
                        .unwrap()
                        .#tree_method(#key)
                        .expect(#panic_msg)
                }
            }
        }
    }).collect()
}

// ── generate_mutation_enum ────────────────────────────────────────────────────

pub fn generate_mutation_enum(
    struct_name: &Ident,
    fields:      &[FieldConfig],
) -> TokenStream {
    let mutation_enum_name     = format_ident!("{}Mutation",         struct_name);
    let mutation_receiver_name = format_ident!("{}MutationReceiver", struct_name);

    let variants: Vec<TokenStream> = fields.iter().map(|field| {
        let variant_ident = format_ident!("{}", to_pascal_case(&field.ident.to_string()));
        let return_type   = rust_return_type(&field.mutagenesis);
        quote! {
            #variant_ident {
                old: Option<#return_type>,
                new: #return_type,
            }
        }
    }).collect();

    let conversion_arms: Vec<TokenStream> = fields.iter().map(|field| {
        let key           = &field.key;
        let variant_ident = format_ident!("{}", to_pascal_case(&field.ident.to_string()));
        let fig_variant   = format_ident!("{}", field.mutagenesis.fig_value_variant());
        let enum_name     = &mutation_enum_name;

        if field.mutagenesis.needs_clone() {
            quote! {
                #key => {
                    let new_val = match &__m.new {
                        figtree::FigValue::#fig_variant(v) => v.clone(),
                        _ => return None,
                    };
                    let old_val = __m.old.as_ref().and_then(|o| {
                        if let figtree::FigValue::#fig_variant(v) = o {
                            Some(v.clone())
                        } else {
                            None
                        }
                    });
                    Some(#enum_name::#variant_ident { old: old_val, new: new_val })
                }
            }
        } else {
            quote! {
                #key => {
                    let new_val = match __m.new {
                        figtree::FigValue::#fig_variant(v) => v,
                        _ => return None,
                    };
                    let old_val = __m.old.and_then(|o| {
                        if let figtree::FigValue::#fig_variant(v) = o {
                            Some(v)
                        } else {
                            None
                        }
                    });
                    Some(#enum_name::#variant_ident { old: old_val, new: new_val })
                }
            }
        }
    }).collect();

    quote! {
        #[derive(Debug, Clone)]
        pub enum #mutation_enum_name {
            #(#variants,)*
        }

        pub struct #mutation_receiver_name {
            pub(crate) inner: figtree::MutationReceiver,
        }

        impl #mutation_receiver_name {
            fn convert(__m: figtree::Mutation) -> Option<#mutation_enum_name> {
                match __m.key.as_str() {
                    #(#conversion_arms)*
                    _ => None,
                }
            }

            pub fn recv(&self) -> Option<#mutation_enum_name> {
                loop {
                    match self.inner.recv() {
                        Some(m) => {
                            if let Some(typed) = Self::convert(m) {
                                return Some(typed);
                            }
                        }
                        None => return None,
                    }
                }
            }

            pub fn try_recv(&self) -> Option<#mutation_enum_name> {
                loop {
                    match self.inner.try_recv() {
                        Some(m) => {
                            if let Some(typed) = Self::convert(m) {
                                return Some(typed);
                            }
                        }
                        None => return None,
                    }
                }
            }

            pub fn iter(&self) -> impl Iterator<Item = #mutation_enum_name> + '_ {
                std::iter::from_fn(move || self.recv())
            }
        }
    }
}

// ── generate_struct_definition ────────────────────────────────────────────────

pub fn generate_struct_definition(struct_name: &Ident) -> TokenStream {
    quote! {
        pub struct #struct_name {
            __tree: std::sync::Arc<std::sync::RwLock<figtree::Tree>>,
        }
    }
}

// ── generate_struct_impl ──────────────────────────────────────────────────────

pub fn generate_struct_impl(
    struct_name:   &Ident,
    struct_config: &StructConfig,
    fields:        &[FieldConfig],
) -> TokenStream {
    let mutation_receiver_name = format_ident!("{}MutationReceiver", struct_name);

    let tree_construction = generate_tree_construction(struct_config);
    let registration      = generate_registration(fields);
    let validator_calls   = generate_validator_calls(fields);
    let callback_wiring   = generate_callback_wiring(fields);
    let rule_calls        = generate_rule_calls(fields);
    let resolution        = generate_resolution();
    let getters           = generate_getters(fields);

    quote! {
        impl #struct_name {
            pub fn grow() -> figtree::FigtreeResult<Self> {
                #tree_construction
                #registration
                #validator_calls
                #callback_wiring
                #rule_calls
                #resolution
                Ok(Self {
                    __tree: std::sync::Arc::new(
                        std::sync::RwLock::new(__tree)
                    ),
                })
            }

            #(#getters)*

            pub fn mutations(&self) -> Option<#mutation_receiver_name> {
                let mut tree = self.__tree.write().unwrap();
                tree.mutations().map(|rx| #mutation_receiver_name { inner: rx })
            }

            pub fn pollinate(&self) -> figtree::FigtreeResult<()> {
                self.__tree.write().unwrap().pollinate()
            }

            pub fn pollinate_key(&self, key: &str) -> figtree::FigtreeResult<()> {
                self.__tree.write().unwrap().pollinate_key(key)
            }

            pub fn store(
                &self,
                key:   &str,
                value: figtree::FigValue,
            ) -> figtree::FigtreeResult<()> {
                self.__tree.write().unwrap().store(key, value)
            }

            pub fn usage(&self) -> String {
                self.__tree.read().unwrap().usage()
            }

            pub fn problems(&self) -> Vec<figtree::FigtreeError> {
                self.__tree.read().unwrap().problems()
            }

            pub fn history(&self, key: &str) -> Option<Vec<figtree::FigHistoryEntry>> {
                self.__tree.read().unwrap()
                    .history(key)
                    .map(|h| h.to_vec())
            }

            pub fn history_log(&self, key: &str) -> Option<String> {
                self.__tree.read().unwrap().history_log(key)
            }

            pub fn fig(&self, key: &str) -> Option<figtree::Fig> {
                self.__tree.read().unwrap().fig(key).cloned()
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn rust_return_type(mutagenesis: &FieldMutagenesis) -> TokenStream {
    match mutagenesis {
        FieldMutagenesis::String      => quote! { String },
        FieldMutagenesis::Int         => quote! { i32 },
        FieldMutagenesis::Int64       => quote! { i64 },
        FieldMutagenesis::Int128      => quote! { i128 },
        FieldMutagenesis::Float64     => quote! { f64 },
        // FieldMutagenesis::Float128    => quote! { f64 }, // disabled until rust f128 stable
        FieldMutagenesis::Bool        => quote! { bool },
        FieldMutagenesis::Duration    => quote! { std::time::Duration },
        FieldMutagenesis::ListString  => quote! { Vec<String> },
        FieldMutagenesis::ListInt     => quote! { Vec<i32> },
        FieldMutagenesis::ListInt64   => quote! { Vec<i64> },
        FieldMutagenesis::ListInt128  => quote! { Vec<i128> },
        FieldMutagenesis::ListFloat64 => quote! { Vec<f64> },
        FieldMutagenesis::ListBool    => quote! { Vec<bool> },
        FieldMutagenesis::MapString   => quote! { std::collections::HashMap<String, String> },
        FieldMutagenesis::MapBool     => quote! { std::collections::HashMap<String, bool> },
    }
}

pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None    => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case_single_word() {
        assert_eq!(to_pascal_case("workers"),  "Workers");
        assert_eq!(to_pascal_case("endpoint"), "Endpoint");
    }

    #[test]
    fn test_to_pascal_case_multi_word() {
        assert_eq!(to_pascal_case("api_key"),         "ApiKey");
        assert_eq!(to_pascal_case("my_long_field"),   "MyLongField");
        assert_eq!(to_pascal_case("max_retry_count"), "MaxRetryCount");
    }

    #[test]
    fn test_to_pascal_case_already_pascal() {
        assert_eq!(to_pascal_case("Workers"), "Workers");
    }

    #[test]
    fn test_to_pascal_case_empty() {
        assert_eq!(to_pascal_case(""), "");
    }
}
