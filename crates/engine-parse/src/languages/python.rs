use crate::{LanguageExtractor, ParseError, ParseResult};
use engine_core::{Language, RepoPath};

/// STUB: replace with a real tree-sitter-python backed implementation.
/// See docs/ARCHITECTURE.md "Adding a language" for the extraction rules
/// every extractor must follow (symbol kinds, exported-ness, confidence).
pub struct PythonExtractor;

impl LanguageExtractor for PythonExtractor {
    fn language(&self) -> Language {
        Language::Python
    }

    fn extract(&self, _source: &str, file: &RepoPath) -> Result<ParseResult, ParseError> {
        super::todo_extract(file)
    }
}
