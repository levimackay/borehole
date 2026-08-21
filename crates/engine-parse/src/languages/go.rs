use crate::{LanguageExtractor, ParseError, ParseResult};
use engine_core::{Language, RepoPath};

/// STUB: replace with a real tree-sitter-go backed implementation.
/// See docs/ARCHITECTURE.md "Adding a language" for the extraction rules
/// every extractor must follow (symbol kinds, exported-ness, confidence).
pub struct GoExtractor;

impl LanguageExtractor for GoExtractor {
    fn language(&self) -> Language {
        Language::Go
    }

    fn extract(&self, _source: &str, file: &RepoPath) -> Result<ParseResult, ParseError> {
        super::todo_extract(file)
    }
}
