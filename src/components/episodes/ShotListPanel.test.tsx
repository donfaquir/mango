import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import type { Shot } from "@/lib/bindings/commands";

const listShots = vi.fn();
const reorderShots = vi.fn();
const deleteShot = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listShots: (...args: unknown[]) => listShots(...args),
    reorderShots: (...args: unknown[]) => reorderShots(...args),
    deleteShot: (...args: unknown[]) => deleteShot(...args),
  },
}));

vi.mock("./CreateShotDialog", () => ({
  CreateShotDialog: () => null,
}));

import { ShotListPanel } from "./ShotListPanel";

function makeShot(id: string, orderIndex: number, summary: string): Shot {
  return {
    id,
    episode_id: "e1",
    order_index: orderIndex,
    summary,
    duration_sec: null,
    camera_angle: "",
    shot_type: "",
    mood: "",
    dialogue: "",
    video_prompt: "",
    image_prompt: "",
    status: "draft",
    created_at: "",
    updated_at: "",
  };
}

const SHOTS: Shot[] = [
  makeShot("a", 0, "first"),
  makeShot("b", 1, "second"),
  makeShot("c", 2, "third"),
];

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return render(<ShotListPanel episodeId="e1" />, { wrapper });
}

describe("ShotListPanel", () => {
  beforeEach(() => {
    listShots.mockReset();
    reorderShots.mockReset();
    deleteShot.mockReset();
    listShots.mockResolvedValue({ status: "ok", data: SHOTS });
  });

  it("renders one drag handle per shot", async () => {
    renderPanel();
    const handles = await screen.findAllByLabelText("拖拽排序");
    expect(handles).toHaveLength(3);
  });

  it("displays shots in their server order with sequential indices", async () => {
    renderPanel();
    await screen.findByText("first");
    expect(screen.getByText("first")).toBeInTheDocument();
    expect(screen.getByText("second")).toBeInTheDocument();
    expect(screen.getByText("third")).toBeInTheDocument();
    expect(screen.getByText("#1")).toBeInTheDocument();
    expect(screen.getByText("#2")).toBeInTheDocument();
    expect(screen.getByText("#3")).toBeInTheDocument();
  });

  it("wires each handle as a dnd-kit sortable activator", async () => {
    renderPanel();
    const handles = await screen.findAllByLabelText("拖拽排序");
    for (const handle of handles) {
      expect(handle).toHaveAttribute("aria-roledescription", "sortable");
      expect(handle).toHaveAttribute("aria-describedby");
      expect(handle).toHaveAttribute("tabindex");
    }
  });
});
