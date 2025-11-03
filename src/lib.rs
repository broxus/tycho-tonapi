use std::sync::OnceLock;

pub mod db;
pub mod grpc;
pub mod state;

pub static BIN_VERSION: &str = env!("TYCHO_VERSION");
pub static BIN_BUILD: &str = env!("TYCHO_BUILD");

pub fn version_string() -> &'static str {
    static STRING: OnceLock<String> = OnceLock::new();
    STRING.get_or_init(|| format!("(release {BIN_VERSION}) (build {BIN_BUILD})"))
}

pub fn version_packed() -> u32 {
    static VERSION: OnceLock<u32> = OnceLock::new();
    *VERSION.get_or_init(|| {
        let major = env!("CARGO_PKG_VERSION_MAJOR").parse::<u8>().unwrap();
        let minor = env!("CARGO_PKG_VERSION_MINOR").parse::<u8>().unwrap();
        let patch = env!("CARGO_PKG_VERSION_PATCH").parse::<u8>().unwrap();

        u32::from_be_bytes([0, major, minor, patch])
    })
}
