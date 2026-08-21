//! Tree-sitter based parsing and symbol/reference extraction.
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts"): this is
//! the public API `engine-index` is written against. Implementers may add
//! private items freely but should not change these signatures without
//! updating the architecture doc and every caller.

use engine_core::{Language, RepoPath, Span};
use serde::{Deserialize, Serialize};

pub mod languages;

/// A symbol as seen by the parser, before it has a `SymbolId` (that's
/// assigned by `engine-index` on persist).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedSymbol {
    pub name: String,
    pub qualified_name: String,
    pub kind: engine_core::SymbolKind,
    pub span: Span,
    pub is_exported: bool,
    pub doc_comment: Option<String>,
    pub signature: Option<String>,
}

/// A use-site found by the parser. `to_name` is whatever identifier text
/// was used at the call/reference site — resolving it to a `SymbolId` is
/// `engine-index`'s job (it has cross-file visibility this crate doesn't).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedReference {
    pub from_span: Span,
    /// The symbol (by name, within this file) that contains this
    /// reference, if any — lets `engine-index` set `Reference::from_symbol`.
    pub from_symbol_name: Option<String>,
    pub to_name: String,
    pub kind: engine_core::ReferenceKind,
}

/// One import/use statement, giving `engine-index` the scope information it
/// needs to resolve `ParsedReference::to_name` with better-than-global-guess
/// confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportEdge {
    /// The local name this import binds (post-alias).
    pub local_name: String,
    /// The module/path/crate the name is imported from, exactly as written
    /// in source (resolving it to an actual file is `engine-index`'s job,
    /// since that requires knowing the whole repo's file layout and
    /// language-specific module resolution rules).
    pub source_module: String,
    /// The name as exported by the source module, if different from
    /// `local_name` (i.e. `import { foo as bar }` -> local_name="bar",
    /// imported_name="foo").
    pub imported_name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseResult {
    pub symbols: Vec<ParsedSymbol>,
    pub references: Vec<ParsedReference>,
    pub imports: Vec<ImportEdge>,
    /// Non-fatal issues (e.g. a syntax error tree-sitter recovered from).
    /// A non-empty list does not mean parsing failed — it means the result
    /// may be incomplete, which the caller should fold into `Confidence`.
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("no grammar available for {0}")]
    UnsupportedLanguage(Language),
    #[error("tree-sitter failed to produce a parse tree for {file}")]
    TreeSitterFailure { file: RepoPath },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Every supported language implements this. One implementor per
/// `Language` variant, registered in `parse_file`'s dispatch table.
pub trait LanguageExtractor: Send + Sync {
    fn language(&self) -> Language;
    fn extract(&self, source: &str, file: &RepoPath) -> Result<ParseResult, ParseError>;
}

/// Parse a single file's source text and extract symbols/references/imports.
/// This is the one function `engine-index` calls per file during indexing.
pub fn parse_file(
    language: Language,
    source: &str,
    file: &RepoPath,
) -> Result<ParseResult, ParseError> {
    languages::extractor_for(language)
        .ok_or(ParseError::UnsupportedLanguage(language))?
        .extract(source, file)
}
