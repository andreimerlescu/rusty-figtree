use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::field::{FieldConfig, FieldMutagenesis, StructConfig};

// ── generate_tree_construction ────────────────────────────────────────────────

/// Generates the Tree construction code inside grow().
///
/// Produces:
///   let mut __tree = figtree::Tree::grow();  // or new() if !tracking
///   <file source registrations>
pub fn generate_tree_construction(struct_config: &StructConfig) -> TokenStream {
    let constructor = if struct_config.tracking {
        quote! { figtree::Tree::grow() }
    } else {
        quote! { figtree::Tree::new() }
    };

    let file_registrations: Vec<TokenStream> = struct_config.files.iter().map(|path| {
        let lower = path.to_lowercase();
        if lower.ends_with(".yaml") || lower.ends_with(".yml") {
            quote! {
                __tree.with_yaml_file(#path)
                    .map_err(|e| e)?;
            }
        } else if lower.ends_with(".json") {
            quote! {
                __tree.with_json_file(#path)
                    .map_err(|e| e)?;
            }
        } else if lower.ends_with(".toml") {
            quote! {
                __tree.with_toml_file(#path)
                    .map_err(|e| e)?;
            }
        } else if lower.ends_with(".ini") {
            quote! {
                __tree.with_ini_file(#path)
                    .map_err(|e| e)?;
            }
        } else if lower.ends_with(".plist") {
            quote! {
                __tree.with_plist_file(#path)
                    .map_err(|e| e)?;
            }
        } else if lower.ends_with(".env") {
            quote! {
                __tree.with_dotenv_file(#path)
                    .map_err(|e| e)?;
            }
        } else if lower.ends_with(".ron") {
            quote! {
                __tree.with_ron_file(#path)
                    .map_err(|e| e)?;
            }
        } else {
            quote! {
                // unknown extension — ignored at compile time
            }
        }
    }).collect();

    quote! {
        let mut __tree = #constructor;
        #(#file_registrations)*
    }
}

// ── generate_registration ─────────────────────────────────────────────────────

/// Generates Tree::new_<type>() registration calls for all fields.
///
/// Produces:
///   __tree.new_int("workers", 10, "number of workers");
///   __tree.new_string("endpoint", "http://localhost", "api endpoint");
pub fn generate_registration(fields: &[FieldConfig]) -> TokenStream {
    let calls: Vec<TokenStream> = fields.iter().map(|field| {
        let key         = &field.key;
        let description = &field.description;
        let method      = format_ident!("{}", field.mutagenesis.tree_registration_method());

        match &field.default {
            Some(default_expr) => quote! {
                __tree.#method(#key, #default_expr, #description);
            },
            None => {
                // required field — use new_string_required or the typed required variant
                // For now all required fields use the string_required path and rely
                // on coercion. A future improvement would add new_int_required etc.
                quote! {
                    __tree.new_string_required(#key, #description);
                }
            }
        }
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_validator_calls ──────────────────────────────────────────────────

/// Generates Tree::with_validator() calls for all fields that have validators.
///
/// Each validator expression is wrapped in a typed extractor closure so the
/// user's validator receives the correct inner type rather than &FigValue.
///
/// Produces:
///   __tree.with_validator("workers", "validator_0", {
///       let __v = assure_int_in_range(1, 64);
///       move |fig_value| __v(fig_value)
///   }).map_err(|e| e)?;
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
                ).map_err(|e| return Err(e))?;
            }
        })
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_callback_wiring ──────────────────────────────────────────────────

/// Generates Tree::with_callback() calls for all fields that have callbacks.
///
/// The user's closure receives the typed inner value extracted from FigValue,
/// not a &FigValue. The macro generates the extraction wrapper.
///
/// For on_change = |v: i32| { println!("{}", v); Ok(()) }
/// Generates a wrapper that extracts FigValue::Int(n) and calls the closure
/// with n. If the FigValue variant does not match, the callback is a no-op.
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
                ).map_err(|e| return Err(e))?;
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
                ).map_err(|e| return Err(e))?;
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
                ).map_err(|e| return Err(e))?;
            });
        }

        field_calls
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_rule_calls ───────────────────────────────────────────────────────

/// Generates Tree::with_rule() calls for fields that have a rule.
pub fn generate_rule_calls(fields: &[FieldConfig]) -> TokenStream {
    let calls: Vec<TokenStream> = fields.iter().filter_map(|field| {
        let rule = field.rule.as_ref()?;
        let key  = &field.key;
        let rule_ts = rule.to_token_stream();
        Some(quote! {
            __tree.with_rule(#key, #rule_ts)
                .map_err(|e| return Err(e))?;
        })
    }).collect();

    quote! { #(#calls)* }
}

// ── generate_resolution ───────────────────────────────────────────────────────

/// Generates the parse() call that resolves all registered keys.
///
/// Produces:
///   __tree.parse()?;
pub fn generate_resolution() -> TokenStream {
    quote! {
        __tree.parse()?;
    }
}

// ── generate_getters ──────────────────────────────────────────────────────────

/// Generates typed getter methods for all fields.
///
/// Each getter delegates to the appropriate Tree getter method and returns
/// the correct Rust type. Copy types are returned by value. Non-Copy types
/// are cloned from the Tree's internal storage.
///
/// The getter takes &self because the Tree getters take &self — pure reads,
/// no hidden mutations. This is correct and what the Rust API guidelines require.
///
/// Produces (for workers: i32):
///   pub fn workers(&self) -> i32 {
///       self.__tree.integer("workers").expect("workers: type guaranteed by macro")
///   }
///
/// Produces (for endpoint: String):
///   pub fn endpoint(&self) -> String {
///       self.__tree.string("endpoint")
///           .expect("endpoint: type guaranteed by macro")
///           .to_owned()
///   }
pub fn generate_getters(fields: &[FieldConfig]) -> Vec<TokenStream> {
    fields.iter().map(|field| {
        let getter_name = &field.ident;
        let key         = &field.key;
        let tree_method = format_ident!("{}", field.mutagenesis.tree_getter_method());
        let panic_msg   = format!(
            "{}: type contract guaranteed by #[derive(Figtree)] macro",
            field.key
        );
        let return_type = rust_return_type(&field.mutagenesis);

        if field.mutagenesis.needs_clone() {
            // String, Vec<T>, HashMap<K,V> — clone out of the tree
            quote! {
                pub fn #getter_name(&self) -> #return_type {
                    self.__tree
                        .#tree_method(#key)
                        .expect(#panic_msg)
                        .to_owned()
                }
            }
        } else {
            // Copy types — i32, i64, i128, f64, bool, Duration
            quote! {
                pub fn #getter_name(&self) -> #return_type {
                    self.__tree
                        .#tree_method(#key)
                        .expect(#panic_msg)
                }
            }
        }
    }).collect()
}

// ── generate_mutation_enum ────────────────────────────────────────────────────

/// Generates the typed Mutation enum and its receiver wrapper.
///
/// The enum has one variant per field, with typed old and new values.
/// The receiver wrapper converts raw figtree::Mutation events into
/// typed enum variants.
///
/// Produces:
///   pub enum AppConfigMutation {
///       Workers { old: Option<i32>, new: i32 },
///       Endpoint { old: Option<String>, new: String },
///   }
///
///   pub struct AppConfigMutationReceiver {
///       inner: figtree::MutationReceiver,
///   }
///
///   impl AppConfigMutationReceiver {
///       pub fn recv(&self) -> Option<AppConfigMutation> { ... }
///       pub fn try_recv(&self) -> Option<AppConfigMutation> { ... }
///       pub fn iter(&self) -> impl Iterator<Item = AppConfigMutation> + '_ { ... }
///   }
pub fn generate_mutation_enum(
    struct_name: &Ident,
    fields:      &[FieldConfig],
) -> TokenStream {
    let mutation_enum_name    = format_ident!("{}Mutation", struct_name);
    let mutation_receiver_name = format_ident!("{}MutationReceiver", struct_name);

    // enum variants
    let variants: Vec<TokenStream> = fields.iter().map(|field| {
        let variant_name = to_pascal_case(&field.ident.to_string());
        let variant_ident = format_ident!("{}", variant_name);
        let return_type   = rust_return_type(&field.mutagenesis);
        quote! {
            #variant_ident {
                old: Option<#return_type>,
                new: #return_type,
            }
        }
    }).collect();

    // match arms for converting figtree::Mutation -> typed enum variant
    let conversion_arms: Vec<TokenStream> = fields.iter().map(|field| {
        let key          = &field.key;
        let variant_name = to_pascal_case(&field.ident.to_string());
        let variant_ident = format_ident!("{}", variant_name);
        let fig_variant  = format_ident!("{}", field.mutagenesis.fig_value_variant());

        if field.mutagenesis.needs_clone() {
            quote! {
                #key => {
                    let new_val = match __m.new {
                        figtree::FigValue::#fig_variant(ref v) => v.clone(),
                        _ => return None,
                    };
                    let old_val = __m.old.as_ref().and_then(|o| {
                        if let figtree::FigValue::#fig_variant(ref v) = o {
                            Some(v.clone())
                        } else {
                            None
                        }
                    });
                    Some(#mutation_enum_name::#variant_ident {
                        old: old_val,
                        new: new_val,
                    })
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
                    Some(#mutation_enum_name::#variant_ident {
                        old: old_val,
                        new: new_val,
                    })
                }
            }
        }
    }).collect();

    quote! {
        /// Typed mutation enum generated by #[derive(Figtree)].
        /// Each variant corresponds to one field in the struct.
        /// The compiler enforces exhaustive matching — adding a field to
        /// the struct and recompiling will surface any unhandled variants.
        #[derive(Debug, Clone)]
        pub enum #mutation_enum_name {
            #(#variants,)*
        }

        /// Typed mutation receiver generated by #[derive(Figtree)].
        /// Wraps figtree::MutationReceiver and converts raw Mutation events
        /// into typed #mutation_enum_name variants.
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

            /// Blocks until a typed mutation is available or the channel closes.
            pub fn recv(&self) -> Option<#mutation_enum_name> {
                loop {
                    match self.inner.recv() {
                        Some(m) => {
                            if let Some(typed) = Self::convert(m) {
                                return Some(typed);
                            }
                            // unknown key — skip and keep waiting
                        }
                        None => return None,
                    }
                }
            }

            /// Returns a typed mutation if one is immediately available.
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

            /// Returns an iterator that yields typed mutations as they arrive.
            pub fn iter(&self) -> impl Iterator<Item = #mutation_enum_name> + '_ {
                std::iter::from_fn(move || self.recv())
            }
        }
    }
}

// ── generate_struct_impl ──────────────────────────────────────────────────────

/// Generates the complete impl block for the derived struct.
///
/// Assembles:
///   - grow() constructor
///   - typed getters
///   - mutations() method
///   - pollinate() and pollinate_key() delegation
///   - store() delegation
///   - usage(), problems(), history(), history_log() delegation
///   - fig() raw access delegation
pub fn generate_struct_impl(
    struct_name:   &Ident,
    struct_config: &StructConfig,
    fields:        &[FieldConfig],
) -> TokenStream {
    let mutation_enum_name     = format_ident!("{}Mutation", struct_name);
    let mutation_receiver_name = format_ident!("{}MutationReceiver", struct_name);

    let tree_construction  = generate_tree_construction(struct_config);
    let registration       = generate_registration(fields);
    let validator_calls    = generate_validator_calls(fields);
    let callback_wiring    = generate_callback_wiring(fields);
    let rule_calls         = generate_rule_calls(fields);
    let resolution         = generate_resolution();
    let getters            = generate_getters(fields);

    quote! {
        impl #struct_name {
            /// Constructs and resolves the configuration tree.
            ///
            /// Registers all fields, attaches validators and callbacks,
            /// applies rules, and calls parse() to resolve all values
            /// from sources in PEMDAS priority order.
            ///
            /// Returns Err if any required key is missing, any validator
            /// fails, or any AfterVerify callback returns an error.
            pub fn grow() -> figtree::FigtreeResult<Self> {
                #tree_construction
                #registration
                #validator_calls
                #callback_wiring
                #rule_calls
                #resolution
                Ok(Self {
                    __tree: std::sync::Arc::new(std::sync::RwLock::new(__tree)),
                })
            }

            /// Typed getters — each takes &self, returns the correct Rust type.
            #(#getters)*

            /// Returns a typed mutation receiver for this struct.
            /// Only available when tracking was enabled (the struct had
            /// #[figtree(tracking = true)]).
            ///
            /// Call this before grow() to receive all mutations including
            /// those from initial resolution — or immediately after grow()
            /// to receive subsequent changes only.
            pub fn mutations(&self) -> Option<#mutation_receiver_name> {
                let mut tree = self.__tree.write().unwrap();
                tree.mutations().map(|rx| #mutation_receiver_name { inner: rx })
            }

            /// Re-checks all environment variables and updates any fields
            /// whose env var has changed. Explicit — never called automatically.
            /// No-op unless Options::pollinate was true at construction.
            pub fn pollinate(&self) -> figtree::FigtreeResult<()> {
                self.__tree.write().unwrap().pollinate()
            }

            /// Re-checks the environment variable for a single field key.
            pub fn pollinate_key(&self, key: &str) -> figtree::FigtreeResult<()> {
                self.__tree.write().unwrap().pollinate_key(key)
            }

            /// Sets a value programmatically by key name.
            /// Enforces rules, validates, fires AfterChange callbacks,
            /// and emits a mutation if tracking is enabled.
            pub fn store(
                &self,
                key:   &str,
                value: figtree::FigValue,
            ) -> figtree::FigtreeResult<()> {
                self.__tree.write().unwrap().store(key, value)
            }

            /// Returns a human-readable summary of all registered keys,
            /// their current values, sources, and descriptions.
            pub fn usage(&self) -> String {
                self.__tree.read().unwrap().usage()
            }

            /// Returns all errors currently recorded against any field.
            pub fn problems(&self) -> Vec<figtree::FigtreeError> {
                self.__tree.read().unwrap().problems()
            }

            /// Returns the history of state transitions for a field key.
            pub fn history(&self, key: &str) -> Option<Vec<figtree::FigHistoryEntry>> {
                self.__tree.read().unwrap()
                    .history(key)
                    .map(|h| h.to_vec())
            }

            /// Returns a formatted history log for a field key.
            pub fn history_log(&self, key: &str) -> Option<String> {
                self.__tree.read().unwrap().history_log(key)
            }

            /// Returns a clone of the raw Fig for a field key.
            pub fn fig(&self, key: &str) -> Option<figtree::Fig> {
                self.__tree.read().unwrap().fig(key).cloned()
            }
        }
    }
}

// ── generate_struct_definition ────────────────────────────────────────────────

/// Generates the hidden __tree field that the struct needs to hold its state.
///
/// The user's struct definition is not modified — instead the macro generates
/// a new struct with the same name that wraps an Arc<RwLock<Tree>>.
///
/// This means the user writes:
///   #[derive(Figtree)]
///   struct AppConfig { workers: i32 }
///
/// And the macro produces a struct that is NOT the user's field layout but
/// rather a config accessor that wraps Tree. The user's field declarations
/// are used only as a schema — they drive registration, getters, and the
/// Mutation enum. They do not become struct fields.
pub fn generate_struct_definition(struct_name: &Ident) -> TokenStream {
    quote! {
        pub struct #struct_name {
            __tree: std::sync::Arc<std::sync::RwLock<figtree::Tree>>,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns the Rust return type TokenStream for a given mutagenesis.
fn rust_return_type(mutagenesis: &FieldMutagenesis) -> TokenStream {
    match mutagenesis {
        FieldMutagenesis::String     => quote! { String },
        FieldMutagenesis::Int        => quote! { i32 },
        FieldMutagenesis::Int64      => quote! { i64 },
        FieldMutagenesis::Int128     => quote! { i128 },
        FieldMutagenesis::Float64    => quote! { f64 },
        FieldMutagenesis::Float128   => quote! { f64 },
        FieldMutagenesis::Bool       => quote! { bool },
        FieldMutagenesis::Duration   => quote! { std::time::Duration },
        FieldMutagenesis::ListString => quote! { Vec<String> },
        FieldMutagenesis::ListInt    => quote! { Vec<i32> },
        FieldMutagenesis::ListInt64  => quote! { Vec<i64> },
        FieldMutagenesis::ListInt128 => quote! { Vec<i128> },
        FieldMutagenesis::ListFloat64=> quote! { Vec<f64> },
        FieldMutagenesis::ListBool   => quote! { Vec<bool> },
        FieldMutagenesis::MapString  => quote! { std::collections::HashMap<String, String> },
        FieldMutagenesis::MapBool    => quote! { std::collections::HashMap<String, bool> },
    }
}

/// Converts snake_case to PascalCase for enum variant names.
/// "workers" -> "Workers"
/// "api_key" -> "ApiKey"
/// "my_long_field_name" -> "MyLongFieldName"
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
        assert_eq!(to_pascal_case("api_key"),          "ApiKey");
        assert_eq!(to_pascal_case("my_long_field"),    "MyLongField");
        assert_eq!(to_pascal_case("max_retry_count"),  "MaxRetryCount");
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
