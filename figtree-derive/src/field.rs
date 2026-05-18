use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, DeriveInput, Expr, Field, Fields, Ident, Lit, Type,
    spanned::Spanned,
};

use crate::error::syn_error;

// ── StructConfig ──────────────────────────────────────────────────────────────

/// Parsed attributes from the #[figtree()] annotation on the struct itself.
#[derive(Debug, Default)]
pub struct StructConfig {
    pub files:     Vec<String>,
    pub tracking:  bool,
    pub pollinate: bool,
    pub germinate: bool,
}

impl StructConfig {
    pub fn from_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut config = StructConfig::default();

        for attr in attrs {
            if !attr.path().is_ident("figtree") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("file") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        config.files.push(s.value());
                    } else {
                        return Err(syn_error(&meta.path, "file must be a string literal"));
                    }
                    return Ok(());
                }
                if meta.path.is_ident("tracking") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Bool(b) = lit {
                        config.tracking = b.value();
                    } else {
                        return Err(syn_error(&meta.path, "tracking must be a bool literal"));
                    }
                    return Ok(());
                }
                if meta.path.is_ident("pollinate") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Bool(b) = lit {
                        config.pollinate = b.value();
                    } else {
                        return Err(syn_error(&meta.path, "pollinate must be a bool literal"));
                    }
                    return Ok(());
                }
                if meta.path.is_ident("germinate") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Bool(b) = lit {
                        config.germinate = b.value();
                    } else {
                        return Err(syn_error(&meta.path, "germinate must be a bool literal"));
                    }
                    return Ok(());
                }
                Err(syn_error(
                    &meta.path,
                    &format!(
                        "unknown struct-level figtree attribute '{}'. \
                         Expected: file, tracking, pollinate, germinate",
                        meta.path.get_ident()
                            .map(|i| i.to_string())
                            .unwrap_or_default()
                    ),
                ))
            })?;
        }

        Ok(config)
    }
}

// ── FieldMutagenesis ──────────────────────────────────────────────────────────

/// The mutagenesis type of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldMutagenesis {
    String,
    Int,
    Int64,
    Int128,
    Float64,
    // Float128, // disabled until f128 is stable
    Bool,
    Duration,
    ListString,
    ListInt,
    ListInt64,
    ListInt128,
    ListFloat64,
    ListBool,
    MapString,
    MapBool,
}

impl FieldMutagenesis {
    pub fn from_type(ty: &Type) -> syn::Result<Self> {
        let path = match ty {
            Type::Path(tp) => &tp.path,
            _ => return Err(syn::Error::new(
                ty.span(),
                "figtree: field type must be a path type (e.g. i32, String, Vec<String>)",
            )),
        };

        let segment = path.segments.last().ok_or_else(|| {
            syn::Error::new(ty.span(), "figtree: empty type path")
        })?;

        let ident = segment.ident.to_string();

        match ident.as_str() {
            "String"                    => return Ok(FieldMutagenesis::String),
            "i32"                       => return Ok(FieldMutagenesis::Int),
            "i64"                       => return Ok(FieldMutagenesis::Int64),
            "i128"                      => return Ok(FieldMutagenesis::Int128),
            "f64"                       => return Ok(FieldMutagenesis::Float64),
            "bool"                      => return Ok(FieldMutagenesis::Bool),
            "Duration" | "StdDuration"  => return Ok(FieldMutagenesis::Duration),
            _                           => {}
        }

        if ident == "Vec" {
            let inner = extract_first_generic_arg(segment)?;
            return match inner.as_str() {
                "String" => Ok(FieldMutagenesis::ListString),
                "i32"    => Ok(FieldMutagenesis::ListInt),
                "i64"    => Ok(FieldMutagenesis::ListInt64),
                "i128"   => Ok(FieldMutagenesis::ListInt128),
                "f64"    => Ok(FieldMutagenesis::ListFloat64),
                "bool"   => Ok(FieldMutagenesis::ListBool),
                other    => Err(syn::Error::new(
                    ty.span(),
                    &format!(
                        "figtree: Vec<{}> is not a supported mutagenesis. \
                         Supported element types: String, i32, i64, i128, f64, bool",
                        other
                    ),
                )),
            };
        }

        if ident == "HashMap" {
            let (key, val) = extract_map_generic_args(segment)?;
            if key != "String" {
                return Err(syn::Error::new(
                    ty.span(),
                    "figtree: HashMap key must be String",
                ));
            }
            return match val.as_str() {
                "String" => Ok(FieldMutagenesis::MapString),
                "bool"   => Ok(FieldMutagenesis::MapBool),
                other    => Err(syn::Error::new(
                    ty.span(),
                    &format!(
                        "figtree: HashMap<String, {}> is not a supported mutagenesis. \
                         Supported value types: String, bool",
                        other
                    ),
                )),
            };
        }

        Err(syn::Error::new(
            ty.span(),
            &format!(
                "figtree: '{}' is not a recognized mutagenesis type. \
                 Supported: String, i32, i64, i128, f64, bool, Duration, \
                 Vec<String|i32|i64|i128|f64|bool>, \
                 HashMap<String, String|bool>",
                ident
            ),
        ))
    }

    /*
    // disabled until rust f128 is stable
    pub fn tree_registration_method(&self) -> &'static str {
        match self {
            FieldMutagenesis::String      => "new_string",
            FieldMutagenesis::Int         => "new_int",
            FieldMutagenesis::Int64       => "new_int64",
            FieldMutagenesis::Int128      => "new_int128",
            FieldMutagenesis::Float64     => "new_float64",
            // FieldMutagenesis::Float128    => "new_float128", // disabled until rust f128 is stable
            FieldMutagenesis::Bool        => "new_bool",
            FieldMutagenesis::Duration    => "new_duration",
            FieldMutagenesis::ListString  => "new_list_string",
            FieldMutagenesis::ListInt     => "new_list_int",
            FieldMutagenesis::ListInt64   => "new_list_int64",
            FieldMutagenesis::ListInt128  => "new_list_int128",
            FieldMutagenesis::ListFloat64 => "new_list_float64",
            FieldMutagenesis::ListBool    => "new_list_bool",
            FieldMutagenesis::MapString   => "new_map_string",
            FieldMutagenesis::MapBool     => "new_map_bool",
        }
    }

    pub fn tree_getter_method(&self) -> &'static str {
        match self {
            FieldMutagenesis::String      => "string",
            FieldMutagenesis::Int         => "integer",
            FieldMutagenesis::Int64       => "int64",
            FieldMutagenesis::Int128      => "int128",
            FieldMutagenesis::Float64     => "float64",
            // FieldMutagenesis::Float128    => "float128", // disabed until rust f128 is stable
            FieldMutagenesis::Bool        => "boolean",
            FieldMutagenesis::Duration    => "duration",
            FieldMutagenesis::ListString  => "list_string",
            FieldMutagenesis::ListInt     => "list_int",
            FieldMutagenesis::ListInt64   => "list_int64",
            FieldMutagenesis::ListInt128  => "list_int128",
            FieldMutagenesis::ListFloat64 => "list_float64",
            FieldMutagenesis::ListBool    => "list_bool",
            FieldMutagenesis::MapString   => "map_string",
            FieldMutagenesis::MapBool     => "map_bool",
        }
    }

    pub fn fig_value_variant(&self) -> &'static str {
        match self {
            FieldMutagenesis::String      => "String",
            FieldMutagenesis::Int         => "Int",
            FieldMutagenesis::Int64       => "Int64",
            FieldMutagenesis::Int128      => "Int128",
            FieldMutagenesis::Float64     => "Float64",
            // FieldMutagenesis::Float128    => "Float128", // disabled until rust f128 stable
            FieldMutagenesis::Bool        => "Bool",
            FieldMutagenesis::Duration    => "Duration",
            FieldMutagenesis::ListString  => "ListString",
            FieldMutagenesis::ListInt     => "ListInt",
            FieldMutagenesis::ListInt64   => "ListInt64",
            FieldMutagenesis::ListInt128  => "ListInt128",
            FieldMutagenesis::ListFloat64 => "ListFloat64",
            FieldMutagenesis::ListBool    => "ListBool",
            FieldMutagenesis::MapString   => "MapString",
            FieldMutagenesis::MapBool     => "MapBool",
        }
    }

    pub fn needs_clone(&self) -> bool {
        matches!(
            self,
            FieldMutagenesis::String
                | FieldMutagenesis::ListString
                | FieldMutagenesis::ListInt
                | FieldMutagenesis::ListInt64
                | FieldMutagenesis::ListInt128
                | FieldMutagenesis::ListFloat64
                | FieldMutagenesis::ListBool
                | FieldMutagenesis::MapString
                | FieldMutagenesis::MapBool
        )
    }
    // disabled until rust f128 is stable */
}

// ── FieldRule ─────────────────────────────────────────────────────────────────

/// A rule parsed from #[figtree(rule = ...)]
#[derive(Debug, Clone)]
pub enum FieldRule {
    PreventChange,
    PanicOnChange,
    NoValidations,
    NoCallbacks,
    NoFlags,
    NoEnv,
    NoMaps,
    NoLists,
    CondemnedFromResurrection,
}

impl FieldRule {
    pub fn from_ident(ident: &Ident) -> syn::Result<Self> {
        match ident.to_string().as_str() {
            "RulePreventChange"             => Ok(FieldRule::PreventChange),
            "RulePanicOnChange"             => Ok(FieldRule::PanicOnChange),
            "RuleNoValidations"             => Ok(FieldRule::NoValidations),
            "RuleNoCallbacks"               => Ok(FieldRule::NoCallbacks),
            "RuleNoFlags"                   => Ok(FieldRule::NoFlags),
            "RuleNoEnv"                     => Ok(FieldRule::NoEnv),
            "RuleNoMaps"                    => Ok(FieldRule::NoMaps),
            "RuleNoLists"                   => Ok(FieldRule::NoLists),
            "RuleCondemnedFromResurrection" => Ok(FieldRule::CondemnedFromResurrection),
            other => Err(syn_error(
                ident,
                &format!(
                    "unknown rule '{}'. Expected one of: RulePreventChange, \
                     RulePanicOnChange, RuleNoValidations, RuleNoCallbacks, \
                     RuleNoFlags, RuleNoEnv, RuleNoMaps, RuleNoLists, \
                     RuleCondemnedFromResurrection",
                    other
                ),
            )),
        }
    }

    pub fn to_token_stream(&self) -> TokenStream {
        match self {
            FieldRule::PreventChange             => quote! { figtree::Rule::PreventChange },
            FieldRule::PanicOnChange             => quote! { figtree::Rule::PanicOnChange },
            FieldRule::NoValidations             => quote! { figtree::Rule::NoValidations },
            FieldRule::NoCallbacks               => quote! { figtree::Rule::NoCallbacks },
            FieldRule::NoFlags                   => quote! { figtree::Rule::NoFlags },
            FieldRule::NoEnv                     => quote! { figtree::Rule::NoEnv },
            FieldRule::NoMaps                    => quote! { figtree::Rule::NoMaps },
            FieldRule::NoLists                   => quote! { figtree::Rule::NoLists },
            FieldRule::CondemnedFromResurrection => quote! { figtree::Rule::CondemnedFromResurrection },
        }
    }
}

// ── FieldConfig ───────────────────────────────────────────────────────────────

/// Complete parsed configuration for one field in the derived struct.
///
/// Debug intentionally omitted — syn::Type and syn::Expr do not implement
/// Debug without the syn extra-traits feature. This is an internal type
/// used only during macro expansion and never needs to be printed.
pub struct FieldConfig {
    pub ident:       Ident,
    #[allow(unused)]
    pub ty:          Type,
    pub mutagenesis: FieldMutagenesis,
    pub key:         String,
    pub default:     Option<Expr>,
    pub env:         Option<String>,
    pub validators:  Vec<Expr>,
    pub on_change:   Option<Expr>,
    pub on_verify:   Option<Expr>,
    pub on_read:     Option<Expr>,
    pub rule:        Option<FieldRule>,
    pub description: String,
}

impl FieldConfig {
    pub fn from_field(field: &Field) -> syn::Result<Self> {
        let ident = field.ident.clone().ok_or_else(|| {
            syn::Error::new(
                field.span(),
                "figtree: tuple struct fields are not supported",
            )
        })?;

        let ty          = field.ty.clone();
        let mutagenesis = FieldMutagenesis::from_type(&ty)?;
        let key         = ident.to_string();

        let mut config = FieldConfig {
            ident,
            ty,
            mutagenesis,
            key,
            default:     None,
            env:         None,
            validators:  Vec::new(),
            on_change:   None,
            on_verify:   None,
            on_read:     None,
            rule:        None,
            description: String::new(),
        };

        for attr in &field.attrs {
            if !attr.path().is_ident("figtree") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                config.parse_field_attr(&meta)
            })?;
        }

        Ok(config)
    }

    fn parse_field_attr(
        &mut self,
        meta: &syn::meta::ParseNestedMeta,
    ) -> syn::Result<()> {
        if meta.path.is_ident("key") {
            let value = meta.value()?;
            let lit: Lit = value.parse()?;
            if let Lit::Str(s) = lit {
                self.key = s.value();
            } else {
                return Err(syn_error(&meta.path, "key must be a string literal"));
            }
            return Ok(());
        }

        if meta.path.is_ident("default") {
            let value = meta.value()?;
            let expr: Expr = value.parse()?;
            self.default = Some(expr);
            return Ok(());
        }

        if meta.path.is_ident("env") {
            let value = meta.value()?;
            let lit: Lit = value.parse()?;
            if let Lit::Str(s) = lit {
                self.env = Some(s.value());
            } else {
                return Err(syn_error(&meta.path, "env must be a string literal"));
            }
            return Ok(());
        }

        if meta.path.is_ident("validate") {
            let value = meta.value()?;
            let expr: Expr = value.parse()?;
            self.validators.push(expr);
            return Ok(());
        }

        if meta.path.is_ident("on_change") {
            let value = meta.value()?;
            let expr: Expr = value.parse()?;
            self.on_change = Some(expr);
            return Ok(());
        }

        if meta.path.is_ident("on_verify") {
            let value = meta.value()?;
            let expr: Expr = value.parse()?;
            self.on_verify = Some(expr);
            return Ok(());
        }

        if meta.path.is_ident("on_read") {
            let value = meta.value()?;
            let expr: Expr = value.parse()?;
            self.on_read = Some(expr);
            return Ok(());
        }

        if meta.path.is_ident("rule") {
            let value = meta.value()?;
            let ident: Ident = value.parse()?;
            self.rule = Some(FieldRule::from_ident(&ident)?);
            return Ok(());
        }

        if meta.path.is_ident("description") {
            let value = meta.value()?;
            let lit: Lit = value.parse()?;
            if let Lit::Str(s) = lit {
                self.description = s.value();
            } else {
                return Err(syn_error(&meta.path, "description must be a string literal"));
            }
            return Ok(());
        }

        Err(syn_error(
            &meta.path,
            &format!(
                "unknown field-level figtree attribute '{}'. \
                 Expected: key, default, env, validate, on_change, \
                 on_verify, on_read, rule, description",
                meta.path.get_ident()
                    .map(|i| i.to_string())
                    .unwrap_or_default()
            ),
        ))
    }
}

// ── parse_fields ──────────────────────────────────────────────────────────────

pub fn parse_fields(input: &DeriveInput) -> syn::Result<Vec<FieldConfig>> {
    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            Fields::Unnamed(_) => {
                return Err(syn::Error::new(
                    input.ident.span(),
                    "figtree: #[derive(Figtree)] requires a struct with named fields",
                ))
            }
            Fields::Unit => {
                return Err(syn::Error::new(
                    input.ident.span(),
                    "figtree: #[derive(Figtree)] requires a struct with named fields",
                ))
            }
        },
        syn::Data::Enum(_) => {
            return Err(syn::Error::new(
                input.ident.span(),
                "figtree: #[derive(Figtree)] cannot be applied to enums",
            ))
        }
        syn::Data::Union(_) => {
            return Err(syn::Error::new(
                input.ident.span(),
                "figtree: #[derive(Figtree)] cannot be applied to unions",
            ))
        }
    };

    fields.iter().map(FieldConfig::from_field).collect()
}

// ── Generic arg helpers ───────────────────────────────────────────────────────

fn extract_first_generic_arg(segment: &syn::PathSegment) -> syn::Result<String> {
    match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => {
            let arg = args.args.first().ok_or_else(|| {
                syn::Error::new(segment.ident.span(), "expected a generic argument")
            })?;
            match arg {
                syn::GenericArgument::Type(Type::Path(tp)) => {
                    Ok(tp.path.segments.last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default())
                }
                _ => Err(syn::Error::new(
                    segment.ident.span(),
                    "figtree: generic argument must be a simple type path",
                )),
            }
        }
        _ => Err(syn::Error::new(
            segment.ident.span(),
            "figtree: expected angle-bracketed generic arguments",
        )),
    }
}

fn extract_map_generic_args(
    segment: &syn::PathSegment,
) -> syn::Result<(String, String)> {
    match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => {
            let mut iter = args.args.iter();

            let key = match iter.next() {
                Some(syn::GenericArgument::Type(Type::Path(tp))) => {
                    tp.path.segments.last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default()
                }
                _ => return Err(syn::Error::new(
                    segment.ident.span(),
                    "figtree: HashMap key must be a simple type",
                )),
            };

            let val = match iter.next() {
                Some(syn::GenericArgument::Type(Type::Path(tp))) => {
                    tp.path.segments.last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default()
                }
                _ => return Err(syn::Error::new(
                    segment.ident.span(),
                    "figtree: HashMap value must be a simple type",
                )),
            };

            Ok((key, val))
        }
        _ => Err(syn::Error::new(
            segment.ident.span(),
            "figtree: expected angle-bracketed generic arguments on HashMap",
        )),
    }
}
