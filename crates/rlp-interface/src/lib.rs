//! Client-side subset of Reflect Liquid Protection. See ../README.md for provenance.
//! This crate contains no program entrypoint or on-chain instruction handlers.
pub mod accounts;
pub mod constants;
pub mod errors;
pub mod helpers;
pub mod instruction;
pub mod instructions;
pub mod states;

anchor_lang::declare_id!("JrXLmS6aYJNJDVxdAfjNJE5wikT8ubf3TA9iL2JA9Av");

/// Source revision of the bundled interface and simulation fixture, not a build dependency.
pub const SOURCE_REVISION: &str = "ff753a9325ec2ce4d5d6f3210fad9548d0a94f36";
