pub mod age;
mod agenix;
pub mod secret;

pub use age::{decrypt, encrypt};
pub use agenix::{build_rekey_args, rekey};
pub use secret::{
    create_secret, detect_secret_format, edit_secret, extract_key, generate_mock_secrets,
    parse_key_values, show_secret, validate_key_value, SecretFormat,
};
