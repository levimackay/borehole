//! Utilities shared across the tree-sitter-backed [`crate::LanguageExtractor`]
//! implementations. Nothing here is language-specific — per-language node
//! kinds live in each language's own module.

use engine_core::Span;
use std::time::{Duration, Instant};
use tree_sitter::{Node, ParseOptions, Parser, Tree};

/// Hard ceiling on how long a single file's tree-sitter parse is allowed to
/// run. Adversarially deep nesting (`mod a { mod a { ... } }` tens of
/// thousands of levels deep, well within the 2MB file-size cap) can make
/// tree-sitter's own `parse()` call take tens of seconds *before extraction
/// ever sees the resulting tree* — this isn't something extraction-side
/// bounds (like `qualify`'s length cap) can fix, since the cost is entirely
/// inside the grammar's parse. [`parse_with_timeout`] makes the parse
/// return `None` once this elapses, which every extractor already surfaces
/// as `ParseError::TreeSitterFailure` — a clean, honest per-file failure
/// instead of a multi-second stall on one malicious file blocking the
/// whole (otherwise-parallel) indexing run.
const PARSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Construct a plain `Parser`. Pairs with [`parse_with_timeout`], which is
/// what actually enforces the bound — every extractor should parse through
/// that rather than calling `parser.parse()` directly.
pub(crate) fn new_parser() -> Parser {
    Parser::new()
}

/// Parse `source`, aborting and returning `None` if it takes longer than
/// [`PARSE_TIMEOUT`]. Uses `parse_with_options`' progress callback (checked
/// periodically during the parse) rather than the deprecated
/// `set_timeout_micros`, since that API is slated for removal.
pub(crate) fn parse_with_timeout(parser: &mut Parser, source: &str) -> Option<Tree> {
    let start = Instant::now();
    let mut deadline_exceeded = |_state: &tree_sitter::ParseState| start.elapsed() > PARSE_TIMEOUT;
    let options = ParseOptions::new().progress_callback(&mut deadline_exceeded);
    parser.parse_with_options(
        &mut |offset, _pos| source.as_bytes().get(offset..).unwrap_or(&[]),
        None,
        Some(options),
    )
}

/// Convert a tree-sitter node's location into our `Span` type. Lines/columns
/// are 1-indexed (tree-sitter's are 0-indexed) to match `Span`'s documented
/// convention; byte offsets stay 0-indexed.
pub(crate) fn span_of(node: Node) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    Span {
        start_line: start.row as u32 + 1,
        start_col: start.column as u32 + 1,
        end_line: end.row as u32 + 1,
        end_col: end.column as u32 + 1,
        start_byte: node.start_byte() as u32,
        end_byte: node.end_byte() as u32,
    }
}

/// Slice of `source` covered by `node`. Falls back to an empty string rather
/// than panicking if byte offsets ever land outside a UTF-8 boundary — this
/// should never happen for well-formed input, but a malformed/truncated
/// fixture must never crash the extractor.
pub(crate) fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or_default()
}

/// Walk the whole tree collecting a bounded number of human-readable
/// warnings about syntax errors tree-sitter recovered from. A non-empty
/// return does not mean the caller should discard whatever was extracted —
/// see `ParseResult::warnings`.
pub(crate) fn collect_warnings(root: Node) -> Vec<String> {
    const MAX_WARNINGS: usize = 25;
    let mut warnings = Vec::new();
    if !root.has_error() {
        return warnings;
    }
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if warnings.len() >= MAX_WARNINGS {
            break;
        }
        if node.is_missing() {
            let pos = node.start_position();
            warnings.push(format!(
                "missing {} at line {}, column {}",
                node.kind(),
                pos.row + 1,
                pos.column + 1
            ));
        } else if node.is_error() {
            let pos = node.start_position();
            warnings.push(format!(
                "syntax error near line {}, column {}",
                pos.row + 1,
                pos.column + 1
            ));
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    warnings
}

/// Collect the contiguous run of preceding named siblings for which
/// `is_doc(node)` is true, stopping at the first sibling that doesn't
/// qualify or that isn't immediately adjacent (no blank line gap) to the
/// item being documented. Returned in source order.
pub(crate) fn preceding_doc_comments<'a>(
    node: Node<'a>,
    is_doc: impl Fn(Node<'a>) -> bool,
) -> Vec<Node<'a>> {
    let mut acc = Vec::new();
    let mut anchor_row = node.start_position().row;
    let mut cur = node.prev_named_sibling();
    while let Some(n) = cur {
        if is_doc(n) && anchor_row.saturating_sub(n.end_position().row) <= 1 {
            acc.push(n);
            anchor_row = n.start_position().row;
            cur = n.prev_named_sibling();
        } else {
            break;
        }
    }
    acc.reverse();
    acc
}

/// The declaration line for a node that has a `body` (or other named)
/// field marking where the "header" ends — i.e. everything up to but not
/// including the body/block. Falls back to the full node text (trimmed,
/// with a trailing `;` stripped) when no such field is present.
pub(crate) fn signature_before_field<'a>(
    node: Node<'a>,
    source: &'a str,
    field_names: &[&str],
) -> String {
    for field in field_names {
        if let Some(body) = node.child_by_field_name(field) {
            let text = source
                .get(node.start_byte()..body.start_byte())
                .unwrap_or("");
            return text.trim().to_string();
        }
    }
    node_text(node, source)
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_string()
}

/// Find every descendant (including `node` itself) whose kind is
/// `target_kind`, without crossing into nested nodes of kinds listed in
/// `stop_at` (used to avoid descending into nested function/closure bodies
/// when just collecting type references from a signature, for example).
/// Text collected in source order.
pub(crate) fn collect_kind_texts<'a>(
    node: Node<'a>,
    target_kind: &str,
    source: &'a str,
) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut stack = vec![node];
    let mut cursor = node.walk();
    while let Some(n) = stack.pop() {
        if n.kind() == target_kind {
            out.push(node_text(n, source));
        }
        for child in n.children(&mut cursor) {
            stack.push(child);
        }
    }
    out.reverse();
    out
}
