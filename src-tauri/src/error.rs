use serde::Serialize;

/// Tauri commands must return a `Serialize` error type (unlike CLI/library
/// code, which uses `anyhow`/`thiserror` freely) — this is the one
/// conversion point where internal error types become a string the
/// frontend can display. We deliberately don't leak internal error
/// variants/types across the IPC boundary, just a human-readable message.
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub message: String,
}

impl<E: std::fmt::Display> From<E> for CommandError {
    fn from(e: E) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
