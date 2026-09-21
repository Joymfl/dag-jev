import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import cytoscape from "cytoscape";

type Edge = { id: string; source: string; target: string; weight: number };
type GraphJson = {
  nodes: { id: string; label: string }[];
  edges: Edge[];
};

// Drop edges already implied by a longer path (a->c when a->b->c exists).
// Weakest edges are tried first, so when two edges are mutually redundant
// (possible inside a cycle) only the weaker one goes.
function transitiveReduction(edges: Edge[]): Edge[] {
  const adj = new Map<string, Set<string>>();
  for (const e of edges) {
    if (!adj.has(e.source)) adj.set(e.source, new Set());
    adj.get(e.source)!.add(e.target);
  }
  const reachable = (from: string, to: string) => {
    const seen = new Set<string>([from]);
    const stack = [from];
    while (stack.length) {
      for (const next of adj.get(stack.pop()!) ?? []) {
        if (next === to) return true;
        if (!seen.has(next)) {
          seen.add(next);
          stack.push(next);
        }
      }
    }
    return false;
  };
  const redundant = new Set<string>();
  for (const e of [...edges].sort((a, b) => a.weight - b.weight)) {
    adj.get(e.source)!.delete(e.target);
    if (reachable(e.source, e.target)) redundant.add(e.id);
    else adj.get(e.source)!.add(e.target);
  }
  return edges.filter((e) => !redundant.has(e.id));
}

export function App() {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);
  const layoutRef = useRef<cytoscape.Layouts | null>(null);
  const [data, setData] = useState<GraphJson | null>(null);
  const [threshold, setThreshold] = useState(0.65);
  const [reduce, setReduce] = useState(true);

  useEffect(() => {
    fetch("/graph.json")
      .then((r) => r.json())
      .then(setData)
      .catch((e) => console.error("Failed to load graph.json", e));
  }, []);

  useEffect(() => {
    if (!containerRef.current) return;
    const cy = cytoscape({
      container: containerRef.current,
      minZoom: 0.2,
      maxZoom: 3,
      style: [
        {
          selector: "node",
          style: {
            shape: "round-rectangle",
            width: 190,
            height: 64,
            "background-color": "#eef2f8",
            "border-width": 1,
            "border-color": "#4c78a8",
            label: "data(label)",
            color: "#1b1f27",
            "font-size": 11,
            "text-wrap": "wrap",
            "text-max-width": "176px",
            "text-valign": "center",
            "text-halign": "center",
          },
        },
        {
          selector: "edge",
          style: {
            width: "mapData(weight, 0, 1, 1, 3)",
            "line-color": "#8a93a3",
            "target-arrow-color": "#8a93a3",
            "target-arrow-shape": "triangle",
            "arrow-scale": 0.9,
            "curve-style": "bezier",
            opacity: (ele: cytoscape.EdgeSingular) =>
              0.4 + 0.6 * ele.data("weight"),
          },
        },
        {
          selector: ".faded",
          style: { opacity: 0.12 },
        },
        {
          selector: "node.focus",
          style: { "border-width": 2, "background-color": "#dbe6f5" },
        },
      ],
    });
    cy.on("mouseover", "node", (evt) => {
      cy.elements().addClass("faded");
      evt.target.closedNeighborhood().removeClass("faded");
      evt.target.addClass("focus");
    });
    cy.on("mouseout", "node", () => {
      cy.elements().removeClass("faded focus");
    });
    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, []);

  const visibleEdges = useMemo(() => {
    if (!data) return [];
    // String() guards against numeric ids in older graph.json dumps
    const passing = data.edges
      .filter((e) => e.weight >= threshold)
      .map((e) => ({ ...e, source: String(e.source), target: String(e.target) }));
    return reduce ? transitiveReduction(passing) : passing;
  }, [data, threshold, reduce]);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !data) return;
    layoutRef.current?.stop();
    cy.elements().remove();
    cy.add([
      ...data.nodes.map((n) => ({ data: { id: n.id, label: n.label } })),
      ...visibleEdges.map((e) => ({ data: e })),
    ]);
    const layout = cy.layout({
      name: "elk",
      animate: true,
      animationDuration: 300,
      fit: true,
      padding: 40,
      elk: {
        algorithm: "layered",
        "elk.direction": "RIGHT",
        "elk.spacing.nodeNode": 30,
        "elk.layered.spacing.nodeNodeBetweenLayers": 80,
        "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
      },
    } as cytoscape.LayoutOptions);
    layoutRef.current = layout;
    layout.run();
  }, [data, visibleEdges]);

  const passing = data
    ? data.edges.filter((e) => e.weight >= threshold).length
    : 0;

  return (
    <>
      <div class="controls">
        <label>
          threshold: {threshold.toFixed(2)}
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={threshold}
            onInput={(e) =>
              setThreshold(parseFloat((e.target as HTMLInputElement).value))
            }
          />
        </label>
        <label>
          <input
            type="checkbox"
            checked={reduce}
            onChange={(e) => setReduce((e.target as HTMLInputElement).checked)}
          />
          hide transitive edges
        </label>
        <span>
          {visibleEdges.length} shown · {passing} pass threshold ·{" "}
          {data?.edges.length ?? 0} total
        </span>
      </div>
      <div id="cy" ref={containerRef} />
    </>
  );
}
