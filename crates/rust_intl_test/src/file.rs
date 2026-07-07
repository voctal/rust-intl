use rust_intl::t;

use crate::{Locale, get_current_locale, set_current_locale};

pub fn other_file() {
    println!("Same but in another file");

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
}
