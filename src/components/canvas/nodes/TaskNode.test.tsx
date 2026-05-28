import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import type { NodeProps } from "@xyflow/react";

let taskData: ReturnType<typeof makeTask> | undefined;

vi.mock("@/hooks/useTasks", () => ({
  useTaskById: () => ({ data: taskData }),
}));

import { TaskNode } from "./TaskNode";
import type { TaskNode as TaskNodeType } from "./types";
import { NodeTestProviders } from "./test-utils";

function makeTask(status: "pending" | "running" | "success" | "failed" | "cancelled") {
  return {
    id: "t1",
    project_id: "p1",
    shot_id: null,
    provider_id: "bailian",
    model_id: "wan2.7-image-pro",
    account_id: "acc1",
    task_type: "image" as const,
    params_json: "{}",
    status,
    result_asset_id: null,
    external_task_id: null,
    started_at: null,
    finished_at: null,
    error_message: status === "failed" ? "boom" : null,
    retry_count: 0,
    created_at: "",
  };
}

function renderNode() {
  const props = {
    id: "task-t1",
    data: { kind: "task", taskId: "t1" },
    selected: false,
    type: "task",
    zIndex: 0,
    isConnectable: true,
    xPos: 0,
    yPos: 0,
    dragging: false,
  } as unknown as NodeProps<TaskNodeType>;
  return render(
    <NodeTestProviders>
      <TaskNode {...props} />
    </NodeTestProviders>,
  );
}

describe("TaskNode", () => {
  beforeEach(() => {
    taskData = makeTask("pending");
  });

  it("hides diagnostic button for pending tasks", () => {
    taskData = makeTask("pending");
    renderNode();
    expect(screen.queryByText("查看诊断")).toBeNull();
  });

  it("shows diagnostic button when status=failed", () => {
    taskData = makeTask("failed");
    renderNode();
    expect(screen.getByText("查看诊断")).toBeInTheDocument();
  });
});
