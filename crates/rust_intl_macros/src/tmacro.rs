//! `t!()` macro implementation.
//!
//! Supported call forms:
//!
//! - t!("key")
//! - t!("key", name = val)
//! - t!("key", locale = Locale::Fr)
//! - t!("key", locale = Locale::Fr, name = val)
//! - t!(lang, "key")
//! - t!(lang, "key", name = val)

use crate::loader::get_schema;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, LitStr, Token};

pub struct TCall {
    /// `Some(expr)` when the locale was passed positionally as the first arg.
    pub locale_pos: Option<Expr>,
    /// `Some(expr)` when `locale = <expr>` was given as a named arg.
    pub locale_named: Option<Expr>,
    pub key: LitStr,
    pub args: Vec<(Ident, Expr)>,
}

impl Parse for TCall {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut locale_pos: Option<Expr> = None;
        let mut locale_named: Option<Expr> = None;
        let key: LitStr;

        if input.peek(LitStr) {
            key = input.parse()?;
        } else {
            let expr: Expr = input.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    "expected either a string literal key or a locale expression:\n  \
                     t!(\"ns.key\", ...)\n  t!(lang, \"ns.key\", ...)",
                )
            })?;
            input.parse::<Token![,]>().map_err(|_| {
                syn::Error::new_spanned(
                    &expr,
                    "expected a comma and string literal key after the locale expression:\n  t!(lang, \"ns.key\", ...)",
                )
            })?;
            key = input.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    "expected a string literal key as second argument in t!(locale, \"key\", ...)",
                )
            })?;
            locale_pos = Some(expr);
        }

        let mut args: Vec<(Ident, Expr)> = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }

            let name: Ident = input.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    "expected `name = value` for a translation argument",
                )
            })?;
            input.parse::<Token![=]>()?;
            let expr: Expr = input.parse()?;

            if name == "locale" {
                if locale_pos.is_some() {
                    return Err(syn::Error::new(
                        name.span(),
                        "`locale = ...` is redundant when a locale is already given as the first positional argument to t!()",
                    ));
                }
                if locale_named.is_some() {
                    return Err(syn::Error::new(name.span(), "duplicate `locale` argument"));
                }
                locale_named = Some(expr);
            } else {
                args.push((name, expr));
            }
        }

        Ok(TCall {
            locale_pos,
            locale_named,
            key,
            args,
        })
    }
}

pub fn expand(call: TCall) -> TokenStream {
    let schema = match get_schema() {
        Ok(s) => s,
        Err(e) => {
            return syn::Error::new(call.key.span(), format!("rust_intl: {e}")).to_compile_error();
        }
    };

    let key_str = call.key.value();
    let info = match schema.key_info.get(&key_str) {
        Some(i) => i,
        None => {
            let msg = match schema.suggest(&key_str) {
                Some(s) => format!("unknown translation key '{key_str}'. Did you mean '{s}'?"),
                None => format!(
                    "unknown translation key '{key_str}' (no key in locales/{}/ matched)",
                    schema.default_locale
                ),
            };
            return syn::Error::new(call.key.span(), msg).to_compile_error();
        }
    };

    let provided: HashMap<String, &Expr> =
        call.args.iter().map(|(n, e)| (n.to_string(), e)).collect();

    if provided.len() != call.args.len() {
        return syn::Error::new(
            call.key.span(),
            format!("duplicate argument name in t!(\"{key_str}\", ...)"),
        )
        .to_compile_error();
    }

    let required: Vec<&str> = info.vars.iter().map(|(n, _)| n.as_str()).collect();
    let missing: Vec<&str> = required
        .iter()
        .filter(|n| !provided.contains_key(**n))
        .copied()
        .collect();
    let extra: Vec<&(Ident, Expr)> = call
        .args
        .iter()
        .filter(|(n, _)| !required.contains(&n.to_string().as_str()))
        .collect();

    if !missing.is_empty() || !extra.is_empty() {
        let mut msg = format!("translation key '{key_str}' argument mismatch.\n");
        if required.is_empty() {
            msg += "  this key takes no arguments.\n";
        } else {
            msg += &format!("  expected: {}\n", required.join(", "));
        }
        if !missing.is_empty() {
            msg += &format!("  missing:  {}\n", missing.join(", "));
        }
        if !extra.is_empty() {
            let names: Vec<String> = extra.iter().map(|(n, _)| n.to_string()).collect();
            msg += &format!("  extra:    {}\n", names.join(", "));
        }
        let span = extra
            .first()
            .map(|(n, _)| n.span())
            .unwrap_or_else(|| call.key.span());
        return syn::Error::new(span, msg).to_compile_error();
    }

    let locale_tok = match (&call.locale_pos, &call.locale_named) {
        // t!(lang, "key") -> use LocaleProvider
        (Some(e), _) => quote! { <_ as crate::LocaleProvider>::i18n_locale(&(#e)) },
        // t!("key", locale = expr) -> direct Locale expression
        (None, Some(e)) => quote! { #e },
        // t!("key") -> use the default locale
        (None, None) => quote! { get_current_locale!() },
    };

    let ordered_args: Vec<TokenStream> = info
        .vars
        .iter()
        .map(|(name, _)| {
            let e = provided[name];
            quote! { #e }
        })
        .collect();

    let fn_path = crate::codegen::crate_fn_path(&key_str);

    // t!() always returns String for a consistent return type
    if info.vars.is_empty() {
        quote! { ::std::string::ToString::to_string(#fn_path(#locale_tok)) }
    } else {
        quote! { #fn_path(#locale_tok, #(#ordered_args),*) }
    }
}
