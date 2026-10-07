//! Git CLI integration. All process spawning goes through [`exec`].

pub mod binary;
pub mod branches;
pub mod commit;
pub mod diff;
pub mod discard;
pub mod error;
pub mod exec;
pub mod identity;
pub mod ignore;
pub mod log;
pub mod parse;
pub mod repo_root;
pub mod stage;
pub mod status;
#[cfg(test)]
pub mod test_support;
pub mod version;
