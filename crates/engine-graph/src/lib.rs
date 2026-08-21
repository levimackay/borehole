//! Relationship graph traversal over `engine-index`: callers/callees
//! expansion, subclass/implementation discovery, and the node/edge shape
//! the desktop UI's interactive graph view renders directly.
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts").

use engine_core::{Reference, ReferenceKind, Symbol, SymbolId};
use engine_index::Index;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error(transparent)]
    Index(#[from] engine_index::IndexError),
    #[error("symbol {0} not found")]
    SymbolNotFound(SymbolId),
}

pub type Result<T> = std::result::Result<T, GraphError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub symbol: Symbol,
    /// Hops from the traversal's root (0 for the root itself).
    pub depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: SymbolId,
    pub to: SymbolId,
    pub reference: Reference,
}

/// A bounded-depth subgraph, ready for the UI's zoom/pan/expand/collapse
/// graph view — every node and edge here corresponds to at least one real
/// `Reference` row, never a synthesized/decorative connection (brief
/// section 12: "Every node/edge should correspond to actual evidence").
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub root: Option<SymbolId>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// True if traversal stopped early because `max_depth` was reached,
    /// not because there was nothing more to find — the UI should offer
    /// "expand further" rather than implying the graph is complete.
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Callers,
    Callees,
}

/// Breadth-first expansion of callers (who references this symbol),
/// bounded by `max_depth`. Cycle-safe: a symbol already visited is never
/// re-added as a node, even if reached by a second path (its edge is still
/// recorded).
pub fn expand_callers(index: &Index, root: SymbolId, max_depth: u32) -> Result<DependencyGraph> {
    build_graph(index, root, max_depth, Direction::Callers)
}

/// Breadth-first expansion of callees (what this symbol references),
/// bounded by `max_depth`. Same cycle-safety contract as
/// [`expand_callers`].
pub fn expand_callees(index: &Index, root: SymbolId, max_depth: u32) -> Result<DependencyGraph> {
    build_graph(index, root, max_depth, Direction::Callees)
}

fn fetch_refs(index: &Index, id: SymbolId, dir: Direction) -> engine_index::Result<Vec<Reference>> {
    match dir {
        Direction::Callers => index.references_to(id),
        Direction::Callees => index.references_from(id),
    }
}

/// The neighbor a reference introduces, for this traversal direction:
/// for callers, the caller (`from_symbol`); for callees, the callee
/// (`to_symbol`). `None` when that end of the reference didn't resolve to
/// a known symbol — nothing to add to the graph.
fn neighbor_of(r: &Reference, dir: Direction) -> Option<SymbolId> {
    match dir {
        Direction::Callers => r.from_symbol,
        Direction::Callees => r.to_symbol,
    }
}

fn build_graph(
    index: &Index,
    root: SymbolId,
    max_depth: u32,
    dir: Direction,
) -> Result<DependencyGraph> {
    let root_symbol = index
        .find_symbol(root)?
        .ok_or(GraphError::SymbolNotFound(root))?;

    let mut visited: HashSet<SymbolId> = new_visited();
    visited.insert(root);
    let mut nodes = vec![GraphNode {
        symbol: root_symbol,
        depth: 0,
    }];
    let mut edges = Vec::new();
    let mut frontier = vec![root];
    let mut truncated = false;
    let mut depth = 0u32;

    while !frontier.is_empty() {
        if depth >= max_depth {
            // Depth budget exhausted. Peek (without adding to the graph)
            // whether any node still on the frontier actually has further
            // edges — only then is this a real truncation, not merely
            // "we stopped exactly when there was nothing left anyway".
            truncated = frontier.iter().any(|&id| {
                fetch_refs(index, id, dir)
                    .map(|refs| refs.iter().any(|r| neighbor_of(r, dir).is_some()))
                    .unwrap_or(false)
            });
            break;
        }

        let mut next_frontier = Vec::new();
        for &sym_id in &frontier {
            let refs = fetch_refs(index, sym_id, dir)?;
            for r in refs {
                let Some(neighbor_id) = neighbor_of(&r, dir) else {
                    continue;
                };
                let (from, to) = match dir {
                    Direction::Callers => (neighbor_id, sym_id),
                    Direction::Callees => (sym_id, neighbor_id),
                };
                // Edge is recorded every time it's found, even if the
                // neighbor was already visited via a different path.
                edges.push(GraphEdge {
                    from,
                    to,
                    reference: r,
                });
                if visited.insert(neighbor_id) {
                    if let Some(sym) = index.find_symbol(neighbor_id)? {
                        nodes.push(GraphNode {
                            symbol: sym,
                            depth: depth + 1,
                        });
                    }
                    next_frontier.push(neighbor_id);
                }
            }
        }
        frontier = next_frontier;
        depth += 1;
    }

    Ok(DependencyGraph {
        root: Some(root),
        nodes,
        edges,
        truncated,
    })
}

/// Symbols whose `Extends`/`Implements` references point at `root`,
/// transitively — a subclass of a subclass of `root` still counts (see the
/// `inheritance` fixture: `subclasses_of(Shape)` finds both the class that
/// extends it directly and the class that extends *that* class).
/// Cycle-safe via the same visited-set discipline as [`expand_callers`]/
/// [`expand_callees`]; unbounded depth since class hierarchies are shallow
/// in practice and the frozen signature takes no `max_depth`.
pub fn subclasses_of(index: &Index, root: SymbolId) -> Result<Vec<Symbol>> {
    let mut visited: HashSet<SymbolId> = new_visited();
    visited.insert(root);
    let mut out = Vec::new();
    let mut frontier = vec![root];

    while !frontier.is_empty() {
        let mut next_frontier = Vec::new();
        for id in frontier {
            let refs = index.references_to(id)?;
            for r in refs {
                if !matches!(r.kind, ReferenceKind::Extends | ReferenceKind::Implements) {
                    continue;
                }
                let Some(from_id) = r.from_symbol else {
                    continue;
                };
                if visited.insert(from_id) {
                    if let Some(sym) = index.find_symbol(from_id)? {
                        out.push(sym);
                    }
                    next_frontier.push(from_id);
                }
            }
        }
        frontier = next_frontier;
    }

    Ok(out)
}

/// Shared cycle-guard for BFS implementations in this module.
pub(crate) fn new_visited() -> HashSet<SymbolId> {
    HashSet::new()
}
