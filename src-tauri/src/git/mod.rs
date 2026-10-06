//! Git CLI integration. All process spawning goes through [`exec`].

pub mod binary;
pub mod branches;
pub mod diff;
pub mod error;
pub mod exec;
pub mod log;
pub mod parse;
pub mod repo_root;
pub mod status;
#[cfg(test)]
pub mod test_support;
pub mod version;
