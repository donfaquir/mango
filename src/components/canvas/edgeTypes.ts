import type { EdgeTypes } from "@xyflow/react";
import { CharacterToShotEdge } from "./edges/CharacterToShotEdge";
import { ShotToTaskEdge } from "./edges/ShotToTaskEdge";
import { TaskToAssetEdge } from "./edges/TaskToAssetEdge";

export const edgeTypes: EdgeTypes = {
  character_to_shot: CharacterToShotEdge,
  shot_to_task: ShotToTaskEdge,
  task_to_asset: TaskToAssetEdge,
  // All "X → asset" relations share the same dashed-bezier renderer; the
  // distinct kind strings only matter for serialization and removal hooks.
  storyboard_to_asset: TaskToAssetEdge,
  character_to_asset: TaskToAssetEdge,
  asset_to_asset: TaskToAssetEdge,
};
