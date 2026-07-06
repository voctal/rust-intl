use crate::icu::{ArmKey, AstNode, VarKind};
use crate::loader::Schema;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

// Identifier helpers

pub fn sanitize_ident(s: &str) -> Ident {
    let mut out = String::new();
    let mut last_sep = false;
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
            last_sep = false;
        } else if !last_sep {
            out.push('_');
            last_sep = true;
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    if out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    match out.as_str() {
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern" | "false"
        | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod" | "move"
        | "mut" | "pub" | "ref" | "return" | "self" | "Self" | "static" | "struct" | "super"
        | "trait" | "true" | "type" | "unsafe" | "use" | "where" | "while" | "async" | "await"
        | "dyn" | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override"
        | "priv" | "typeof" | "unsized" | "virtual" | "yield" | "try" => {
            format_ident!("r#{}", out)
        }
        _ => format_ident!("{}", out),
    }
}

fn locale_variant_ident(locale: &str) -> Ident {
    let mut out = String::new();
    let mut cap = true;
    for c in locale.chars() {
        if c == '-' || c == '_' {
            cap = true;
            continue;
        }
        if cap {
            out.extend(c.to_uppercase());
            cap = false;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    format_ident!("{}", out)
}

fn flat_method_ident(key: &str) -> Ident {
    sanitize_ident(&format!("t_{}", key.replace('.', "_")))
}

fn key_segments(key: &str) -> (Vec<&str>, &str) {
    let segs: Vec<&str> = key.split('.').collect();
    let (mods, leaf) = segs.split_at(segs.len() - 1);
    (mods.to_vec(), leaf[0])
}

// Module tree

#[derive(Default)]
struct ModuleTree {
    children: std::collections::BTreeMap<String, ModuleTree>,
    funcs: Vec<TokenStream>,
}

impl ModuleTree {
    fn insert(&mut self, path: &[&str], func: TokenStream) {
        match path.first() {
            None => self.funcs.push(func),
            Some(seg) => self
                .children
                .entry(seg.to_string())
                .or_default()
                .insert(&path[1..], func),
        }
    }
    fn emit(&self) -> TokenStream {
        let funcs = &self.funcs;
        let children = self.children.iter().map(|(name, child)| {
            let ident = sanitize_ident(name);
            let inner = child.emit();
            quote! { #[allow(non_snake_case)] pub mod #ident { use super::Locale; #inner } }
        });
        quote! { #(#funcs)* #(#children)* }
    }
}

fn inner_fn_path(mod_path: &[&str], leaf: &str) -> TokenStream {
    let mods = mod_path.iter().map(|s| sanitize_ident(s));
    let leaf = sanitize_ident(leaf);
    if mod_path.is_empty() {
        quote! { translations::#leaf }
    } else {
        quote! { translations::#(#mods::)* #leaf }
    }
}

pub fn crate_fn_path(key: &str) -> TokenStream {
    let (mods, leaf) = key_segments(key);
    let mods = mods.iter().map(|s| sanitize_ident(s));
    let leaf = sanitize_ident(leaf);
    quote! { crate::translations::#(#mods::)* #leaf }
}

// AST to tokens

fn category_tok(c: &str) -> TokenStream {
    match c {
        "zero" => quote! { ::rust_intl::runtime::Category::Zero },
        "one" => quote! { ::rust_intl::runtime::Category::One },
        "two" => quote! { ::rust_intl::runtime::Category::Two },
        "few" => quote! { ::rust_intl::runtime::Category::Few },
        "many" => quote! { ::rust_intl::runtime::Category::Many },
        _ => quote! { ::rust_intl::runtime::Category::Other },
    }
}

fn arm_key_tok(k: &ArmKey) -> TokenStream {
    match k {
        ArmKey::Exact(n) => quote! { ::rust_intl::runtime::PluralArm::Exact(#n) },
        ArmKey::Category(c) => {
            let cat = category_tok(c);
            quote! { ::rust_intl::runtime::PluralArm::Category(#cat) }
        }
    }
}

fn nodes_tok(nodes: &[AstNode]) -> TokenStream {
    let items = nodes.iter().map(node_tok);
    quote! { &[ #(#items),* ] }
}

fn node_tok(node: &AstNode) -> TokenStream {
    match node {
        AstNode::Text(s) => quote! { ::rust_intl::runtime::Node::Text(#s) },
        AstNode::Var(n) => quote! { ::rust_intl::runtime::Node::Var(#n) },
        AstNode::Plural { var, ordinal, arms } => {
            let arms = arms.iter().map(|(k, body)| {
                let k = arm_key_tok(k);
                let b = nodes_tok(body);
                quote! { (#k, #b) }
            });
            quote! { ::rust_intl::runtime::Node::Plural { var: #var, ordinal: #ordinal, arms: &[ #(#arms),* ] } }
        }
        AstNode::Select { var, arms } => {
            let arms = arms.iter().map(|(k, body)| {
                let b = nodes_tok(body);
                quote! { (#k, #b) }
            });
            quote! { ::rust_intl::runtime::Node::Select { var: #var, arms: &[ #(#arms),* ] } }
        }
    }
}

// Param helpers.
//
// For VarKind::Number the generated function accepts `impl Into<NumberArg>`
// For VarKind::Text the generated function accepts `impl Display` and
// formats inline inside the ctx slice literal.

/// Returns `(param_tokens, binding_tokens, ctx_entry_tokens)`.
/// `binding_tokens` must be emitted as a statement before the ctx slice.
fn param_binding_ctx(name: &str, kind: VarKind) -> (TokenStream, TokenStream, TokenStream) {
    let ident = sanitize_ident(name);
    match kind {
        VarKind::Number => (
            quote! { #ident: impl Into<::rust_intl::runtime::NumberArg> },
            quote! { let #ident: ::rust_intl::runtime::NumberArg = #ident.into(); },
            quote! { (#name, ::rust_intl::runtime::Value::Number(&#ident)) },
        ),
        VarKind::Text => (
            quote! { #ident: impl ::std::fmt::Display },
            quote! {},
            quote! { (#name, ::rust_intl::runtime::Value::String(&::std::format!("{}", #ident))) },
        ),
    }
}

// Main entry point

pub fn generate(schema: &Schema) -> TokenStream {
    let locale_idents: Vec<Ident> = schema
        .locales
        .iter()
        .map(|l| locale_variant_ident(l))
        .collect();
    let default_ident = locale_variant_ident(&schema.default_locale);
    let locale_codes = &schema.locales;

    let file_tracking = schema.files.iter().enumerate().map(|(i, f)| {
        let path = f.to_string_lossy().to_string();
        let name = format_ident!("__I18N_TRACK_{i}");
        quote! { const #name: &[u8] = include_bytes!(#path); }
    });

    let locale_enum = quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Locale {
            #(#locale_idents,)*
        }

        impl Locale {
            pub const ALL: &'static [Locale] = &[ #(Locale::#locale_idents,)* ];
            pub const DEFAULT: Locale = Locale::#default_ident;

            pub fn code(&self) -> &'static str {
                match self { #(Locale::#locale_idents => #locale_codes,)* }
            }
            pub fn from_code(code: &str) -> ::std::option::Option<Locale> {
                match code {
                    #(#locale_codes => ::std::option::Option::Some(Locale::#locale_idents),)*
                    _ => ::std::option::Option::None,
                }
            }
        }

        impl ::std::default::Default for Locale {
            fn default() -> Self { Locale::DEFAULT }
        }
        impl ::std::fmt::Display for Locale {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.code())
            }
        }
    };

    let current_locale_macros = quote! {
        /// Returns the current [`Locale`], as last set by
        /// [`set_current_locale!`], or [`Locale::DEFAULT`] if it was never set.
        ///
        /// It is mostly meant for single-user apps (desktop, CLI, ...) where only one
        /// locale is active at a time.
        ///
        /// It returns a `Locale` by value, so it's cheap to call,
        /// and thread-safe.
        ///
        /// ```ignore
        /// set_current_locale!(Locale::Fr);
        /// assert_eq!(get_current_locale!(), Locale::Fr);
        /// ```
        #[macro_export]
        macro_rules! get_current_locale {
            () => {
                ::rust_intl::runtime::get_current_locale::<Locale>()
            };
        }

        /// Sets the current [`Locale`]. See
        /// [`get_current_locale!`].
        #[macro_export]
        macro_rules! set_current_locale {
            ($locale:expr) => {
                ::rust_intl::runtime::set_current_locale::<Locale>($locale)
            };
        }
    };

    let locale_provider = quote! {
        /// Implemented by anything that can return a [`Locale`].
        ///
        /// `Locale` and [`Lang`] implement this automatically. Implement it for
        /// your own app context types to use `t!(ctx, "key", ...)` without
        /// extracting the locale first:
        ///
        /// ```ignore
        /// impl LocaleProvider for CommandCtx {
        ///     fn i18n_locale(&self) -> Locale { self.lang.locale() }
        /// }
        /// t!(ctx, "common.greeting", name = "Foo")
        /// ```
        pub trait LocaleProvider {
            fn i18n_locale(&self) -> Locale;
        }

        impl LocaleProvider for Locale  { fn i18n_locale(&self) -> Locale { *self  } }
        impl LocaleProvider for &Locale { fn i18n_locale(&self) -> Locale { **self } }
        impl LocaleProvider for Lang    { fn i18n_locale(&self) -> Locale { self.locale } }
        impl LocaleProvider for &Lang   { fn i18n_locale(&self) -> Locale { self.locale } }
    };

    let lang_struct = quote! {
        /// A thin `{ locale: Locale }` wrapper generated alongside the
        /// [`Locale`] enum by [`rust_intl::load!`].
        ///
        /// Every translation key is available as a `t_{method}`, e.g.
        /// `lang.t_common_greeting()`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct Lang {
            locale: Locale,
        }

        impl Lang {
            pub fn new(locale: Locale) -> Self { Self { locale } }
            pub fn locale(&self) -> Locale { self.locale }
            pub fn set_locale(&mut self, l: Locale) { self.locale = l; }
        }
        impl ::std::default::Default for Lang {
            fn default() -> Self { Self { locale: Locale::DEFAULT } }
        }
        impl ::std::fmt::Display for Lang {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.locale, f)
            }
        }
    };

    let mut mod_tree = ModuleTree::default();
    let mut locale_methods: Vec<TokenStream> = Vec::new();
    let mut lang_methods: Vec<TokenStream> = Vec::new();

    for key in &schema.keys {
        let info = &schema.key_info[key];
        let (mods, leaf) = key_segments(key);
        let leaf_ident = sanitize_ident(leaf);
        let method_ident = flat_method_ident(key);
        let inner_path = inner_fn_path(&mods, leaf);

        let arms_per_locale = schema.locales.iter().map(|locale| {
            let lident = locale_variant_ident(locale);
            let toks = nodes_tok(schema.ast_for(locale, key));
            quote! { Locale::#lident => #toks }
        });

        if info.vars.is_empty() {
            // &'static str = zero allocation
            let texts = schema.locales.iter().map(|locale| {
                let lident = locale_variant_ident(locale);
                let text: String = schema
                    .ast_for(locale, key)
                    .iter()
                    .map(|n| match n {
                        AstNode::Text(s) => s.as_str(),
                        _ => "",
                    })
                    .collect();
                quote! { Locale::#lident => #text }
            });
            mod_tree.insert(
                &mods,
                quote! {
                    pub fn #leaf_ident(locale: Locale) -> &'static str {
                        match locale { #(#texts,)* }
                    }
                },
            );
            locale_methods.push(quote! {
                pub fn #method_ident(&self) -> &'static str { #inner_path(*self) }
            });
            lang_methods.push(quote! {
                pub fn #method_ident(&self) -> &'static str { #inner_path(self.locale) }
            });
        } else {
            // String
            let triples: Vec<_> = info
                .vars
                .iter()
                .map(|(n, k)| param_binding_ctx(n, *k))
                .collect();

            let params: Vec<&TokenStream> = triples.iter().map(|(p, _, _)| p).collect();
            let bindings: Vec<&TokenStream> = triples.iter().map(|(_, b, _)| b).collect();
            let ctx_entries: Vec<&TokenStream> = triples.iter().map(|(_, _, c)| c).collect();
            let arg_idents: Vec<Ident> = info.vars.iter().map(|(n, _)| sanitize_ident(n)).collect();

            mod_tree.insert(
                &mods,
                quote! {
                    pub fn #leaf_ident(locale: Locale, #(#params),*) -> ::std::string::String {
                        #(#bindings)*
                        let nodes: &[::rust_intl::runtime::Node] = match locale {
                            #(#arms_per_locale,)*
                        };
                        ::rust_intl::runtime::render(locale.code(), nodes, &[ #(#ctx_entries),* ])
                    }
                },
            );

            locale_methods.push(quote! {
                pub fn #method_ident(&self, #(#params),*) -> ::std::string::String {
                    #inner_path(*self, #(#arg_idents),*)
                }
            });
            lang_methods.push(quote! {
                pub fn #method_ident(&self, #(#params),*) -> ::std::string::String {
                    #inner_path(self.locale, #(#arg_idents),*)
                }
            });
        }
    }

    let tree_body = mod_tree.emit();

    quote! {
        #(#file_tracking)*
        #locale_enum
        #current_locale_macros
        #locale_provider
        #lang_struct

        #[allow(non_snake_case, clippy::all)]
        pub mod translations {
            use super::Locale;
            #tree_body
        }

        #[allow(dead_code, clippy::all)]
        impl Locale { #(#locale_methods)* }

        #[allow(dead_code, clippy::all)]
        impl Lang { #(#lang_methods)* }
    }
}
