use std::fmt;

use fixed_decimal::Decimal;
use icu_decimal::DecimalFormatter;

#[derive(Clone, Copy, Debug)]
pub enum Number {
    Int(i64),
    UInt(u64),
    Float(f64),
}

impl Number {
    pub fn as_f64(&self) -> f64 {
        match self {
            Number::Int(v) => *v as f64,
            Number::UInt(v) => *v as f64,
            Number::Float(v) => *v,
        }
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Number::Int(v) => write!(f, "{v}"),
            Number::UInt(v) => write!(f, "{v}"),
            Number::Float(v) => write!(f, "{v}"),
        }
    }
}

macro_rules! impl_from_int  { ($($t:ty),*) => { $(impl From<$t> for Number { fn from(v: $t) -> Self { Number::Int(v as i64)  } })* }; }
macro_rules! impl_from_uint { ($($t:ty),*) => { $(impl From<$t> for Number { fn from(v: $t) -> Self { Number::UInt(v as u64) } })* }; }
impl_from_int!(i8, i16, i32, i64, isize);
impl_from_uint!(u8, u16, u32, u64, usize);
impl From<f32> for Number {
    fn from(v: f32) -> Self {
        Number::Float(v as f64)
    }
}
impl From<f64> for Number {
    fn from(v: f64) -> Self {
        Number::Float(v)
    }
}

/// A numeric argument for keys containing `{n, plural, ...}` or `{n, number}`.
///
/// By default the number is formatted by ICU4X decimal formatter
/// (e.g. `1000` becomes `"1,000"` in `en`, `"1 000"` in `fr`).
/// Use [`NumberArgument::with_display`] to use a custom formatting.
///
/// ```ignore
/// // Auto format using the locale
/// t!(lang, "items", count = 1_000u32)
///
/// // Custom formatting
/// use rust_intl::NumberArgument;
/// let custom = custom_formatter(1_000_000);
/// t!(lang, "items", count = NumberArgument::with_display(1_000_000u64, custom))
/// ```
pub struct NumberArg {
    /// Raw value.
    pub value: Number,
    /// Custom display instead of ICU format.
    display: Option<String>,
}

impl NumberArg {
    pub fn new(n: impl Into<Number>) -> Self {
        Self {
            value: n.into(),
            display: None,
        }
    }

    /// Construct with a custom formatted display string. `n` is still used for
    /// plural-category resolution.
    pub fn with_display(n: impl Into<Number>, display: impl Into<String>) -> Self {
        Self {
            value: n.into(),
            display: Some(display.into()),
        }
    }

    /// Format the number for display in the given locale.
    pub fn format_display(&self, locale_code: &str) -> String {
        match &self.display {
            Some(s) => s.clone(),
            None => format_number_icu(locale_code, &self.value),
        }
    }
}

impl<T: Into<Number>> From<T> for NumberArg {
    fn from(v: T) -> Self {
        Self::new(v)
    }
}

fn format_number_icu(locale_code: &str, n: &Number) -> String {
    let locale = super::parse_icu_locale(locale_code);
    let Ok(fmt) = DecimalFormatter::try_new(locale.into(), Default::default()) else {
        return format!("{n}");
    };

    match n {
        Number::Int(v) => fmt.format(&Decimal::from(*v)).to_string(),
        Number::UInt(v) => fmt.format(&Decimal::from(*v)).to_string(),
        Number::Float(v) => {
            // Parse via the decimal string to keep fraction digits
            let s = format!("{v}");
            match s.parse::<Decimal>() {
                Ok(d) => fmt.format(&d).to_string(),
                Err(_) => s,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_auto_format_en() {
        assert_eq!(NumberArg::new(1000i64).format_display("en"), "1,000");
    }

    #[test]
    fn number_auto_format_fr() {
        let s = NumberArg::new(1000i64).format_display("fr");
        // French uses a narrow no-break space (U+202F) as thousands separator
        assert!(s.contains('1') && s.contains("000"), "got: {s:?}");
        assert_ne!(s, "1000", "should be locale-formatted, not plain");
    }

    #[test]
    fn number_auto_format_de() {
        let s = NumberArg::new(1000i64).format_display("de");
        // German uses a dot as thousands separator
        assert!(s.contains('1') && s.contains("000"), "got: {s:?}");
    }

    #[test]
    fn number_custom_display() {
        let n = NumberArg::with_display(1_000_000i64, "1M");
        assert_eq!(n.format_display("en"), "1M");
        assert_eq!(n.format_display("fr"), "1M");
    }

    #[test]
    fn number_arg_from_primitives() {
        let _: NumberArg = 5i32.into();
        let _: NumberArg = 5u64.into();
        let _: NumberArg = core::f64::consts::PI.into();
    }
}
