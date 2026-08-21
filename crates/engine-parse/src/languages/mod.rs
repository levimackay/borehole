//! One submodule per supported language, each implementing
//! [`crate::LanguageExtractor`]. IMPLEMENTATION NOTE FOR THE AGENT BUILDING
//! THIS: these are currently stubs returning `ParseError::UnsupportedLanguage`
//! at runtime even though the dispatch table lists them — replace each
//! `todo_extractor()` call with a real tree-sitter-backed implementation
//! and wire its grammar dependency into `crates/engine-parse/Cargo.toml`.

use crate::{LanguageExtractor, ParseError, ParseResult};
use engine_core::{Language, RepoPath};

pub mod go;
pub mod javascript;
pub mod python;
pub mod rust;
pub mod typescript;

pub fn extractor_for(language: Language) -> Option<Box<dyn LanguageExtractor>> {
    match language {
        Language::Rust => Some(Box::new(rust::RustExtractor)),
        Language::TypeScript | Language::Tsx => Some(Box::new(typescript::TypeScriptExtractor)),
        Language::JavaScript => Some(Box::new(javascript::JavaScriptExtractor)),
        Language::Python => Some(Box::new(python::PythonExtractor)),
        Language::Go => Some(Box::new(go::GoExtractor)),
    }
}

/// Temporary placeholder body shared by every not-yet-implemented
/// extractor. Delete once the extractor is real.
pub(crate) fn todo_extract(file: &RepoPath) -> Result<ParseResult, ParseError> {
    Err(ParseError::TreeSitterFailure { file: file.clone() })
}
