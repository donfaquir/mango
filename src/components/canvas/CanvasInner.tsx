import { useCallback, useState } from "react";
import { useParams } from "react-router-dom";
import { useShallow } from "zustand/react/shallow";
import {
  Background,
  MiniMap,
  ReactFlow,
  useReactFlow,
  type Edge,
  type Viewport,
  type XYPosition,
} from "@xyflow/react";
import { useCanvasStore } from "@/stores/canvasStore";
import { useProject } from "@/hooks/useProjects";
import { useAutoSaveLayout } from "./persistence/useAutoSave";
import { edgeTypes } from "./edgeTypes";
import { nodeTypes } from "./nodeTypes";
import { createAssetNode } from "./nodeFactory";
import {
  PaneContextMenu,
  type PaneMenuState,
} from "./PaneContextMenu";
import {
  EdgeContextMenu,
  type EdgeContextMenuState,
} from "./edges/EdgeContextMenu";
import { AssetDrawer } from "./drag/AssetDrawer";
import { DropTargetOverlay } from "./drag/DropTargetOverlay";
import { useInternalAssetDrop } from "./drag/useInternalAssetDrop";
import { CanvasToolbar } from "./toolbar/CanvasToolbar";
import { useCanvasShortcuts } from "./toolbar/useCanvasShortcuts";
import { useCheckpointAutoSave } from "@/hooks/useCheckpointAutoSave";
import { useEpisodeDataRestoredListener } from "@/hooks/useCheckpoint";

interface Props {
  episodeId: string;
  initialViewport: Viewport;
}

export function CanvasInner({ episodeId, initialViewport }: Props) {
  const { projectId } = useParams<{ projectId: string }>();
  const { data: project } = useProject(projectId);
  const projectRoot = project?.root_path;

  const { nodes, edges, onNodesChange, onEdgesChange, onConnect, setViewport, addNode } =
    useCanvasStore(
      useShallow((s) => ({
        nodes: s.nodes,
        edges: s.edges,
        onNodesChange: s.onNodesChange,
        onEdgesChange: s.onEdgesChange,
        onConnect: s.onConnect,
        setViewport: s.setViewport,
        addNode: s.addNode,
      })),
    );

  const autoSave = useAutoSaveLayout(episodeId);
  useCanvasShortcuts();
  useCheckpointAutoSave(episodeId);
  useEpisodeDataRestoredListener();

  const onNodeDragStart = useCallback(() => {
    useCanvasStore.temporal.getState().pause();
  }, []);

  const onNodeDragStop = useCallback(() => {
    useCanvasStore.temporal.getState().resume();
  }, []);

  const { screenToFlowPosition } = useReactFlow();
  const [paneMenu, setPaneMenu] = useState<PaneMenuState | null>(null);
  const [edgeMenu, setEdgeMenu] = useState<EdgeContextMenuState | null>(null);

  const handlePlaceAsset = useCallback(
    (assetId: string, flowPos: XYPosition) => {
      addNode(createAssetNode(assetId, flowPos));
    },
    [addNode],
  );

  const handleConnectAsset = useCallback(
    (assetId: string, sourceNodeId: string) => {
      const store = useCanvasStore.getState();
      const sourceNode = store.nodes.find((n) => n.id === sourceNodeId);
      if (!sourceNode) return;
      const offsetX = (sourceNode.measured?.width ?? 240) + 60;
      const newNode = createAssetNode(assetId, {
        x: sourceNode.position.x + offsetX,
        y: sourceNode.position.y,
      });
      store.addNode(newNode);
      store.onConnect({
        source: sourceNodeId,
        target: newNode.id,
        sourceHandle: null,
        targetHandle: null,
      });
    },
    [],
  );

  const internalDrop = useInternalAssetDrop({
    onPlaceAsset: handlePlaceAsset,
    onConnectAsset: handleConnectAsset,
  });

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
    <div
      className="relative h-full w-full"
      onDragOver={internalDrop.onDragOver}
      onDrop={internalDrop.onDrop}
      onDragLeave={internalDrop.onDragLeave}
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onMoveEnd={(_, viewport) => setViewport(viewport)}
        onNodeDragStart={onNodeDragStart}
        onNodeDragStop={onNodeDragStop}
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
      </ReactFlow>
      <DropTargetOverlay show={internalDrop.isOver} />
      <CanvasToolbar episodeId={episodeId} onBeforeSaveVersion={autoSave.flush} />
      {projectId && projectRoot && (
        <AssetDrawer projectId={projectId} projectRoot={projectRoot} />
      )}
      <PaneContextMenu state={paneMenu} onClose={() => setPaneMenu(null)} />
      <EdgeContextMenu state={edgeMenu} onClose={() => setEdgeMenu(null)} />
    </div>
  );
}
