import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const mutateAsync = vi.fn();
const toastSuccess = vi.fn();
const toastError = vi.fn();

vi.mock("@/hooks/useCheckpoint", () => ({
  useCreateCheckpoint: () => ({
    mutateAsync,
    isPending: false,
  }),
}));

vi.mock("sonner", () => ({
  toast: {
    success: (msg: string) => toastSuccess(msg),
    error: (msg: string) => toastError(msg),
  },
}));

import { SaveVersionDialog } from "./SaveVersionDialog";

function renderDialog(props: { onBeforeSave?: () => Promise<void> | void } = {}) {
  const onOpenChange = vi.fn();
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <SaveVersionDialog
        open={true}
        onOpenChange={onOpenChange}
        episodeId="ep1"
        {...props}
      />
    </QueryClientProvider>,
  );
  return { onOpenChange };
}

describe("SaveVersionDialog", () => {
  beforeEach(() => {
    mutateAsync.mockReset().mockResolvedValue({ id: "cp1" });
    toastSuccess.mockReset();
    toastError.mockReset();
  });

  it("passes label when user types one", async () => {
    renderDialog();
    fireEvent.change(screen.getByLabelText("版本标签"), {
      target: { value: "分镜定稿" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存版本" }));

    await waitFor(() =>
      expect(mutateAsync).toHaveBeenCalledWith({
        episode_id: "ep1",
        label: "分镜定稿",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith("已保存版本");
  });

  it("passes null label when input is empty", async () => {
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "保存版本" }));

    await waitFor(() =>
      expect(mutateAsync).toHaveBeenCalledWith({
        episode_id: "ep1",
        label: null,
      }),
    );
  });

  it("calls onBeforeSave before creating checkpoint", async () => {
    const order: string[] = [];
    const onBeforeSave = vi.fn(() => { order.push("flush"); });
    mutateAsync.mockImplementation(async () => {
      order.push("mutate");
      return { id: "cp1" };
    });

    renderDialog({ onBeforeSave });
    fireEvent.click(screen.getByRole("button", { name: "保存版本" }));

    await waitFor(() => expect(toastSuccess).toHaveBeenCalled());
    expect(order).toEqual(["flush", "mutate"]);
  });

  it("shows error toast on mutation failure", async () => {
    mutateAsync.mockRejectedValueOnce(new Error("fail"));
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "保存版本" }));

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastSuccess).not.toHaveBeenCalled();
  });
});
