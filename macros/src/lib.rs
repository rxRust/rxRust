use proc_macro::TokenStream;
use quote::quote;
use syn::{Ident, ItemFn, LitStr, parse_macro_input, spanned::Spanned};

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
  let input = parse_macro_input!(item as ItemFn);

  let is_async = input.sig.asyncness.is_some();

  let raw_args = proc_macro2::TokenStream::from(attr);
  let tokio_args = if raw_args.is_empty() {
    proc_macro2::TokenStream::new()
  } else {
    if !is_async {
      return TokenStream::from(
        syn::Error::new(
          raw_args.span(),
          "rxrust_macro::test flavor args are only supported for async tests. Use \
           #[rxrust_macro::test] for sync tests, or make the function async.",
        )
        .to_compile_error(),
      );
    }

    if let Ok(ident) = syn::parse2::<Ident>(raw_args.clone()) {
      match ident.to_string().as_str() {
        "local" => quote!(flavor = "local"),
        "shared" => quote!(flavor = "multi_thread"),
        _ => {
          return TokenStream::from(
            syn::Error::new(
              ident.span(),
              "rxrust_macro::test only accepts: #[rxrust_macro::test], \
               #[rxrust_macro::test(local)], #[rxrust_macro::test(shared)], or string equivalents",
            )
            .to_compile_error(),
          );
        }
      }
    } else if let Ok(lit) = syn::parse2::<LitStr>(raw_args.clone()) {
      match lit.value().as_str() {
        "local" => quote!(flavor = "local"),
        "shared" => quote!(flavor = "multi_thread"),
        _ => {
          return TokenStream::from(
            syn::Error::new(
              lit.span(),
              "rxrust_macro::test only accepts: \"local\" or \"shared\" when passing a string",
            )
            .to_compile_error(),
          );
        }
      }
    } else {
      return TokenStream::from(
        syn::Error::new(
          raw_args.span(),
          "rxrust_macro::test only accepts: #[rxrust_macro::test], #[rxrust_macro::test(local)], \
           #[rxrust_macro::test(shared)], or string equivalents",
        )
        .to_compile_error(),
      );
    }
  };

  let wasm_attr = if is_async {
    quote!(wasm_bindgen_test::wasm_bindgen_test(async))
  } else {
    quote!(wasm_bindgen_test::wasm_bindgen_test)
  };

  let native_attr = if is_async { quote!(tokio::test(#tokio_args)) } else { quote!(test) };

  let expanded = quote! {
      #[cfg_attr(target_arch = "wasm32", #wasm_attr)]
      #[cfg_attr(not(target_arch = "wasm32"), #native_attr)]
      #input
  };

  TokenStream::from(expanded)
}
