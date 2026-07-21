use super::AstNode;
use std::collections::HashMap;

/// Variable kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    /// Used for `plural`/`selectordinal`.
    Number,
    /// Interpolation, or used for `select`.
    /// Requires a `Display` argument.
    Text,
}

#[derive(Default, Clone, Copy)]
struct Evidence {
    number_pivot: bool,
    select_pivot: bool,
}

/// Collect every variable referenced anywhere in `nodes` (including nested
/// plural/select arm bodies), in first-appearance order, along with the
/// strongest kind it's used as. Returns `Err` if a variable is used both as
/// a plural pivot and a non-numeric (text/select) role, since that can't be
/// represented by a single argument type.
pub fn collect_vars(nodes: &[AstNode]) -> Result<Vec<(String, VarKind)>, String> {
    let mut order: Vec<String> = Vec::new();
    let mut evidence: HashMap<String, Evidence> = HashMap::new();

    fn touch<'a>(
        name: &str,
        order: &mut Vec<String>,
        evidence: &'a mut HashMap<String, Evidence>,
    ) -> &'a mut Evidence {
        evidence.entry(name.to_string()).or_insert_with(|| {
            order.push(name.to_string());
            Evidence::default()
        })
    }

    fn visit(nodes: &[AstNode], order: &mut Vec<String>, evidence: &mut HashMap<String, Evidence>) {
        for node in nodes {
            match node {
                AstNode::Text(_) => {}
                AstNode::Var(name) => {
                    touch(name, order, evidence);
                }
                AstNode::Plural { var, arms, .. } => {
                    touch(var, order, evidence).number_pivot = true;
                    for (_, body) in arms {
                        visit(body, order, evidence);
                    }
                }
                AstNode::Select { var, arms } => {
                    touch(var, order, evidence).select_pivot = true;
                    for (_, body) in arms {
                        visit(body, order, evidence);
                    }
                }
            }
        }
    }

    visit(nodes, &mut order, &mut evidence);

    let mut result = Vec::with_capacity(order.len());
    for name in order {
        let ev = evidence[&name];
        if ev.number_pivot && ev.select_pivot {
            return Err(format!(
                "variable '{name}' is used both as a plural pivot (needs a number) and a select \
                 pivot (needs text), pick one role per variable"
            ));
        }
        let kind = if ev.number_pivot {
            VarKind::Number
        } else {
            VarKind::Text
        };
        result.push((name, kind));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;

    #[test]
    fn plain_text() {
        let ast = parse("Hello there").unwrap();
        assert_eq!(ast, vec![AstNode::Text("Hello there".into())]);
    }

    #[test]
    fn simple_interpolation() {
        let ast = parse("Hello, {name}!").unwrap();
        assert_eq!(
            ast,
            vec![
                AstNode::Text("Hello, ".into()),
                AstNode::Var("name".into()),
                AstNode::Text("!".into()),
            ]
        );
    }

    #[test]
    fn escaped_braces() {
        let ast = parse("Use {{name}} literally").unwrap();
        assert_eq!(ast, vec![AstNode::Text("Use {name} literally".into())]);
    }

    #[test]
    fn escaped_quote() {
        let ast = parse("It's {n}").unwrap();
        assert_eq!(
            ast,
            vec![AstNode::Text("It's ".into()), AstNode::Var("n".into())]
        );
    }

    #[test]
    fn plural_with_exact_and_other() {
        let ast = parse("{count, plural, =0 {none} one {1 item} other {{count} items}}").unwrap();
        assert_eq!(ast.len(), 1);
        match &ast[0] {
            AstNode::Plural { var, ordinal, arms } => {
                assert_eq!(var, "count");
                assert!(!ordinal);
                assert_eq!(arms.len(), 3);
                assert_eq!(arms[0].0, super::super::ArmKey::Exact(0));
                assert_eq!(arms[1].0, super::super::ArmKey::Category("one".into()));
                assert_eq!(arms[2].0, super::super::ArmKey::Category("other".into()));
                assert_eq!(
                    arms[2].1,
                    vec![AstNode::Var("count".into()), AstNode::Text(" items".into())]
                );
            }
            other => panic!("expected Plural, got {other:?}"),
        }
    }

    #[test]
    fn plural_missing_other_is_error() {
        let err = parse("{count, plural, one {1 item}}").unwrap_err();
        assert!(err.contains("other"), "error was: {err}");
    }

    #[test]
    fn plural_invalid_category_is_error() {
        let err = parse("{count, plural, foo {x} other {y}}").unwrap_err();
        assert!(
            err.contains("not a valid plural category"),
            "error was: {err}"
        );
    }

    #[test]
    fn select_basic() {
        let ast = parse("{gender, select, male {He} female {She} other {They}}").unwrap();
        match &ast[0] {
            AstNode::Select { var, arms } => {
                assert_eq!(var, "gender");
                assert_eq!(arms.len(), 3);
                assert_eq!(arms[2].0, "other");
            }
            other => panic!("expected Select, got {other:?}"),
        }
    }

    #[test]
    fn select_missing_other_is_error() {
        let err = parse("{gender, select, male {He} female {She}}").unwrap_err();
        assert!(err.contains("other"), "error was: {err}");
    }

    #[test]
    fn number_type_is_plain_interpolation() {
        let ast = parse("You have {n, number} points").unwrap();
        assert_eq!(
            ast,
            vec![
                AstNode::Text("You have ".into()),
                AstNode::Var("n".into()),
                AstNode::Text(" points".into()),
            ]
        );
    }

    #[test]
    fn nested_plural_inside_select() {
        let ast = parse(
            "{gender, select, male {He has {n, plural, one {1 cat} other {{n} cats}}} other {They have {n, plural, one {1 cat} other {{n} cats}}}}",
        )
        .unwrap();
        assert_eq!(ast.len(), 1);
        assert!(matches!(&ast[0], AstNode::Select { .. }));
    }

    #[test]
    fn collect_vars_allows_pivot_reused_as_display() {
        let ast = parse("{count, plural, one {1 item} other {{count} items}}").unwrap();
        let vars = collect_vars(&ast).unwrap();
        assert_eq!(vars, vec![("count".to_string(), VarKind::Number)]);
    }

    #[test]
    fn collect_vars_conflict_is_error() {
        let mut nodes = parse("{n, plural, one {x} other {y}} {n}").unwrap();
        // this should NOT conflict (Number + plain reuse is fine)
        assert!(collect_vars(&nodes).is_ok());
        nodes.clear();
        // but Number pivot + Select pivot on the same name should conflict
        let a = parse("{n, plural, one {x} other {y}}").unwrap();
        let b = parse("{n, select, foo {x} other {y}}").unwrap();
        let mut combined = a;
        combined.extend(b);
        let err = collect_vars(&combined).unwrap_err();
        assert!(err.contains("pick one role"), "error was: {err}");
    }

    #[test]
    fn first_appearance_order_is_preserved() {
        let ast = parse("{b} and {a} and {b}").unwrap();
        let vars = collect_vars(&ast).unwrap();
        assert_eq!(
            vars.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["b", "a"]
        );
    }
}
