import { create } from "zustand";
import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
  type OnConnect,
  type OnEdgesChange,
  type OnNodesChange,
  type Viewport,
} from "@xyflow/react";

export interface CanvasInitPayload {
  nodes: Node[];
  edges: Edge[];
  viewport: Viewport;
}

export interface CanvasState {
  nodes: Node[];
  edges: Edge[];
  viewport: Viewport;
  init: (s: CanvasInitPayload) => void;
  onNodesChange: OnNodesChange;
  onEdgesChange: OnEdgesChange;
  onConnect: OnConnect;
  setViewport: (v: Viewport) => void;
  reset: () => void;
}

const EMPTY_VIEWPORT: Viewport = { x: 0, y: 0, zoom: 1 };

export const useCanvasStore = create<CanvasState>((set, get) => ({
  nodes: [],
  edges: [],
  viewport: EMPTY_VIEWPORT,
  init: (s) =>
    set({ nodes: s.nodes, edges: s.edges, viewport: s.viewport }),
  onNodesChange: (changes: NodeChange[]) =>
    set({ nodes: applyNodeChanges(changes, get().nodes) }),
  onEdgesChange: (changes: EdgeChange[]) =>
    set({ edges: applyEdgeChanges(changes, get().edges) }),
  onConnect: (params: Connection) =>
    set({ edges: addEdge(params, get().edges) }),
  setViewport: (v: Viewport) => set({ viewport: v }),
  reset: () => set({ nodes: [], edges: [], viewport: EMPTY_VIEWPORT }),
}));
