mod key;
mod target;

pub use key::{
    canonicalize_identity, generate_host_key, GeneratedKey, HOST_KEY_FILE, HOST_PUB_KEY_FILE,
};
pub use target::{parse_target, parse_targets, Target};
