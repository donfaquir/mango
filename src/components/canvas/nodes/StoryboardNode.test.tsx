import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { NodeProps } from "@xyflow/react";

const removeNode = vi.fn();
let shotData: ReturnType<typeof makeShot> | undefined;
let isLoading = false;

vi.mock("@/hooks/useShots", () => ({
  useShot: () => ({ data: shotData, isLoading }),
}));
vi.mock("@/stores/canvasStore", () => ({
  useCanvasStore: (selector: (s: { removeNode: typeof removeNode }) => unknown) =>
    selector({ removeNode }),
}));

import { StoryboardNode } from "./StoryboardNode";
import type { StoryboardNode as StoryboardNodeType } from "./types";
import { NodeTestProviders } from "./test-utils";
import { STATUS_BORDER } from "./shared/NodeShell";

function makeShot(overrides: Partial<{ status: "draft" | "ready" | "generating" | "done"; dialogue: string }> = {}) {
  return {
    id: "sh1",
    episode_id: "ep1",
    order_index: 3,
    summary: "主角拔剑出鞘",
    duration_sec: 5,
    camera_angle: "wide",
    shot_type: "wide",
    mood: "tense",
    dialogue: overrides.dialogue ?? "出来吧!",
    video_prompt: "",
    image_prompt: "",
    status: overrides.status ?? "draft",
    adopted_asset_id: null,
    created_at: "",
    updated_at: "",
  };
}

function renderNode() {
  const props = {
    id: "storyboard-sh1",
    data: { kind: "storyboard", shotId: "sh1" },
    selected: false,
    type: "storyboard",
    zIndex: 0,
    isConnectable: true,
    xPos: 0,
    yPos: 0,
    dragging: false,
  } as unknown as NodeProps<StoryboardNodeType>;
  return render(
    <NodeTestProviders>
      <StoryboardNode {...props} />
    </NodeTestProviders>,
  );
}

describe("StoryboardNode", () => {
  beforeEach(() => {
    removeNode.mockReset();
    isLoading = false;
  });

  it("renders summary, dialogue, and order index", () => {
    shotData = makeShot();
    renderNode();
    expect(screen.getByText(/分镜 #3/)).toBeInTheDocument();
    expect(screen.getByText("主角拔剑出鞘")).toBeInTheDocument();
    expect(screen.getByText(/出来吧!/)).toBeInTheDocument();
  });

  it("applies the status border class for each status", () => {
    for (const status of ["draft", "ready", "generating", "done"] as const) {
      shotData = makeShot({ status });
      const { container, unmount } = renderNode();
      const shell = container.querySelector(`.${STATUS_BORDER[status]}`);
      expect(shell).not.toBeNull();
      unmount();
    }
  });

  it("shows the deleted-state UI when shot lookup returns no data", () => {
    shotData = undefined;
    isLoading = false;
    renderNode();
    expect(screen.getByText("分镜已删除")).toBeInTheDocument();
  });

  it("calls removeNode when the delete button on the deleted-state card is clicked", () => {
    shotData = undefined;
    renderNode();
    const buttons = screen.getAllByRole("button");
    fireEvent.click(buttons[0]!);
    expect(removeNode).toHaveBeenCalledWith("storyboard-sh1");
  });
});
