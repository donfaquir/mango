import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

let shotsData: Array<{ id: string; episode_id: string; order_index: number; summary: string; status: "draft" | "ready" | "generating" | "done" }> = [];
let isLoading = false;
const navigate = vi.fn();

vi.mock("@/hooks/useShots", () => ({
  useShotList: () => ({ data: shotsData, isLoading }),
}));
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>(
    "react-router-dom",
  );
  return {
    ...actual,
    useNavigate: () => navigate,
    useParams: () => ({ projectId: "p1" }),
  };
});

import { ShotPickerDialog } from "./ShotPickerDialog";

function wrap(children: ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={["/project/p1/episodes/ep1/canvas"]}>
        {children}
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function fullShot(s: Pick<{ id: string; episode_id: string; order_index: number; summary: string; status: "draft" | "ready" | "generating" | "done" }, "id" | "order_index" | "summary" | "status">) {
  return {
    ...s,
    episode_id: "ep1",
    duration_sec: null,
    camera_angle: "",
    shot_type: "",
    mood: "",
    dialogue: "",
    video_prompt: "",
    image_prompt: "",
    adopted_asset_id: null,
    created_at: "",
    updated_at: "",
  };
}

describe("ShotPickerDialog", () => {
  beforeEach(() => {
    navigate.mockReset();
    isLoading = false;
  });

  it("renders the shot list when shots exist", () => {
    shotsData = [
      fullShot({ id: "s1", order_index: 1, summary: "A", status: "draft" }),
      fullShot({ id: "s2", order_index: 2, summary: "B", status: "ready" }),
    ] as never;
    wrap(
      <ShotPickerDialog
        open
        episodeId="ep1"
        onOpenChange={() => {}}
        onSelect={() => {}}
      />,
    );
    expect(screen.getByText("A")).toBeInTheDocument();
    expect(screen.getByText("B")).toBeInTheDocument();
  });

  it("filters by keyword", () => {
    shotsData = [
      fullShot({ id: "s1", order_index: 1, summary: "alpha", status: "draft" }),
      fullShot({ id: "s2", order_index: 2, summary: "bravo", status: "ready" }),
    ] as never;
    wrap(
      <ShotPickerDialog
        open
        episodeId="ep1"
        onOpenChange={() => {}}
        onSelect={() => {}}
      />,
    );
    fireEvent.change(screen.getByPlaceholderText("搜索摘要..."), {
      target: { value: "alpha" },
    });
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.queryByText("bravo")).toBeNull();
  });

  it("shows empty state with a navigation button", () => {
    shotsData = [];
    wrap(
      <ShotPickerDialog
        open
        episodeId="ep1"
        onOpenChange={() => {}}
        onSelect={() => {}}
      />,
    );
    expect(screen.getByText("该剧集还没有分镜")).toBeInTheDocument();
    fireEvent.click(screen.getByText("跳转剧集详情"));
    expect(navigate).toHaveBeenCalledWith("/project/p1/episodes/ep1");
  });
});
