//! tree-sitter-typescript (0.23) backed extractor. Handles both
//! `Language::TypeScript` and `Language::Tsx` — the grammar crate ships two
//! separate grammars (`LANGUAGE_TYPESCRIPT` and `LANGUAGE_TSX`) that agree on
//! every node kind this extractor cares about, so the choice between them is
//! made from the file's own extension rather than from `self`. See
//! `ts_js_common` for the shared extraction logic and its grounding notes.

use crate::languages::ts_js_common;
use crate::{LanguageExtractor, ParseError, ParseResult};
use engine_core::{Language, RepoPath};

pub struct TypeScriptExtractor;

impl LanguageExtractor for TypeScriptExtractor {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn extract(&self, source: &str, file: &RepoPath) -> Result<ParseResult, ParseError> {
        let grammar = if file.as_str().ends_with(".tsx") {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        };
        ts_js_common::extract(source, file, grammar, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{ReferenceKind, SymbolKind};

    fn extract(source: &str) -> ParseResult {
        TypeScriptExtractor
            .extract(source, &RepoPath::new("test.ts"))
            .expect("parse should succeed")
    }

    #[test]
    fn extracts_a_simple_function() {
        let result = extract("export function greet(name: string): string { return name; }");
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
export class Circle implements Shape {
  radius: number;
  area(): number { return this.radius; }
}
"#;
        let result = extract(source);
        let class = result.symbols.iter().find(|s| s.name == "Circle").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert!(class.is_exported);

        let method = result.symbols.iter().find(|s| s.name == "area").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.qualified_name, "Circle.area");

        let implements = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Implements)
            .unwrap();
        assert_eq!(implements.to_name, "Shape");
    }

    #[test]
    fn extracts_named_import_with_alias() {
        let result = extract(r#"import { Foo, Bar as Baz } from "./foo";"#);
        assert_eq!(result.imports.len(), 2);
        let baz = result
            .imports
            .iter()
            .find(|i| i.local_name == "Baz")
            .unwrap();
        assert_eq!(baz.imported_name, "Bar");
        assert_eq!(baz.source_module, "./foo");
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
    fn extracts_instantiation_and_extends() {
        let source = r#"
class Base {}
export class Derived extends Base {
  method() {
    const c = new Base();
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
        assert_eq!(extends.to_name, "Base");
        let inst = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Instantiation)
            .unwrap();
        assert_eq!(inst.to_name, "Base");
        assert_eq!(inst.from_symbol_name.as_deref(), Some("method"));
    }

    #[test]
    fn extracts_arrow_function_const_as_function_symbol() {
        let result = extract("export const add = (a: number, b: number): number => a + b;");
        let sym = result.symbols.iter().find(|s| s.name == "add").unwrap();
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.is_exported);
    }

    #[test]
    fn private_class_member_is_not_exported() {
        let source = "export class Foo { private secret = 1; }";
        let result = extract(source);
        let sym = result.symbols.iter().find(|s| s.name == "secret").unwrap();
        assert!(!sym.is_exported);
    }

    #[test]
    fn recovers_from_syntax_errors_with_warning() {
        let result = extract("const x: = ;\nfunction ok() {}\n");
        assert!(!result.warnings.is_empty());
        assert!(result.symbols.iter().any(|s| s.name == "ok"));
    }

    #[test]
    fn empty_file_yields_no_symbols() {
        let result = extract("");
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn tsx_file_extension_uses_tsx_grammar() {
        let result = TypeScriptExtractor
            .extract(
                "export function App() {\n  return <div>hi</div>;\n}\n",
                &RepoPath::new("src/App.tsx"),
            )
            .expect("tsx source should parse with the tsx grammar");
        let sym = result.symbols.iter().find(|s| s.name == "App").unwrap();
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.is_exported);
        assert!(result.warnings.is_empty());
    }
}
