import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import type { VideoClip } from "@/lib/bindings/commands";

const listVideoClips = vi.fn();
const deleteVideoClip = vi.fn();
const reorderVideoClips = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listVideoClips: (...args: unknown[]) => listVideoClips(...args),
    deleteVideoClip: (...args: unknown[]) => deleteVideoClip(...args),
    reorderVideoClips: (...args: unknown[]) => reorderVideoClips(...args),
  },
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

import { ClipAssembly } from "./ClipAssembly";

function makeClip(id: string, orderIndex: number): VideoClip {
  return {
    id,
    project_id: "p1",
    episode_id: "e1",
    source_asset_id: `asset-${id}`,
    label: `Clip ${id}`,
    trim_start_ms: null,
    trim_end_ms: null,
    order_index: orderIndex,
    created_at: "",
  };
}

const CLIPS: VideoClip[] = [makeClip("a", 0), makeClip("b", 1)];

function renderAssembly(clips: VideoClip[] = CLIPS) {
  listVideoClips.mockResolvedValue({ status: "ok", data: clips });
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return render(
    <ClipAssembly
      episodeId="e1"
      onAddClip={vi.fn()}
      onExport={vi.fn()}
    />,
    { wrapper },
  );
}

describe("ClipAssembly", () => {
  beforeEach(() => {
    listVideoClips.mockReset();
    deleteVideoClip.mockReset();
    reorderVideoClips.mockReset();
  });

  it("renders the correct number of clip cards", async () => {
    renderAssembly();
    await screen.findByText("Clip a");
    expect(screen.getByText("Clip b")).toBeInTheDocument();
    expect(screen.getByText("片段列表 (2)")).toBeInTheDocument();
  });

  it("shows empty state when no clips exist", async () => {
    renderAssembly([]);
    const empty = await screen.findByText(/暂无片段/);
    expect(empty).toBeInTheDocument();
  });

  it("disables export button when there are no clips", async () => {
    renderAssembly([]);
    await screen.findByText(/暂无片段/);
    const exportBtn = screen.getByRole("button", { name: "导出" });
    expect(exportBtn).toBeDisabled();
  });

  it("enables export button when clips exist", async () => {
    renderAssembly();
    await screen.findByText("Clip a");
    const exportBtn = screen.getByRole("button", { name: "导出" });
    expect(exportBtn).toBeEnabled();
  });
});
