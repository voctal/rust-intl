//! Handle the t! rewrites to add namespace & remove '/'.
//!
//! Note: we cannot use `syn::fold::Fold` because it does not recurse into the
//! `TokenStream` bodies of macro invocations (e.g. `vec![t!(...)]`).

use proc_macro2::{Group, Spacing, Span, TokenStream, TokenTree};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemFn, LitStr, Token};

pub struct NsArgs {
    pub namespace: String,
}

impl Parse for NsArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "expected `#[t_ns(namespace = \"...\")]`"))?;
        if ident != "namespace" {
            return Err(syn::Error::new(
                ident.span(),
                "expected `namespace`, e.g. `#[t_ns(namespace = \"commands.ping\")]`",
            ));
        }
        input.parse::<Token![=]>()?;
        let lit: LitStr = input.parse()?;
        Ok(NsArgs {
            namespace: lit.value(),
        })
    }
}

/// Rewrite every `t!(...)` invocation found at any nesting
/// level, including inside `vec![...]`, `match { ... }`, `if`, async
/// blocks, etc.
fn rewrite_tokens(ts: TokenStream, namespace: &str) -> Result<TokenStream, syn::Error> {
    let mut out: Vec<TokenTree> = Vec::new();
    let mut iter = ts.into_iter().peekable();

    while let Some(tt) = iter.next() {
        match tt {
            TokenTree::Ident(ref id) if id == "t" => {
                let is_bang =
                    matches!(iter.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '!');
                if is_bang {
                    let bang = iter.next().unwrap();
                    let is_group = matches!(iter.peek(), Some(TokenTree::Group(_)));
                    if is_group {
                        let group_tt = iter.next().unwrap();
                        if let TokenTree::Group(group) = group_tt {
                            let rewritten = rewrite_key(group.stream(), namespace, group.span())?;
                            let mut ng = Group::new(group.delimiter(), rewritten);
                            ng.set_span(group.span());
                            out.push(tt); // `t`
                            out.push(bang); // `!`
                            out.push(TokenTree::Group(ng));
                            continue;
                        }
                    }
                    // Not a macro call (e.g. `t!` as a value somehow)
                    out.push(tt);
                    out.push(bang);
                } else {
                    out.push(tt);
                }
            }

            // Recurse into every Group
            TokenTree::Group(group) => {
                let rewritten = rewrite_tokens(group.stream(), namespace)?;
                let mut ng = Group::new(group.delimiter(), rewritten);
                ng.set_span(group.span());
                out.push(TokenTree::Group(ng));
            }

            other => out.push(other),
        }
    }

    Ok(out.into_iter().collect())
}

/// Rewrite the inner token stream of a `t!(...)`, to add the
/// namespace to the key or to strip the `/` for absolute keys.
fn rewrite_key(
    tokens: TokenStream,
    namespace: &str,
    span: Span,
) -> Result<TokenStream, syn::Error> {
    let segs = split_top_commas(tokens);

    if segs.is_empty() {
        return Err(syn::Error::new(
            span,
            "t_ns: expected t!(\"key\", ...) or t!(locale, \"key\", ...) inside the attributed function",
        ));
    }

    // to find the key: t!("key", ...) -> seg[0]; t!(locale, "key", ...) -> seg[1].
    let key_idx = if is_str_lit_stream(&segs[0]) { 0 } else { 1 };

    if key_idx >= segs.len() {
        return Err(syn::Error::new(
            span,
            "t_ns: could not locate the key string literal inside t!(locale, \"key\", ...)",
        ));
    }

    let key_lit: LitStr = syn::parse2(segs[key_idx].clone()).map_err(|_| {
        syn::Error::new(span, "t_ns: the translation key must be a plain string literal (e.g. t!(\"title\") or t!(lang, \"title\"))")
    })?;

    let key_str = key_lit.value();

    let new_key = if let Some(abs) = key_str.strip_prefix('/') {
        // Absolute key
        abs.to_string()
    } else {
        format!("{namespace}.{key_str}")
    };

    let new_lit = LitStr::new(&new_key, key_lit.span());

    let mut rebuilt: Vec<TokenTree> = Vec::new();
    for (i, seg) in segs.into_iter().enumerate() {
        if i > 0 {
            rebuilt.push(TokenTree::Punct(proc_macro2::Punct::new(
                ',',
                Spacing::Alone,
            )));
        }
        if i == key_idx {
            rebuilt.extend(quote::quote! { #new_lit });
        } else {
            rebuilt.extend(seg);
        }
    }
    Ok(rebuilt.into_iter().collect())
}

/// Split on top-level commas only (not inside nested groups)
fn split_top_commas(ts: TokenStream) -> Vec<TokenStream> {
    let mut segs = Vec::new();
    let mut cur = TokenStream::new();
    for tt in ts {
        match &tt {
            TokenTree::Punct(p) if p.as_char() == ',' && p.spacing() == Spacing::Alone => {
                segs.push(std::mem::take(&mut cur));
            }
            _ => cur.extend(std::iter::once(tt)),
        }
    }
    segs.push(cur);
    segs
}

/// Returns `true` if the first real token is a string literal. (possibly whitespace skipped)
fn is_str_lit_stream(ts: &TokenStream) -> bool {
    ts.clone()
        .into_iter()
        .next()
        .map(|tt| match tt {
            TokenTree::Literal(lit) => {
                let s = lit.to_string();
                s.starts_with('"') || s.starts_with('r')
            }
            _ => false,
        })
        .unwrap_or(false)
}

pub fn expand(args: NsArgs, func: ItemFn) -> TokenStream {
    let func_tokens = quote::quote! { #func };
    match rewrite_tokens(func_tokens, &args.namespace) {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error(),
    }
}
