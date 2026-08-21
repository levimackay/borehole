//! Shared types for the Borehole analysis engine.
//!
//! Every other engine crate (`engine-parse`, `engine-index`, `engine-graph`,
//! `engine-git`, `engine-evidence`) depends on this crate for its data model,
//! but this crate depends on none of them. That one-way dependency edge is
//! what keeps the pipeline (parse -> index -> graph -> evidence) acyclic.

pub mod config;
pub mod ignore_rules;
pub mod language;
pub mod repo;
pub mod types;

pub use config::BoreholeConfig;
pub use language::Language;
pub use repo::{RepoHandle, RepoWalker};
pub use types::*;
