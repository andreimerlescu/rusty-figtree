use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

/// Converts a syn::Error into a TokenStream that emits a compile_error!
/// at the correct source location. Used throughout the macro to surface
/// attribute parsing errors with precise span information.
pub fn to_compile_error(error: syn::Error) -> TokenStream {
    error.to_compile_error()
}

/// Emits a compile_error! pointing at the given token with the given message.
/// Use when you have a spanned token to point at.
pub fn spanned_error<T: Spanned>(token: &T, message: &str) -> TokenStream {
    syn::Error::new(token.span(), message).to_compile_error()
}

/// Emits a compile_error! at the call site with the given message.
/// Use when you do not have a specific token to point at.
pub fn call_site_error(message: &str) -> TokenStream {
    syn::Error::new(proc_macro2::Span::call_site(), message).to_compile_error()
}

/// Returns a syn::Error pointing at the given token.
/// Use when you want to propagate the error via ? rather than returning
/// a TokenStream directly.
pub fn syn_error<T: Spanned>(token: &T, message: &str) -> syn::Error {
    syn::Error::new(token.span(), message)
}
