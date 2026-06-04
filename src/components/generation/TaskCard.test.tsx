import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type {
  GenerationTask,
  GenerationTaskEvent,
} from "@/lib/bindings/commands";

const listTaskEventsMock = vi.fn<
  (taskId: string) => Promise<{ status: "ok"; data: GenerationTaskEvent[] }>
>();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listTaskEvents: (taskId: string) => listTaskEventsMock(taskId),
    cancelTask: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    getTask: vi.fn(),
    submitTask: vi.fn(),
    getAsset: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    getProject: vi.fn(() => Promise.resolve({ status: "ok", data: { id: "p1", root_path: "/tmp" } })),
  },
}));

vi.mock("@/hooks/useAssets", () => ({
  useAsset: () => ({ data: undefined }),
}));

vi.mock("@/hooks/useResolvedAssetUrl", () => ({
  useResolvedAssetUrl: () => null,
}));

vi.mock("@/hooks/useProjects", () => ({
  useProject: () => ({ data: { id: "p1", root_path: "/tmp" } }),
}));

vi.mock("./ResultPreviewDialog", () => ({
  ResultPreviewDialog: () => null,
}));

vi.mock("./TaskDiagnosticsDialog", () => ({
  TaskDiagnosticsDialog: () => <div data-testid="diagnostics-dialog" />,
}));

import { TaskCard } from "./TaskCard";

function makeTask(overrides: Partial<GenerationTask> = {}): GenerationTask {
  return {
    id: "task-1",
    project_id: "p1",
    shot_id: null,
    provider_id: "bailian",
    model_id: "wan2.7-image-pro",
    account_id: "a1",
    task_type: "image",
    params_json: '{"prompt":"hello"}',
    status: "running",
    result_asset_id: null,
    external_task_id: null,
    started_at: null,
    finished_at: null,
    error_message: null,
    retry_count: 0,
    created_at: new Date().toISOString().replace("T", " ").slice(0, 19),
    batch_id: null,
    ...overrides,
  };
}

function renderCard(task: GenerationTask, events: GenerationTaskEvent[] = []) {
  listTaskEventsMock.mockReset();
  listTaskEventsMock.mockResolvedValue({ status: "ok", data: events });
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <TaskCard task={task} projectId="p1" />
    </QueryClientProvider>,
  );
}

describe("TaskCard", () => {
  it("shows the latest info event message while running", async () => {
    renderCard(makeTask({ status: "running" }), [
      {
        id: 1,
        task_id: "task-1",
        occurred_at: "2026-05-26 10:00:00",
        phase: "submit_call",
        severity: "info",
        request_id: null,
        http_status: null,
        details_json: "{}",
        message: "提交到云端",
      },
      {
        id: 2,
        task_id: "task-1",
        occurred_at: "2026-05-26 10:00:01",
        phase: "poll",
        severity: "info",
        request_id: null,
        http_status: null,
        details_json: "{}",
        message: "云端开始生成",
      },
    ]);

    await waitFor(() => {
      expect(screen.getByText(/云端开始生成/)).toBeInTheDocument();
    });
  });

  it("falls back to a placeholder when no events have arrived yet", async () => {
    renderCard(makeTask({ status: "running" }), []);
    expect(screen.getByText(/▸ 准备中…/)).toBeInTheDocument();
  });

  it("renders the diagnostics button only on failed tasks", () => {
    renderCard(
      makeTask({ status: "failed", error_message: "boom" }),
      [],
    );
    expect(
      screen.getByRole("button", { name: "诊断" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
  });

  it("does not render the diagnostics button on running tasks", () => {
    renderCard(makeTask({ status: "running" }), []);
    expect(
      screen.queryByRole("button", { name: "诊断" }),
    ).not.toBeInTheDocument();
  });
});
