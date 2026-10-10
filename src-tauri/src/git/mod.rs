//! Git CLI integration. All process spawning goes through [`exec`].

pub mod binary;
pub mod branch_name;
pub mod branches;
pub mod clone;
pub mod commit;
pub mod diff;
pub mod discard;
pub mod error;
pub mod exec;
pub mod identity;
pub mod ignore;
pub mod init;
pub mod lfs;
pub mod log;
pub mod merge;
pub mod parse;
mod process_tree;
pub mod remote;
pub mod repo_root;
pub mod stage;
pub mod stash;
pub mod status;
pub mod switch;
pub mod sync_plan;
pub mod sync_state;
#[cfg(test)]
pub mod test_support;
pub mod undo;
pub mod version;
