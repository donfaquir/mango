import { create } from "zustand";
import { temporal } from "zundo";
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
import { toast } from "sonner";
import { commands } from "@/lib/bindings/commands";
import type {
  CanvasNode,
  CanvasNodeData,
  CharacterNodeData,
  StoryboardNodeData,
} from "@/components/canvas/nodes/types";
import type { EdgeKind } from "@/components/canvas/edges/types";

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
  addNode: (node: CanvasNode) => void;
  updateNodeData: <T extends CanvasNodeData>(
    id: string,
    partial: Partial<T>,
  ) => void;
  removeNode: (id: string) => void;
  removeEdge: (id: string) => void;
}

const EMPTY_VIEWPORT: Viewport = { x: 0, y: 0, zoom: 1 };
const UNDO_STACK_LIMIT = 50;

export function inferEdgeKind(
  s: CanvasNodeData,
  t: CanvasNodeData,
): EdgeKind | null {
  if (s.kind === "character" && t.kind === "storyboard")
    return "character_to_shot";
  if (s.kind === "storyboard" && t.kind === "task") return "shot_to_task";
  if (s.kind === "task" && t.kind === "asset") return "task_to_asset";
  if (s.kind === "storyboard" && t.kind === "asset")
    return "storyboard_to_asset";
  if (s.kind === "character" && t.kind === "asset")
    return "character_to_asset";
  if (s.kind === "asset" && t.kind === "asset") return "asset_to_asset";
  return null;
}

function unlinkCharacterShotPair(
  characterId: string,
  shotId: string,
): void {
  commands
    .unlinkShotSubject(shotId, characterId, "character")
    .catch((err) => console.error("unlinkShotSubject failed", err));
}

export const useCanvasStore = create<CanvasState>()(
  temporal(
    (set, get) => ({
      nodes: [],
      edges: [],
      viewport: EMPTY_VIEWPORT,

      init: (s) => {
        // Loading a saved layout must not push a snapshot — otherwise pressing
        // Cmd+Z once after opening an episode wipes the canvas. Pause around
        // the set, then clear stale stacks from any previous episode.
        const t = useCanvasStore.temporal.getState();
        t.pause();
        set({ nodes: s.nodes, edges: s.edges, viewport: s.viewport });
        t.resume();
        t.clear();
      },

      onNodesChange: (changes: NodeChange[]) => {
        const nodes = get().nodes;
        const edges = get().edges;
        for (const change of changes) {
          if (change.type !== "remove") continue;
          const node = nodes.find((n) => n.id === change.id);
          if (!node) continue;
          const affected = edges.filter(
            (e) =>
              e.type === "character_to_shot" &&
              (e.source === node.id || e.target === node.id),
          );
          for (const e of affected) {
            const char = nodes.find((n) => n.id === e.source);
            const shot = nodes.find((n) => n.id === e.target);
            const charData = char?.data as CanvasNodeData | undefined;
            const shotData = shot?.data as CanvasNodeData | undefined;
            if (charData?.kind === "character" && shotData?.kind === "storyboard") {
              unlinkCharacterShotPair(charData.characterId, shotData.shotId);
            }
          }
        }
        set({ nodes: applyNodeChanges(changes, nodes) });
      },

      onEdgesChange: (changes: EdgeChange[]) => {
        const nodes = get().nodes;
        const edges = get().edges;
        for (const change of changes) {
          if (change.type !== "remove") continue;
          const edge = edges.find((e) => e.id === change.id);
          if (edge?.type !== "character_to_shot") continue;
          const sourceNode = nodes.find((n) => n.id === edge.source);
          const targetNode = nodes.find((n) => n.id === edge.target);
          const sourceData = sourceNode?.data as CanvasNodeData | undefined;
          const targetData = targetNode?.data as CanvasNodeData | undefined;
          if (
            sourceData?.kind === "character" &&
            targetData?.kind === "storyboard"
          ) {
            unlinkCharacterShotPair(sourceData.characterId, targetData.shotId);
          }
        }
        set({ edges: applyEdgeChanges(changes, edges) });
      },

      onConnect: (params: Connection) => {
        const nodes = get().nodes;
        const sourceNode = nodes.find((n) => n.id === params.source);
        const targetNode = nodes.find((n) => n.id === params.target);
        if (!sourceNode || !targetNode) return;

        const sourceData = sourceNode.data as CanvasNodeData;
        const targetData = targetNode.data as CanvasNodeData;
        const edgeKind = inferEdgeKind(sourceData, targetData);
        if (!edgeKind) {
          toast.error(
            `不支持 ${sourceData.kind} → ${targetData.kind} 的连线`,
          );
          return;
        }

        const edgeId = `edge-${crypto.randomUUID()}`;
        set({
          edges: addEdge(
            { ...params, id: edgeId, type: edgeKind },
            get().edges,
          ),
        });

        if (edgeKind === "character_to_shot") {
          const { characterId } = sourceData as CharacterNodeData;
          const { shotId } = targetData as StoryboardNodeData;
          commands
            .linkShotSubject(shotId, characterId, "character")
            .catch((err) => {
              toast.error(`绑定角色到分镜失败：${err}`);
              set({ edges: get().edges.filter((edge) => edge.id !== edgeId) });
            });
        }
      },

      setViewport: (v: Viewport) => set({ viewport: v }),

      reset: () => {
        // Same rationale as init: never let a programmatic clear leak into
        // the undo stack and reappear via Cmd+Z.
        const t = useCanvasStore.temporal.getState();
        t.pause();
        set({ nodes: [], edges: [], viewport: EMPTY_VIEWPORT });
        t.resume();
        t.clear();
      },

      addNode: (node) => {
        if (get().nodes.some((n) => n.id === node.id)) {
          toast.error("该节点已在画布上");
          return;
        }
        set({ nodes: [...get().nodes, node as Node] });
      },

      updateNodeData: (id, partial) =>
        set({
          nodes: get().nodes.map((n) =>
            n.id === id ? { ...n, data: { ...n.data, ...partial } } : n,
          ),
        }),

      removeNode: (id) => {
        get().onNodesChange([{ type: "remove", id }]);
      },

      removeEdge: (id) => {
        get().onEdgesChange([{ type: "remove", id }]);
      },
    }),
    {
      limit: UNDO_STACK_LIMIT,
      // Track only nodes + edges. Viewport pan/zoom must not pollute the stack.
      // Drag-frame deduplication is handled at the React Flow event boundary
      // via onNodeDragStart/Stop pause/resume in CanvasInner — see spec-24.
      partialize: (state) => ({
        nodes: state.nodes,
        edges: state.edges,
      }),
      // Skip the snapshot when nodes & edges references are unchanged — this
      // is what keeps viewport-only setState calls (panning, zooming) out of
      // the undo stack.
      equality: (a, b) => a.nodes === b.nodes && a.edges === b.edges,
    },
  ),
);
