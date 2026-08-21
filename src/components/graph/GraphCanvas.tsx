import { useMemo } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  type Node,
  type Edge,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { Confidence, DependencyGraph } from "../../lib/ipc-types";
import { formatKind } from "../../lib/format";
import "./GraphCanvas.css";

const CONFIDENCE_STROKE: Record<Confidence, string> = {
  high: "var(--bh-confidence-high)",
  medium: "var(--bh-confidence-medium)",
  low: "var(--bh-confidence-low)",
};

const DEPTH_COLUMN_WIDTH = 240;
const ROW_HEIGHT = 64;

function SymbolNode({ data }: NodeProps<Node<{ name: string; kind: string; isRoot: boolean }>>) {
  return (
    <div className={`bh-graph-node${data.isRoot ? " bh-graph-node--root" : ""}`}>
      <span className="bh-graph-node__kind">{data.kind}</span>
      <span className="bh-graph-node__name bh-mono">{data.name}</span>
    </div>
  );
}

const nodeTypes = { symbol: SymbolNode };

/**
 * Basic tiered layout by `GraphNode.depth` — nodes fan out left-to-right by
 * distance from the root, stacked vertically within a depth column. Not a
 * force-directed layout; fine for the depths this graph realistically
 * returns, a known gap for anything wide.
 */
export function GraphCanvas({ graph }: { graph: DependencyGraph }) {
  const { nodes, edges } = useMemo(() => {
    const byDepth = new Map<number, typeof graph.nodes>();
    for (const n of graph.nodes) {
      const list = byDepth.get(n.depth) ?? [];
      list.push(n);
      byDepth.set(n.depth, list);
    }

    const flowNodes: Node[] = [];
    for (const [depth, group] of byDepth) {
      group.forEach((n, i) => {
        flowNodes.push({
          id: String(n.symbol.id),
          type: "symbol",
          position: { x: depth * DEPTH_COLUMN_WIDTH, y: i * ROW_HEIGHT },
          data: {
            name: n.symbol.name,
            kind: formatKind(n.symbol.kind),
            isRoot: n.symbol.id === graph.root,
          },
          draggable: true,
        });
      });
    }

    const flowEdges: Edge[] = graph.edges.map((e, i) => ({
      id: `${e.from}-${e.to}-${i}`,
      source: String(e.from),
      target: String(e.to),
      label: e.reference.kind,
      style: { stroke: CONFIDENCE_STROKE[e.reference.confidence] },
      labelStyle: { fill: "var(--bh-text-secondary)", fontSize: 10 },
    }));

    return { nodes: flowNodes, edges: flowEdges };
  }, [graph]);

  if (nodes.length === 0) {
    return <p className="bh-graph-canvas__empty">No nodes to display.</p>;
  }

  return (
    <div className="bh-graph-canvas">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        fitView
        proOptions={{ hideAttribution: true }}
      >
        <Background gap={20} color="var(--bh-border)" />
        <Controls showInteractive={false} />
      </ReactFlow>
      {graph.truncated && (
        <p className="bh-graph-canvas__truncated">
          Result truncated by the backend — this is a partial graph, not the full picture.
        </p>
      )}
    </div>
  );
}
