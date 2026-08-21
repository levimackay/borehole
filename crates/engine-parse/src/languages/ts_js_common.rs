//! Shared extraction logic for TypeScript, TSX and JavaScript. All three
//! grammars (tree-sitter-typescript's two dialects and tree-sitter-javascript)
//! agree on node kinds/field names for the subset of syntax this module
//! cares about (confirmed against real parses of representative samples for
//! each grammar — see the individual dumps referenced from `rust.rs`'s
//! header comment for the general approach). The one structural difference
//! observed between the JS and TS class-body shapes is that TS class members
//! are unnamed children while JS class members carry a `member` field; this
//! module iterates class bodies by kind rather than by field name so both
//! shapes work unmodified.
//!
//! TypeScript-only constructs (`interface`, `type X = ...`, `enum`,
//! `implements`) are handled unconditionally — on a JS file that syntax
//! simply never occurs, so the extra match arms are inert there.
//!
//! Known limitations (documented, not silent gaps):
//! - `qualified_name` reflects only intra-file nesting (`Class.method`), not
//!   a file/module-path prefix — full module-qualified paths need
//!   repo-layout knowledge that belongs to `engine-index`, not this crate.
//! - `import * as ns from "..."` (namespace imports) and re-export forms
//!   (`export { foo } from "..."`) are not extracted — the grammar shapes
//!   weren't in the grounding sample and guessing node kinds is exactly
//!   what this crate must not do.
//! - `export default <expression>` without a name is not extracted.
//! - `private`/`protected` class members are always treated as not
//!   exported (grounded via a real parse showing `accessibility_modifier`
//!   as the member's first named child); everything else mirrors the
//!   enclosing class's exported-ness, since JS/TS has no per-member
//!   equivalent of Rust's `pub`.

use crate::languages::support::{
    collect_kind_texts, collect_warnings, node_text, preceding_doc_comments,
    signature_before_field, span_of,
};
use crate::{ImportEdge, ParseError, ParseResult, ParsedReference, ParsedSymbol};
use engine_core::{ReferenceKind, RepoPath, Span, SymbolKind};
use tree_sitter::{Language as TsLanguage, Node, Parser};

pub(crate) fn extract(
    source: &str,
    file: &RepoPath,
    grammar: TsLanguage,
    is_typescript: bool,
) -> Result<ParseResult, ParseError> {
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|_| ParseError::TreeSitterFailure { file: file.clone() })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| ParseError::TreeSitterFailure { file: file.clone() })?;
    let root = tree.root_node();

    let mut result = ParseResult {
        warnings: collect_warnings(root),
        ..Default::default()
    };

    walk_statements(root, source, None, is_typescript, &mut result);
    walk_refs(root, source, None, &mut result.references);

    Ok(result)
}

fn qualify(qualifier: Option<&str>, name: &str) -> String {
    match qualifier {
        Some(q) if !q.is_empty() => format!("{q}.{name}"),
        _ => name.to_string(),
    }
}

fn doc_comment_for<'a>(node: Node<'a>, source: &'a str) -> Option<String> {
    let comments = preceding_doc_comments(node, |n| n.kind() == "comment");
    if comments.is_empty() {
        return None;
    }
    let lines: Vec<String> = comments
        .into_iter()
        .flat_map(|c| {
            let text = node_text(c, source);
            let stripped = text
                .trim()
                .trim_start_matches("/**")
                .trim_start_matches("/*")
                .trim_start_matches("//")
                .trim_end_matches("*/");
            stripped
                .lines()
                .map(|l| l.trim().trim_start_matches('*').trim())
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    let joined = lines.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn field_text<'a>(node: Node<'a>, field: &str, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name(field)
        .map(|n| node_text(n, source))
}

fn string_literal_value(node: Node, source: &str) -> String {
    collect_kind_texts(node, "string_fragment", source)
        .first()
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn push_type_usages(
    type_node: Node,
    source: &str,
    from_symbol: Option<&str>,
    refs: &mut Vec<ParsedReference>,
) {
    for name in collect_kind_texts(type_node, "type_identifier", source) {
        refs.push(ParsedReference {
            from_span: span_of(type_node),
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
    if let Some(params) = fn_node.child_by_field_name("parameters") {
        push_type_usages(params, source, Some(name), refs);
    }
    if let Some(ret) = fn_node.child_by_field_name("return_type") {
        push_type_usages(ret, source, Some(name), refs);
    }
}

/// Statement-list level extraction: symbols, imports, and class/interface
/// heritage relationships. Handles `export_statement` by unwrapping to the
/// underlying declaration and marking it exported.
#[allow(clippy::too_many_lines)]
fn walk_statements(
    container: Node,
    source: &str,
    qualifier: Option<&str>,
    is_typescript: bool,
    out: &mut ParseResult,
) {
    let mut cursor = container.walk();
    let children: Vec<Node> = container.named_children(&mut cursor).collect();
    for child in children {
        match child.kind() {
            "import_statement" => extract_import(child, source, &mut out.imports),
            "export_statement" => {
                if let Some(decl) = child.child_by_field_name("declaration") {
                    extract_declaration(decl, source, qualifier, true, is_typescript, out);
                }
                // `export { a, b }` and `export default <expr>` (no
                // `declaration` field) are not extracted — see module docs.
            }
            _ => extract_declaration(child, source, qualifier, false, is_typescript, out),
        }
    }
}

#[allow(clippy::too_many_lines)]
fn extract_declaration(
    node: Node,
    source: &str,
    qualifier: Option<&str>,
    exported: bool,
    is_typescript: bool,
    out: &mut ParseResult,
) {
    match node.kind() {
        "function_declaration" => {
            let Some(name) = field_text(node, "name", source) else {
                return;
            };
            let name = name.to_string();
            out.symbols.push(ParsedSymbol {
                name: name.clone(),
                qualified_name: qualify(qualifier, &name),
                kind: SymbolKind::Function,
                span: span_of(node),
                is_exported: exported,
                doc_comment: doc_comment_for(node, source),
                signature: Some(signature_before_field(node, source, &["body"])),
            });
            push_signature_type_usages(node, source, &name, &mut out.references);
        }
        "class_declaration" => {
            let Some(name) = field_text(node, "name", source) else {
                return;
            };
            let name = name.to_string();
            out.symbols.push(ParsedSymbol {
                name: name.clone(),
                qualified_name: qualify(qualifier, &name),
                kind: SymbolKind::Class,
                span: span_of(node),
                is_exported: exported,
                doc_comment: doc_comment_for(node, source),
                signature: Some(signature_before_field(node, source, &["body"])),
            });
            let mut cur = node.walk();
            for heritage in node.named_children(&mut cur) {
                if heritage.kind() != "class_heritage" {
                    continue;
                }
                let mut hc = heritage.walk();
                for clause in heritage.named_children(&mut hc) {
                    match clause.kind() {
                        // TypeScript wraps the base class in an
                        // `extends_clause` (`value` field); the JavaScript
                        // grammar has no such wrapper and puts the base
                        // class expression directly under `class_heritage`
                        // — both grounded against real parses.
                        "extends_clause" => {
                            if let Some(value) = clause.child_by_field_name("value") {
                                out.references.push(ParsedReference {
                                    from_span: span_of(clause),
                                    from_symbol_name: None,
                                    to_name: node_text(value, source).to_string(),
                                    kind: ReferenceKind::Extends,
                                });
                            }
                        }
                        "identifier" | "member_expression" => {
                            out.references.push(ParsedReference {
                                from_span: span_of(clause),
                                from_symbol_name: None,
                                to_name: node_text(clause, source).to_string(),
                                kind: ReferenceKind::Extends,
                            });
                        }
                        "implements_clause" => {
                            for type_name in collect_kind_texts(clause, "type_identifier", source) {
                                out.references.push(ParsedReference {
                                    from_span: span_of(clause),
                                    from_symbol_name: None,
                                    to_name: type_name.to_string(),
                                    kind: ReferenceKind::Implements,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            if let Some(body) = node.child_by_field_name("body") {
                let new_qualifier = qualify(qualifier, &name);
                let mut bc = body.walk();
                for member in body.named_children(&mut bc) {
                    extract_class_member(member, source, &new_qualifier, exported, out);
                }
            }
        }
        "interface_declaration" if is_typescript => {
            let Some(name) = field_text(node, "name", source) else {
                return;
            };
            let name = name.to_string();
            out.symbols.push(ParsedSymbol {
                name: name.clone(),
                qualified_name: qualify(qualifier, &name),
                kind: SymbolKind::Interface,
                span: span_of(node),
                is_exported: exported,
                doc_comment: doc_comment_for(node, source),
                signature: Some(signature_before_field(node, source, &["body"])),
            });
            if let Some(body) = node.child_by_field_name("body") {
                let new_qualifier = qualify(qualifier, &name);
                let mut bc = body.walk();
                for member in body.named_children(&mut bc) {
                    if member.kind() == "method_signature" {
                        if let Some(mname) = field_text(member, "name", source) {
                            let mname = mname.to_string();
                            out.symbols.push(ParsedSymbol {
                                name: mname.clone(),
                                qualified_name: qualify(Some(&new_qualifier), &mname),
                                kind: SymbolKind::Method,
                                span: span_of(member),
                                is_exported: exported,
                                doc_comment: doc_comment_for(member, source),
                                signature: Some(signature_before_field(member, source, &[])),
                            });
                            push_signature_type_usages(member, source, &mname, &mut out.references);
                        }
                    }
                }
            }
        }
        "type_alias_declaration" if is_typescript => {
            let Some(name) = field_text(node, "name", source) else {
                return;
            };
            out.symbols.push(ParsedSymbol {
                name: name.to_string(),
                qualified_name: qualify(qualifier, name),
                kind: SymbolKind::TypeAlias,
                span: span_of(node),
                is_exported: exported,
                doc_comment: doc_comment_for(node, source),
                signature: Some(signature_before_field(node, source, &[])),
            });
            if let Some(value) = node.child_by_field_name("value") {
                push_type_usages(value, source, None, &mut out.references);
            }
        }
        "enum_declaration" if is_typescript => {
            let Some(name) = field_text(node, "name", source) else {
                return;
            };
            out.symbols.push(ParsedSymbol {
                name: name.to_string(),
                qualified_name: qualify(qualifier, name),
                kind: SymbolKind::Enum,
                span: span_of(node),
                is_exported: exported,
                doc_comment: doc_comment_for(node, source),
                signature: Some(signature_before_field(node, source, &["body"])),
            });
        }
        "lexical_declaration" | "variable_declaration" => {
            let mut cur = node.walk();
            for declarator in node.named_children(&mut cur) {
                if declarator.kind() != "variable_declarator" {
                    continue;
                }
                let Some(name) = field_text(declarator, "name", source) else {
                    continue;
                };
                let name = name.to_string();
                let value_kind = declarator.child_by_field_name("value").map(|v| v.kind());
                let is_function_like = matches!(
                    value_kind,
                    Some("arrow_function") | Some("function_expression") | Some("function")
                );
                let kind = if is_function_like {
                    SymbolKind::Function
                } else if node_text(node, source).trim_start().starts_with("const") {
                    SymbolKind::Constant
                } else {
                    SymbolKind::Variable
                };
                let signature = if is_function_like {
                    let value = declarator.child_by_field_name("value").unwrap();
                    format!(
                        "{} {}",
                        node_text(node, source)
                            .split_whitespace()
                            .next()
                            .unwrap_or("const"),
                        signature_before_field(value, source, &["body"])
                    )
                    .trim()
                    .to_string()
                } else {
                    signature_before_field(node, source, &[])
                };
                out.symbols.push(ParsedSymbol {
                    name: name.clone(),
                    qualified_name: qualify(qualifier, &name),
                    kind,
                    span: span_of(declarator),
                    is_exported: exported,
                    doc_comment: doc_comment_for(node, source),
                    signature: Some(signature),
                });
                if is_function_like {
                    let value = declarator.child_by_field_name("value").unwrap();
                    push_signature_type_usages(value, source, &name, &mut out.references);
                }
            }
        }
        _ => {}
    }
}

/// A `private`/`protected` member is never part of the public interface,
/// regardless of the enclosing class's own exported-ness.
fn is_private_or_protected(member: Node) -> bool {
    member
        .named_child(0)
        .filter(|c| c.kind() == "accessibility_modifier")
        .map(|c| c.child(0).map(|k| k.kind()).unwrap_or(""))
        .map(|kw| kw == "private" || kw == "protected")
        .unwrap_or(false)
}

fn extract_class_member(
    member: Node,
    source: &str,
    class_qualifier: &str,
    class_exported: bool,
    out: &mut ParseResult,
) {
    let exported = class_exported && !is_private_or_protected(member);
    match member.kind() {
        "method_definition" => {
            let Some(name) = field_text(member, "name", source) else {
                return;
            };
            let name = name.to_string();
            out.symbols.push(ParsedSymbol {
                name: name.clone(),
                qualified_name: qualify(Some(class_qualifier), &name),
                kind: SymbolKind::Method,
                span: span_of(member),
                is_exported: exported,
                doc_comment: doc_comment_for(member, source),
                signature: Some(signature_before_field(member, source, &["body"])),
            });
            push_signature_type_usages(member, source, &name, &mut out.references);
        }
        "public_field_definition" | "field_definition" => {
            let Some(name) = field_text(member, "name", source) else {
                return;
            };
            out.symbols.push(ParsedSymbol {
                name: name.to_string(),
                qualified_name: qualify(Some(class_qualifier), name),
                kind: SymbolKind::Variable,
                span: span_of(member),
                is_exported: exported,
                doc_comment: doc_comment_for(member, source),
                signature: Some(signature_before_field(member, source, &["value"])),
            });
            if let Some(ty) = member.child_by_field_name("type") {
                push_type_usages(ty, source, Some(class_qualifier), &mut out.references);
            }
        }
        _ => {}
    }
}

fn extract_import(stmt: Node, source: &str, imports: &mut Vec<ImportEdge>) {
    let Some(source_node) = stmt.child_by_field_name("source") else {
        return;
    };
    let module = string_literal_value(source_node, source);
    let span = span_of(stmt);
    extract_import_clause(stmt, source, &module, span, imports);
}

fn extract_import_clause(
    stmt: Node,
    source: &str,
    module: &str,
    span: Span,
    imports: &mut Vec<ImportEdge>,
) {
    let mut cursor = stmt.walk();
    for child in stmt.named_children(&mut cursor) {
        if child.kind() != "import_clause" {
            continue;
        }
        let mut cc = child.walk();
        for part in child.named_children(&mut cc) {
            match part.kind() {
                "identifier" => {
                    let name = node_text(part, source).to_string();
                    imports.push(ImportEdge {
                        local_name: name,
                        source_module: module.to_string(),
                        imported_name: "default".to_string(),
                        span,
                    });
                }
                "named_imports" => {
                    let mut nc = part.walk();
                    for spec in part.named_children(&mut nc) {
                        if spec.kind() != "import_specifier" {
                            continue;
                        }
                        let Some(name) = field_text(spec, "name", source) else {
                            continue;
                        };
                        let alias = field_text(spec, "alias", source);
                        imports.push(ImportEdge {
                            local_name: alias.unwrap_or(name).to_string(),
                            source_module: module.to_string(),
                            imported_name: name.to_string(),
                            span,
                        });
                    }
                }
                _ => {}
            }
        }
    }
}

fn call_target<'a>(function_node: Node<'a>, source: &'a str) -> &'a str {
    match function_node.kind() {
        "member_expression" => field_text(function_node, "property", source).unwrap_or(""),
        _ => node_text(function_node, source),
    }
}

fn walk_refs(node: Node, source: &str, current: Option<&str>, refs: &mut Vec<ParsedReference>) {
    let next_current: Option<String> = match node.kind() {
        "function_declaration" | "method_definition" => {
            field_text(node, "name", source).map(|s| s.to_string())
        }
        "variable_declarator" => {
            let value_kind = node.child_by_field_name("value").map(|v| v.kind());
            if matches!(
                value_kind,
                Some("arrow_function") | Some("function_expression") | Some("function")
            ) {
                field_text(node, "name", source).map(|s| s.to_string())
            } else {
                current.map(|s| s.to_string())
            }
        }
        _ => current.map(|s| s.to_string()),
    };
    let next_ref = next_current.as_deref().or(current);

    match node.kind() {
        "call_expression" => {
            if let Some(func) = node.child_by_field_name("function") {
                refs.push(ParsedReference {
                    from_span: span_of(node),
                    from_symbol_name: next_ref.map(|s| s.to_string()),
                    to_name: call_target(func, source).to_string(),
                    kind: ReferenceKind::Call,
                });
            }
        }
        "new_expression" => {
            if let Some(ctor) = node.child_by_field_name("constructor") {
                refs.push(ParsedReference {
                    from_span: span_of(node),
                    from_symbol_name: next_ref.map(|s| s.to_string()),
                    to_name: node_text(ctor, source).to_string(),
                    kind: ReferenceKind::Instantiation,
                });
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_refs(child, source, next_ref, refs);
    }
}
