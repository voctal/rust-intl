#![doc = include_str!("../README.md")]

pub mod runtime;

pub use runtime::NumberArg;

pub use rust_intl_macros::load;
pub use rust_intl_macros::t;
pub use rust_intl_macros::t_ns;
