mod file;

use rust_intl::{NumberArg, t, t_ns};

use crate::file::other_file;

// Load locales
rust_intl::load!();

// Custom context for the tests
struct CustomContext {
    pub lang: Lang,
}
impl LocaleProvider for CustomContext {
    fn i18n_locale(&self) -> Locale {
        self.lang.locale()
    }
}
impl LocaleProvider for &CustomContext {
    fn i18n_locale(&self) -> Locale {
        self.lang.locale()
    }
}

// Tests

#[t_ns(namespace = "settings")]
async fn show_settings(lang: Lang) -> Vec<String> {
    vec![
        t!(lang, "title"),
        t!(lang, "saved", guild = "My Server"),
        t!(lang, "/common.static_text"),
    ]
}

fn handle(ctx: &CustomContext, count: u32) -> Vec<String> {
    let mut out = Vec::new();

    // t!() forms
    out.push(t!("common.static_text")); // Default local
    out.push(t!("common.static_text", locale = ctx.lang.locale()));
    out.push(t!(ctx, "common.greeting", name = "Ada")); // LocaleProvider
    out.push(t!(ctx.lang, "common.greeting", name = "Ada")); // Lang
    out.push(t!(Locale::En, "common.greeting", name = "hardcoded EN"));

    // numbers, icu formatting
    out.push(t!(ctx, "common.status.active", count = count));

    // numbers, custom formatting
    out.push(t!(
        ctx,
        "common.status.active",
        count = NumberArg::with_display(count, format!("{count} star"))
    ));

    out.push(t!(ctx, "common.farewell", name = "Ada", days = 3u32));
    out.push(t!(
        ctx,
        "settings.role.selected",
        gender = "female",
        role = "Mod"
    ));
    out.push(t!(
        ctx,
        "errors.missing_permission",
        permission = "MANAGE_ROLES"
    ));

    // lang.t_<key>() form
    out.push(ctx.lang.t_common_static_text().to_string());
    out.push(ctx.lang.t_common_greeting("Ada"));
    out.push(ctx.lang.t_common_status_active(count));

    // custom display via the method form too
    out.push(
        ctx.lang
            .t_common_status_active(NumberArg::with_display(count, "unlimited")),
    );

    out.push(ctx.lang.t_errors_not_found().to_string());

    out
}

fn print_locale(label: &str, ctx: &CustomContext, count: u32) {
    println!("{label}");
    for s in handle(ctx, count) {
        println!("  {s}");
    }
}

async fn print_settings_demo(en: &CustomContext, fr: &CustomContext) {
    println!("async + t_ns + / escape");
    for s in show_settings(en.lang).await {
        println!("  {s}");
    }
    println!("  ---");
    for s in show_settings(fr.lang).await {
        println!("  {s}");
    }
}

fn print_number_formatting() {
    println!("Number formatting");
    println!(
        "  en 1000 = {}",
        NumberArg::new(1_000i32).format_display("en")
    );
    println!(
        "  fr 1000 = {}",
        NumberArg::new(1_000i32).format_display("fr")
    );
    println!(
        "  de 1000 = {}",
        NumberArg::new(1_000i32).format_display("de")
    );
    println!(
        "  custom  = {}",
        NumberArg::with_display(1_000i32, "1K").format_display("en")
    );
}

fn print_locale_metadata() {
    println!("Locale metadata");
    for l in Locale::ALL {
        println!("  {} = {:?}", l.code(), l);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let en = CustomContext {
        lang: Lang::new(Locale::En),
    };
    let fr = CustomContext {
        lang: Lang::new(Locale::Fr),
    };

    print_locale("English", &en, 1_000);
    println!(
        "  zero = {}",
        t!(Locale::En, "common.status.active", count = 0u32)
    );

    println!();
    print_locale("French", &fr, 1_000);

    println!();
    print_settings_demo(&en, &fr).await;

    println!();
    print_number_formatting();

    println!();
    print_locale_metadata();

    // Below should cause compile time errors, uncomment to see
    // t!("no_such_key");
    // t!("common.greting", name = "typo");    // suggests "greeting"
    // t!("common.greeting");                  // missing `name`
    // t!("common.greeting", name = "Ada", x = 1); // unexpected `x`
    // t!(ctx.lang, "key", locale = Locale::En);   // locale= redundant with positional

    println!("current_locale macros testing:");

    assert_eq!(get_current_locale(), Locale::DEFAULT);
    println!(
        "{} {}",
        get_current_locale(),
        t!("common.greeting", name = "someone")
    );
    set_current_locale(Locale::Fr);
    assert_eq!(get_current_locale(), Locale::Fr);
    println!(
        "{} {}",
        get_current_locale(),
        t!("common.greeting", name = "someone")
    );
    set_current_locale(Locale::DEFAULT);

    other_file();
}
