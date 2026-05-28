import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import type { Asset, Shot } from "@/lib/bindings/commands";

const useAssetMock = vi.fn();
const useAssignAssetToShotMock = vi.fn();
vi.mock("@/hooks/useAssets", () => ({
  useAsset: (...args: unknown[]) => useAssetMock(...args),
  useAssignAssetToShot: () => useAssignAssetToShotMock(),
}));

const useShotMock = vi.fn();
vi.mock("@/hooks/useShots", () => ({
  useShot: (...args: unknown[]) => useShotMock(...args),
}));

const toastError = vi.fn();
const toastSuccess = vi.fn();
vi.mock("sonner", () => ({
  toast: {
    error: (...args: unknown[]) => toastError(...args),
    success: (...args: unknown[]) => toastSuccess(...args),
  },
}));

import { BindAssetToShotDialog } from "./BindAssetToShotDialog";

const baseAsset: Asset = {
  id: "a1",
  project_id: "p1",
  shot_id: null,
  asset_type: "image",
  original_name: "ref.png",
  file_path: "assets/a1.png",
  thumbnail_path: null,
  file_size: 0,
  content_hash: "h",
  metadata_json: null,
  source: "imported",
  label: "",
  created_at: "",
  updated_at: "",
};

const targetShot: Shot = {
  id: "s-target",
  episode_id: "e1",
  order_index: 4,
  summary: "客厅特写",
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

const otherShot: Shot = { ...targetShot, id: "s-other", summary: "天台远景" };

const mutateAsync = vi.fn();

function setupHooks(
  assetOverrides: Partial<Asset> = {},
  shotMap: Record<string, Shot> = {},
) {
  useAssetMock.mockImplementation((id?: string | null) => ({
    data: id ? { ...baseAsset, ...assetOverrides } : undefined,
  }));
  useShotMock.mockImplementation((id?: string) => ({
    data: id ? (shotMap[id] ?? null) : undefined,
  }));
  useAssignAssetToShotMock.mockReturnValue({ mutateAsync, isPending: false });
}

beforeEach(() => {
  useAssetMock.mockReset();
  useShotMock.mockReset();
  useAssignAssetToShotMock.mockReset();
  mutateAsync.mockReset();
  toastError.mockReset();
  toastSuccess.mockReset();
});

describe("BindAssetToShotDialog", () => {
  it("renders nothing when there is no request", () => {
    setupHooks();
    const { container } = render(
      <BindAssetToShotDialog request={null} onClose={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the plain confirm CTA when the asset is currently unbound", () => {
    setupHooks({ shot_id: null }, { "s-target": targetShot });
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "确认绑定" })).toBeInTheDocument();
    expect(screen.queryByText(/覆盖原绑定/)).not.toBeInTheDocument();
  });

  it("treats binding to the same shot as a no-op confirm (not an overwrite)", () => {
    setupHooks({ shot_id: "s-target" }, { "s-target": targetShot });
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "确认绑定" })).toBeInTheDocument();
    expect(screen.queryByText(/覆盖原绑定/)).not.toBeInTheDocument();
  });

  it("warns and switches the CTA label when the asset is bound elsewhere", () => {
    setupHooks(
      { shot_id: "s-other" },
      { "s-target": targetShot, "s-other": otherShot },
    );
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "覆盖并绑定" })).toBeInTheDocument();
    expect(screen.getByText(/继续操作会覆盖原绑定/)).toBeInTheDocument();
    expect(screen.getByText("天台远景")).toBeInTheDocument();
  });

  it("calls the mutation, fires the image toast, and closes on success", async () => {
    setupHooks({ shot_id: null }, { "s-target": targetShot });
    mutateAsync.mockResolvedValueOnce(undefined);
    const onClose = vi.fn();
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "确认绑定" }));
    await waitFor(() => expect(mutateAsync).toHaveBeenCalled());
    expect(mutateAsync).toHaveBeenCalledWith({
      id: "a1",
      shotId: "s-target",
    });
    expect(toastSuccess).toHaveBeenCalledWith("已设为分镜的参考素材");
    expect(onClose).toHaveBeenCalled();
  });

  it("fires the video toast when the asset is a video", async () => {
    setupHooks(
      { shot_id: null, asset_type: "video" },
      { "s-target": targetShot },
    );
    mutateAsync.mockResolvedValueOnce(undefined);
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "确认绑定" }));
    await waitFor(() =>
      expect(toastSuccess).toHaveBeenCalledWith("已采用为分镜的视频结果"),
    );
  });

  it("toasts on failure without closing the dialog", async () => {
    setupHooks({ shot_id: null }, { "s-target": targetShot });
    mutateAsync.mockRejectedValueOnce(new Error("db locked"));
    const onClose = vi.fn();
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "确认绑定" }));
    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]?.[0]).toContain("db locked");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("disables the confirm button while the mutation is pending", () => {
    setupHooks({ shot_id: null }, { "s-target": targetShot });
    useAssignAssetToShotMock.mockReturnValue({ mutateAsync, isPending: true });
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={vi.fn()}
      />,
    );
    const cta = screen.getByRole("button", { name: "确认绑定" });
    expect(cta).toBeDisabled();
  });

  it("falls back to `Shot N` when the target has no summary", () => {
    const blank: Shot = { ...targetShot, summary: "  " };
    setupHooks({ shot_id: null }, { "s-target": blank });
    render(
      <BindAssetToShotDialog
        request={{ assetId: "a1", shotId: "s-target" }}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText("Shot 5")).toBeInTheDocument();
  });
});
