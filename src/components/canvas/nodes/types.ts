import type { Node } from "@xyflow/react";

export type NodeKind = "script" | "storyboard" | "character" | "task" | "asset";

export interface ScriptNodeData extends Record<string, unknown> {
  kind: "script";
  episodeId: string;
  collapsed: boolean;
}

export interface StoryboardNodeData extends Record<string, unknown> {
  kind: "storyboard";
  shotId: string;
}

export interface CharacterNodeData extends Record<string, unknown> {
  kind: "character";
  characterId: string;
}

export interface TaskNodeData extends Record<string, unknown> {
  kind: "task";
  taskId: string;
}

export interface AssetNodeData extends Record<string, unknown> {
  kind: "asset";
  assetId: string;
}

export type CanvasNodeData =
  | ScriptNodeData
  | StoryboardNodeData
  | CharacterNodeData
  | TaskNodeData
  | AssetNodeData;

export type ScriptNode = Node<ScriptNodeData, "script">;
export type StoryboardNode = Node<StoryboardNodeData, "storyboard">;
export type CharacterNode = Node<CharacterNodeData, "character">;
export type TaskNode = Node<TaskNodeData, "task">;
export type AssetNode = Node<AssetNodeData, "asset">;
export type CanvasNode =
  | ScriptNode
  | StoryboardNode
  | CharacterNode
  | TaskNode
  | AssetNode;
