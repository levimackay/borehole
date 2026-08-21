//! Relationship graph traversal over `engine-index`: callers/callees
//! expansion, subclass/implementation discovery, and the node/edge shape
//! the desktop UI's interactive graph view renders directly.
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts").

use engine_core::{Reference, Symbol, SymbolId};
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

/// Breadth-first expansion of callers (who references this symbol),
/// bounded by `max_depth`. Cycle-safe: a symbol already visited is never
/// re-added as a node, even if reached by a second path (its edge is still
/// recorded).
pub fn expand_callers(index: &Index, root: SymbolId, max_depth: u32) -> Result<DependencyGraph> {
    let _ = (index, root, max_depth);
    todo!("engine-graph: expand_callers — BFS over Index::references_to, see module docs")
}

/// Breadth-first expansion of callees (what this symbol references),
/// bounded by `max_depth`. Same cycle-safety contract as
/// [`expand_callers`].
pub fn expand_callees(index: &Index, root: SymbolId, max_depth: u32) -> Result<DependencyGraph> {
    let _ = (index, root, max_depth);
    todo!("engine-graph: expand_callees — BFS over Index::references_from, see module docs")
}

/// Symbols whose `Extends`/`Implements` references point at `root`.
pub fn subclasses_of(index: &Index, root: SymbolId) -> Result<Vec<Symbol>> {
    let _ = (index, root);
    todo!("engine-graph: subclasses_of — filter Index::references_to by ReferenceKind")
}

/// Shared cycle-guard for BFS implementations in this module.
pub(crate) fn new_visited() -> HashSet<SymbolId> {
    HashSet::new()
}
