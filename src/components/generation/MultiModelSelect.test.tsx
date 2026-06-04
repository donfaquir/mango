import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Model } from "@/lib/bindings/commands";

const listModelsMock = vi.fn<
  (providerId: string | null) => Promise<{ status: "ok"; data: Model[] }>
>();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listModels: (providerId: string | null) => listModelsMock(providerId),
  },
}));

import { MultiModelSelect } from "./MultiModelSelect";

function bailianModels(): Model[] {
  return [
    {
      id: "wan2.7-image-pro",
      provider_id: "bailian",
      name: "通义万相 2.7 Pro",
      model_type: "image",
      capabilities_json:
        '{"task_types":["image"],"requires_reference_media":false}',
      default_params_json: null,
    },
    {
      id: "happyhorse-1.0-r2v",
      provider_id: "bailian",
      name: "快乐马 1.0",
      model_type: "video",
      capabilities_json:
        '{"task_types":["video"],"requires_reference_media":true}',
      default_params_json: null,
    },
  ];
}

function renderSelect(props: {
  taskType: "image" | "video";
  value?: string[];
  onChange?: (next: string[]) => void;
}) {
  listModelsMock.mockResolvedValue({ status: "ok", data: bailianModels() });
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const onChange = props.onChange ?? vi.fn();
  const utils = render(
    <QueryClientProvider client={qc}>
      <MultiModelSelect
        taskType={props.taskType}
        value={props.value ?? []}
        onChange={onChange}
      />
    </QueryClientProvider>,
  );
  return { ...utils, onChange };
}

describe("MultiModelSelect", () => {
  beforeEach(() => {
    listModelsMock.mockReset();
  });

  it("filters models by task_type", async () => {
    renderSelect({ taskType: "image" });
    await waitFor(() => {
      expect(screen.getByText("通义万相 2.7 Pro")).toBeInTheDocument();
    });
    expect(screen.queryByText("快乐马 1.0")).not.toBeInTheDocument();
  });

  it("renders requires_reference_media badge for matching models", async () => {
    renderSelect({ taskType: "video" });
    await waitFor(() => {
      expect(screen.getByText("快乐马 1.0")).toBeInTheDocument();
    });
    expect(screen.getByText("需参考图")).toBeInTheDocument();
  });

  it("toggles selection on click and reports via onChange", async () => {
    const onChange = vi.fn();
    renderSelect({ taskType: "image", onChange });
    const item = await screen.findByRole("checkbox", {
      name: /通义万相 2.7 Pro/,
    });
    expect(item).toHaveAttribute("aria-checked", "false");
    fireEvent.click(item);
    expect(onChange).toHaveBeenCalledWith(["wan2.7-image-pro"]);
  });

  it("reflects selected state via aria-checked", async () => {
    renderSelect({
      taskType: "image",
      value: ["wan2.7-image-pro"],
    });
    const item = await screen.findByRole("checkbox", {
      name: /通义万相 2.7 Pro/,
    });
    expect(item).toHaveAttribute("aria-checked", "true");
  });

  it("removes a selection on second click", async () => {
    const onChange = vi.fn();
    renderSelect({
      taskType: "image",
      value: ["wan2.7-image-pro"],
      onChange,
    });
    const item = await screen.findByRole("checkbox", {
      name: /通义万相 2.7 Pro/,
    });
    fireEvent.click(item);
    expect(onChange).toHaveBeenCalledWith([]);
  });

  it("shows empty state when no model matches the task_type", async () => {
    listModelsMock.mockResolvedValue({ status: "ok", data: [] });
    const qc = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={qc}>
        <MultiModelSelect
          taskType="audio"
          value={[]}
          onChange={vi.fn()}
        />
      </QueryClientProvider>,
    );
    await waitFor(() => {
      expect(screen.getByText(/没有可用于 音频/)).toBeInTheDocument();
    });
  });
});
