//! Git CLI integration. All process spawning goes through [`exec`].

pub mod binary;
pub mod error;
pub mod exec;
pub mod parse;
pub mod version;
