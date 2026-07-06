use super::plural::{pick_plural_arm, plural_category};
use super::{Node, Value};

/// Renders a message AST to a string.
pub fn render(locale_code: &str, nodes: &[Node], ctx: &[(&str, Value<'_>)]) -> String {
    let mut out = String::new();
    render_nodes(locale_code, nodes, ctx, &mut out);
    out
}

/// Finds the argument bound to `name` in `ctx`, if any.
fn lookup<'a, 'b>(ctx: &'b [(&str, Value<'a>)], name: &str) -> Option<&'b Value<'a>> {
    ctx.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
}

/// Render all `nodes` into `out`.
fn render_nodes(locale_code: &str, nodes: &[Node], ctx: &[(&str, Value<'_>)], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(t) => out.push_str(t),

            Node::Var(name) => match lookup(ctx, name) {
                Some(Value::String(s)) => out.push_str(s),
                Some(Value::Number(n)) => out.push_str(&n.format_display(locale_code)),
                None => {
                    out.push('{');
                    out.push_str(name);
                    out.push('}');
                }
            },

            Node::Plural { var, ordinal, arms } => {
                if let Some(Value::Number(n)) = lookup(ctx, var) {
                    let cat = plural_category(locale_code, &n.value, *ordinal);
                    if let Some(body) = pick_plural_arm(arms, &n.value, cat) {
                        render_nodes(locale_code, body, ctx, out);
                    }
                }
            }

            Node::Select { var, arms } => {
                if let Some(Value::String(s)) = lookup(ctx, var) {
                    let body = arms
                        .iter()
                        .find(|(k, _)| k == s)
                        .or_else(|| arms.iter().find(|(k, _)| *k == "other"));
                    if let Some((_, body)) = body {
                        render_nodes(locale_code, body, ctx, out);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_plain_var() {
        let nodes: &[Node] = &[Node::Text("Hi "), Node::Var("name"), Node::Text("!")];
        assert_eq!(
            render("en", nodes, &[("name", Value::String("Ada"))]),
            "Hi Ada!"
        );
    }

    #[test]
    fn renders_select() {
        let nodes: &[Node] = &[Node::Select {
            var: "g",
            arms: &[
                ("male", &[Node::Text("He")]),
                ("other", &[Node::Text("They")]),
            ],
        }];
        assert_eq!(render("en", nodes, &[("g", Value::String("male"))]), "He");
        assert_eq!(
            render("en", nodes, &[("g", Value::String("unknown"))]),
            "They"
        );
    }
}
