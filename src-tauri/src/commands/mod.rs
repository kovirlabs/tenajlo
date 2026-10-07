//! Tauri commands. Thin: validate input, call core modules, map errors to [`crate::error::AppError`].

pub mod accounts;
pub mod app;
pub mod auth;
pub mod branches;
pub mod changes;
pub mod clone;
pub mod merge;
pub mod repo;
pub mod repos;
pub mod settings;
pub mod sync;
