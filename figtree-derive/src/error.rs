use proc_macro2::TokenStream;
use syn::spanned::Spanned;

/// Converts a syn::Error into a TokenStream that emits a compile_error!
/// at the correct source location. Used throughout the macro to surface
/// attribute parsing errors with precise span information.
pub fn to_compile_error(error: syn::Error) -> TokenStream {
    error.to_compile_error()
}

/// Returns a syn::Error pointing at the given token.
/// Use when you want to propagate the error via ? rather than returning
/// a TokenStream directly.
pub fn syn_error<T: Spanned>(token: &T, message: &str) -> syn::Error {
    syn::Error::new(token.span(), message)
}
