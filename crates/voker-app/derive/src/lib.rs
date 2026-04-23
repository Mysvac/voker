use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, ItemFn, parse_macro_input};

/// Derives the `AppLabel` trait implementation.
///
/// # Required Traits
///
/// The target type must implement the following traits:
/// - `Clone`
/// - `Debug`
/// - `Hash`
/// - `Eq`
///
/// # Examples
///
/// ```ignore
/// #[derive(AppLabel)]
/// struct RenderApp;
/// ```
#[proc_macro_derive(AppLabel)]
pub fn derive_app_label(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let voker_app = voker_macro_utils::crate_path!(voker_app);
    let trait_path = quote! { #voker_app::AppLabel };

    voker_macro_utils::derive_label(ast, trait_path)
}

/// Generates the required main function boilerplate for Android.
#[proc_macro_attribute]
pub fn voker_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    assert_eq!(
        input.sig.ident, "main",
        "`voker_main` can only be used on a function called 'main'."
    );

    let voker_app = voker_macro_utils::crate_path!(voker_app);
    let android_activity = quote! { #voker_app::exports::android_activity::AndroidApp };
    let static_android_activity = quote! { #voker_app::exports::ANDROID_APP };

    TokenStream::from(quote! {
        #[unsafe(no_mangle)]
        #[cfg(target_os = "android")]
        fn android_main(android_app: #android_activity) {
            let _ = #static_android_activity.set(android_app);
            main();
        }

        #input
    })
}
