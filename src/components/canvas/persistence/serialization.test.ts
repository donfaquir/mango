import { describe, expect, it } from "vitest";
import type { Edge, Node, Viewport } from "@xyflow/react";
import {
  deserializeLayout,
  hashLayout,
  serializeLayout,
} from "./serialization";

const sampleNodes: Node[] = [
  { id: "n1", position: { x: 10, y: 20 }, data: { label: "A" } },
  { id: "n2", position: { x: 30, y: 40 }, data: { label: "B" } },
];

const sampleEdges: Edge[] = [{ id: "e1", source: "n1", target: "n2" }];

const sampleViewport: Viewport = { x: 5, y: 6, zoom: 1.5 };

describe("serializeLayout / deserializeLayout", () => {
  it("round-trips nodes / edges / viewport via JSON", () => {
    const stored = serializeLayout({
      nodes: sampleNodes,
      edges: sampleEdges,
      viewport: sampleViewport,
    });

    const restored = deserializeLayout({
      id: "cl1",
      episode_id: "e1",
      ...stored,
      updated_at: "2026-05-27 12:00:00",
    });

    expect(restored.nodes).toEqual(sampleNodes);
    expect(restored.edges).toEqual(sampleEdges);
    expect(restored.viewport).toEqual(sampleViewport);
  });

  it("returns empty nodes/edges + default viewport when no layout stored", () => {
    const restored = deserializeLayout(null);
    expect(restored.nodes).toEqual([]);
    expect(restored.edges).toEqual([]);
    expect(restored.viewport).toEqual({ x: 0, y: 0, zoom: 1 });
  });
});

describe("hashLayout", () => {
  it("returns identical hash for identical serialized content", () => {
    const a = serializeLayout({
      nodes: sampleNodes,
      edges: sampleEdges,
      viewport: sampleViewport,
    });
    const b = serializeLayout({
      nodes: sampleNodes,
      edges: sampleEdges,
      viewport: sampleViewport,
    });
    expect(hashLayout(a)).toEqual(hashLayout(b));
  });

  it("differs when node order is swapped", () => {
    const original = hashLayout(
      serializeLayout({
        nodes: sampleNodes,
        edges: sampleEdges,
        viewport: sampleViewport,
      }),
    );
    const swapped = hashLayout(
      serializeLayout({
        nodes: [sampleNodes[1]!, sampleNodes[0]!],
        edges: sampleEdges,
        viewport: sampleViewport,
      }),
    );
    expect(original).not.toEqual(swapped);
  });

  it("differs when viewport zoom changes", () => {
    const original = hashLayout(
      serializeLayout({
        nodes: sampleNodes,
        edges: sampleEdges,
        viewport: sampleViewport,
      }),
    );
    const zoomed = hashLayout(
      serializeLayout({
        nodes: sampleNodes,
        edges: sampleEdges,
        viewport: { ...sampleViewport, zoom: 2 },
      }),
    );
    expect(original).not.toEqual(zoomed);
  });
});
