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

const PROVIDERS = [
  {
    id: "bailian",
    name: "阿里云百炼",
    base_url: "https://dashscope.aliyuncs.com/api/v1",
    auth_type: "bearer",
    docs_url: "",
  },
  {
    id: "jimeng",
    name: "即梦",
    base_url: "",
    auth_type: "bearer",
    docs_url: "",
  },
];

function renderDialog(defaultProviderId?: string) {
  listProvidersMock.mockResolvedValue({
    status: "ok",
    data: PROVIDERS,
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
  it("submits create with trimmed label and api_key (no OSS for jimeng)", async () => {
    createApiAccountMock.mockResolvedValueOnce({
      status: "ok",
      data: {
        id: "a1",
        provider_id: "jimeng",
        label: "main",
        key_last4: "wxyz",
        oss_config: null,
        usage_quota: null,
        usage_used: 0,
        last_used_at: null,
        created_at: "2026-05-22",
      },
    });
    const { onOpenChange } = renderDialog("jimeng");

    const labelInput = await screen.findByLabelText(/账号备注/);
    const keyInput = await screen.findByLabelText("API Key *");

    fireEvent.change(labelInput, { target: { value: "  main  " } });
    fireEvent.change(keyInput, { target: { value: "  sk-test-key  " } });
    fireEvent.click(screen.getByRole("button", { name: /^添加$/ }));

    await waitFor(() =>
      expect(createApiAccountMock).toHaveBeenCalledWith({
        provider_id: "jimeng",
        label: "main",
        api_key: "sk-test-key",
        oss: null,
      }),
    );
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("keeps the add button disabled when fields are empty", async () => {
    renderDialog("jimeng");
    const addBtn = await screen.findByRole("button", { name: /^添加$/ });
    expect(addBtn).toBeDisabled();
  });

  it("hides OSS fields when provider is jimeng", async () => {
    renderDialog("jimeng");
    await screen.findByLabelText(/账号备注/);
    expect(screen.queryByLabelText(/Endpoint/)).toBeNull();
    expect(screen.queryByLabelText(/AccessKey ID/)).toBeNull();
  });

  it("shows OSS fields when provider is bailian and disables submit until they are filled", async () => {
    renderDialog("bailian");

    await screen.findByLabelText(/账号备注/);
    // OSS fields visible
    const endpointInput = await screen.findByLabelText(/Endpoint/);
    const bucketInput = await screen.findByLabelText(/Bucket/);
    const akIdInput = await screen.findByLabelText(/AccessKey ID/);
    const akSecretInput = await screen.findByLabelText(/AccessKey Secret/);

    // The secret field should be a password input (masked).
    expect(akSecretInput).toHaveAttribute("type", "password");

    const labelInput = screen.getByLabelText(/账号备注/);
    const keyInput = screen.getByLabelText("API Key *");
    fireEvent.change(labelInput, { target: { value: "main" } });
    fireEvent.change(keyInput, { target: { value: "sk-x" } });

    const addBtn = screen.getByRole("button", { name: /^添加$/ });
    // Still disabled because OSS fields are empty.
    expect(addBtn).toBeDisabled();

    fireEvent.change(endpointInput, {
      target: { value: "oss-cn-hangzhou.aliyuncs.com" },
    });
    fireEvent.change(bucketInput, { target: { value: "mango-ref" } });
    fireEvent.change(akIdInput, { target: { value: "LTAI5tABC" } });
    // Still disabled until secret is filled.
    expect(addBtn).toBeDisabled();

    fireEvent.change(akSecretInput, { target: { value: "secret-xyz" } });
    expect(addBtn).not.toBeDisabled();
  });

  it("submits with an oss block when provider is bailian", async () => {
    createApiAccountMock.mockResolvedValueOnce({
      status: "ok",
      data: {
        id: "a2",
        provider_id: "bailian",
        label: "bailian-main",
        key_last4: "wxyz",
        oss_config: {
          endpoint: "oss-cn-hangzhou.aliyuncs.com",
          bucket: "mango-ref",
          access_key_id: "LTAI5tABC",
          region: null,
          url_expires_seconds: 3600,
        },
        usage_quota: null,
        usage_used: 0,
        last_used_at: null,
        created_at: "2026-05-25",
      },
    });
    const { onOpenChange } = renderDialog("bailian");

    fireEvent.change(await screen.findByLabelText(/账号备注/), {
      target: { value: "bailian-main" },
    });
    fireEvent.change(screen.getByLabelText("API Key *"), {
      target: { value: "sk-dashscope" },
    });
    fireEvent.change(await screen.findByLabelText(/Endpoint/), {
      target: { value: "oss-cn-hangzhou.aliyuncs.com" },
    });
    fireEvent.change(screen.getByLabelText(/Bucket/), {
      target: { value: "mango-ref" },
    });
    fireEvent.change(screen.getByLabelText(/AccessKey ID/), {
      target: { value: "LTAI5tABC" },
    });
    fireEvent.change(screen.getByLabelText(/AccessKey Secret/), {
      target: { value: "secret-xyz" },
    });

    fireEvent.click(screen.getByRole("button", { name: /^添加$/ }));

    await waitFor(() =>
      expect(createApiAccountMock).toHaveBeenCalledWith({
        provider_id: "bailian",
        label: "bailian-main",
        api_key: "sk-dashscope",
        oss: {
          endpoint: "oss-cn-hangzhou.aliyuncs.com",
          bucket: "mango-ref",
          access_key_id: "LTAI5tABC",
          access_key_secret: "secret-xyz",
          region: null,
          url_expires_seconds: 3600,
        },
      }),
    );
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });
});
