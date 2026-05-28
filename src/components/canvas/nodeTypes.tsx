import type { NodeProps, NodeTypes } from "@xyflow/react";
import { ScriptNode } from "./nodes/ScriptNode";
import { StoryboardNode } from "./nodes/StoryboardNode";
import { CharacterNode } from "./nodes/CharacterNode";
import { TaskNode } from "./nodes/TaskNode";
import { AssetNode } from "./nodes/AssetNode";
import { NodeContextMenu } from "./nodes/NodeContextMenu";
import type { CanvasNodeData } from "./nodes/types";

function withContextMenu<P extends NodeProps>(
  Inner: React.ComponentType<P>,
): React.ComponentType<P> {
  const Wrapped = (props: P) => (
    <NodeContextMenu nodeId={props.id} data={props.data as CanvasNodeData}>
      <div>
        <Inner {...props} />
      </div>
    </NodeContextMenu>
  );
  Wrapped.displayName = `WithContextMenu(${Inner.displayName ?? Inner.name})`;
  return Wrapped;
}

export const nodeTypes: NodeTypes = {
  script: withContextMenu(ScriptNode),
  storyboard: withContextMenu(StoryboardNode),
  character: withContextMenu(CharacterNode),
  task: withContextMenu(TaskNode),
  asset: withContextMenu(AssetNode),
};
