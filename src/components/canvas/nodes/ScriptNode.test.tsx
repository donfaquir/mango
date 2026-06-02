import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { NodeProps } from "@xyflow/react";

const updateNodeData = vi.fn();
let episodeData: { id: string; title: string; script_text: string } | undefined;

vi.mock("@/hooks/useEpisodes", () => ({
  useEpisode: () => ({ data: episodeData }),
}));
vi.mock("@/stores/canvasStore", () => ({
  useCanvasStore: (selector: (s: { updateNodeData: typeof updateNodeData }) => unknown) =>
    selector({ updateNodeData }),
}));

import { ScriptNode } from "./ScriptNode";
import type { ScriptNode as ScriptNodeType } from "./types";
import { NodeTestProviders } from "./test-utils";

function renderNode(collapsed: boolean) {
  const props = {
    id: "script-ep1",
    data: { kind: "script", episodeId: "ep1", collapsed },
    selected: false,
    type: "script",
    zIndex: 0,
    isConnectable: true,
    xPos: 0,
    yPos: 0,
    dragging: false,
  } as unknown as NodeProps<ScriptNodeType>;

  return render(
    <NodeTestProviders>
      <ScriptNode {...props} />
    </NodeTestProviders>,
  );
}

describe("ScriptNode", () => {
  beforeEach(() => {
    updateNodeData.mockReset();
    episodeData = {
      id: "ep1",
      title: "第一集",
      script_text: "line a\nline b\nline c\nline d",
    };
  });

  it("renders the episode title and the first 3 script lines when expanded", () => {
    renderNode(false);
    expect(screen.getByText("第一集")).toBeInTheDocument();
    expect(screen.getByText("line a")).toBeInTheDocument();
    expect(screen.getByText("line c")).toBeInTheDocument();
    expect(screen.queryByText("line d")).not.toBeInTheDocument();
  });

  it("hides the preview block when collapsed=true", () => {
    renderNode(true);
    expect(screen.queryByText("line a")).not.toBeInTheDocument();
  });

  it("toggles collapsed via updateNodeData on double-click", () => {
    const { container } = renderNode(false);
    const shell = container.firstChild?.firstChild as HTMLElement;
    fireEvent.doubleClick(shell);
    expect(updateNodeData).toHaveBeenCalledWith("script-ep1", {
      collapsed: true,
    });
  });

  it("falls back to a placeholder when the episode has no script_text", () => {
    episodeData = { id: "ep1", title: "第一集", script_text: "" };
    renderNode(false);
    expect(screen.getByText("未填写剧本")).toBeInTheDocument();
  });
});
