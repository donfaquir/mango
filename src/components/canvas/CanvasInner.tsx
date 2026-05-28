import { useCallback, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  useReactFlow,
  type Edge,
  type Viewport,
} from "@xyflow/react";
import { useCanvasStore } from "@/stores/canvasStore";
import { useAutoSaveLayout } from "./persistence/useAutoSave";
import { edgeTypes } from "./edgeTypes";
import { nodeTypes } from "./nodeTypes";
import {
  PaneContextMenu,
  type PaneMenuState,
} from "./PaneContextMenu";
import {
  EdgeContextMenu,
  type EdgeContextMenuState,
} from "./edges/EdgeContextMenu";

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

  const { screenToFlowPosition } = useReactFlow();
  const [paneMenu, setPaneMenu] = useState<PaneMenuState | null>(null);
  const [edgeMenu, setEdgeMenu] = useState<EdgeContextMenuState | null>(null);

  const handlePaneContextMenu = useCallback(
    (event: React.MouseEvent | MouseEvent) => {
      event.preventDefault();
      const flow = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      setPaneMenu({ clientX: event.clientX, clientY: event.clientY, flow });
    },
    [screenToFlowPosition],
  );

  const handleEdgeContextMenu = useCallback(
    (event: React.MouseEvent, edge: Edge) => {
      event.preventDefault();
      setEdgeMenu({
        edgeId: edge.id,
        clientX: event.clientX,
        clientY: event.clientY,
      });
    },
    [],
  );

  return (
    <>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onMoveEnd={(_, viewport) => setViewport(viewport)}
        onPaneContextMenu={handlePaneContextMenu}
        onEdgeContextMenu={handleEdgeContextMenu}
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
      <PaneContextMenu state={paneMenu} onClose={() => setPaneMenu(null)} />
      <EdgeContextMenu state={edgeMenu} onClose={() => setEdgeMenu(null)} />
    </>
  );
}
