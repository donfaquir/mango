import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const importAssetMock = vi.fn();
const pickImageFileMock = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    importAsset: (...args: unknown[]) => importAssetMock(...args),
    pickImageFile: (...args: unknown[]) => pickImageFileMock(...args),
  },
}));

vi.mock("@/hooks/useResolvedAssetUrl", () => ({
  useResolvedAssetUrl: () => null,
}));

import { ReferenceImageDropzone } from "./ReferenceImageDropzone";

function renderDropzone(currentPath: string | null = null) {
  const onChange = vi.fn();
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const utils = render(
    <QueryClientProvider client={queryClient}>
      <ReferenceImageDropzone
        projectId="p1"
        projectRoot="/tmp/project"
        currentRelativePath={currentPath}
        onChange={onChange}
      />
    </QueryClientProvider>,
  );
  return { ...utils, onChange };
}

describe("ReferenceImageDropzone", () => {
  it("shows placeholder hint when no image", () => {
    renderDropzone(null);
    expect(screen.getByText(/点击下方按钮选择图片/)).toBeInTheDocument();
  });

  it("invokes onChange after picking and importing a file", async () => {
    pickImageFileMock.mockResolvedValueOnce({
      status: "ok",
      data: "/Users/x/photo.png",
    });
    importAssetMock.mockResolvedValueOnce({
      status: "ok",
      data: { id: "a1", file_path: "assets/a1.png" },
    });

    const { onChange } = renderDropzone(null);
    fireEvent.click(screen.getByRole("button", { name: /选择文件/ }));

    await waitFor(() => expect(importAssetMock).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(onChange).toHaveBeenCalledWith("assets/a1.png"),
    );
  });

  it("clears the image when the X button is clicked", () => {
    const { onChange } = renderDropzone("assets/old.png");
    fireEvent.click(screen.getByRole("button", { name: /清空参考图/ }));
    expect(onChange).toHaveBeenCalledWith(null);
  });
});
