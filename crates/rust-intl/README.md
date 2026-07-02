<div align="center">
    <h1>rust-intl</h1>
    <p>
        <a href="https://voctal.dev/discord"><img src="https://img.shields.io/discord/1336303640725553213?color=5865F2&logo=discord&logoColor=white" alt="Discord server" /></a>
        <a href="https://github.com/voctal/rust-intl/commits/main"><img alt="Last commit" src="https://img.shields.io/github/last-commit/voctal/rust-intl?logo=github&logoColor=ffffff" /></a>
    </p>
</div>

## About

Compile-time validated i18n library for Rust. It uses the [ICU4X](https://github.com/unicode-org/icu4x) crate from Unicode and follows some of the ICU MessageFormat syntax: `{name}`, `{count, plural, ...}`, `{gender, select, ...}`. Unknown keys, missing/extra arguments, wrong argument types, and missing `other` arms are compile errors.

> [!IMPORTANT]
> `rust-intl` is in development and is not ready for production use.

## Features

- `t!` macro
- `lang.t_<key>()` functions
- Compile-time validation:
    ```rs
    t!("no_such_key");                          // unknown translation key 'no_such_key'
    t!("common.greting", name = "typo");        // unknown translation key 'common.greting'. Did you mean 'common.greeting'?
    t!("common.greeting");                      // argument mismatch. expected: name, missing: name
    t!("common.greeting", name = "Foo", x = 1); // argument mismatch. expected: name, extra: x
    ```

## Setup

```rs
// main.rs
rust_intl::load!(); // scans ./locales/[locale]/[namespace].json by default
```

```rs
// build.rs (rebuild when translation files change)
fn main() {
    println!("cargo:rerun-if-changed=locales");
}
```

To load a different directory:

```rust
rust_intl::load!(dir = "./translations");
```

## Message syntax

```rs
"{name}"                                                           // interpolation, any Display type
"{count, plural, =0 {none} one {1 item} other {{count} items}}"    // CLDR plural; `other` required
"{n, selectordinal, one {1st} two {2nd} few {3rd} other {{n}th}}"  // ordinal plural
"{gender, select, male {He} female {She} other {They}}"            // `other` required
"{v, number}"                                                      // and {v, date} / {v, time}: plain Display interpolation
"Use '{name}' literally"                                           // '{...}' escapes braces as plain text
```

## Calling translations

There is two equivalent forms. `t!()` is a proc-macro so it can validate the key at compile time, `lang.t_key()` is a plain generated if you don't want the macro.

```rs
// t! macro
t!("common.greeting", name = "Foo");
t!("common.greeting", locale = Locale::Fr, name = "Foo");
t!(lang, "common.greeting", name = "Foo"); // lang implements LocaleProvider

// Methods
let lang = Lang::new(Locale::Fr);
lang.t_common_greeting("Foo");
```

The method name is the dotted key flattened with underscores and a `t_`, e.g. `common.status.active` becomes `t_common_status_active()`.

`t!()` always returns `String`. `lang.t_key()` returns `&'static str` for no-argument keys (zero allocation) and `String` for keys with arguments.

## Numbers

```rs
// ICU formatting, english: 1000 -> "1,000", french: 1000 -> "1 000"
t!("items", count = 1_000u32);

// custom display string
use rust_intl::NumberArg;
t!(lang, "items", count = NumberArg::with_display(1_000u32, "1K"));
```

## Namespacing a whole function

`#[t_ns(namespace = "...")]` prefixes every `t!()` call inside the function. Works on `async fn` too:

```rs
#[t_ns(namespace = "settings")]
async fn show_settings(lang: Lang) -> String {
    t!(lang, "title") // becomes "settings.title"
}
```

Prefix a key with `/` to bypass the namespace prefix:

```rs
t!(lang, "/common.static_text"); // becomes "common.static_text" anyway
```
