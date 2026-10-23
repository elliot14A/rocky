pub mod age;
mod agenix;

pub use age::{decrypt, encrypt};
pub use agenix::{build_rekey_args, rekey};
