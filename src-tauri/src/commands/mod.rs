//! Tauri commands. Thin: validate input, call core modules, map errors to [`crate::error::AppError`].

pub mod app;
pub mod auth;
pub mod branches;
pub mod changes;
pub mod repo;
pub mod repos;
pub mod sync;
