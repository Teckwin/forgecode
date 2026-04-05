mod auto_dump;
mod compact;
mod config;
mod decimal;
mod error;
mod http;
mod legacy;
mod manager;
mod memory_settings;
mod model;
mod percentage;
mod permission_settings;
mod reader;
mod retry;
mod rules_settings;
mod sandbox_settings;
mod writer;

pub use auto_dump::*;
pub use compact::*;
pub use config::*;
pub use decimal::*;
pub use error::Error;
pub use http::*;
pub use manager::*;
pub use memory_settings::*;
pub use model::*;
pub use percentage::*;
pub use permission_settings::*;
pub use reader::*;
pub use retry::*;
pub use rules_settings::*;
pub use sandbox_settings::*;
pub use writer::*;

/// A `Result` type alias for this crate's [`Error`] type.
pub type Result<T> = std::result::Result<T, Error>;
