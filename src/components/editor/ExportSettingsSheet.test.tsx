import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    exportVideoClips: vi.fn(),
  },
}));

vi.mock("@/hooks/useFFmpegProgress", () => ({
  useFFmpegProgress: () => ({ progress: null, reset: vi.fn() }),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

import { ExportSettingsSheet } from "./ExportSettingsSheet";

describe("ExportSettingsSheet", () => {
  it("renders sheet content when open", () => {
    render(
      <ExportSettingsSheet
        open={true}
        onOpenChange={vi.fn()}
        episodeId="e1"
        defaultOutputPath="/tmp/output.mp4"
      />,
    );

    expect(screen.getByText("导出视频")).toBeInTheDocument();
    expect(screen.getByLabelText("输出路径")).toBeInTheDocument();
    expect(screen.getByText("开始导出")).toBeInTheDocument();
  });

  it("populates the output path input with defaultOutputPath", () => {
    render(
      <ExportSettingsSheet
        open={true}
        onOpenChange={vi.fn()}
        episodeId="e1"
        defaultOutputPath="/home/user/video.mp4"
      />,
    );

    const input = screen.getByLabelText("输出路径") as HTMLInputElement;
    expect(input.value).toBe("/home/user/video.mp4");
  });

  it("disables export button when output path is empty", () => {
    render(
      <ExportSettingsSheet
        open={true}
        onOpenChange={vi.fn()}
        episodeId="e1"
        defaultOutputPath=""
      />,
    );

    const btn = screen.getByRole("button", { name: "开始导出" });
    expect(btn).toBeDisabled();
  });
});
