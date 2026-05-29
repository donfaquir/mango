import { useCallback, useState } from "react";
import { useReactFlow, type XYPosition } from "@xyflow/react";
import { DRAG_MIME_ASSET_ID } from "./constants";

export interface UseInternalAssetDropOptions {
  onPlaceAsset: (assetId: string, flowPos: XYPosition) => void;
  onConnectAsset: (assetId: string, sourceNodeId: string) => void;
}

export interface InternalAssetDropResult {
  onDragOver: (e: React.DragEvent) => void;
  onDrop: (e: React.DragEvent) => void;
  onDragLeave: (e: React.DragEvent) => void;
  isOver: boolean;
}

export function useInternalAssetDrop(
  opts: UseInternalAssetDropOptions,
): InternalAssetDropResult {
  const [isOver, setIsOver] = useState(false);
  const { screenToFlowPosition, getIntersectingNodes } = useReactFlow();

  const onDragOver = useCallback((e: React.DragEvent) => {
    if (!e.dataTransfer.types.includes(DRAG_MIME_ASSET_ID)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    setIsOver(true);
  }, []);

  const onDragLeave = useCallback((e: React.DragEvent) => {
    // Only clear when the pointer leaves the container itself, not when it
    // crosses into a child element (which also fires dragleave on the parent).
    if (e.currentTarget === e.target) setIsOver(false);
  }, []);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsOver(false);
      const assetId = e.dataTransfer.getData(DRAG_MIME_ASSET_ID);
      if (!assetId) return;

      const flowPos = screenToFlowPosition({ x: e.clientX, y: e.clientY });

      const hit = getIntersectingNodes({
        x: flowPos.x,
        y: flowPos.y,
        width: 1,
        height: 1,
      });
      const sourceNode = hit[0];
      if (sourceNode) {
        opts.onConnectAsset(assetId, sourceNode.id);
        return;
      }

      opts.onPlaceAsset(assetId, flowPos);
    },
    [screenToFlowPosition, getIntersectingNodes, opts],
  );

  return { onDragOver, onDrop, onDragLeave, isOver };
}
