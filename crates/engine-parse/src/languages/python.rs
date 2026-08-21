//! tree-sitter-python (0.25) backed extractor. Node kinds and field names
//! confirmed against real parses (plain/aliased/multi-name imports, typed
//! parameters and return types, docstrings, decorators) — see `rust.rs`'s
//! header comment for the general grounding approach.
//!
//! Known limitations (documented, not silent gaps):
//! - `is_exported` follows the task's stated convention literally (no
//!   leading underscore, best-effort) — this marks dunder methods like
//!   `__init__` as "not exported" even though they're part of the public
//!   API, which is a known false negative of the convention, not a bug.
//! - Python has no `new`/instantiation keyword, so a call to a
//!   capitalized name (`Circle(5)`) is heuristically recorded as
//!   `Instantiation` and everything else as `Call` — this is the standard
//!   PascalCase-convention heuristic, not a semantic guarantee.
//! - Forward-reference string type hints (`x: "Foo"`) aren't extracted as
//!   `TypeUsage` — the annotation is a string literal, not an identifier
//!   node, in this grammar.
//! - Purely relative imports with no module name (`from . import x`) are
//!   skipped rather than guessed at, since that shape wasn't in the
//!   grounding sample.
//! - Decorators themselves aren't modeled as references.

use crate::languages::support::{collect_kind_texts, collect_warnings, node_text, span_of};
use crate::{
    ImportEdge, LanguageExtractor, ParseError, ParseResult, ParsedReference, ParsedSymbol,
};
use engine_core::{Language, ReferenceKind, RepoPath, SymbolKind};
use tree_sitter::{Node, Parser};

pub struct PythonExtractor;

impl LanguageExtractor for PythonExtractor {
    fn language(&self) -> Language {
        Language::Python
    }

    fn extract(&self, source: &str, file: &RepoPath) -> Result<ParseResult, ParseError> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|_| ParseError::TreeSitterFailure { file: file.clone() })?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ParseError::TreeSitterFailure { file: file.clone() })?;
        let root = tree.root_node();

        let mut result = ParseResult {
            warnings: collect_warnings(root),
            ..Default::default()
        };

        walk_block(root, source, None, &mut result);
        walk_refs(root, source, None, &mut result.references);

        Ok(result)
    }
}

fn qualify(qualifier: Option<&str>, name: &str) -> String {
    match qualifier {
        Some(q) if !q.is_empty() => format!("{q}.{name}"),
        _ => name.to_string(),
    }
}

fn is_exported(name: &str) -> bool {
    !name.starts_with('_')
}

fn field_text<'a>(node: Node<'a>, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field)
        .map(|n| node_text(n, source))
}

fn python_signature(node: Node, source: &str) -> String {
    let text = match node.child_by_field_name("body") {
        Some(body) => source
            .get(node.start_byte()..body.start_byte())
            .unwrap_or(""),
        None => node_text(node, source),
    };
    text.trim().trim_end_matches(':').trim().to_string()
}

/// A function/class's docstring: the first statement of its body, if that
/// statement is a bare string-literal expression.
fn docstring(node: Node, source: &str) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let first = body.named_child(0)?;
    if first.kind() != "expression_statement" {
        return None;
    }
    let inner = first.named_child(0)?;
    if inner.kind() != "string" {
        return None;
    }
    let content = collect_kind_texts(inner, "string_content", source).join("");
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn push_type_usages_from(
    container: Node,
    field: &str,
    source: &str,
    from_symbol: &str,
    refs: &mut Vec<ParsedReference>,
) {
    if let Some(ty) = container.child_by_field_name(field) {
        for ident in collect_kind_texts(ty, "identifier", source) {
            refs.push(ParsedReference {
                from_span: span_of(ty),
                from_symbol_name: Some(from_symbol.to_string()),
                to_name: ident.to_string(),
                kind: ReferenceKind::TypeUsage,
            });
        }
    }
}

fn push_signature_type_usages(
    fn_node: Node,
    source: &str,
    name: &str,
    refs: &mut Vec<ParsedReference>,
) {
    if let Some(params) = fn_node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for param in params.named_children(&mut cursor) {
            push_type_usages_from(param, "type", source, name, refs);
        }
    }
    push_type_usages_from(fn_node, "return_type", source, name, refs);
}

fn push_function(node: Node, source: &str, qualifier: Option<&str>, out: &mut ParseResult) {
    let Some(name) = field_text(node, "name", source) else {
        return;
    };
    let name = name.to_string();
    let kind = if qualifier.is_some() {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    };
    out.symbols.push(ParsedSymbol {
        name: name.clone(),
        qualified_name: qualify(qualifier, &name),
        kind,
        span: span_of(node),
        is_exported: is_exported(&name),
        doc_comment: docstring(node, source),
        signature: Some(python_signature(node, source)),
    });
    push_signature_type_usages(node, source, &name, &mut out.references);
}

fn push_class(node: Node, source: &str, qualifier: Option<&str>, out: &mut ParseResult) {
    let Some(name) = field_text(node, "name", source) else {
        return;
    };
    let name = name.to_string();
    out.symbols.push(ParsedSymbol {
        name: name.clone(),
        qualified_name: qualify(qualifier, &name),
        kind: SymbolKind::Class,
        span: span_of(node),
        is_exported: is_exported(&name),
        doc_comment: docstring(node, source),
        signature: Some(python_signature(node, source)),
    });
    if let Some(supers) = node.child_by_field_name("superclasses") {
        let mut cursor = supers.walk();
        for base in supers.named_children(&mut cursor) {
            if matches!(base.kind(), "identifier" | "attribute") {
                out.references.push(ParsedReference {
                    from_span: span_of(base),
                    from_symbol_name: None,
                    to_name: node_text(base, source).to_string(),
                    kind: ReferenceKind::Extends,
                });
            }
        }
    }
    if let Some(body) = node.child_by_field_name("body") {
        let new_qualifier = qualify(qualifier, &name);
        walk_block(body, source, Some(&new_qualifier), out);
    }
}

fn push_assignment(stmt: Node, source: &str, qualifier: Option<&str>, out: &mut ParseResult) {
    let Some(assign) = stmt.named_child(0).filter(|n| n.kind() == "assignment") else {
        return;
    };
    let Some(left) = assign
        .child_by_field_name("left")
        .filter(|n| n.kind() == "identifier")
    else {
        return;
    };
    let name = node_text(left, source).to_string();
    let looks_like_constant = name.chars().any(char::is_alphabetic)
        && name
            .chars()
            .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit());
    out.symbols.push(ParsedSymbol {
        name: name.clone(),
        qualified_name: qualify(qualifier, &name),
        kind: if looks_like_constant {
            SymbolKind::Constant
        } else {
            SymbolKind::Variable
        },
        span: span_of(stmt),
        is_exported: is_exported(&name),
        doc_comment: None,
        signature: Some(node_text(stmt, source).trim().to_string()),
    });
}

fn walk_block(container: Node, source: &str, qualifier: Option<&str>, out: &mut ParseResult) {
    let mut cursor = container.walk();
    let children: Vec<Node> = container.named_children(&mut cursor).collect();
    for child in children {
        match child.kind() {
            "import_statement" => extract_import_plain(child, source, &mut out.imports),
            "import_from_statement" => extract_import_from(child, source, &mut out.imports),
            "function_definition" => push_function(child, source, qualifier, out),
            "class_definition" => push_class(child, source, qualifier, out),
            "decorated_definition" => {
                if let Some(def) = child.child_by_field_name("definition") {
                    match def.kind() {
                        "function_definition" => push_function(def, source, qualifier, out),
                        "class_definition" => push_class(def, source, qualifier, out),
                        _ => {}
                    }
                }
            }
            "expression_statement" => push_assignment(child, source, qualifier, out),
            _ => {}
        }
    }
}

/// `import a, b` / `import a.b.c` — a plain `import_statement` binds the
/// first dotted segment locally (Python semantics: `import a.b.c` binds
/// `a`, not `c`), while the full dotted path is the module.
fn extract_import_plain(stmt: Node, source: &str, imports: &mut Vec<ImportEdge>) {
    let span = span_of(stmt);
    let mut cursor = stmt.walk();
    for name_node in stmt.children_by_field_name("name", &mut cursor) {
        match name_node.kind() {
            "dotted_name" => {
                let full = node_text(name_node, source).to_string();
                let first_segment = name_node
                    .named_child(0)
                    .map(|n| node_text(n, source))
                    .unwrap_or(&full)
                    .to_string();
                imports.push(ImportEdge {
                    local_name: first_segment.clone(),
                    source_module: full,
                    imported_name: first_segment,
                    span,
                });
            }
            "aliased_import" => {
                // `aliased_import`'s original-name child is field `name`
                // (not `path`) — grounded against a real parse.
                let Some(orig) = name_node.child_by_field_name("name") else {
                    continue;
                };
                let Some(alias) = field_text(name_node, "alias", source) else {
                    continue;
                };
                let full = node_text(orig, source).to_string();
                imports.push(ImportEdge {
                    local_name: alias.to_string(),
                    source_module: full.clone(),
                    imported_name: full,
                    span,
                });
            }
            _ => {}
        }
    }
}

/// `from foo import Bar, Baz as Qux`.
fn extract_import_from(stmt: Node, source: &str, imports: &mut Vec<ImportEdge>) {
    let Some(module_node) = stmt.child_by_field_name("module_name") else {
        // Purely relative `from . import x` — see module limitations.
        return;
    };
    let module = node_text(module_node, source).to_string();
    let span = span_of(stmt);
    let mut cursor = stmt.walk();
    for name_node in stmt.children_by_field_name("name", &mut cursor) {
        match name_node.kind() {
            "dotted_name" => {
                let name = node_text(name_node, source).to_string();
                imports.push(ImportEdge {
                    local_name: name.clone(),
                    source_module: module.clone(),
                    imported_name: name,
                    span,
                });
            }
            "aliased_import" => {
                let Some(orig) = name_node.child_by_field_name("name") else {
                    continue;
                };
                let Some(alias) = field_text(name_node, "alias", source) else {
                    continue;
                };
                imports.push(ImportEdge {
                    local_name: alias.to_string(),
                    source_module: module.clone(),
                    imported_name: node_text(orig, source).to_string(),
                    span,
                });
            }
            _ => {}
        }
    }
}

fn call_target<'a>(function_node: Node<'a>, source: &'a str) -> &'a str {
    match function_node.kind() {
        "attribute" => field_text(function_node, "attribute", source).unwrap_or(""),
        _ => node_text(function_node, source),
    }
}

fn walk_refs(node: Node, source: &str, current: Option<&str>, refs: &mut Vec<ParsedReference>) {
    let next_current: Option<String> = match node.kind() {
        "function_definition" => field_text(node, "name", source).map(|s| s.to_string()),
        _ => current.map(|s| s.to_string()),
    };
    let next_ref = next_current.as_deref().or(current);

    if node.kind() == "call" {
        if let Some(func) = node.child_by_field_name("function") {
            let to_name = call_target(func, source);
            let looks_like_class = to_name
                .chars()
                .next()
                .map(char::is_uppercase)
                .unwrap_or(false);
            refs.push(ParsedReference {
                from_span: span_of(node),
                from_symbol_name: next_ref.map(|s| s.to_string()),
                to_name: to_name.to_string(),
                kind: if looks_like_class {
                    ReferenceKind::Instantiation
                } else {
                    ReferenceKind::Call
                },
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_refs(child, source, next_ref, refs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(source: &str) -> ParseResult {
        PythonExtractor
            .extract(source, &RepoPath::new("test.py"))
            .expect("parse should succeed")
    }

    #[test]
    fn extracts_a_simple_function() {
        let result = extract("def helper(x):\n    return x + 1\n");
        let sym = result.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.is_exported);
        assert_eq!(sym.signature.as_deref(), Some("def helper(x)"));
    }

    #[test]
    fn underscore_prefixed_function_is_not_exported() {
        let result = extract("def _private():\n    pass\n");
        let sym = result
            .symbols
            .iter()
            .find(|s| s.name == "_private")
            .unwrap();
        assert!(!sym.is_exported);
    }

    #[test]
    fn extracts_class_with_method_and_qualified_name() {
        let source = "class Shape:\n    def area(self):\n        return 0\n";
        let result = extract(source);
        let class = result.symbols.iter().find(|s| s.name == "Shape").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);

        let method = result.symbols.iter().find(|s| s.name == "area").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.qualified_name, "Shape.area");
    }

    #[test]
    fn extracts_docstring() {
        let source = "def helper():\n    \"\"\"Does a thing.\"\"\"\n    pass\n";
        let result = extract(source);
        let sym = result.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(sym.doc_comment.as_deref(), Some("Does a thing."));
    }

    #[test]
    fn extracts_from_import_with_alias() {
        let result = extract("from foo import Bar, Baz as Qux\n");
        assert_eq!(result.imports.len(), 2);
        let bar = result
            .imports
            .iter()
            .find(|i| i.local_name == "Bar")
            .unwrap();
        assert_eq!(bar.source_module, "foo");
        assert_eq!(bar.imported_name, "Bar");
        let qux = result
            .imports
            .iter()
            .find(|i| i.local_name == "Qux")
            .unwrap();
        assert_eq!(qux.imported_name, "Baz");
        assert_eq!(qux.source_module, "foo");
    }

    #[test]
    fn extracts_call_reference_with_enclosing_symbol() {
        let source = "def helper(x):\n    return x\n\ndef caller():\n    return helper(1)\n";
        let result = extract(source);
        let call = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Call && r.to_name == "helper")
            .unwrap();
        assert_eq!(call.from_symbol_name.as_deref(), Some("caller"));
    }

    #[test]
    fn capitalized_call_is_instantiation() {
        let source = "class Circle:\n    pass\n\ndef make():\n    return Circle()\n";
        let result = extract(source);
        let inst = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Instantiation)
            .unwrap();
        assert_eq!(inst.to_name, "Circle");
    }

    #[test]
    fn extracts_extends_reference() {
        let source = "class Shape:\n    pass\n\nclass Circle(Shape):\n    pass\n";
        let result = extract(source);
        let extends = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Extends)
            .unwrap();
        assert_eq!(extends.to_name, "Shape");
    }

    #[test]
    fn recovers_from_syntax_errors_with_warning() {
        let result = extract("def broken(:\n    pass\ndef ok():\n    pass\n");
        assert!(!result.warnings.is_empty());
        assert!(result.symbols.iter().any(|s| s.name == "ok"));
    }

    #[test]
    fn empty_file_yields_no_symbols() {
        let result = extract("");
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn module_level_uppercase_assignment_is_constant() {
        let result = extract("MAX = 100\n");
        let sym = result.symbols.iter().find(|s| s.name == "MAX").unwrap();
        assert_eq!(sym.kind, SymbolKind::Constant);
    }

    #[test]
    fn all_comment_file_yields_no_symbols() {
        let result = extract("# just a comment\n# and another\n");
        assert!(result.symbols.is_empty());
        assert!(result.warnings.is_empty());
    }
}
