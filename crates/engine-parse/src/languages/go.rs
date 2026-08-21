//! tree-sitter-go (0.25) backed extractor. Node kinds and field names
//! confirmed against real parses (imports, const/var/type declarations,
//! struct/interface types, methods with pointer receivers, multi-value
//! returns, and composite literals) — see `rust.rs`'s header comment for
//! the general grounding approach.
//!
//! Known limitations (documented, not silent gaps):
//! - `Instantiation` references are only recorded for composite literals of
//!   a plain named type (`Point{...}`, `&Point{...}`) — slice/map/array
//!   literal types (`[]int{...}`) aren't treated as "instantiating" a user
//!   type and are skipped.
//! - Interface embedding (`type Named interface { Shape; ... }`) is
//!   recorded as an `Extends` reference — the closest available
//!   `ReferenceKind` to Go's actual embedding semantics.
//! - Go has no per-file/package "exported" qualifier beyond the
//!   capitalized-identifier convention the task specifies; this extractor
//!   applies that convention literally, including to unexported package
//!   internals that are nonetheless "correct" Go style.

use crate::languages::support::{
    collect_kind_texts, collect_warnings, node_text, preceding_doc_comments,
    signature_before_field, span_of,
};
use crate::{
    ImportEdge, LanguageExtractor, ParseError, ParseResult, ParsedReference, ParsedSymbol,
};
use engine_core::{Language, ReferenceKind, RepoPath, SymbolKind};
use tree_sitter::Node;

pub struct GoExtractor;

impl LanguageExtractor for GoExtractor {
    fn language(&self) -> Language {
        Language::Go
    }

    fn extract(&self, source: &str, file: &RepoPath) -> Result<ParseResult, ParseError> {
        let mut parser = super::support::new_parser();
        parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .map_err(|_| ParseError::TreeSitterFailure { file: file.clone() })?;
        let tree = super::support::parse_with_timeout(&mut parser, source)
            .ok_or_else(|| ParseError::TreeSitterFailure { file: file.clone() })?;
        let root = tree.root_node();

        let mut result = ParseResult {
            warnings: collect_warnings(root),
            ..Default::default()
        };

        walk_top_level(root, source, &mut result);
        walk_refs(root, source, &mut result.references);

        Ok(result)
    }
}

fn is_exported(name: &str) -> bool {
    name.chars().next().map(char::is_uppercase).unwrap_or(false)
}

fn field_text<'a>(node: Node<'a>, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field)
        .map(|n| node_text(n, source))
}

fn is_doc_comment(node: Node) -> bool {
    node.kind() == "comment"
}

fn doc_comment_for<'a>(node: Node<'a>, source: &'a str) -> Option<String> {
    let comments = preceding_doc_comments(node, is_doc_comment);
    if comments.is_empty() {
        return None;
    }
    let lines: Vec<String> = comments
        .into_iter()
        .map(|c| {
            node_text(c, source)
                .trim()
                .trim_start_matches("//")
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();
    let joined = lines.join("\n");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn push_type_usages(
    container: Node,
    field: &str,
    source: &str,
    from_symbol: Option<&str>,
    refs: &mut Vec<ParsedReference>,
) {
    let Some(ty) = container.child_by_field_name(field) else {
        return;
    };
    for name in collect_kind_texts(ty, "type_identifier", source) {
        refs.push(ParsedReference {
            from_span: span_of(ty),
            from_symbol_name: from_symbol.map(|s| s.to_string()),
            to_name: name.to_string(),
            kind: ReferenceKind::TypeUsage,
        });
    }
}

fn push_signature_type_usages(
    fn_node: Node,
    source: &str,
    name: &str,
    refs: &mut Vec<ParsedReference>,
) {
    push_type_usages(fn_node, "parameters", source, Some(name), refs);
    push_type_usages(fn_node, "result", source, Some(name), refs);
}

/// Unwraps a receiver's declared type down to the bare type name, stripping
/// a pointer indirection (`*Point` -> `Point`) if present.
fn receiver_type_name(method: Node, source: &str) -> Option<String> {
    let receiver = method.child_by_field_name("receiver")?;
    let decl = receiver.named_child(0)?;
    let ty = decl.child_by_field_name("type")?;
    let leaf = if ty.kind() == "pointer_type" {
        ty.named_child(0)?
    } else {
        ty
    };
    Some(node_text(leaf, source).to_string())
}

#[allow(clippy::too_many_lines)]
fn walk_top_level(container: Node, source: &str, out: &mut ParseResult) {
    let mut cursor = container.walk();
    let children: Vec<Node> = container.named_children(&mut cursor).collect();
    for child in children {
        match child.kind() {
            "import_declaration" => extract_import_decl(child, source, &mut out.imports),
            "const_declaration" => extract_value_specs(child, source, SymbolKind::Constant, out),
            "var_declaration" => extract_value_specs(child, source, SymbolKind::Variable, out),
            "type_declaration" => extract_type_decl(child, source, out),
            "function_declaration" => {
                let Some(name) = field_text(child, "name", source) else {
                    continue;
                };
                let name = name.to_string();
                out.symbols.push(ParsedSymbol {
                    name: name.clone(),
                    qualified_name: name.clone(),
                    kind: SymbolKind::Function,
                    span: span_of(child),
                    is_exported: is_exported(&name),
                    doc_comment: doc_comment_for(child, source),
                    signature: Some(signature_before_field(child, source, &["body"])),
                });
                push_signature_type_usages(child, source, &name, &mut out.references);
            }
            "method_declaration" => {
                let Some(name) = field_text(child, "name", source) else {
                    continue;
                };
                let name = name.to_string();
                let receiver_ty = receiver_type_name(child, source);
                let qualified = match &receiver_ty {
                    Some(r) => format!("{r}.{name}"),
                    None => name.clone(),
                };
                out.symbols.push(ParsedSymbol {
                    name: name.clone(),
                    qualified_name: qualified,
                    kind: SymbolKind::Method,
                    span: span_of(child),
                    is_exported: is_exported(&name),
                    doc_comment: doc_comment_for(child, source),
                    signature: Some(signature_before_field(child, source, &["body"])),
                });
                push_signature_type_usages(child, source, &name, &mut out.references);
            }
            _ => {}
        }
    }
}

fn extract_value_specs(decl: Node, source: &str, kind: SymbolKind, out: &mut ParseResult) {
    let mut cursor = decl.walk();
    for spec in decl.named_children(&mut cursor) {
        if spec.kind() != "const_spec" && spec.kind() != "var_spec" {
            continue;
        }
        let Some(name) = field_text(spec, "name", source) else {
            continue;
        };
        let name = name.to_string();
        out.symbols.push(ParsedSymbol {
            name: name.clone(),
            qualified_name: name.clone(),
            kind,
            span: span_of(spec),
            is_exported: is_exported(&name),
            doc_comment: doc_comment_for(decl, source),
            signature: Some(node_text(spec, source).trim().to_string()),
        });
        push_type_usages(spec, "type", source, None, &mut out.references);
    }
}

/// `type Point struct` / `type Shape interface` / `type Alias = Foo` — the
/// declaration line up to (not including) the body brace, since
/// `field_declaration_list`/`interface`'s member list has no field name of
/// its own to cut at via `signature_before_field`.
fn go_type_signature(spec: Node, source: &str) -> String {
    let full = node_text(spec, source);
    match full.find('{') {
        Some(brace_pos) => full[..brace_pos].trim().to_string(),
        None => full.trim().to_string(),
    }
}

fn extract_type_decl(decl: Node, source: &str, out: &mut ParseResult) {
    let mut cursor = decl.walk();
    for spec in decl.named_children(&mut cursor) {
        if spec.kind() != "type_spec" {
            continue;
        }
        let Some(name) = field_text(spec, "name", source) else {
            continue;
        };
        let name = name.to_string();
        let Some(ty) = spec.child_by_field_name("type") else {
            continue;
        };
        let kind = match ty.kind() {
            "struct_type" => SymbolKind::Struct,
            "interface_type" => SymbolKind::Interface,
            _ => SymbolKind::TypeAlias,
        };
        out.symbols.push(ParsedSymbol {
            name: name.clone(),
            qualified_name: name.clone(),
            kind,
            span: span_of(spec),
            is_exported: is_exported(&name),
            doc_comment: doc_comment_for(decl, source),
            signature: Some(go_type_signature(spec, source)),
        });

        match ty.kind() {
            "struct_type" => {
                // `field_declaration_list` is a positional child of
                // `struct_type` (no field name of its own) — it's the only
                // named child besides the `struct` keyword token.
                if let Some(fields) = ty.named_child(0) {
                    let mut fc = fields.walk();
                    for field in fields.named_children(&mut fc) {
                        push_type_usages(field, "type", source, Some(&name), &mut out.references);
                    }
                }
            }
            "interface_type" => {
                let mut ic = ty.walk();
                for member in ty.named_children(&mut ic) {
                    match member.kind() {
                        "method_elem" => {
                            if let Some(mname) = field_text(member, "name", source) {
                                let mname = mname.to_string();
                                out.symbols.push(ParsedSymbol {
                                    name: mname.clone(),
                                    qualified_name: format!("{name}.{mname}"),
                                    kind: SymbolKind::Method,
                                    span: span_of(member),
                                    is_exported: is_exported(&mname),
                                    doc_comment: None,
                                    signature: Some(node_text(member, source).trim().to_string()),
                                });
                                push_signature_type_usages(
                                    member,
                                    source,
                                    &mname,
                                    &mut out.references,
                                );
                            }
                        }
                        "type_elem" => {
                            out.references.push(ParsedReference {
                                from_span: span_of(member),
                                from_symbol_name: None,
                                to_name: node_text(member, source).to_string(),
                                kind: ReferenceKind::Extends,
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_import_decl(decl: Node, source: &str, imports: &mut Vec<ImportEdge>) {
    let mut cursor = decl.walk();
    for child in decl.named_children(&mut cursor) {
        match child.kind() {
            "import_spec_list" => {
                let mut lc = child.walk();
                for spec in child.named_children(&mut lc) {
                    extract_import_spec(spec, source, imports);
                }
            }
            "import_spec" => extract_import_spec(child, source, imports),
            _ => {}
        }
    }
}

fn extract_import_spec(spec: Node, source: &str, imports: &mut Vec<ImportEdge>) {
    let Some(path_node) = spec.child_by_field_name("path") else {
        return;
    };
    let path_text: String =
        collect_kind_texts(path_node, "interpreted_string_literal_content", source)
            .first()
            .map(|s| s.to_string())
            .unwrap_or_default();
    let span = span_of(spec);
    let local_name = match field_text(spec, "name", source) {
        Some(explicit) => explicit.to_string(),
        None => path_text
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(&path_text)
            .to_string(),
    };
    imports.push(ImportEdge {
        local_name: local_name.clone(),
        source_module: path_text,
        imported_name: local_name,
        span,
    });
}

fn call_target<'a>(function_node: Node<'a>, source: &'a str) -> &'a str {
    match function_node.kind() {
        "selector_expression" => field_text(function_node, "field", source).unwrap_or(""),
        _ => node_text(function_node, source),
    }
}

/// Iterative (explicit-stack) traversal — deliberately not natural
/// recursion; see the identical rationale on `walk_refs` in `rust.rs`.
/// Deeply nested Go expressions (nested calls, parenthesized groups) are
/// adversarial input that would otherwise blow the native call stack and
/// abort the process.
fn walk_refs(root: Node, source: &str, refs: &mut Vec<ParsedReference>) {
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((node, current)) = stack.pop() {
        let next_current: Option<String> = match node.kind() {
            "function_declaration" | "method_declaration" => {
                field_text(node, "name", source).map(|s| s.to_string())
            }
            _ => current,
        };

        match node.kind() {
            "call_expression" => {
                if let Some(func) = node.child_by_field_name("function") {
                    refs.push(ParsedReference {
                        from_span: span_of(node),
                        from_symbol_name: next_current.clone(),
                        to_name: call_target(func, source).to_string(),
                        kind: ReferenceKind::Call,
                    });
                }
            }
            "composite_literal" => {
                if let Some(ty) = node.child_by_field_name("type") {
                    if ty.kind() == "type_identifier" {
                        refs.push(ParsedReference {
                            from_span: span_of(node),
                            from_symbol_name: next_current.clone(),
                            to_name: node_text(ty, source).to_string(),
                            kind: ReferenceKind::Instantiation,
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        let children: Vec<Node> = node.named_children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((child, next_current.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(source: &str) -> ParseResult {
        GoExtractor
            .extract(source, &RepoPath::new("test.go"))
            .expect("parse should succeed")
    }

    #[test]
    fn extracts_a_simple_function() {
        let result = extract("package main\n\nfunc Helper(x int) int {\n\treturn x + 1\n}\n");
        let sym = result.symbols.iter().find(|s| s.name == "Helper").unwrap();
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.is_exported);
    }

    #[test]
    fn lowercase_function_is_not_exported() {
        let result = extract("package main\n\nfunc helper() {}\n");
        let sym = result.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert!(!sym.is_exported);
    }

    #[test]
    fn extracts_struct_with_method_and_qualified_name() {
        let source = "package main\n\ntype Point struct {\n\tX int\n}\n\nfunc (p *Point) Area() float64 {\n\treturn 0\n}\n";
        let result = extract(source);
        let strct = result.symbols.iter().find(|s| s.name == "Point").unwrap();
        assert_eq!(strct.kind, SymbolKind::Struct);

        let method = result.symbols.iter().find(|s| s.name == "Area").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.qualified_name, "Point.Area");
    }

    #[test]
    fn extracts_import_with_alias() {
        let result = extract("package main\n\nimport (\n\t\"fmt\"\n\tbar \"example.com/bar\"\n)\n");
        assert_eq!(result.imports.len(), 2);
        let fmt = result
            .imports
            .iter()
            .find(|i| i.local_name == "fmt")
            .unwrap();
        assert_eq!(fmt.source_module, "fmt");
        let bar = result
            .imports
            .iter()
            .find(|i| i.local_name == "bar")
            .unwrap();
        assert_eq!(bar.source_module, "example.com/bar");
    }

    #[test]
    fn extracts_call_reference_with_enclosing_symbol() {
        let source = "package main\n\nfunc helper(x int) int {\n\treturn x\n}\n\nfunc caller() int {\n\treturn helper(1)\n}\n";
        let result = extract(source);
        let call = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Call && r.to_name == "helper")
            .unwrap();
        assert_eq!(call.from_symbol_name.as_deref(), Some("caller"));
    }

    #[test]
    fn extracts_composite_literal_instantiation() {
        let source = "package main\n\ntype Point struct {\n\tX int\n}\n\nfunc make() Point {\n\treturn Point{X: 1}\n}\n";
        let result = extract(source);
        let inst = result
            .references
            .iter()
            .find(|r| r.kind == ReferenceKind::Instantiation)
            .unwrap();
        assert_eq!(inst.to_name, "Point");
    }

    #[test]
    fn extracts_interface_embedding_as_extends() {
        let source = "package main\n\ntype Shape interface {\n\tArea() float64\n}\n\ntype Named interface {\n\tShape\n\tName() string\n}\n";
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
        let result =
            extract("package main\n\nfunc broken(x int {\n\treturn x\n}\n\nfunc Ok() {}\n");
        assert!(!result.warnings.is_empty());
        assert!(result.symbols.iter().any(|s| s.name == "Ok"));
    }

    #[test]
    fn empty_file_yields_no_symbols() {
        let result = extract("");
        assert!(result.symbols.is_empty());
    }
}
