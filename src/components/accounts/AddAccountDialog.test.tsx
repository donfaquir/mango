import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const listProvidersMock = vi.fn();
const createApiAccountMock = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listProviders: (...args: unknown[]) => listProvidersMock(...args),
    createApiAccount: (...args: unknown[]) => createApiAccountMock(...args),
  },
}));

import { AddAccountDialog } from "./AddAccountDialog";

function renderDialog(defaultProviderId?: string) {
  listProvidersMock.mockResolvedValue({
    status: "ok",
    data: [
      {
        id: "kling",
        name: "Kling",
        base_url: "https://api.klingai.com",
        auth_type: "bearer",
        docs_url: "",
      },
    ],
  });
  const onOpenChange = vi.fn();
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const utils = render(
    <QueryClientProvider client={queryClient}>
      <AddAccountDialog
        open={true}
        onOpenChange={onOpenChange}
        defaultProviderId={defaultProviderId}
      />
    </QueryClientProvider>,
  );
  return { ...utils, onOpenChange };
}

describe("AddAccountDialog", () => {
  it("submits create with trimmed label and api_key", async () => {
    createApiAccountMock.mockResolvedValueOnce({
      status: "ok",
      data: {
        id: "a1",
        provider_id: "kling",
        label: "main",
        key_last4: "wxyz",
        usage_quota: null,
        usage_used: 0,
        last_used_at: null,
        created_at: "2026-05-22",
      },
    });
    const { onOpenChange } = renderDialog("kling");

    const labelInput = await screen.findByLabelText(/账号备注/);
    const keyInput = await screen.findByLabelText(/API Key/);

    fireEvent.change(labelInput, { target: { value: "  main  " } });
    fireEvent.change(keyInput, { target: { value: "  sk-test-key  " } });
    fireEvent.click(screen.getByRole("button", { name: /^添加$/ }));

    await waitFor(() =>
      expect(createApiAccountMock).toHaveBeenCalledWith({
        provider_id: "kling",
        label: "main",
        api_key: "sk-test-key",
      }),
    );
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("keeps the add button disabled when fields are empty", async () => {
    renderDialog("kling");
    const addBtn = await screen.findByRole("button", { name: /^添加$/ });
    expect(addBtn).toBeDisabled();
  });
});
