import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { VideoClip } from "@/lib/bindings/commands";
import { ClipCard } from "./ClipCard";

function makeClip(overrides: Partial<VideoClip> = {}): VideoClip {
  return {
    id: "c1",
    project_id: "p1",
    episode_id: "e1",
    source_asset_id: "asset-abcdef12",
    label: null,
    trim_start_ms: null,
    trim_end_ms: null,
    order_index: 0,
    created_at: "",
    ...overrides,
  };
}

describe("ClipCard", () => {
  it("renders trim range and duration for a trimmed clip", () => {
    const clip = makeClip({
      label: "Scene 1",
      trim_start_ms: 2000,
      trim_end_ms: 8000,
    });
    render(<ClipCard clip={clip} onDelete={vi.fn()} />);

    expect(screen.getByText("Scene 1")).toBeInTheDocument();
    expect(screen.getByText(/0:02\.0 – 0:08\.0/)).toBeInTheDocument();
    expect(screen.getByText(/6\.0s/)).toBeInTheDocument();
  });

  it("shows full-segment label when no trim is set", () => {
    const clip = makeClip();
    render(<ClipCard clip={clip} onDelete={vi.fn()} />);

    expect(screen.getByText("完整片段")).toBeInTheDocument();
  });

  it("falls back to short asset id when label is null", () => {
    const clip = makeClip({ label: null, source_asset_id: "asset-abcdef12" });
    render(<ClipCard clip={clip} onDelete={vi.fn()} />);

    expect(screen.getByText("asset-ab")).toBeInTheDocument();
  });

  it("calls onDelete with clip id when delete button is clicked", async () => {
    const onDelete = vi.fn();
    const clip = makeClip({ id: "clip-42" });
    render(<ClipCard clip={clip} onDelete={onDelete} />);

    const btn = screen.getByRole("button");
    fireEvent.click(btn);

    expect(onDelete).toHaveBeenCalledWith("clip-42");
  });
});
