//! Error types shared across the `tko` crate.
//!
//! These variants are consumed by the config, backend, and command modules as
//! they land; the `allow(dead_code)` keeps the scaffolding warning-clean until
//! then.
#![allow(dead_code)]

use std::path::PathBuf;

/// Errors surfaced by `tko`'s core operations.
///
/// Command-level plumbing uses [`anyhow`] for ergonomic propagation; this typed
/// error captures the categories of failure that callers may want to match on.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No `tko.toml` was found walking up from the current directory.
    #[error("no tko.toml found in {0} or any parent directory")]
    ConfigNotFound(PathBuf),

    /// A `tko.toml` was found but could not be parsed.
    #[error("failed to parse tko.toml: {0}")]
    ConfigParse(String),

    /// The backend access token could not be resolved from any source.
    #[error("no backend token found (checked env var {env}, .env, and secrets.env)")]
    TokenNotFound { env: String },

    /// A ticket document failed to parse or serialize.
    #[error("invalid ticket document: {0}")]
    TicketFormat(String),

    /// A ticket id did not resolve to exactly one ticket.
    #[error("ticket not found: {0}")]
    TicketNotFound(String),

    /// An ambiguous partial id matched more than one ticket.
    #[error("ambiguous ticket id {id}: matches {matches:?}")]
    AmbiguousId { id: String, matches: Vec<String> },

    /// A backend request failed.
    #[error("backend error: {0}")]
    Backend(String),

    /// An I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Convenience result alias for `tko` operations.
pub type Result<T> = std::result::Result<T, Error>;
