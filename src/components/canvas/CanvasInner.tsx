import { useShallow } from "zustand/react/shallow";
import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  type Viewport,
} from "@xyflow/react";
import { useCanvasStore } from "@/stores/canvasStore";
import { useAutoSaveLayout } from "./persistence/useAutoSave";
import { edgeTypes } from "./edgeTypes";
import { nodeTypes } from "./nodeTypes";

interface Props {
  episodeId: string;
  initialViewport: Viewport;
}

export function CanvasInner({ episodeId, initialViewport }: Props) {
  const { nodes, edges, onNodesChange, onEdgesChange, onConnect, setViewport } =
    useCanvasStore(
      useShallow((s) => ({
        nodes: s.nodes,
        edges: s.edges,
        onNodesChange: s.onNodesChange,
        onEdgesChange: s.onEdgesChange,
        onConnect: s.onConnect,
        setViewport: s.setViewport,
      })),
    );

  useAutoSaveLayout(episodeId);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={onConnect}
      onMoveEnd={(_, viewport) => setViewport(viewport)}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      defaultViewport={initialViewport}
      minZoom={0.1}
      maxZoom={2}
      fitView={false}
    >
      <Background gap={20} />
      <MiniMap pannable zoomable />
      <Controls />
    </ReactFlow>
  );
}
