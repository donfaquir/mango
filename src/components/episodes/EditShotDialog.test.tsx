import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import type { Shot } from "@/lib/bindings/commands";

const updateShot = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    updateShot: (...args: unknown[]) => updateShot(...args),
  },
}));

import { EditShotDialog } from "./EditShotDialog";

function makeShot(overrides: Partial<Shot> = {}): Shot {
  return {
    id: "shot-1",
    episode_id: "ep-1",
    order_index: 0,
    summary: "opening",
    duration_sec: 3.5,
    camera_angle: "low",
    shot_type: "wide",
    mood: "tense",
    dialogue: "hi",
    video_prompt: "",
    image_prompt: "a city skyline",
    status: "draft",
    created_at: "",
    updated_at: "",
    ...overrides,
  };
}

function renderDialog(shot: Shot = makeShot()) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  );
  return render(
    <EditShotDialog
      episodeId="ep-1"
      shot={shot}
      open
      onOpenChange={vi.fn()}
    />,
    { wrapper },
  );
}

describe("EditShotDialog", () => {
  beforeEach(() => {
    updateShot.mockReset();
    updateShot.mockResolvedValue({
      status: "ok",
      data: makeShot(),
    });
  });

  it("pre-fills fields from the shot", () => {
    renderDialog();
    const imagePrompt = screen.getByLabelText("图像 prompt") as HTMLTextAreaElement;
    expect(imagePrompt.value).toBe("a city skyline");
    const duration = screen.getByLabelText("时长（秒）") as HTMLInputElement;
    expect(duration.value).toBe("3.5");
  });

  it("submits edited image_prompt to updateShot", async () => {
    renderDialog();
    const imagePrompt = screen.getByLabelText("图像 prompt") as HTMLTextAreaElement;
    fireEvent.change(imagePrompt, { target: { value: "a new prompt" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(updateShot).toHaveBeenCalledTimes(1));
    const [, input] = updateShot.mock.calls[0];
    expect(input.image_prompt).toBe("a new prompt");
  });

  it("clears duration_sec to null when the input is emptied", async () => {
    renderDialog(makeShot({ duration_sec: 5 }));
    const duration = screen.getByLabelText("时长（秒）") as HTMLInputElement;
    fireEvent.change(duration, { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(updateShot).toHaveBeenCalledTimes(1));
    const [, input] = updateShot.mock.calls[0];
    expect(input.duration_sec).toBeNull();
  });

  it("rejects non-numeric duration with an inline error", async () => {
    renderDialog();
    const duration = screen.getByLabelText("时长（秒）") as HTMLInputElement;
    fireEvent.change(duration, { target: { value: "abc" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(screen.getByText(/需要非负数或留空/)).toBeInTheDocument(),
    );
    expect(updateShot).not.toHaveBeenCalled();
  });
});
