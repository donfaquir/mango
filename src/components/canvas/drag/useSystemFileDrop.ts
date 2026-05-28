import { useCallback, useEffect, useRef } from "react";
import { toast } from "sonner";
import { useReactFlow, type XYPosition } from "@xyflow/react";
import { useGlobalDropTarget, type DropHandler } from "@/hooks/useGlobalDropTarget";
import { useImportAsset } from "@/hooks/useAssets";
import { ACCEPTED_EXTENSIONS, isAcceptedExtension } from "./constants";

export interface UseSystemFileDropOptions {
  projectId: string;
  onPlaceAsset: (assetId: string, flowPos: XYPosition) => void;
  enabled?: boolean;
}

export interface SystemFileDropResult {
  isSystemDragOver: boolean;
}

const STAGGER_OFFSET_PX = 20;

const ACCEPTED_LABEL = ACCEPTED_EXTENSIONS
  .map((ext) => ext.toUpperCase())
  .join(" / ");

export function useSystemFileDrop(
  opts: UseSystemFileDropOptions,
): SystemFileDropResult {
  const { screenToFlowPosition } = useReactFlow();
  const importAsset = useImportAsset();

  // Stabilize the latest values in a ref so the global drop handler keeps
  // stable identity. The handler installs once at mount; relying on React
  // Query mutation refs in deps would re-install it on every render.
  const latestRef = useRef({ ...opts, importAsset, screenToFlowPosition });
  useEffect(() => {
    latestRef.current = { ...opts, importAsset, screenToFlowPosition };
  });

  const handler = useCallback<DropHandler>(async (paths, position) => {
    const {
      projectId,
      onPlaceAsset,
      importAsset,
      screenToFlowPosition,
    } = latestRef.current;

    const accepted = paths.filter(isAcceptedExtension);
    if (accepted.length === 0) {
      toast.error(`不支持的文件类型，仅支持 ${ACCEPTED_LABEL}`);
      return;
    }
    if (paths.length > accepted.length) {
      toast.warning(`已忽略 ${paths.length - accepted.length} 个不支持的文件`);
    }

    // Tauri payload.position is in PHYSICAL pixels; divide by devicePixelRatio
    // before handing it to React Flow, which expects CSS pixels.
    const dpr = window.devicePixelRatio || 1;
    const flowPos = screenToFlowPosition({
      x: position.x / dpr,
      y: position.y / dpr,
    });

    for (const [idx, path] of accepted.entries()) {
      try {
        const asset = await importAsset.mutateAsync({
          project_id: projectId,
          source_path: path,
          shot_id: null,
        });
        onPlaceAsset(asset.id, {
          x: flowPos.x + idx * STAGGER_OFFSET_PX,
          y: flowPos.y + idx * STAGGER_OFFSET_PX,
        });
      } catch (err) {
        const name = path.split("/").pop() ?? path;
        const msg = err instanceof Error ? err.message : String(err);
        toast.error(`导入失败：${name} — ${msg}`);
      }
    }
  }, []);

  useGlobalDropTarget(handler, opts.enabled ?? true);

  return { isSystemDragOver: false };
}
