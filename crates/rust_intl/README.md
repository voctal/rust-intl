<div align="center">
    <img src="docs/images/ferris.png" width="152" alt="Ferris">
    <h1>rust_intl</h1>
    <p>
        <a href="https://voctal.dev/discord"><img src="https://img.shields.io/discord/1336303640725553213?color=5865F2&logo=discord&logoColor=white" alt="Discord server" /></a>
        <a href="https://github.com/voctal/rust-intl/commits/main"><img alt="Last commit" src="https://img.shields.io/github/last-commit/voctal/rust-intl?logo=github&logoColor=ffffff" /></a>
    </p>
</div>

## About

Compile-time validated i18n library for Rust. It uses the [ICU4X](https://github.com/unicode-org/icu4x) crate from Unicode and follows some of the ICU MessageFormat syntax: `{name}`, `{count, plural, ...}`, `{gender, select, ...}`. Unknown keys, missing/extra arguments, wrong argument types, and missing `other` arms are compile errors.

> [!IMPORTANT]
> `rust_intl` is still in development but has been tested in production apps. We plan to support more of the ICU message syntax and allow other locales file formats (e.g. yml, toml) and backend customization (e.g. translations loaded/fetched at startup with no compile-time validation).

## Features

- `t!` macro with compile-time validation
- Typesafe auto-generated functions (`t_your_key(args)`)
- Function namespacing with `#[t_ns(namespace = "prefix")]`
- Global locale for single-user apps: `set_current_locale(Locale::Fr)`
- Partial support of ICU Message Syntax

## Installation

```sh
cargo add rust_intl
```

## Usage

The `load!` macro will generate many types in your crates, including `Locale`, `Lang`, `get_current_locale`, `set_current_locale`, etc.

```rust,ignore
// main.rs

// loads locales/[locale]/[namespace].json by default
rust_intl::load!(default = "en");
```

```rust,ignore
// build.rs
fn main() {
    println!("cargo:rerun-if-changed=locales");
}
```

To load a different directory:

```rust,ignore
rust_intl::load!(path = "./translations");
```

## Translations

### t! macro

The main way to translate is using the `t!` macro. The macro always returns a `String`.

```rust,ignore
// Using the current or default locale
t!("common.greeting", name = "Foo");

// You can override the locale with "locale = ..."
t!("key", locale = Locale::Fr);
// or
t!(Locale::Fr, "key");
```

You can also use any type that implement LocaleProvider as the first argument:

```rust,ignore
struct MyContext {
    locale: Locale,
}
impl LocaleProvider for MyContext {
    fn i18n_locale(&self) -> crate::Locale {
        self.locale
    }
}

fn my_function(ctx: MyContext) -> String {
    // `ctx` as the first arg, to use ctx.locale as the locale
    t!(ctx, "common.greeting", name = "Foo")
}
```

### get_current_locale and set_current_locale

For single-user apps, like desktop apps, usually only one locale can be used at a time so there is no need to use `t!(ctx, "key")` everywhere.

Instead, you can use

```rust,ignore
// Change the "global" locale for every t! that dont have "locale = ..."
set_current_locale(Locale::Fr);

t!("key"); // will be in fr

// Get the locale if needed
let locale = get_current_locale();
```

### Namespacing a whole function

`#[t_ns(namespace = "...")]` prefixes every `t!()` call inside the function. Works on `async fn` too. Example:

Instead of:

```rust,ignore
async fn some_function(locale: Locale) {
    t!(locale, "common.informations.menu.errors.key1");
    t!(locale, "common.informations.menu.errors.key2");
    t!(locale, "common.informations.menu.errors.key3");
}
```

You can use:

```rust,ignore
#[t_ns(namespace = "common.informations.menu.errors")]
async fn some_function(locale: Locale) {
    t!(locale, "key1");
    t!(locale, "key2");
    t!(locale, "key3");
}
```

However if you need keys from outside the prefix, you will need `/` to start back at the root:

```rust,ignore
t!(lang, "/common.another_path.key"); // -> "common.another_path.key"
```

### Lang & auto-generated functions

Every keys from your translations are also translated into static functions on `Locale` & `Lang`.

```rust,ignore
t!("hello.world");
Locale::Fr.t_hello_world(); // all keys are prefixed with t_
```

A `Lang` struct is also available:

```rust,ignore
let lang = Lang::new(Locale::Fr);
lang.t_hello_world_with_args("Foo");
```

`lang.t_key()` returns `&'static str` for no-argument keys (zero allocation) and `String` for keys with arguments.

## Message syntax

```rust,ignore
"{name}"                                                           // interpolation, any Display type
"{count, plural, =0 {none} one {1 item} other {{count} items}}"    // CLDR plural; `other` required
"{n, selectordinal, one {1st} two {2nd} few {3rd} other {{n}th}}"  // ordinal plural
"{gender, select, male {He} female {She} other {They}}"            // `other` required
"{v, number}"                                                      // and {v, date} / {v, time}: plain Display interpolation
"Use {{name}} literally"                                           // escapes braces as plain text
```

## Numbers

```rust,ignore
// ICU formatting, english: 1000 -> "1,000", french: 1000 -> "1 000"
t!("items", count = 1_000u32);

// custom display string
use rust_intl::NumberArg;
t!(lang, "items", count = NumberArg::with_display(1_000u32, "1K"));
```
