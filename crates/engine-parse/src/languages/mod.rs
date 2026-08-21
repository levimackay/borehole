//! One submodule per supported language, each implementing
//! [`crate::LanguageExtractor`] on top of that language's tree-sitter
//! grammar. `support` holds the tree-walking helpers shared across all of
//! them (span conversion, doc-comment collection, syntax-error warnings).

use crate::LanguageExtractor;
use engine_core::Language;

pub mod go;
pub mod javascript;
pub mod python;
pub mod rust;
pub(crate) mod support;
mod ts_js_common;
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
