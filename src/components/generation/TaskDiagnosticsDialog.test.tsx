import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  },
}));

const writeTextMock = vi.fn<(text: string) => Promise<void>>(() =>
  Promise.resolve(),
);
Object.assign(navigator, {
  clipboard: { writeText: writeTextMock },
});

import { TaskDiagnosticsDialog } from "./TaskDiagnosticsDialog";

function makeTask(overrides: Partial<GenerationTask> = {}): GenerationTask {
  return {
    id: "task-aaaaaaaa-bbbb-cccc",
    project_id: null,
    shot_id: null,
    provider_id: "bailian",
    model_id: "wan2.7-image-pro",
    account_id: "a1",
    task_type: "image",
    params_json: "{}",
    status: "failed",
    result_asset_id: null,
    external_task_id: "ext-123",
    started_at: null,
    finished_at: "2026-05-26 10:00:01",
    error_message: "认证失败",
    retry_count: 0,
    created_at: "2026-05-26 10:00:00",
    ...overrides,
  };
}

function makeEvent(
  overrides: Partial<GenerationTaskEvent> = {},
): GenerationTaskEvent {
  return {
    id: 1,
    task_id: "task-aaaaaaaa-bbbb-cccc",
    occurred_at: "2026-05-26 10:00:00",
    phase: "submit_call",
    severity: "info",
    request_id: null,
    http_status: null,
    details_json: "{}",
    message: "提交到云端",
    ...overrides,
  };
}

function renderDialog(events: GenerationTaskEvent[], task = makeTask()) {
  listTaskEventsMock.mockReset();
  listTaskEventsMock.mockResolvedValue({ status: "ok", data: events });
  writeTextMock.mockClear();

  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <TaskDiagnosticsDialog task={task} onClose={() => {}} />
    </QueryClientProvider>,
  );
}

describe("TaskDiagnosticsDialog", () => {
  it("renders the event timeline with phase labels and messages", async () => {
    renderDialog([
      makeEvent({ id: 1, message: "提交到云端", phase: "submit_call" }),
      makeEvent({
        id: 2,
        phase: "poll",
        severity: "error",
        message: "认证失败",
        request_id: "rid-xyz",
        http_status: 401,
      }),
    ]);

    await waitFor(() => {
      expect(screen.getByText("提交到云端")).toBeInTheDocument();
    });
    expect(screen.getByText("认证失败")).toBeInTheDocument();
    expect(screen.getByText("提交")).toBeInTheDocument();
    expect(screen.getByText("轮询")).toBeInTheDocument();
  });

  it("shows empty-state copy when there are no events", async () => {
    renderDialog([]);
    await waitFor(() => {
      expect(screen.getByText("（暂无诊断事件）")).toBeInTheDocument();
    });
  });

  it("copies the latest non-null request_id when the button is clicked", async () => {
    renderDialog([
      makeEvent({ id: 1, request_id: "rid-old", message: "first" }),
      makeEvent({ id: 2, request_id: null, message: "middle" }),
      makeEvent({ id: 3, request_id: "rid-latest", message: "tail" }),
    ]);

    // Wait for the events query to settle so the dialog has populated state.
    await screen.findByText("tail");
    fireEvent.click(screen.getByRole("button", { name: /复制 request_id/ }));
    await waitFor(() =>
      expect(writeTextMock).toHaveBeenCalledWith("rid-latest"),
    );
  });

  it("includes the task id and event timeline in the diagnostic report", async () => {
    renderDialog([
      makeEvent({ id: 1, message: "提交到云端" }),
      makeEvent({ id: 2, message: "认证失败", severity: "error" }),
    ]);

    await screen.findByText("提交到云端");
    fireEvent.click(screen.getByRole("button", { name: /复制诊断报告/ }));
    await waitFor(() => expect(writeTextMock).toHaveBeenCalledTimes(1));
    const report = writeTextMock.mock.calls[0]?.[0] ?? "";
    expect(report).toContain("task-aaaaaaaa-bbbb-cccc");
    expect(report).toContain("提交到云端");
    expect(report).toContain("认证失败");
    expect(report).toContain("外部任务 ID: ext-123");
  });
});
