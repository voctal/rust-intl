//! Simple parser for ICU MessageFormat. It only implements a subset of it.
//!
//! Interpolation (`{name}`), plurals, selectordinal, select,
//! numbers, dates, times.

mod parser;
mod vars;

pub use parser::parse;
pub use vars::{VarKind, collect_vars};

#[derive(Clone, Debug, PartialEq)]
pub enum AstNode {
    Text(String),
    Var(String),
    Plural {
        var: String,
        ordinal: bool,
        arms: Vec<(ArmKey, Vec<AstNode>)>,
    },
    Select {
        var: String,
        arms: Vec<(String, Vec<AstNode>)>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArmKey {
    Exact(i64),
    Category(String),
}
