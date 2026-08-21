//! tree-sitter-javascript (0.25) backed extractor. Shares its extraction
//! logic with the TypeScript extractor via `ts_js_common` (grounded against
//! a real parse showing the JS grammar's class-body shape differs from TS's
//! — JS class members carry a `member` field, TS's don't — which
//! `ts_js_common` handles by iterating members by kind, not by field name).

use crate::languages::ts_js_common;
use crate::{LanguageExtractor, ParseError, ParseResult};
use engine_core::{Language, RepoPath};

pub struct JavaScriptExtractor;

impl LanguageExtractor for JavaScriptExtractor {
    fn language(&self) -> Language {
        Language::JavaScript
    }

    fn extract(&self, source: &str, file: &RepoPath) -> Result<ParseResult, ParseError> {
        ts_js_common::extract(source, file, tree_sitter_javascript::LANGUAGE.into(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{ReferenceKind, SymbolKind};

    fn extract(source: &str) -> ParseResult {
        JavaScriptExtractor
            .extract(source, &RepoPath::new("test.js"))
            .expect("parse should succeed")
    }

    #[test]
    fn extracts_a_simple_function() {
        let result = extract("export function greet(name) { return name; }");
        let sym = result.symbols.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.is_exported);
    }

    #[test]
    fn non_exported_function_is_not_exported() {
        let result = extract("function greet() {}");
        let sym = result.symbols.iter().find(|s| s.name == "greet").unwrap();
        assert!(!sym.is_exported);
    }

    #[test]
    fn extracts_class_with_method_and_qualified_name() {
        let source = r#"
export class Circle {
  constructor(radius) {
    this.radius = radius;
  }

  area() {
    return this.radius;
  }
}
"#;
        let result = extract(source);
        let class = result.symbols.iter().find(|s| s.name == "Circle").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert!(class.is_exported);

        let method = result.symbols.iter().find(|s| s.name == "area").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.qualified_name, "Circle.area");
    }

    #[test]
    fn extracts_default_import() {
        let result = extract(r#"import Default from "./default";"#);
        assert_eq!(result.imports.len(), 1);
        let imp = &result.imports[0];
        assert_eq!(imp.local_name, "Default");
        assert_eq!(imp.imported_name, "default");
        assert_eq!(imp.source_module, "./default");
    }

    #[test]
    fn extracts_call_reference_with_enclosing_symbol() {
        let source = r#"
function helper() {}
function caller() {
  helper();
}
"#;
        let result = extract(source);
        let call = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Call && r.to_name == "helper")
            .unwrap();
        assert_eq!(call.from_symbol_name.as_deref(), Some("caller"));
    }

    #[test]
    fn extracts_extends_and_instantiation() {
        let source = r#"
class Circle {}
class Derived extends Circle {
  method() {
    const c = new Circle();
    return c;
  }
}
"#;
        let result = extract(source);
        let extends = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Extends)
            .unwrap();
        assert_eq!(extends.to_name, "Circle");
        let inst = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Instantiation)
            .unwrap();
        assert_eq!(inst.to_name, "Circle");
    }

    #[test]
    fn recovers_from_syntax_errors_with_warning() {
        let result = extract("if (x {\n}\nfunction ok() {}\n");
        assert!(!result.warnings.is_empty());
        assert!(result.symbols.iter().any(|s| s.name == "ok"));
    }

    #[test]
    fn empty_file_yields_no_symbols() {
        let result = extract("");
        assert!(result.symbols.is_empty());
    }
}
