import type { NodeTypes } from "@xyflow/react";

// spec-21 registers an empty map here; spec-22 wires in the 5 business node
// kinds (Script / Storyboard / Character / Task / Asset). Keeping it as a
// named export means downstream consumers import the same identity that
// `<ReactFlow nodeTypes={nodeTypes}>` receives.
export const nodeTypes: NodeTypes = {};
