import type { XYPosition } from "@xyflow/react";
import type {
  AssetNode,
  CanvasNode,
  CharacterNode,
  ScriptNode,
  StoryboardNode,
  TaskNode,
} from "./nodes/types";

const DEFAULT_POSITION: XYPosition = { x: 0, y: 0 };

export function createScriptNode(
  episodeId: string,
  position: XYPosition = DEFAULT_POSITION,
): ScriptNode {
  return {
    id: `script-${episodeId}`,
    type: "script",
    position,
    data: { kind: "script", episodeId, collapsed: false },
  };
}

export function createStoryboardNode(
  shotId: string,
  position: XYPosition = DEFAULT_POSITION,
): StoryboardNode {
  return {
    id: `storyboard-${shotId}`,
    type: "storyboard",
    position,
    data: { kind: "storyboard", shotId },
  };
}

export function createCharacterNode(
  characterId: string,
  position: XYPosition = DEFAULT_POSITION,
): CharacterNode {
  return {
    id: `character-${characterId}`,
    type: "character",
    position,
    data: { kind: "character", characterId },
  };
}

export function createTaskNode(
  taskId: string,
  position: XYPosition = DEFAULT_POSITION,
): TaskNode {
  return {
    id: `task-${taskId}`,
    type: "task",
    position,
    data: { kind: "task", taskId },
  };
}

export function createAssetNode(
  assetId: string,
  position: XYPosition = DEFAULT_POSITION,
): AssetNode {
  return {
    id: `asset-${assetId}`,
    type: "asset",
    position,
    data: { kind: "asset", assetId },
  };
}

export type AnyCanvasNode = CanvasNode;
