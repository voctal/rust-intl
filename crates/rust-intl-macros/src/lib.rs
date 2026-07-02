//! Proc-macro implementations.

mod attr;
mod codegen;
mod icu;
mod loader;
mod tmacro;

use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token, parse_macro_input};

struct LoadArgs {
    path: Option<String>,
    default: Option<String>,
}

impl Parse for LoadArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut path = None;
        let mut default = None;

        // Loop as long as there are arguments left to parse
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let val: LitStr = input.parse()?;

            if key == "path" {
                path = Some(val.value());
            } else if key == "default" {
                default = Some(val.value());
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    format!(
                        "unexpected argument `{key}`. rust_intl::load!() only accepts `path` and `default`.\n\
                         Example: load!(path = \"./my-locales\", default = \"en\")"
                    ),
                ));
            }

            // If there's a next argument, it must be preceded by a comma
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(LoadArgs { path, default })
    }
}

/// See `rust_intl::load!`.
#[proc_macro]
pub fn load(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as LoadArgs);

    let cfg = loader::LoadConfig {
        path: args.path,
        default_locale: args.default,
    };

    match loader::get_schema_with(cfg) {
        Ok(s) => codegen::generate(&s).into(),
        Err(e) => syn::Error::new(proc_macro2::Span::call_site(), format!("rust_intl: {e}"))
            .to_compile_error()
            .into(),
    }
}

/// See `rust_intl::t!`.
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    let call = parse_macro_input!(input as tmacro::TCall);
    tmacro::expand(call).into()
}

/// See `rust_intl::t_ns`.
#[proc_macro_attribute]
pub fn t_ns(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as attr::NsArgs);
    let func = parse_macro_input!(item as syn::ItemFn);
    attr::expand(args, func).into()
}
