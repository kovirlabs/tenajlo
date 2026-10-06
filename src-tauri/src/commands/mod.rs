//! Tauri commands. Thin: validate input, call core modules, map errors to [`crate::error::AppError`].

pub mod app;
pub mod repo;
pub mod repos;
