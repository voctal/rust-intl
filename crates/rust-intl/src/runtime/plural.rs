use fixed_decimal::Decimal;
use icu_plurals::{PluralOperands, PluralRuleType, PluralRules};

use super::Number;

/// Plural category. Mirrors ICU4X PluralCategory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

fn icu_to_category(cat: icu_plurals::PluralCategory) -> Category {
    use icu_plurals::PluralCategory as P;
    match cat {
        P::Zero => Category::Zero,
        P::One => Category::One,
        P::Two => Category::Two,
        P::Few => Category::Few,
        P::Many => Category::Many,
        P::Other => Category::Other,
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PluralArm {
    Exact(i64),
    Category(Category),
}

/// Resolves the ICU plural category for `n` in `locale_code`, using
/// cardinal rules by default or ordinal rules when `ordinal` is `true`.
/// Falls back to `Category::Other` if the locale's plural rules can't be
/// loaded.
pub(crate) fn plural_category(locale_code: &str, n: &Number, ordinal: bool) -> Category {
    let locale = super::parse_icu_locale(locale_code);
    let rule_type = if ordinal {
        PluralRuleType::Ordinal
    } else {
        PluralRuleType::Cardinal
    };
    let Ok(rules) = PluralRules::try_new(locale.into(), rule_type.into()) else {
        return Category::Other;
    };

    let ops: PluralOperands = match n {
        Number::Int(v) => PluralOperands::from(*v),
        Number::UInt(v) => PluralOperands::from(*v),
        Number::Float(v) => {
            let s = format!("{v}");
            let decimal = s.parse::<Decimal>().unwrap_or_else(|_| Decimal::from(0));
            PluralOperands::from(&decimal)
        }
    };

    let cat = rules.category_for(ops);
    icu_to_category(cat)
}

/// Picks the arm matching `n`. In order, it tries:
///
/// - exact value match
/// - matching plural category
/// - `other` fallback
pub(crate) fn pick_plural_arm<'n>(
    arms: &'n [(PluralArm, &'static [super::Node])],
    n: &Number,
    cat: Category,
) -> Option<&'n [super::Node]> {
    let exact = n.as_f64();
    arms.iter()
        .find(|(arm, _)| matches!(arm, PluralArm::Exact(v) if *v as f64 == exact))
        .or_else(|| {
            arms.iter()
                .find(|(arm, _)| matches!(arm, PluralArm::Category(c) if *c == cat))
        })
        .or_else(|| {
            arms.iter()
                .find(|(arm, _)| matches!(arm, PluralArm::Category(Category::Other)))
        })
        .map(|(_, body)| *body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_en_cardinal() {
        assert_eq!(plural_category("en", &Number::Int(1), false), Category::One);
        assert_eq!(
            plural_category("en", &Number::Int(2), false),
            Category::Other
        );
    }

    #[test]
    fn plural_fr_zero_is_one() {
        // French: 0 and 1 are both singular
        assert_eq!(plural_category("fr", &Number::Int(0), false), Category::One);
        assert_eq!(plural_category("fr", &Number::Int(1), false), Category::One);
        assert_eq!(
            plural_category("fr", &Number::Int(2), false),
            Category::Other
        );
    }

    #[test]
    fn plural_ru_three_way() {
        assert_eq!(plural_category("ru", &Number::Int(1), false), Category::One);
        assert_eq!(plural_category("ru", &Number::Int(2), false), Category::Few);
        assert_eq!(
            plural_category("ru", &Number::Int(5), false),
            Category::Many
        );
        assert_eq!(
            plural_category("ru", &Number::Int(21), false),
            Category::One
        );
    }

    #[test]
    fn plural_ar_six_way() {
        assert_eq!(
            plural_category("ar", &Number::Int(0), false),
            Category::Zero
        );
        assert_eq!(plural_category("ar", &Number::Int(1), false), Category::One);
        assert_eq!(plural_category("ar", &Number::Int(2), false), Category::Two);
        assert_eq!(plural_category("ar", &Number::Int(5), false), Category::Few);
        assert_eq!(
            plural_category("ar", &Number::Int(50), false),
            Category::Many
        );
    }

    #[test]
    fn plural_en_ordinal() {
        assert_eq!(plural_category("en", &Number::Int(1), true), Category::One);
        assert_eq!(plural_category("en", &Number::Int(2), true), Category::Two);
        assert_eq!(plural_category("en", &Number::Int(3), true), Category::Few);
        assert_eq!(
            plural_category("en", &Number::Int(4), true),
            Category::Other
        );
    }

    #[test]
    fn plural_unknown_locale_falls_back() {
        // Should not panic
        let _ = plural_category("und", &Number::Int(1), false);
    }
}
