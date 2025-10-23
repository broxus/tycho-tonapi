use std::sync::OnceLock;

pub mod db;
pub mod grpc;
pub mod state;
pub mod util {
    pub mod serde_helpers;
}

pub static BIN_VERSION: &str = env!("TYCHO_VERSION");
pub static BIN_BUILD: &str = env!("TYCHO_BUILD");

pub fn version_string() -> &'static str {
    static STRING: OnceLock<String> = OnceLock::new();
    STRING.get_or_init(|| format!("(release {BIN_VERSION}) (build {BIN_BUILD})"))
}
