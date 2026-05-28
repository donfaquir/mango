import type { Edge, Node, Viewport } from "@xyflow/react";
import type { CanvasLayout } from "@/lib/bindings/commands";

export interface RawLayout {
  nodes: Node[];
  edges: Edge[];
  viewport: Viewport;
}

export interface SerializedLayout {
  nodes_json: string;
  edges_json: string;
  viewport_json: string;
}

const EMPTY_VIEWPORT: Viewport = { x: 0, y: 0, zoom: 1 };

export function serializeLayout(raw: RawLayout): SerializedLayout {
  return {
    nodes_json: JSON.stringify(raw.nodes),
    edges_json: JSON.stringify(raw.edges),
    viewport_json: JSON.stringify(raw.viewport),
  };
}

export function deserializeLayout(stored: CanvasLayout | null): RawLayout {
  if (!stored) {
    return { nodes: [], edges: [], viewport: { ...EMPTY_VIEWPORT } };
  }
  return {
    nodes: JSON.parse(stored.nodes_json) as Node[],
    edges: JSON.parse(stored.edges_json) as Edge[],
    viewport: JSON.parse(stored.viewport_json) as Viewport,
  };
}

// Simple djb2 hash. Not cryptographic — only needs "same content → same hash"
// so the auto-save loop can skip redundant DB writes after viewport jitter or
// equivalent React Flow change events.
export function hashLayout(s: SerializedLayout): string {
  return `${djb2(s.nodes_json)}:${djb2(s.edges_json)}:${djb2(s.viewport_json)}`;
}

function djb2(s: string): string {
  let h = 5381;
  for (let i = 0; i < s.length; i++) {
    h = (h << 5) + h + s.charCodeAt(i);
  }
  return (h >>> 0).toString(36);
}
