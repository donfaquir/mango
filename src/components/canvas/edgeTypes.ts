import type { EdgeTypes } from "@xyflow/react";
import { CharacterToShotEdge } from "./edges/CharacterToShotEdge";
import { ShotToTaskEdge } from "./edges/ShotToTaskEdge";
import { TaskToAssetEdge } from "./edges/TaskToAssetEdge";

export const edgeTypes: EdgeTypes = {
  character_to_shot: CharacterToShotEdge,
  shot_to_task: ShotToTaskEdge,
  task_to_asset: TaskToAssetEdge,
};
